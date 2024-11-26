use actix_web::{middleware, web, App, HttpServer};
use actix_files as fs;
use actix_cors::Cors;
use lib::server::broadcaster;
use log::info;
use std::{env, sync::Arc};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "dev".to_string());
    if environment == "dev" {
        dotenv::dotenv().ok();
    } else if environment == "prod" {
        dotenv::from_filename(".prod.env").ok();
    }
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "localhost".to_string());
    let frontend_origin = env::var("FRONTEND_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());
    let path_to_front_end = env::var("PATH_TO_FRONTEND").unwrap_or_else(|_| "./frontend/build".to_string());
    
    info!("starting at http://{}:{}", host, port);
    info!("frontend origin: {}", frontend_origin);

    let broadcasters = broadcaster::new_broadcasters();
    let broadcasters = Arc::new(broadcasters);

    HttpServer::new(move || {
        // CORS configuration for development
        let cors = Cors::default()
            .allowed_origin(&frontend_origin)
            .allow_any_method()
            .allow_any_header()
            .supports_credentials();
        let broadcasters = Arc::clone(&broadcasters);
        let broadcasters = web::Data::new(broadcasters);

        App::new()
            .wrap(middleware::Logger::default())
            .wrap(cors)
            .app_data(broadcasters)
            // API routes
            .service(lib::server::controller::outbound::hello)
            // WebSocket route
            .service(lib::server::controller::inbound::data_inbound_ws)
            // SSE route
            .service(lib::server::controller::outbound::data_outbound_sse)
            // API route to get the current registered applications
            .service(lib::server::controller::outbound::current_registered_applications)
            // In production, serve the built frontend
            .service(
                fs::Files::new("/", &path_to_front_end)
                    .index_file("index.html")
                    .default_handler(|req: actix_web::dev::ServiceRequest| {
                        let (http_req, _payload) = req.into_parts();
                        async {
                            let response = fs::NamedFile::open("./frontend/build/index.html")?
                                .into_response(&http_req);
                            Ok(actix_web::dev::ServiceResponse::new(http_req, response))
                        }
                    })
            )
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}