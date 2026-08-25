use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_addr: String,
    pub database_url: String,
    pub cassandra_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub jwt_exp_secs: i64,
    pub sms_mock: bool,
    pub otp_ttl_secs: i64,
    pub otp_country_prefix: String,
    pub rustfs_endpoint: String,
    pub rustfs_bucket: String,
    pub rustfs_access_key: String,
    pub rustfs_secret_key: String,
}

fn expand_vars(s: String) -> String {
    // Expand ${VAR} and ${VAR:-default} using current env
    let re = regex::Regex::new(r"\$\{([^}:]+)(?::-(.*?))?\}").unwrap();
    re.replace_all(&s, |caps: &regex::Captures| {
        let key = &caps[1];
        let default = caps.get(2).map(|m| m.as_str());
        std::env::var(key).unwrap_or_else(|_| default.unwrap_or("").to_string())
    })
    .into_owned()
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            server_addr: std::env::var("SERVER_HOST")
                .unwrap_or_else(|_| "0.0.0.0".into())
                + ":"
                + &std::env::var("SERVER_PORT").unwrap_or_else(|_| "50051".into()),
            database_url: expand_vars(
                std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            ),
            cassandra_url: expand_vars(
                std::env::var("CASSANDRA_URL").expect("CASSANDRA_URL must be set"),
            ),
            redis_url: expand_vars(
                std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379".into()),
            ),
            jwt_secret: std::env::var("JWT_SECRET")
                .expect("JWT_SECRET must be set"),
            jwt_exp_secs: std::env::var("JWT_EXP_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7 * 24 * 3600),
            sms_mock: std::env::var("SMS_MOCK")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
            otp_ttl_secs: std::env::var("OTP_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            otp_country_prefix: std::env::var("OTP_COUNTRY_PREFIX")
                .unwrap_or_else(|_| "".into()),
            rustfs_endpoint: std::env::var("RUSTFS_ENDPOINT")
                .unwrap_or_else(|_| "http://rustfs:9000".into()),
            rustfs_bucket: std::env::var("RUSTFS_BUCKET")
                .unwrap_or_else(|_| "messenger-attachments".into()),
            rustfs_access_key: std::env::var("RUSTFS_ACCESS_KEY")
                .unwrap_or_else(|_| "rustfsadmin".into()),
            rustfs_secret_key: std::env::var("RUSTFS_SECRET_KEY")
                .unwrap_or_else(|_| "rustfsadmin".into()),
        })
    }
}
