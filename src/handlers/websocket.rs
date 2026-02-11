// Replace src/handlers/websocket.rs entirely
use crate::{
    error::{AppError, Result},
    models::{Message as DbMessage, MessageResponse},
    services::Claims,
    AppState,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct WsQuery {
    token: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<WsQuery>,
) -> Result<Response> {
    // Validate JWT token
    let claims = state
        .jwt_service
        .validate_token(&params.token)
        .map_err(|e| AppError::Validation(format!("Invalid token: {}", e)))?;

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, claims)))
}

async fn handle_socket(socket: WebSocket, state: AppState, claims: Claims) {
    let (mut sender, mut receiver) = socket.split();
    let user_id = claims.sub.clone();
    let username = claims.username.clone();
    
    // Subscribe to broadcast channel
    let mut rx = state.broadcast_tx.subscribe();
    
    // Spawn task to receive broadcasts and send to this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });
    
    // Main task: receive messages from this client and store in DB
    let db = state.db.clone();
    let broadcast_tx = state.broadcast_tx.clone();
    let stego_service = state.stego_service.clone();
    
    let value = user_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            // Parse the incoming message
            let incoming: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };
            
            let content = match incoming.get("content").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => continue,
            };
            
            // Encode message with steganography
            let encoded_content = match stego_service.encode(content) {
                Ok(e) => e,
                Err(e) => {
                    tracing::error!("Failed to encode message: {:?}", e);
                    continue;
                }
            };
            
            // Store in database
            let user_uuid = match uuid::Uuid::parse_str(&value) {
                Ok(u) => u,
                Err(_) => continue,
            };
            
            let message = match sqlx::query_as!(
                DbMessage,
                r#"
                INSERT INTO messages (user_id, content)
                VALUES ($1, $2)
                RETURNING id, user_id, content, created_at as "created_at!", edited_at, reply_to
                "#,
                user_uuid,
                encoded_content
            )
            .fetch_one(&db)
            .await
            {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Failed to store message: {:?}", e);
                    continue;
                }
            };
            
            // Decode for broadcast (so clients receive plaintext)
            let decoded_content = match stego_service.decode(&message.content) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Failed to decode message: {:?}", e);
                    continue;
                }
            };
            
            // Create response
            let response = MessageResponse {
                id: message.id,
                user_id: message.user_id,
                username: Some(username.clone()),
                content: decoded_content,
                created_at: message.created_at,
                edited_at: message.edited_at,
                reply_to: message.reply_to,
            };
            
            // Broadcast to all clients
            if let Ok(json) = serde_json::to_string(&response) {
                let _ = broadcast_tx.send(json);
            }
        }
    });
    
    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
    
    tracing::info!("WebSocket connection closed for user: {}", user_id);
}