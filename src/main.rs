use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{State, Query},
};
use axum::response::IntoResponse;
use axum::http::{StatusCode, Method, Uri, HeaderValue};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::sync::Arc;
use tokio::sync::broadcast;

mod db;
mod models;
mod handlers;
mod error;
mod services;
mod middleware;

#[derive(Clone)]
struct AppState {
    db: sqlx::PgPool,
    jwt_service: Arc<services::JwtService>,
    stego_service: Arc<services::SteganographyService>,
    broadcast_tx: broadcast::Sender<String>,
}

#[derive(Deserialize)]
struct CreateInviteQuery {
    admin_id: String,
}

async fn health_check(State(state): State<AppState>) -> Json<Value> {
    let db_status = match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    Json(json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "database": db_status,
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "secure_chat_backend=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()?;
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    tracing::info!("Connecting to db...");
    let pool = db::create_pool(&database_url).await?;
    tracing::info!("Database connected");

    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET must be set!");
    let jwt_expiry = std::env::var("JWT_ACCESS_EXPIRY")
        .unwrap_or_else(|_| "900".to_string())
        .parse::<i64>()?;

    let jwt_service = Arc::new(services::JwtService::new(jwt_secret, jwt_expiry));

    let stego_salt = std::env::var("DEFAULT_STEGO_SALT")
        .expect("DEFAULT_STEGO_SALT must be set");

    let stego_service = Arc::new(
        services::SteganographyService::new(&stego_salt)
        .map_err(|e| anyhow::anyhow!("Failed to create stego service: {:?}", e))?
    );

    let (broadcast_tx, _) = broadcast::channel(100);

    let state = AppState { 
        db: pool, 
        jwt_service,
        stego_service,
        broadcast_tx
    };

    // Setup CORS
    let cors = CorsLayer::new()
        .allow_origin("https://https://crypto-kyo.onrender.com".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
        .allow_credentials(true);

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/register", post(handlers::register))
        .route("/api/auth/login", post(handlers::login))
        .route("/api/auth/refresh", post(handlers::refresh_token))
        .route("/api/auth/me", get(handlers::get_current_user))
        .route("/api/invite", post(handlers::create_invitation))
        .route("/api/messages", get(handlers::get_messages))
        .route("/api/ws", get(handlers::ws_handler))
        .fallback(fallback)
        .with_state(state)
        // .layer(cors);
        .layer(tower_http::trace::TraceLayer::new_for_http());
    
    tracing::info!("Routes registered: /health, /api/auth/register, /api/auth/login, /api/invitations");
    
    // Start server
    let addr = SocketAddr::from((host.parse::<std::net::IpAddr>()?, port));
    tracing::info!("🚀 Server starting on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn fallback(method: Method, uri: Uri) -> impl IntoResponse {
    println!("DEBUG: Received {} request at {}", method, uri);
    (StatusCode::NOT_FOUND, format!("No route found for {} {}", method, uri))
}