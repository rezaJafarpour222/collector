mod config;
mod fetch;
mod timestamps;
mod upload;

use anyhow::{Context, Result};
use chrono::Utc;
use fetch::Fetcher;
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use upload::Uploader;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = Arc::new(config::Config::from_env()?);

    // Run the job; log success or failure, but never let the process exit.
    match run_job(&cfg).await {
        Ok(()) => info!("job finished successfully"),
        Err(e) => error!(error = %format!("{e:#}"), "job failed"),
    }

    // Park forever. Container stays `Running` with ~0% CPU. Kill the container
    // (or the orchestrator's lifecycle) to stop it.
    info!("idling — container stays up; kill it to exit");
    std::future::pending::<()>().await;
    Ok(()) // unreachable at runtime
}

async fn run_job(cfg: &Arc<config::Config>) -> Result<()> {
    // 1. Timestamp list (Jalali, hourly)
    let timestamps = timestamps::generate_hourly_timestamps(&cfg.start_date, &cfg.end_date)?;
    let total = timestamps.len();
    info!(count = total, "generated hourly timestamps");

    // 2. Temp files
    let temp_dir = tempfile::tempdir()?;
    let jsonl_path = temp_dir.path().join("data.jsonl");
    let zst_path = temp_dir.path().join("data.jsonl.zst");

    // 3. Writer task
    let (tx, mut rx) = mpsc::channel::<(String, Result<serde_json::Value>)>(256);
    let writer_path = jsonl_path.clone();
    let writer = tokio::spawn(async move {
        let file = tokio::fs::File::create(&writer_path).await?;
        let mut buf = tokio::io::BufWriter::new(file);
        let mut succeeded = 0usize;
        let mut failures: Vec<String> = Vec::new();

        while let Some((ts, result)) = rx.recv().await {
            let line = match result {
                Ok(v) => {
                    succeeded += 1;
                    serde_json::json!({ "timestamp": ts, "data": v })
                }
                Err(e) => {
                    failures.push(ts.clone());
                    serde_json::json!({ "timestamp": ts, "error": e.to_string() })
                }
            };
            buf.write_all(serde_json::to_string(&line)?.as_bytes())
                .await?;
            buf.write_all(b"\n").await?;
        }

        let meta = serde_json::json!({
            "_meta": {
                "total": succeeded + failures.len(),
                "succeeded": succeeded,
                "failed": failures.len(),
                "failures": failures,
            }
        });
        buf.write_all(serde_json::to_string(&meta)?.as_bytes())
            .await?;
        buf.write_all(b"\n").await?;
        buf.flush().await?;

        Ok::<_, anyhow::Error>((succeeded, failures))
    });

    // 4. Fetch
    let fetcher = Arc::new(Fetcher::new(cfg.fetch_max_retries, cfg.fetch_timeout_secs)?);
    fetcher
        .warm_up()
        .await
        .context("failed to establish ASP.NET session cookie")?;

    let completed = Arc::new(AtomicUsize::new(0));
    let delay = std::time::Duration::from_millis(cfg.fetch_delay_ms);

    stream::iter(timestamps)
        .map(|ts| {
            let fetcher = Arc::clone(&fetcher);
            let tx = tx.clone();
            let completed = Arc::clone(&completed);
            async move {
                let result = fetcher.fetch_hour(&ts).await;
                if let Err(e) = tx.send((ts.clone(), result)).await {
                    warn!(ts, "writer closed: {e}");
                }
                let n = completed.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 100 == 0 || n == total {
                    info!(progress = %format!("{n}/{total}"), "fetch progress");
                }
                tokio::time::sleep(delay).await;
            }
        })
        .buffer_unordered(cfg.fetch_concurrency)
        .for_each(|_| async {})
        .await;

    drop(tx);
    let (succeeded, failures) = writer.await.context("writer panicked")??;
    info!(succeeded, failed = failures.len(), "fetch phase done");

    // 5. Compress
    info!("compressing with zstd");
    let jp = jsonl_path.clone();
    let zp = zst_path.clone();
    tokio::task::spawn_blocking(move || upload::compress_zstd(&jp, &zp)).await??;

    let raw = tokio::fs::metadata(&jsonl_path).await?.len();
    let zst = tokio::fs::metadata(&zst_path).await?.len();
    info!(
        raw_bytes = raw,
        zst_bytes = zst,
        ratio = format!("{:.1}%", (zst as f64 / raw.max(1) as f64) * 100.0),
        "compressed"
    );

    // 6. Upload
    let uploader = Uploader::new(
        &cfg.s3_endpoint,
        &cfg.s3_region,
        &cfg.s3_bucket,
        &cfg.s3_access_key,
        &cfg.s3_secret_key,
        cfg.s3_path_style,
        cfg.upload_max_retries,
    )
    .await?;

    let key = format!(
        "{}/{}_to_{}_{}.jsonl.zst",
        cfg.s3_key_prefix.trim_end_matches('/'),
        cfg.start_date.replace('/', "-"),
        cfg.end_date.replace('/', "-"),
        Utc::now().format("%Y%m%dT%H%M%SZ"),
    );
    info!(bucket = %cfg.s3_bucket, key = %key, "uploading");
    uploader.upload_file(&zst_path, &key).await?;

    // 7. Optional temp-file preservation
    if cfg.keep_temp {
        std::fs::create_dir_all("aqms-out")?;
        std::fs::copy(&jsonl_path, "aqms-out/data.jsonl")?;
        std::fs::copy(&zst_path, "aqms-out/data.jsonl.zst")?;
        info!("copied temp files to ./aqms-out/");
    }

    info!("done");
    Ok(())
}
