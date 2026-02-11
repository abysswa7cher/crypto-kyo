// src/middleware/auth.rs
use crate::{
    error::AppError,
    services::Claims,
    AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};

impl FromRequestParts<AppState> for Claims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract the token from the authorization header
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| AppError::Validation("Missing authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Validation("Invalid authorization header format".to_string()))?;

        // Validate token and extract claims
        state
            .jwt_service
            .validate_token(token)
            .map_err(|e| AppError::Validation(format!("Invalid token: {}", e)))
    }
}