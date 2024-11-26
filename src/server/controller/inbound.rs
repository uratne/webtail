use std::{sync::Arc, time::Duration};

use actix_web::{rt, web, Error, HttpRequest, HttpResponse, Result};
use actix_ws::{AggregatedMessage, ProtocolError, Session};
use log::{debug, error, info, trace, warn};
use tokio::{sync::broadcast::{self, Sender}, time::sleep};

use crate::{message::{BinaryMessage, Message, SystemMessage, SystemMessages}, server::broadcaster::Broadcasters, Applicatiton};

#[actix_web::get("/ws")]
pub async fn data_inbound_ws(req: HttpRequest, stream: web::Payload, broadcasters: web::Data<Arc<Broadcasters>>) -> Result<HttpResponse, Error> {
    match req.peer_addr() {
        Some(addr) => info!("WebSocket connection request from {}", addr),
        None => warn!("WebSocket connection request from unknown source"),
    }
    
    let (res, mut session, stream) = actix_ws::handle(&req, stream)?;

    let application = match req.headers().get("Application") {
        Some(app) => match app.to_str() {
            Ok(app) => app,
            Err(e) => {
                error!("Failed to parse application header: {:?} with error: {}", app, e);
                return Ok(HttpResponse::FailedDependency().finish());
            }
        },
        None => {
            error!("No Application header provided");
            return Ok(HttpResponse::BadRequest().finish());
        }
    };
    
    let application: Applicatiton = match serde_json::from_str(&application) {
        Ok(app) => app,
        Err(err) => {
            error!("Failed to parse application JSON: {} with error: {}", application, err);
            return Ok(HttpResponse::UnprocessableEntity().finish());
        },
    };
    
    let mut stream = stream
    .aggregate_continuations()
    // aggregate continuation frames up to 1MiB
    .max_continuation_size(2_usize.pow(20));
    
    info!("WebSocket connection established for application: {}", application.name());

    let (tx, _) = broadcast::channel(100);

    let mut locked_broadcasters = broadcasters.lock().await;
    locked_broadcasters.insert(application.clone(), tx.clone());
    drop(locked_broadcasters);

    let start_message = Message::System(SystemMessage::new(application.clone(), SystemMessages::Start));
    let start_message = match serde_json::to_string(&start_message) {
        Ok(msg) => msg,
        Err(err) => {
            error!("Failed to serialize start message: {}", err);
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    match session.text(start_message).await {
        Ok(_) => {},
        Err(err) => {
            error!("Failed to send start message: {}", err);
            return Ok(HttpResponse::InternalServerError().finish());
        },
    };

    let app = application.clone();

    let mut ping_session = session.clone();
    let handle = rt::spawn(async move {
        while let Some(msg) = stream.recv().await {
            if tx.receiver_count() > 0 {
                match handle_message(msg, &mut session, &tx).await {
                    false => break,
                    _ => {
                        continue;
                    }
                }
            }

            let pause_message = Message::System(SystemMessage::new(application.clone(), SystemMessages::Pause));
            let pause_message = match serde_json::to_string(&pause_message) {
                Ok(msg) => msg,
                Err(err) => {
                    error!("Failed to serialize pause message: {}", err);
                    break;
                }
            };
            match session.text(pause_message).await {
                Ok(_) => {},
                Err(err) => {
                    error!("Failed to send pause message: {}", err);
                    break;
                }
            };
            
            loop {
                if tx.receiver_count() > 0 {
                    // Consume any pending messages in the stream buffer
                    while let Ok(msg) = tokio::time::timeout(
                        Duration::from_millis(50), 
                        stream.recv()
                    ).await {
                        debug!("Consuming pending message: {:?}", msg);
                        continue;
                    }

                    let resume_message = Message::System(SystemMessage::new(application.clone(), SystemMessages::Resume));
                    let resume_message = match serde_json::to_string(&resume_message) {
                        Ok(msg) => msg,
                        Err(err) => {
                            error!("Failed to serialize resume message: {}", err);
                            break;
                        }
                    };
                    match session.text(resume_message).await {
                        Ok(_) => {},
                        Err(err) => {
                            error!("Failed to send resume message: {}", err);
                            break;
                        },
                    };
                    break;
                }
                sleep(Duration::from_secs(1)).await;
            }
        }
        info!("webSocket connection closed");
    });

    rt::spawn(async move {
        let ping_interval = Duration::from_secs(1);
        
        while let Ok(()) = ping_session.ping(b"ping").await {
            sleep(ping_interval).await;
        }
        
        info!("Ping failed, aborting message handler");
        let mut locked_broadcasters = broadcasters.lock().await;
        let rx = locked_broadcasters.remove(&app);
        drop(locked_broadcasters);
        match rx {
            Some(rx) => {
                let _ = rx.send(Message::ClientDisconnect);
            },
            None => {
                error!("No broadcaster found for application: {}", app.name());
            }
        }
        handle.abort();
        info!("WebSocket connection terminated by ping monitor");
    });
    
    Ok(res)
}

async fn handle_message(msg: Result<AggregatedMessage, ProtocolError>, session: &mut Session, tx: &Sender<Message>) -> bool {
    match msg {
        Ok(AggregatedMessage::Text(text)) => {
            // echo text message
            let message: Result<Message, serde_json::Error> = serde_json::from_str(&text);
            match message {
                Ok(message) => {
                    debug!("Received message: {:#?}", message);
                    match tx.send(message) {
                        Ok(n) => trace!("message broadcasted to {} subscribers", n),
                        Err(err) => error!("error broadcasting message: {:?}", err),
                    }
                }
                Err(e) => {
                    error!("Failed to parse message: {:?}", e);
                }
            }
        }
        
        Ok(AggregatedMessage::Binary(bin)) => {
            // process binary message
            let binary_message: Result<BinaryMessage, std::io::Error> = borsh::from_slice(&bin);
            match binary_message {
                Ok(binary_message) => {
                    let message = Message::from(binary_message);
                    debug!("Received binary message: {:#?}", message);
                    match tx.send(message) {
                        Ok(n) => trace!("message broadcasted to {} subscribers", n),
                        Err(err) => error!("error broadcasting message: {:?}", err),
                    }
                },
                Err(e) => {
                    error!("Failed to parse message: {:?}", e)
                },
            }
        }
        
        Ok(AggregatedMessage::Ping(msg)) => {
            // respond to PING frame with PONG frame
            match session.pong(&msg).await {
                Ok(_) => {},
                Err(err) => {
                    error!("Failed to send PONG message: {}", err);
                    return false;
                }
            };
        }
        
        Ok(AggregatedMessage::Close(reason)) => {
            // close the session
            if reason.is_some() {
                info!("Closing session with reason : {:?}", reason.clone());
            } else {
                error!("Closing session without reason");
            }

            return false;
        }

        _ => {}
    }

    true
}