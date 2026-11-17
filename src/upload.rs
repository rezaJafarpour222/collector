use anyhow::{Context, Result};
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use std::path::Path;
use tracing::info;

pub struct Uploader {
    client: S3Client,
    bucket: String,
    max_retries: u32,
}

impl Uploader {
    pub async fn new(
        endpoint: &str,
        region: &str,
        bucket: &str,
        access_key: &str,
        secret_key: &str,
        path_style: bool,
        max_retries: u32,
    ) -> Result<Self> {
        let creds = Credentials::new(access_key, secret_key, None, None, "aqms");
        let cfg = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(creds)
            .region(Region::new(region.to_string()))
            .endpoint_url(endpoint)
            .force_path_style(path_style)
            .build();
        Ok(Self {
            client: S3Client::from_conf(cfg),
            bucket: bucket.to_string(),
            max_retries,
        })
    }

    pub async fn upload_file(&self, path: &Path, key: &str) -> Result<()> {
        let attempts = self.max_retries + 1;
        let mut last_err: Option<anyhow::Error> = None;

        for attempt in 0..attempts {
            if attempt > 0 {
                let backoff = std::time::Duration::from_secs(1u64 << attempt);
                info!(attempt, backoff_s = backoff.as_secs(), "retrying upload");
                tokio::time::sleep(backoff).await;
            }

            let body = match ByteStream::from_path(path).await {
                Ok(b) => b,
                Err(e) => {
                    last_err = Some(anyhow::Error::from(e));
                    continue;
                }
            };

            match self
                .client
                .put_object()
                .bucket(self.bucket.clone())
                .key(key)
                .body(body)
                .content_type("application/zstd")
                .send()
                .await
            {
                Ok(_) => {
                    info!(key, "upload ok");
                    return Ok(());
                }
                Err(e) => {
                    last_err = Some(anyhow::anyhow!("S3 upload failed: {e:?}"));
                }
            }
        }
        Err(last_err
            .unwrap_or_else(|| anyhow::anyhow!("upload failed"))
            .context("upload exhausted retries"))
    }
}

/// Streaming zstd compression. Run this inside `spawn_blocking`.
pub fn compress_zstd(input: &Path, output: &Path) -> Result<()> {
    let mut src = std::fs::File::open(input)?;
    let dst = std::fs::File::create(output)?;
    let mut enc = zstd::Encoder::new(dst, 3)?;
    std::io::copy(&mut src, &mut enc)?;
    enc.finish()?;
    Ok(())
}
