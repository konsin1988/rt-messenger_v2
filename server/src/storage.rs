use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::Client;

/// Create an S3 client configured for RustFS (S3-compatible).
/// `endpoint` must include scheme, e.g. `http://rustfs:9000`.
pub fn create_s3_client(endpoint: &str, access_key: &str, secret_key: &str) -> Client {
    let credentials = Credentials::new(access_key, secret_key, None, None, "rustfs");

    let s3_config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .endpoint_url(endpoint)
        .credentials_provider(credentials)
        .force_path_style(true)
        .build();

    Client::from_conf(s3_config)
}

/// Ensure `bucket` exists. Creates it if HEAD returns NotFound/404.
/// Uses HeadBucket + CreateBucket with retry handling for already-exists race.
pub async fn ensure_bucket_exists(client: &Client, bucket: &str) -> Result<()> {
    match client.head_bucket().bucket(bucket).send().await {
        Ok(_) => {
            tracing::info!("RustFS bucket already exists: {bucket}");
            return Ok(());
        }
        Err(err) => {
            // Inspect service error — if it's 404/NoSuchBucket, we need to create.
            // For other errors (e.g. 403, network), we treat as "not found" and still try to create,
            // then propagate if creation fails for a different reason.
            let service_err = err.as_service_error();
            if let Some(e) = service_err {
                tracing::warn!("HeadBucket failed for {bucket}: {e:?} — attempting create");
            } else {
                tracing::warn!("HeadBucket failed for {bucket}: {err:?} — attempting create");
            }
        }
    }

    // Try to create bucket; if it already exists (409 BucketAlreadyOwnedByYou) it's ok.
    match client.create_bucket().bucket(bucket).send().await {
        Ok(_) => {
            tracing::info!("Created RustFS bucket: {bucket}");
            Ok(())
        }
        Err(err) => {
            // If bucket already exists, treat as success
            if let Some(service_err) = err.as_service_error() {
                use aws_sdk_s3::error::ProvideErrorMetadata;
                let code = service_err
                    .message()
                    .unwrap_or_default()
                    .to_lowercase();
                // Some S3 impls return "BucketAlreadyOwnedByYou" / "BucketAlreadyExists"
                if code.contains("already") || code.contains("exists") {
                    tracing::info!("RustFS bucket already exists (race): {bucket}");
                    return Ok(());
                }
                // HeadBucket 404 case with xml error also falls here
                tracing::error!("CreateBucket failed for {bucket}: {service_err:?}");
            }
            // Check generic error string as fallback
            let err_str = err.to_string().to_lowercase();
            if err_str.contains("already") || err_str.contains("exists") {
                tracing::info!("RustFS bucket already exists (string match): {bucket}");
                return Ok(());
            }
            Err(err).context(format!("failed to create RustFS bucket '{bucket}'"))
        }
    }
}
