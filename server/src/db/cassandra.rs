use scylla::Session;
use scylla::transport::session::SessionConfig;

pub async fn create_session(cassandra_url: &str) -> anyhow::Result<Session> {
    let mut config = SessionConfig::new();
    config.add_known_node(cassandra_url);

    let session = Session::connect(config).await?;

    tracing::info!("connected to cassandra");

    Ok(session)
}
