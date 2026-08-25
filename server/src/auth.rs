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

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn make_token(secret: &str, sub: &str, exp_offset_secs: i64) -> String {
        let now = chrono::Utc::now().timestamp() as usize;
        let exp = (chrono::Utc::now() + chrono::Duration::seconds(exp_offset_secs)).timestamp() as usize;
        let claims = Claims { sub: sub.to_string(), exp, iat: now };
        encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
    }

    #[test]
    fn test_verify_token_ok() {
        let token = make_token("secret123", "550e8400-e29b-41d4-a716-446655440000", 3600);
        let claims = verify_token("secret123", &token).unwrap();
        assert_eq!(claims.sub, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_verify_token_wrong_secret() {
        let token = make_token("secret123", "sub", 3600);
        let err = verify_token("wrong", &token).unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn test_verify_token_expired() {
        // Use exp = 1 (epoch) to guarantee expiry regardless of leeway
        let claims = Claims { sub: "sub".to_string(), exp: 1, iat: 1 };
        let token = encode(&Header::default(), &claims, &EncodingKey::from_secret("secret123".as_bytes())).unwrap();
        let err = verify_token("secret123", &token).unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unauthenticated);
        assert!(err.message().contains("ExpiredSignature") || err.message().contains("Invalid token"));
    }

    #[test]
    fn test_extract_user_id_via_metadata() {
        let secret = "test-secret".to_string();
        let interceptor = AuthInterceptor::new(secret.clone());
        let user_id = uuid::Uuid::new_v4();
        let token = make_token(&secret, &user_id.to_string(), 3600);
        let mut meta = tonic::metadata::MetadataMap::new();
        meta.insert("authorization", format!("Bearer {}", token).parse().unwrap());
        let got = interceptor.extract_user_id(&meta).unwrap();
        assert_eq!(got, user_id);
    }

    #[test]
    fn test_extract_user_id_missing_header() {
        let interceptor = AuthInterceptor::new("secret".to_string());
        let meta = tonic::metadata::MetadataMap::new();
        let err = interceptor.extract_user_id(&meta).unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unauthenticated);
    }
}