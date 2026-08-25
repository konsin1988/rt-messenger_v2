use sqlx::PgPool;

pub async fn test_pool() -> PgPool {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        // Fallback for CI / local: expand from .env template if needed
        let user = std::env::var("POSTGRES_USER").unwrap_or_else(|_| "messenger_user".into());
        let pass = std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "1234".into());
        let host = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".into());
        let port = std::env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".into());
        let db = std::env::var("POSTGRES_DB").unwrap_or_else(|_| "messenger".into());
        format!("postgres://{user}:{pass}@{host}:{port}/{db}")
    });
    let pool = PgPool::connect(&database_url)
        .await
        .expect("DATABASE_URL must be reachable for integration tests; ensure `docker compose up -d postgres`");
    // Ensure migrations are applied (idempotent)
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");
    pool
}

pub async fn clean_db(pool: &PgPool) {
    // Order matters due to FK
    let _ = sqlx::query("TRUNCATE phone_verification CASCADE").execute(pool).await;
    let _ = sqlx::query("TRUNCATE user_profile CASCADE").execute(pool).await;
    let _ = sqlx::query(r#"TRUNCATE "user" CASCADE"#).execute(pool).await;
}

pub fn test_config() -> (String, i64, i64, String) {
    let secret = "integration-test-secret-32chars-long!!".to_string();
    let jwt_exp = 3600i64;
    let otp_ttl = 300i64;
    let prefix = "".to_string();
    (secret, jwt_exp, otp_ttl, prefix)
}
