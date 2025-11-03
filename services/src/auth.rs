use async_trait::async_trait;
use axum::{
	extract::{FromRequestParts, FromRef},
	http::{request::Parts, StatusCode},
	response::{IntoResponse, Response},
	RequestPartsExt,
};
use axum_extra::{headers::{authorization::Bearer, Authorization}, TypedHeader};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, DecodingKey, Validation, Algorithm};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;

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
		let jwks = self.client.get(&self.jwks_url).send().await?.json::<JwkSet>().await?;
		Ok(jwks)
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
	pub sub: String,
	pub exp: usize,
}

pub async fn verify_token(token: &str, jwks_client: &JwksClient) -> anyhow::Result<Claims> {
	let header = decode_header(token)?;
	let kid = header.kid.ok_or_else(|| anyhow::anyhow!("Missing kid in token header"))?;

	let jwks = jwks_client.get_jwks().await?;
	let jwk = jwks.find(&kid).ok_or_else(|| anyhow::anyhow!("JWK not found for kid"))?;

	let mut validation = Validation::new(Algorithm::from_str(&jwk.common.key_algorithm.unwrap().to_string())?);
	validation.validate_exp = true;

	let decoding_key = DecodingKey::from_jwk(jwk)?;
	let token_data = decode::<Claims>(token, &decoding_key, &validation)?;

	Ok(token_data.claims)
}

#[async_trait]
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
    (PgPool, JwksClient): FromRef<S>,
{
	type Rejection = AuthError;

	async fn from_request_parts(
		parts: &mut Parts,
		state: &S,
	) -> Result<Self, Self::Rejection> {
		let TypedHeader(Authorization(bearer)) =
			parts.extract::<TypedHeader<Authorization<Bearer>>>().await.map_err(|_| AuthError::InvalidToken)?;

        let (_, jwks_client) = <(PgPool, JwksClient)>::from_ref(state);

		let claims = verify_token(bearer.token(), &jwks_client).await.map_err(|_| AuthError::InvalidToken)?;

		Ok(claims)
	}
}

pub enum AuthError {
	InvalidToken,
}

impl IntoResponse for AuthError {
	fn into_response(self) -> Response {
		let (status, error_message) = match self {
			AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
		};
		(status, error_message).into_response()
	}
}
