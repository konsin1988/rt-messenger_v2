use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_addr: String,
    pub database_url: String,
    pub cassandra_url: String,
    pub jwt_secret: String,
    pub rustfs_endpoint: String,
    pub rustfs_bucket: String,
    pub rustfs_access_key: String,
    pub rustfs_secret_key: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            server_addr: std::env::var("SERVER_HOST")
                .unwrap_or_else(|_| "0.0.0.0".into())
                + ":"
                + &std::env::var("SERVER_PORT").unwrap_or_else(|_| "8080".into()),
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            cassandra_url: std::env::var("CASSANDRA_URL")
                .expect("CASSANDRA_URL must be set"),
            jwt_secret: std::env::var("JWT_SECRET")
                .expect("JWT_SECRET must be set"),
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
