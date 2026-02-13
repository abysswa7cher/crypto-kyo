use crate::{
    error::{AppError, Result},
    models::{CreateUserRequest, User, UserResponse},
    AppState,
};
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use serde::Deserialize;
use uuid::Uuid;
use chrono::Duration;


pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>)> {
    
    //Validate invitation token
    let invitation = sqlx::query_as::<_, crate::models::Invitation>(
        r#"
        SELECT token, created_by, expires_at, used_by, used_at, created_at
        FROM invitations
        WHERE token = $1 AND used_by IS NULL AND expires_at > NOW()
        "#
    )
        .bind(&payload.invitation_token)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| AppError::Database(e))?
        .ok_or_else(|| AppError::Validation("Invalid or expired invitation token".to_string()))?;


    //Validate input
    if payload.username.len() < 3 {
        return Err(AppError::Validation(
            "Username must be at least 3 characters long".to_string(),
        ));
    }

    if payload.password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters long".to_string(),
        ));
    }

    //Hash pwd
    let password_hash = hash(&payload.password, DEFAULT_COST)?;

    //Create user
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (username, email, password_hash, is_admin)
        VALUES ($1, $2, $3, false)
        RETURNING id, username, email, password_hash, is_admin, created_at, last_seen
        "#
    )
        .bind(&payload.username)
        .bind(&payload.email)
        .bind(&password_hash)
        .fetch_one(&state.db)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.is_unique_violation() {
                    return AppError::Validation("Username or email already used".to_string());
                }
            }
            AppError::Database(e)
        })?;

    sqlx::query(
        r#"
        UPDATE invitations
        SET used_by = $1, used_at = NOW()
        WHERE token = $2
        "#
    )
    .bind(&user.id)
    .bind(&invitation.token)
    .execute(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?;
    
    tracing::info!("User crated: {} ({})", user.username, user.id);

    Ok((StatusCode::CREATED, Json(user.into())))
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<crate::models::LoginRequest>,
) -> Result<Json<crate::services::AuthResponse>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, email, password_hash, is_admin, created_at, last_seen
        FROM users
        WHERE email = $1
        "#
    )
    .bind(&payload.email)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?
    .ok_or_else(|| AppError::Validation("Invalid credentials".to_string()))?;

    let is_valid = verify(&payload.password, &user.password_hash)?;

    if !is_valid {
        return Err(AppError::Validation("Invalid credentials".to_string()));
    }

    sqlx::query(
        "UPDATE users SET last_seen = NOW() WHERE id = $1"
    )
    .bind(&user.id)
    .execute(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?;

    let access_token = state
        .jwt_service
        .generate_access_token(&user)
        .map_err(|e| AppError::Internal(format!("Token generation failed: {}", e)))?;
    
    let refresh_token = state.jwt_service.generate_refresh_token();
    let refresh_expires = chrono::Utc::now() + Duration::days(state.jwt_service.refresh_expiry_days());
    
    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (user_id, token, expires_at)
        VALUES ($1, $2, $3)
        "#
    )
    .bind(&user.id)
    .bind(&refresh_token)
    .bind(&refresh_expires)
    .execute(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?;
    
    tracing::info!("User logged in: {} ({})", user.username, user.id);

    Ok(Json(crate::services::AuthResponse {
        user: user.into(),
        access_token,
        refresh_token,
        expires_in: state.jwt_service.expiry_seconds(),
    }))
}

pub async fn create_invitation(
    State(state): State<AppState>,
    claims: crate::services::Claims,  // Now extracts from JWT automatically
) -> Result<Json<crate::models::InvitationResponse>> {


    // Check if user is admin (from JWT claims)
    if !claims.is_admin {
        return Err(AppError::Validation("Admin access required".to_string()));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Internal("Invalid user ID in token".to_string()))?;

    // Generate token
    let token = Uuid::new_v4().to_string();
    let expiry_hours = std::env::var("INVITATION_EXPIRY_HOURS")
        .unwrap_or_else(|_| "48".to_string())
        .parse::<i64>()
        .unwrap_or(48);
    let expires_at = chrono::Utc::now() + Duration::hours(expiry_hours);

    // Create invitation
    sqlx::query(
        r#"
        INSERT INTO invitations (token, created_by, expires_at)
        VALUES ($1, $2, $3)
        "#
    )
    .bind(&token)
    .bind(&user_id)
    .bind(&expires_at)
    .execute(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?;

    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:4200".to_string());
    let invite_url = format!("{}/register?token={}", frontend_url, token);

    tracing::info!("Invitation created by user {}", user_id);

    Ok(Json(crate::models::InvitationResponse {
        token,
        invite_url,
        expires_at,
    }))
}

pub async fn get_current_user(
    claims: crate::services::Claims,
) -> Result<Json<serde_json::Value>> {
    use serde_json::json;
    
    Ok(Json(json!({
        "id": claims.sub,
        "username": claims.username,
        "email": claims.email,
        "is_admin": claims.is_admin,
    })))
}

#[derive(Deserialize)]
pub struct StegoTestRequest {
    pub message: String,
}

#[derive(serde::Serialize)]
pub struct StegoTestResponse {
    pub original: String,
    pub encoded: String,
    pub decoded: String,
    pub matches: bool,
}

pub async fn test_steganography(
    State(state): State<AppState>,
    Json(payload): Json<StegoTestRequest>,
) -> Result<Json<StegoTestResponse>> {
    let encoded = state.stego_service.encode(&payload.message)?;
    let decoded = state.stego_service.decode(&encoded)?;
    
    Ok(Json(StegoTestResponse {
        original: payload.message.clone(),
        encoded: encoded.clone(),
        decoded: decoded.clone(),
        matches: payload.message == decoded,
    }))
}

#[derive(Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}


pub async fn refresh_token(
    State(state): State<AppState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Result<Json<crate::services::AuthResponse>> {
    use chrono::Duration;

    // Validate refresh token
    let token_record = sqlx::query_as::<_, crate::models::RefreshToken>(
        r#"
        SELECT id, user_id, token, expires_at, created_at, revoked
        FROM refresh_tokens
        WHERE token = $1 AND expires_at > NOW() AND revoked = false
        "#
    )
    .bind(&payload.refresh_token)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?
    .ok_or_else(|| AppError::Validation("Invalid or expired refresh token".to_string()))?;

    // Get user
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, email, password_hash, is_admin, created_at, last_seen
        FROM users
        WHERE id = $1
        "#
    )
    .bind(&token_record.user_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?;

    // Revoke old refresh token
    sqlx::query(
        "UPDATE refresh_tokens SET revoked = true WHERE id = $1"
    )
    .bind(token_record.id)
    .execute(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?;

    // Generate new tokens
    let access_token = state
        .jwt_service
        .generate_access_token(&user)
        .map_err(|e| AppError::Internal(format!("Token generation failed: {}", e)))?;
    
    let refresh_token = state.jwt_service.generate_refresh_token();
    let refresh_expires = chrono::Utc::now() + Duration::days(state.jwt_service.refresh_expiry_days());

    // Store new refresh token
    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (user_id, token, expires_at)
        VALUES ($1, $2, $3)
        "#
    )
    .bind(&user.id)
    .bind(&refresh_token)
    .bind(&refresh_expires)
    .execute(&state.db)
    .await
    .map_err(|e| AppError::Database(e))?;

    tracing::info!("Token refreshed for user: {}", user.id);

    Ok(Json(crate::services::AuthResponse {
        user: user.into(),
        access_token,
        refresh_token,
        expires_in: state.jwt_service.expiry_seconds(),
    }))
}