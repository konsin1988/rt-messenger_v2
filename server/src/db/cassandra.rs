use scylla::Session;
use scylla::transport::session::SessionConfig;

pub async fn create_session(cassandra_url: &str) -> anyhow::Result<Session> {
    let mut config = SessionConfig::new();
    
    // Extract host:port from URL like "cassandra://localhost:9042"
    let host_port = cassandra_url
        .strip_prefix("cassandra://")
        .unwrap_or(cassandra_url);
    
    config.add_known_node(host_port);

    let session = Session::connect(config).await?;

    tracing::info!("connected to cassandra");

    Ok(session)
}
