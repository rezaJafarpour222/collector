use anyhow::{Context, Result};
use std::env;

pub struct Config {
    pub start_date: String,
    pub end_date: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_key_prefix: String,
    pub s3_path_style: bool,
    pub fetch_concurrency: usize,
    pub fetch_delay_ms: u64,
    pub fetch_max_retries: u32,
    pub fetch_timeout_secs: u64,
    pub upload_max_retries: u32,
    pub keep_temp: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bool_env = |k: &str, default: bool| -> bool {
            env::var(k)
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(default)
        };
        let num_env = |k: &str, default: u64| -> u64 {
            env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };

        Ok(Self {
            start_date: env::var("START_DATE").context("START_DATE required")?,
            end_date: env::var("END_DATE").context("END_DATE required")?,
            s3_endpoint: env::var("S3_ENDPOINT").context("S3_ENDPOINT required")?,
            s3_region: env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into()),
            s3_bucket: env::var("S3_BUCKET").context("S3_BUCKET required")?,
            s3_access_key: env::var("S3_ACCESS_KEY_ID").context("S3_ACCESS_KEY_ID required")?,
            s3_secret_key: env::var("S3_SECRET_ACCESS_KEY")
                .context("S3_SECRET_ACCESS_KEY required")?,
            s3_key_prefix: env::var("S3_KEY_PREFIX").unwrap_or_else(|_| "aqms".into()),
            s3_path_style: bool_env("S3_PATH_STYLE", false),
            fetch_concurrency: num_env("FETCH_CONCURRENCY", 4).max(1) as usize,
            fetch_delay_ms: num_env("FETCH_DELAY_MS", 1000),
            fetch_max_retries: num_env("FETCH_MAX_RETRIES", 1) as u32,
            fetch_timeout_secs: num_env("FETCH_TIMEOUT_SECS", 30),
            upload_max_retries: num_env("UPLOAD_MAX_RETRIES", 3) as u32,
            keep_temp: bool_env("KEEP_TEMP", false),
        })
    }
}
