use crate::models::User;
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::RngExt;
use rand::distr::Alphanumeric;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub username: String,
    pub is_admin: bool,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Clone)]
pub struct JwtService {
    secret: String,
    access_expiry: i64,
}

impl JwtService {
    pub fn new(secret: String, access_expiry: i64) -> Self {
        Self {
            secret,
            access_expiry,
        }
    }

    pub fn generate_access_token(
        &self,
        user: &User,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.access_expiry);

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            username: user.username.clone(),
            is_admin: user.is_admin,
            exp: exp.timestamp(),
            iat: now.timestamp(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
    }

    pub fn expiry_seconds(&self) -> i64 {
        self.access_expiry
    }

    pub fn generate_refresh_token(&self) -> String {
        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect()
    }

    pub fn refresh_expiry_days(&self) -> i64 {
        7
    }
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: crate::models::UserResponse,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}
