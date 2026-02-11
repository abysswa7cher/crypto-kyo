// src/handlers/messages.rs
use crate::{
    error::{AppError, Result},
    models::{Message, MessageResponse},
    services::Claims,
    AppState,
};
use axum::{extract::State, Json};

pub async fn get_messages(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<MessageResponse>>> {
    // Fetch recent messages (last 100)
    let messages = sqlx::query_as!(
        Message,
        r#"
        SELECT id, user_id, content, created_at as "created_at!", edited_at, reply_to
        FROM messages
        ORDER BY created_at DESC
        LIMIT 100
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?;
    
    // Decode messages and fetch usernames
    let mut responses = Vec::new();
    
    for message in messages {
        // Decode content
        let decoded_content = state.stego_service.decode(&message.content)?;
        
        // Get username if user_id exists
        let username = if let Some(uid) = message.user_id {
            sqlx::query_scalar!(
                "SELECT username FROM users WHERE id = $1",
                uid
            )
            .fetch_optional(&state.db)
            .await
            .map_err(|e| AppError::Database(e))?
        } else {
            None
        };
        
        responses.push(MessageResponse {
            id: message.id,
            user_id: message.user_id,
            username,
            content: decoded_content,
            created_at: message.created_at,
            edited_at: message.edited_at,
            reply_to: message.reply_to,
        });
    }
    
    // Reverse to get chronological order (oldest first)
    responses.reverse();
    
    Ok(Json(responses))
}