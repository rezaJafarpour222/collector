use anyhow::Result;
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT,
};
use serde_json::Value;
use std::time::Duration as StdDuration;
use tokio::time::sleep;
use tracing::{debug, warn};

const URL: &str = "https://aqms.doe.ir/Home/GetAQIData/";

pub struct Fetcher {
    client: reqwest::Client,
    max_retries: u32,
}

impl Fetcher {
    pub fn new(max_retries: u32, timeout_secs: u64) -> Result<Self> {
        let mut h = HeaderMap::new();
        h.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (X11; Linux x86_64; rv:159.0) Gecko/20100101 Firefox/159.0",
            ),
        );
        h.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/javascript, */*; q=0.01"),
        );
        h.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("en-US,en;q=0.9,fa;q=0.8"),
        );
        h.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded; charset=UTF-8"),
        );
        h.insert(ORIGIN, HeaderValue::from_static("https://aqms.doe.ir"));
        h.insert(
            REFERER,
            HeaderValue::from_static("https://aqms.doe.ir/Home/AQI"),
        );
        h.insert(
            "x-requested-with",
            HeaderValue::from_static("XMLHttpRequest"),
        );

        let client = reqwest::Client::builder()
            .default_headers(h)
            .timeout(StdDuration::from_secs(timeout_secs))
            .cookie_store(true)
            .build()?;

        Ok(Self {
            client,
            max_retries,
        })
    }

    pub async fn fetch_hour(&self, ts: &str) -> Result<Value> {
        let attempts = self.max_retries + 1;
        let mut last_err: Option<anyhow::Error> = None;

        for attempt in 0..attempts {
            if attempt > 0 {
                let backoff = StdDuration::from_secs(1u64 << attempt);
                debug!(ts, attempt, backoff_s = backoff.as_secs(), "retrying fetch");
                sleep(backoff).await;
            }
            match self.try_fetch(ts).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    warn!(ts, attempt, error = %e, "fetch attempt failed");
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("unknown fetch error")))
    }

    async fn try_fetch(&self, ts: &str) -> Result<Value> {
        let resp = self
            .client
            .post(URL)
            .form(&[("Date", ts), ("type", "1")])
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP {status}"));
        }
        let text = resp.text().await?;
        if text.trim().is_empty() {
            return Err(anyhow::anyhow!("empty body"));
        }
        Ok(serde_json::from_str::<Value>(&text)?)
    }
    pub async fn warm_up(&self) -> Result<()> {
        let resp = self
            .client
            .get("https://aqms.doe.ir/Home/AQI")
            .send()
            .await?;
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "warm-up returned non-2xx");
        }
        Ok(())
    }
}
