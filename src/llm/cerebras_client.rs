use crate::config::Config;
use crate::llm::usage::{RequestUsage, TokenStats};
use crate::llm::{task_gen, MathTask};
use serde_json::{json, Value};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct CerebrasClient {
    api_key: Option<String>,
    model: String,
    base_url: String,
    /// Shared by every clone, so the request threads spawned by
    /// `request_task_async` report into the same totals the HUD reads.
    usage: Arc<Mutex<TokenStats>>,
}

impl CerebrasClient {
    pub fn new(cfg: &Config) -> Self {
        Self {
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
            base_url: cfg.base_url.clone(),
            usage: Arc::default(),
        }
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// A copy of the session's token usage so far, for the HUD.
    pub fn token_stats(&self) -> TokenStats {
        self.stats().clone()
    }

    fn stats(&self) -> MutexGuard<'_, TokenStats> {
        // A panicked request thread can't leave the counters half-written
        // in a way that matters for a readout, so ignore poisoning.
        self.usage.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Fires a background-thread request for a fresh math task and returns a
    /// receiver the game loop can poll with `try_recv()` each frame.
    ///
    /// macroquad's `next_frame().await` only yields once per frame — it is
    /// not a real executor that can drive arbitrary futures — so the
    /// network call runs on a plain OS thread instead of being awaited
    /// in-frame.
    pub fn request_task_async(&self, difficulty: u32) -> Receiver<anyhow::Result<MathTask>> {
        let (tx, rx) = mpsc::channel();
        let client = self.clone();

        std::thread::spawn(move || {
            let result = client.request_task_blocking(difficulty);
            let _ = tx.send(result);
        });

        rx
    }

    fn request_task_blocking(&self, difficulty: u32) -> anyhow::Result<MathTask> {
        let (system, user) = task_gen::build_prompt(difficulty);
        let content = self.request_raw_blocking(&system, &user)?;

        let json_slice = task_gen::extract_json_object(&content)
            .ok_or_else(|| anyhow::anyhow!("no JSON object in model response: {content}"))?;

        Ok(serde_json::from_str(json_slice)?)
    }

    /// Lower-level call shared by live math-task generation and the
    /// build-time dialogue batch generator (`src/bin/gen_content.rs`).
    pub fn request_raw_blocking(&self, system: &str, user: &str) -> anyhow::Result<String> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no CEREBRAS_API_KEY set"))?;

        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "temperature": 0.7,
        });

        let started = Instant::now();
        let resp = match self.post_chat(api_key, &body) {
            Ok(resp) => resp,
            Err(e) => {
                self.stats().record_failure();
                return Err(e);
            }
        };
        if let Some(usage) = RequestUsage::from_response(&resp, started.elapsed()) {
            self.stats().record(usage);
        }

        resp["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("unexpected response shape: {resp}"))
    }

    fn post_chat(&self, api_key: &str, body: &Value) -> anyhow::Result<Value> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()?;

        Ok(http
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(api_key)
            .json(body)
            .send()?
            .error_for_status()?
            .json()?)
    }
}
