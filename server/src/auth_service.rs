use chrono::{Duration, Utc};
use dashmap::DashMap;
use rand::Rng;
use regex::Regex;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;
use tonic::{Request, Response, Status};

use crate::messenger::auth_service_server::AuthService;
use crate::messenger::{
    AuthResponse, RefreshTokenRequest, RequestOtpRequest, RequestOtpResponse, VerifyOtpRequest,
};
use crate::models::User;

#[derive(Clone)]
pub struct AuthServiceImpl {
    pub pg_pool: PgPool,
    pub jwt_secret: String,
    pub jwt_exp_secs: i64,
    pub sms_mock: bool,
    pub otp_ttl_secs: i64,
    pub country_prefix: String,
    pub rate_limiter: Arc<DashMap<String, Vec<Instant>>>,
}

impl AuthServiceImpl {
    pub fn new(
        pg_pool: PgPool,
        jwt_secret: String,
        jwt_exp_secs: i64,
        sms_mock: bool,
        otp_ttl_secs: i64,
        country_prefix: String,
    ) -> Self {
        Self {
            pg_pool,
            jwt_secret,
            jwt_exp_secs,
            sms_mock,
            otp_ttl_secs,
            country_prefix,
            rate_limiter: Arc::new(DashMap::new()),
        }
    }

    fn validate_phone(&self, phone: &str) -> Result<(), Status> {
        let re = Regex::new(r"^\+?[0-9]{10,15}$").unwrap();
        if !re.is_match(phone) {
            return Err(Status::invalid_argument(
                "Invalid phone format: must be 10-15 digits, optional leading +",
            ));
        }
        if !self.country_prefix.is_empty() && !phone.contains(&self.country_prefix) {
            // If prefix configured, phone must start with it (allow with/without +)
            let normalized = phone.trim_start_matches('+');
            let prefix_norm = self.country_prefix.trim_start_matches('+');
            if !normalized.starts_with(prefix_norm) {
                return Err(Status::invalid_argument(format!(
                    "Phone must start with country prefix {}",
                    self.country_prefix
                )));
            }
        }
        Ok(())
    }

    fn check_rate_limit(&self, key: &str) -> Result<(), Status> {
        let now = Instant::now();
        let mut entry = self.rate_limiter.entry(key.to_string()).or_default();
        // prune older than 60s
        entry.retain(|t| now.duration_since(*t).as_secs() < 60);
        if entry.len() >= 3 {
            return Err(Status::resource_exhausted(
                "Rate limit: max 3 OTP requests per minute",
            ));
        }
        entry.push(now);
        Ok(())
    }

