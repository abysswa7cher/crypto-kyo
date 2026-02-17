// Replace src/handlers/websocket.rs entirely
use crate::{
    AppState,
    error::{AppError, Result},
    models::{Message as DbMessage, MessageResponse},
    services::Claims,
};
use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;

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

// src/handlers/websocket.rs
async fn handle_socket(socket: WebSocket, state: AppState, claims: Claims) {
    let (mut sender, mut receiver) = socket.split();
    let user_id = claims.sub.clone();
    let user_id_for_log = user_id.clone();
    let username = claims.username.clone();

    let mut rx = state.broadcast_tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    let db = state.db.clone();
    let broadcast_tx = state.broadcast_tx.clone();
    // REMOVE stego_service - no longer needed

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let incoming: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let content = match incoming.get("content").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => continue,
            };

            // NO ENCRYPTION - content is already encrypted by client
            // Just store it as-is
            let user_uuid = match uuid::Uuid::parse_str(&user_id) {
                Ok(u) => u,
                Err(_) => continue,
            };

            let message = match sqlx::query_as::<_, DbMessage>(
                r#"
                INSERT INTO messages (user_id, content)
                VALUES ($1, $2)
                RETURNING id, user_id, content, created_at, edited_at, reply_to
                "#,
            )
            .bind(&user_uuid)
            .bind(&content)
            .fetch_one(&db)
            .await
            {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Failed to store message: {:?}", e);
                    continue;
                }
            };

            // Broadcast encrypted content (clients will decrypt)
            let response = MessageResponse {
                id: message.id,
                user_id: message.user_id,
                username: Some(username.clone()),
                content: message.content.clone(), // Send encrypted content
                created_at: message.created_at,
                edited_at: message.edited_at,
                reply_to: message.reply_to,
            };

            if let Ok(json) = serde_json::to_string(&response) {
                let _ = broadcast_tx.send(json);
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    tracing::info!("WebSocket connection closed for user: {}", user_id_for_log);
}
