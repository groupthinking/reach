use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod audit;
mod auth;
mod errors;
mod models;
mod nip11;
mod relay;
mod tenant;

pub use models::*;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub config: Arc<Config>,
    pub nip98_seen: Arc<moka::future::Cache<String, ()>>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub relay_name: String,
    pub relay_description: String,
    pub relay_pubkey: String,
    pub relay_contact: String,
    pub software: String,
    pub version: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            relay_name: std::env::var("RELAY_NAME").unwrap_or_else(|_| "Reach Relay".into()),
            relay_description: std::env::var("RELAY_DESCRIPTION")
                .unwrap_or_else(|_| "Buzz multi-tenant Nostr relay".into()),
            relay_pubkey: std::env::var("RELAY_PUBKEY").unwrap_or_default(),
            relay_contact: std::env::var("RELAY_CONTACT").unwrap_or_default(),
            software: "https://github.com/block/buzz".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    let nip98_seen: moka::future::Cache<String, ()> = moka::future::Cache::builder()
        .max_capacity(10_000)
        .time_to_live(std::time::Duration::from_secs(120))
        .build();

    let config = Arc::new(Config::default());
    let state = AppState {
        pool,
        config: config.clone(),
        nip98_seen: Arc::new(nip98_seen),
    };

    let app = Router::new()
        .route("/", get(nip11::relay_info_handler))
        .route("/ws", get(relay::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "0.0.0.0:8080";
    tracing::info!("Reach relay listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
