use log::{debug, error, info, warn};
use tokio::{sync::mpsc::Sender, time};
use tokio_tungstenite::connect_async;
use futures_util::{SinkExt, StreamExt};
use tungstenite::{handshake::client::generate_key, http::Request, Message, Error};

use crate::{client::file_tailer::FileTailer, message::{self, BinaryMessage, DataMessage}};

use super::configuration::LogConfiguration;

pub async fn file(config: LogConfiguration) {
    loop {
        process_until_error(config.clone()).await;
        time::sleep(time::Duration::from_secs(20)).await;
    }
}

async fn process_until_error(config: LogConfiguration) {
    let host = config.get_server_host();
    let port = config.get_server_port();
    let path = config.get_server_path();
    let host = if port == 0 { host } else { format!("{}:{}", host, port) };
    let uri = format!("ws://{}/{}", host, path);

    info!("connecting to {}", uri);
    // Connect to WebSocket server
    let request = Request::builder()
        .uri(uri)
        .header("Host", host)
        .header("Sec-WebSocket-Key", generate_key())
        .header("Sec-WebSocket-Version", "13")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Application", config.get_application().to_string())
        .body(())
        .map_err(|err| error!("Error creating request: {}", err))
        .unwrap();
    
    let (ws_stream, _) = match connect_async(request).await {
        Ok(data) => data,
        Err(err) => {
            error!("Error connecting to WebSocket server: {}", err);
            return;
        },
    };

    info!("webSocket connected");
    
    // Split the WebSocket stream
    let (mut write, mut read) = ws_stream.split();
    let (tx, mut rx) = tokio::sync::mpsc::channel(config.get_channel_buffer());
    let (tx_client_abort, mut rx_client_abort) = tokio::sync::mpsc::channel::<()>(1);
    let (tx_server_abort, mut rx_server_abort) = tokio::sync::mpsc::channel::<()>(1);
    
    // Spawn a task to handle incoming messages
    let tx_clone = tx.clone();
    let receive_task = tokio::spawn(async move {
        let mut abort_send_task = true;
        loop {
            tokio::select! {
                _ = rx_client_abort.recv() => {
                    info!("client receive task aborted");
                    abort_send_task = false;
                    break;
                },
                message = read.next() => {
                    if message.is_some() && !process_message(message.unwrap(), &tx_clone).await {
                        break;
                    }
                }
            }
        }
        
        if abort_send_task {
            match tx_server_abort.send(()).await {
                Ok(_) => {},
                Err(err) => {
                    error!("Error sending abort message: {}", err);
                },
            };
        }
        info!("client receive task stopped");
    });

    let file_tailer = FileTailer::new(config.get_log_file_name_regex(), config.get_log_file_dir()).await;

    match file_tailer {
        Some(mut file_tailer) => {
            tokio::spawn(async move {
                file_tailer.tail(tx, config).await;
            });
        }
        None => {
            error!("No file found. Waiting for a file");
            tokio::spawn(async move {
                let mut file_tailer = loop {
                    let file_tailer = FileTailer::new(config.get_log_file_name_regex(), config.get_log_file_dir()).await;
                    match file_tailer {
                        Some(file_tailer) => {
                            break file_tailer
                        }
                        None => {
                            time::sleep(time::Duration::from_secs(2)).await;
                        }
                    }
                };

                file_tailer.tail(tx, config).await;
            });
        }
    }

    // Send messages
    let send_task = tokio::spawn(async move {
        // Keep the connection alive
        let mut send = false;
        let mut abort_receive_task= true;
        loop {
            let msg = tokio::select! {
                _ = rx_server_abort.recv() => {
                    info!("client send task aborted");
                    abort_receive_task = false;
                    break;
                },
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => msg,
                        None => break,
                    }
                }
            };

            if msg.system().is_some() {
                match msg.system().unwrap().message() {
                    message::SystemMessages::Stop => {
                        info!("stopped sending messages");
                        break;
                    }, 
                    message::SystemMessages::Start => {
                        info!("starting to send messages");
                        send = true;
                    },
                    message::SystemMessages::Pause => {
                        info!("paused sending messages");
                        send = false;
                    },
                    message::SystemMessages::Resume => {
                        info!("resumed sending messages");
                        send = true;
                    },
                    _ => {}
                }
            }
            let binary_message = BinaryMessage::from(msg);
            let binary_msg = match borsh::to_vec(&binary_message) {
                Ok(msg) => msg,
                Err(err) => {
                    error!("Failed to serialize message: {}", err);
                    continue;
                },
            };


            if send {
                if let Err(e) = write.send(Message::Binary(binary_msg)).await {
                    error!("Error sending message: {}", e);
                    break;
                }
            }
        }

        if abort_receive_task {
            match tx_client_abort.send(()).await {
                Ok(_) => {},
                Err(err) => {
                    error!("Error sending abort message: {}", err);
                },
            };
        }
        info!("client send task stopped");
    });

    let _ = tokio::join!(receive_task, send_task);
    info!("client stopped");
}

