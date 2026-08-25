mod api;
mod db;
mod radio;
mod radio_ffi;
mod worker;

use crate::api::AppState;
use crate::db::Database;
use crate::radio::Radio;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "httplora=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    info!("Starting httplora gateway");
    
    let db_path = env::var("HTTPLORA_DB_PATH").unwrap_or_else(|_| "httplora.db".to_string());
    let device_path = env::var("HTTPLORA_DEVICE_PATH").ok();
    let bind_addr = env::var("HTTPLORA_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let auth_token = env::var("HTTPLORA_TOKEN").ok();
    
    if auth_token.is_some() {
        info!("Bearer token authentication enabled");
    }
    
    let db = Arc::new(Database::new(&db_path).await?);
    db.initialize().await?;
    info!("Database initialized at {}", db_path);
    
    let mut radio = Radio::new(device_path.as_deref());
    if let Err(e) = radio.initialize() {
        error!("Radio initialization failed: {}", e);
        info!("Continuing with disconnected radio (fail-closed)");
    } else {
        info!("Radio initialized: {:?}", radio.backend_type());
    }
    
    let radio = Arc::new(Mutex::new(radio));
    
    let state = AppState {
        db: db.clone(),
        radio: radio.clone(),
        auth_token,
    };
    
    let worker = worker::Worker::new(db.clone(), radio.clone());
    tokio::spawn(async move {
        worker.run().await;
    });
    
    let app = api::create_router(state);
    
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("HTTP server listening on {}", bind_addr);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}
