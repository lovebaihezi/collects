use jsonwebtoken::{decode, decode_header, jwk::JwkSet, DecodingKey, Validation, Algorithm};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

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

#[derive(Debug, Serialize, Deserialize, Clone)]
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


use axum::body::Body;

pub async fn auth_middleware(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    if let Some(auth_header) = auth_header {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            let jwks_client = req.extensions().get::<JwksClient>().unwrap();
            let claims = verify_token(token, jwks_client).await;
            if let Ok(claims) = claims {
                req.extensions_mut().insert(claims);
                let res = next.run(req).await;
                return Ok(res);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}
