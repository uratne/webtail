use lib::client::{configuration::ClientConfiguration, process};
use log::info;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let client_configuration = ClientConfiguration::read_from_file();
    let mut process_thread_handlers = vec![];

    for config in client_configuration.get_configurations() {
        let handler = tokio::spawn(async move {
            process::file(config).await;
        });
        process_thread_handlers.push(handler);
    }

    for handler in process_thread_handlers {
        match handler.await {
            Ok(_) => {},
            Err(err) => {
                info!("Error processing file: {}", err);
            }
        };
    }

    info!("webtail client stopped");
}