use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tonic::{metadata::MetadataMap, Request, Status};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Clone)]
pub struct AuthInterceptor {
    jwt_secret: Arc<String>,
}

impl AuthInterceptor {
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_secret: Arc::new(jwt_secret),
        }
    }

    pub fn extract_user_id(&self, metadata: &MetadataMap) -> Result<uuid::Uuid, Status> {
        let auth_header = metadata
            .get("authorization")
            .ok_or_else(|| Status::unauthenticated("Missing authorization header"))?
            .to_str()
            .map_err(|_| Status::unauthenticated("Invalid authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| Status::unauthenticated("Invalid token format"))?;

        let validation = Validation::new(Algorithm::HS256);
        let decoding_key = DecodingKey::from_secret(self.jwt_secret.as_bytes());

        let token_data = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|e| Status::unauthenticated(format!("Invalid token: {}", e)))?;

        uuid::Uuid::parse_str(&token_data.claims.sub)
            .map_err(|_| Status::unauthenticated("Invalid user ID in token"))
    }
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let user_id = self.extract_user_id(request.metadata())?;
        request.extensions_mut().insert(user_id);
        Ok(request)
    }
}

pub fn verify_token(jwt_secret: &str, token: &str) -> Result<Claims, Status> {
    let validation = Validation::new(Algorithm::HS256);
    let decoding_key = DecodingKey::from_secret(jwt_secret.as_bytes());
    decode::<Claims>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|e| Status::unauthenticated(format!("Invalid token: {}", e)))
}