use redis::Client;

/// Create a Redis client from `redis_url` (e.g. `redis://redis:6379`).
/// The client is cheap to clone; get a multiplexed connection per request via
/// `client.get_multiplexed_tokio_connection().await` or use `redis::aio::ConnectionManager`.
pub fn create_client(redis_url: &str) -> anyhow::Result<Client> {
    let client = Client::open(redis_url)?;
    tracing::info!("created redis client for {}", redis_url);
    Ok(client)
}

/// Ping redis to verify connectivity. Used at startup (fail-fast).
pub async fn ping(client: &Client) -> anyhow::Result<()> {
    let mut conn = client.get_multiplexed_tokio_connection().await?;
    let pong: String = redis::cmd("PING").query_async(&mut conn).await?;
    if pong.to_uppercase() != "PONG" {
        anyhow::bail!("unexpected redis PING response: {pong}");
    }
    tracing::info!("connected to redis (PING=PONG)");
    Ok(())
}
