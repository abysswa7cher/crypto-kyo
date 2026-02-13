// src/handlers/messages.rs
use crate::{
    error::{AppError, Result},
    models::{Message, MessageResponse},
    services::Claims,
    AppState,
};
use axum::{extract::State, Json};

// src/handlers/messages.rs
pub async fn get_messages(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<MessageResponse>>> {
    let messages = sqlx::query_as::<_, Message>(
        r#"
        SELECT id, user_id, content, created_at, edited_at, reply_to
        FROM messages
        ORDER BY created_at DESC
        LIMIT 100
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?;
    
    let mut responses = Vec::new();
    
    for message in messages {
        // NO DECRYPTION - send encrypted content to client
        // Client will decrypt with their salt
        
        let username = if let Some(uid) = message.user_id {
            sqlx::query_scalar(
                "SELECT username FROM users WHERE id = $1",
            )
            .bind(&uid)
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
            content: message.content,  // Send encrypted content
            created_at: message.created_at,
            edited_at: message.edited_at,
            reply_to: message.reply_to,
        });
    }
    
    responses.reverse();
    Ok(Json(responses))
}