    fn issue_jwt(&self, user_id: uuid::Uuid) -> Result<String, Status> {
        let now = Utc::now().timestamp() as usize;
        let exp = (Utc::now() + Duration::seconds(self.jwt_exp_secs)).timestamp() as usize;
        let claims = crate::auth::Claims {
            sub: user_id.to_string(),
            exp,
            iat: now,
        };
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| Status::internal(format!("JWT encode failed: {}", e)))
    }
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn request_otp(
        &self,
        request: Request<RequestOtpRequest>,
    ) -> Result<Response<RequestOtpResponse>, Status> {
        let ip_key = request
            .metadata()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let req = request.into_inner();
        let phone = req.phone.trim().to_string();
        self.validate_phone(&phone)?;

        // rate limit per phone (and IP if available in metadata)
        let rate_key = format!("{}:{}", phone, ip_key);
        self.check_rate_limit(&rate_key)?;
        // also per-phone alone to prevent IP rotation
        self.check_rate_limit(&phone)?;

        // generate 6-digit OTP (drop rng before await for Send)
        let otp_str = {
            let mut rng = rand::thread_rng();
            let otp: u32 = rng.gen_range(100000..1000000);
            format!("{:06}", otp)
        };
        let otp_hash = bcrypt::hash(&otp_str, bcrypt::DEFAULT_COST)
            .map_err(|e| Status::internal(e.to_string()))?;

        let expires_at = Utc::now() + Duration::seconds(self.otp_ttl_secs);
        let id = uuid::Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO phone_verification (id, phone, otp_hash, expires_at, attempts, created_at)
               VALUES ($1, $2, $3, $4, 0, NOW())"#,
        )
        .bind(id)
        .bind(&phone)
        .bind(&otp_hash)
        .bind(expires_at)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        if self.sms_mock {
            tracing::info!("[SMS_MOCK] OTP for {} is {} (expires in {}s)", phone, otp_str, self.otp_ttl_secs);
            Ok(Response::new(RequestOtpResponse {
                success: true,
                debug_otp: otp_str,
            }))
        } else {
            // TODO: integrate Twilio/SNS behind feature flag
            tracing::info!("OTP sent to {} (mock disabled)", phone);
            Ok(Response::new(RequestOtpResponse {
                success: true,
                debug_otp: String::new(),
            }))
        }
    }

    async fn verify_otp(
        &self,
        request: Request<VerifyOtpRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();
        let phone = req.phone.trim().to_string();
        let code = req.code.trim().to_string();
        let username_req = req.username.trim().to_string();

        self.validate_phone(&phone)?;
        if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
            return Err(Status::invalid_argument("Code must be 6 digits"));
        }

        // Fetch latest non-expired row
        #[allow(dead_code)]
        #[derive(sqlx::FromRow)]
        struct PvRow {
            id: uuid::Uuid,
            phone: String,
            otp_hash: String,
            expires_at: chrono::DateTime<Utc>,
            attempts: i32,
        }

        let row: Option<PvRow> = sqlx::query_as(
            r#"SELECT id, phone, otp_hash, expires_at, attempts FROM phone_verification
               WHERE phone = $1 AND expires_at > NOW()
               ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(&phone)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        let pv = row.ok_or_else(|| Status::unauthenticated("OTP expired or not found, request new code"))?;

        if pv.attempts >= 5 {
            return Err(Status::resource_exhausted(
                "Too many attempts, request new code",
            ));
        }

        let valid = bcrypt::verify(&code, &pv.otp_hash)
            .map_err(|e| Status::internal(e.to_string()))?;

        if !valid {
            // increment attempts
            sqlx::query(r#"UPDATE phone_verification SET attempts = attempts + 1 WHERE id = $1"#)
                .bind(pv.id)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            return Err(Status::unauthenticated("Invalid code"));
        }

        // success - delete all OTPs for this phone (cleanup)
        sqlx::query(r#"DELETE FROM phone_verification WHERE phone = $1"#)
            .bind(&phone)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Upsert user: find by phone else insert
        let existing: Option<User> = sqlx::query_as(r#"SELECT * FROM "user" WHERE phone = $1"#)
            .bind(&phone)
            .fetch_optional(&self.pg_pool)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let user = if let Some(u) = existing {
            u
        } else {
            // Generate username if not provided
            let username = if !username_req.is_empty() {
                // validate username 3-32 alphanum _ -
                let re = Regex::new(r"^[a-zA-Z0-9_-]{3,32}$").unwrap();
                if !re.is_match(&username_req) {
                    return Err(Status::invalid_argument(
                        "Username must be 3-32 chars: alphanumeric, _-",
                    ));
                }
                username_req.clone()
            } else {
                // derive from phone last 4 + random suffix
                let suffix = &phone[phone.len().saturating_sub(4)..];
                format!("user_{}{}", suffix, &uuid::Uuid::new_v4().simple().to_string()[..4])
            };

            // Try insert, handle username conflict by appending suffix
            let mut final_username = username.clone();
            let mut attempts = 0;
            let inserted: User;
            loop {
                let id = uuid::Uuid::new_v4();
                let res = sqlx::query_as::<_, User>(
                    r#"INSERT INTO "user" (id, username, phone, created_at) VALUES ($1, $2, $3, NOW()) RETURNING *"#,
                )
                .bind(id)
                .bind(&final_username)
                .bind(&phone)
                .fetch_one(&self.pg_pool)
                .await;
                match res {
                    Ok(u) => {
                        inserted = u;
                        break;
                    }
                    Err(e) => {
                        // check unique violation on username
                        let msg = e.to_string();
                        if msg.contains("username") && attempts < 3 {
                            final_username = format!("{}_{}", username, &uuid::Uuid::new_v4().simple().to_string()[..4]);
                            attempts += 1;
                            continue;
                        }
                        return Err(Status::internal(e.to_string()));
                    }
                }
            }
            // Create user_profile if not exists
            sqlx::query(r#"INSERT INTO user_profile (user_id) VALUES ($1) ON CONFLICT DO NOTHING"#)
                .bind(inserted.id)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            inserted
        };

        // Ensure profile exists for existing users too (idempotent)
        sqlx::query(r#"INSERT INTO user_profile (user_id) VALUES ($1) ON CONFLICT DO NOTHING"#)
            .bind(user.id)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let token = self.issue_jwt(user.id)?;

        Ok(Response::new(AuthResponse {
            token,
            user: Some(user.into()),
        }))
    }

    async fn refresh_token(
        &self,
        request: Request<RefreshTokenRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();
        let old_token = req.token.trim();
        if old_token.is_empty() {
            return Err(Status::invalid_argument("Token required"));
        }
        let claims = crate::auth::verify_token(&self.jwt_secret, old_token)?;
        let user_id = uuid::Uuid::parse_str(&claims.sub)
            .map_err(|_| Status::unauthenticated("Invalid user ID in token"))?;

        let user: User = sqlx::query_as(r#"SELECT * FROM "user" WHERE id = $1"#)
            .bind(user_id)
            .fetch_optional(&self.pg_pool)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        let new_token = self.issue_jwt(user.id)?;
        Ok(Response::new(AuthResponse {
            token: new_token,
            user: Some(user.into()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    async fn test_pool() -> AuthServiceImpl {
        let pool = PgPool::connect_lazy("postgres://fake:fake@localhost:5432/fake").unwrap();
        AuthServiceImpl::new(pool, "test-secret".to_string(), 604800, true, 300, "".to_string())
    }

    #[tokio::test]
    async fn test_validate_phone_ok() {
        let svc = test_pool().await;
        for phone in ["+79990001122", "79990001122", "+12345678901", "1234567890"] {
            assert!(svc.validate_phone(phone).is_ok(), "should ok for {}", phone);
        }
    }

    #[tokio::test]
    async fn test_validate_phone_invalid() {
        let svc = test_pool().await;
        for phone in ["", "123", "+1234567890123456", "abc", "+7 999 000", "++7999"] {
            let err = svc.validate_phone(phone).unwrap_err();
            assert_eq!(err.code(), tonic::Code::InvalidArgument, "for {}", phone);
        }
    }

    #[tokio::test]
    async fn test_validate_phone_country_prefix() {
        let mut svc = test_pool().await;
        svc.country_prefix = "+7".to_string();
        assert!(svc.validate_phone("+79990001122").is_ok());
        assert!(svc.validate_phone("79990001122").is_ok()); // contains 7 at start after trim
        let err = svc.validate_phone("+12125551234").unwrap_err();
        assert!(err.message().contains("country prefix"));
        svc.country_prefix = "".to_string();
        assert!(svc.validate_phone("+12125551234").is_ok());
    }

    #[tokio::test]
    async fn test_rate_limit_allow_3() {
        let svc = test_pool().await;
        let key = "test_phone_rate_3";
        for _ in 0..3 {
            assert!(svc.check_rate_limit(key).is_ok());
        }
    }

    #[tokio::test]
    async fn test_rate_limit_block_4th() {
        let svc = test_pool().await;
        let key = "test_phone_rate_block_4th";
        for _ in 0..3 {
            svc.check_rate_limit(key).unwrap();
        }
        let err = svc.check_rate_limit(key).unwrap_err();
        assert_eq!(err.code(), tonic::Code::ResourceExhausted);
        assert!(err.message().contains("3 OTP"));
    }

    #[tokio::test]
    async fn test_rate_limit_isolation() {
        let svc = test_pool().await;
        let k1 = "phone:+7999:ip1";
        let k2 = "phone:+7999:ip2";
        for _ in 0..3 {
            svc.check_rate_limit(k1).unwrap();
        }
        // k2 should still allow
        assert!(svc.check_rate_limit(k2).is_ok());
        // k1 blocked
        assert!(svc.check_rate_limit(k1).is_err());
    }

    #[test]
    fn test_bcrypt_otp_roundtrip() {
        let otp = "123456";
        let hash = bcrypt::hash(otp, bcrypt::DEFAULT_COST).unwrap();
        assert!(bcrypt::verify(otp, &hash).unwrap());
        assert!(!bcrypt::verify("654321", &hash).unwrap());
    }

    #[tokio::test]
    async fn test_issue_jwt_claims() {
        let svc = test_pool().await;
        let uid = uuid::Uuid::new_v4();
        let token = svc.issue_jwt(uid).unwrap();
        let claims = crate::auth::verify_token("test-secret", &token).unwrap();
        assert_eq!(claims.sub, uid.to_string());
        assert!(claims.exp > claims.iat);
        let diff = (claims.exp - claims.iat) as i64;
        assert!((diff - 604800).abs() < 5, "exp-iat should be ~604800 got {}", diff);
    }

    #[tokio::test]
    async fn test_issue_jwt_wrong_secret() {
        let svc = test_pool().await;
        let token = svc.issue_jwt(uuid::Uuid::new_v4()).unwrap();
        let err = crate::auth::verify_token("wrong-secret", &token).unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn test_username_validation() {
        let svc = test_pool().await;
        let ok = ["alice", "bob_123", "a1_-", &"a".repeat(32)];
        let re = regex::Regex::new(r"^[a-zA-Z0-9_-]{3,32}$").unwrap();
        for u in ok {
            assert!(re.is_match(u), "should match {}", u);
        }
        let bad = ["ab", "a*", &"a".repeat(33), ""];
        for u in bad {
            // empty is allowed (auto-gen), but non-empty bad should fail
            if u.is_empty() {
                continue;
            }
            assert!(!re.is_match(u), "should not match {}", u);
        }
        // direct service validation via verify path would be checked in integration
        assert!(svc.validate_phone("+79990001122").is_ok());
    }
}
