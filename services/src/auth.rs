use axum::response::IntoResponse;
use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid token format")]
    InvalidToken,
    #[error("Missing 'kid' in token header")]
    MissingKid,
    #[error("Could not find key in JWKS")]
    KeyNotFound,
    #[error("Failed to fetch JWKS")]
    JwksFetchError(#[from] reqwest::Error),
    #[error("Token validation error")]
    JwtError(#[from] jsonwebtoken::errors::Error),
    #[error("Internal server error")]
    InternalError(#[from] anyhow::Error),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        tracing::error!("Authentication error: {:?}", self);
        (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct JwksClient {
    client: Client,
    jwks_url: String,
}

impl JwksClient {
    pub fn new(clerk_frontend_api: String) -> Self {
        let client = Client::builder().build().unwrap();
        Self {
            client,
            jwks_url: format!("https://{}/.well-known/jwks.json", clerk_frontend_api),
        }
    }

    pub async fn get_jwks(&self) -> anyhow::Result<JwkSet> {
        let jwks = self
            .client
            .get(&self.jwks_url)
            .send()
            .await?
            .json::<JwkSet>()
            .await?;
        Ok(jwks)
    }
}

use axum::body::Body;

pub async fn auth_middleware(mut req: Request<Body>, next: Next) -> Result<Response, AuthError> {
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    if let Some(auth_header) = auth_header
        && let Some(token) = auth_header.strip_prefix("Bearer ")
    {
        let jwks_client = req.extensions().get::<Arc<JwksClient>>().unwrap();
        let claims = verify_token(token, jwks_client).await?;
        req.extensions_mut().insert(claims);
        let res = next.run(req).await;
        return Ok(res);
    }

    Err(AuthError::InvalidToken)
}

pub async fn verify_token(token: &str, jwks_client: &Arc<JwksClient>) -> Result<Claims, AuthError> {
    let header = decode_header(token)?;
    let kid = header.kid.ok_or(AuthError::MissingKid)?;

    let jwks = jwks_client.get_jwks().await?;
    let jwk = jwks.find(&kid).ok_or(AuthError::KeyNotFound)?;

    let mut validation = Validation::new(Algorithm::from_str(
        &jwk.common.key_algorithm.unwrap().to_string(),
    )?);
    validation.validate_exp = true;

    let decoding_key = DecodingKey::from_jwk(jwk)?;
    let token_data = decode::<Claims>(token, &decoding_key, &validation)?;

    Ok(token_data.claims)
}
