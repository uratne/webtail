use std::{collections::HashMap, sync::Arc};

use actix_web::{body::MessageBody, get, web, HttpRequest, HttpResponse, Responder};
use log::{debug, error, info};
use futures::{future, stream::StreamExt};
use serde::Serialize;
use tokio_stream::wrappers::BroadcastStream;

use crate::{message::Message, server::broadcaster::Broadcasters, Applicatiton};

#[get("/api/sse")]
pub async fn data_outbound_sse(_req: HttpRequest, broadcasters: web::Data<Arc<Broadcasters>>, query: web::Query<HashMap<String, String>>,) -> impl Responder {
    let application = match query.get("application") {
        Some(app_str) => {
            match serde_json::from_str(app_str) {
                Ok(app) => app,
                Err(e) => {
                    error!("Failed to parse application JSON: {}", e);
                    return HttpResponse::BadRequest().finish();
                }
            }
        },
        None => {
            error!("No application parameter provided");
            return HttpResponse::BadRequest().finish();
        }
    };
    
    let broadcasters = broadcasters.lock().await;
    let rx = match broadcasters.get(&application) {
        Some(tx) => tx.subscribe(),
        None => {
            error!("No broadcaster found for application: {}", application.name());
            return HttpResponse::BadRequest().finish();
        },
    };
    drop(broadcasters);

    let stream = BroadcastStream::new(rx)
    .take_while(|msg| future::ready(
        match msg {
            Ok(Message::ClientDisconnect) => false,
            Err(_) => false,
            _ => true,
        }
    ))
    .map(|msg| {
        match msg {
            Ok(msg) => match msg {
                Message::Data(data) => {
                    debug!("Sending data message: {:#?}", data);
                    let msg = match serde_json::to_string(&data) {
                        Ok(msg) => msg,
                        Err(err) => {
                            error!("Failed to serialize data message: {}", err);
                            return format!("data: Error: {}\n\n", err).try_into_bytes();
                        },
                    };
                    format!("data: {}\n\n", msg).try_into_bytes()
                }
                Message::System(sys) => {
                    debug!("Sending system message: {:#?}", sys);
                    let msg = match serde_json::to_string(&sys) {
                        Ok(msg) => msg,
                        Err(err) => {
                            error!("Failed to serialize system message: {}", err);
                            return format!("data: Error: {}\n\n", err).try_into_bytes();
                        },
                    };
                    format!("data: {}\n\n", msg).try_into_bytes()
                }
                Message::ClientDisconnect => {
                    info!("Client disconnected");
                    format!("data: Client disconnected\n\n").try_into_bytes()
                }
            },
            Err(err) => {
                error!("Error sending message: {:?}", err);
                format!("data: Error: {}\n\n", err).try_into_bytes()
            },
        }
    });

    HttpResponse::Ok()
        .append_header(("content-type", "text/event-stream"))
        .append_header(("cache-control", "no-cache"))
        .append_header(("connection", "keep-alive"))
        .streaming(stream)
}

#[derive(Serialize)]
struct ApiResponse {
    message: String,
}

#[get("/api/hello")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse {
        message: "Hello from Rust!".to_string(),
    })
}

#[get("/api/applications")]
async fn current_registered_applications(broadcasters: web::Data<Arc<Broadcasters>>) -> impl Responder {
    let broadcasters = broadcasters.lock().await;
    let applications: Vec<Applicatiton> = broadcasters.keys().cloned().collect();
    drop(broadcasters);
    HttpResponse::Ok().json(applications)
}