async fn process_message(message: Result<Message, Error>, tx_clone: &Sender<crate::message::Message>) -> bool {
    match message {
        Ok(msg) => {
            match msg {
                Message::Text(text) => {
                    info!("Received: {}", text);
                    let message: crate::message::Message = match serde_json::from_str(&text) {
                        Ok(message) => message,
                        Err(e) => {
                            error!("Failed to parse message: {}", e);
                            return true
                        },
                    };
                    tx_clone.send(message).await.map_err(|err| error!("Error sending message: {}", err)).ok();
                },
                Message::Binary(data) => info!("Received binary data: {:?}", data),
                Message::Ping(_) => debug!("Received ping"),
                Message::Pong(_) => debug!("Received pong"),
                Message::Close(_) => {
                    info!("Server closed connection");
                    return false
                },
                Message::Frame(_) => warn!("Received raw frame"),
            }
        }
        Err(e) => {
            error!("Error receiving message: {}", e);
            return false
        }
    }

    true
}

pub(crate) async fn process_line(mut line: String, last_line: &mut String, end_by_new_line: &mut bool, tx: &Sender<crate::message::Message>, config: &LogConfiguration) {
    let replacent: &str = "👻🛸👻";
    line = line.replace('\n', replacent);

    // Check if line is a new line
    if line.eq("👻🛸👻") {
        *end_by_new_line = true;
        let message = DataMessage::new("\n".to_string(), config.get_application(), false);
        let message = crate::message::Message::Data(message);
        if tx.is_closed() {
            return;
        }
        match tx.send(message).await{
            Ok(_) => {}
            Err(e) => {
                error!("Error sending message: {}", e);
                return;
            }
        }
        return;
    }

    let lines = line.split('👻');
    for mut line in lines {
        if line.eq("") {
            continue;
        }
        if line.eq("🛸") {
            *end_by_new_line = true;
            continue;
        }
        let append = if *end_by_new_line {
            *end_by_new_line = false;
            false
        } else {
            last_line.push_str(line);
            line = &last_line;
            true
        };
        let message = DataMessage::new(line.to_string(), config.get_application(), append);
        let message = crate::message::Message::Data(message);
        if tx.is_closed() {
            break;
        }
        match tx.send(message).await{
            Ok(_) => {}
            Err(e) => {
                error!("Error sending message: {}", e);
                break;
            }
        }
        debug!("{}", line);
        *last_line = line.to_string();
    }
}

pub(crate) fn match_file_name(file_name: &str, regex: &str) -> bool {
    let re = match regex::Regex::new(regex) {
        Ok(re) => re,
        Err(e) => {
            error!("Error creating regex: {}", e);
            return false
        }
    };
    re.is_match(file_name)
}