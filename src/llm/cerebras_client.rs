use crate::config::Config;
use crate::llm::usage::{RequestKind, RequestUsage, TokenStats};
use crate::llm::{expr, fallback_bank, task_gen, MathTask};
use rand::seq::SliceRandom;
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
    /// `JEKO_DEV=fakemodel:<ms>`: answer every call from the offline bank
    /// after this delay, so the question buffer, the soft deadline and the
    /// wait readout can be tested and screenshotted with no key.
    fake_delay: Option<Duration>,
}

impl CerebrasClient {
    pub fn new(cfg: &Config) -> Self {
        Self {
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
            base_url: cfg.base_url.clone(),
            usage: Arc::default(),
            fake_delay: None,
        }
    }

    /// Makes this client a stand-in model that answers from the offline bank
    /// after `delay` (a dev hook; see `fake_delay`).
    pub fn with_fake_model(mut self, delay: Duration) -> Self {
        self.fake_delay = Some(delay);
        self
    }

    pub fn is_fake(&self) -> bool {
        self.fake_delay.is_some()
    }

    /// How long the stand-in model takes per call, if it's on.
    pub fn fake_delay(&self) -> Option<Duration> {
        self.fake_delay
    }

    /// True when calls can be made: there is a key, or the stand-in model is
    /// on. Without one the game plays from the fallback bank.
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some() || self.is_fake()
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
    pub fn request_task_async(
        &self,
        difficulty: u32,
        topic: Option<String>,
    ) -> Receiver<anyhow::Result<MathTask>> {
        self.request_task_as(difficulty, topic, RequestKind::Question)
    }

    /// Like `request_task_async`, but tagged as a look-ahead request (FR32),
    /// so the token panel can tell it from a question the player waits on.
    pub fn prefetch_task_async(&self, difficulty: u32) -> Receiver<anyhow::Result<MathTask>> {
        self.request_task_as(difficulty, None, RequestKind::Prefetch)
    }

    fn request_task_as(
        &self,
        difficulty: u32,
        topic: Option<String>,
        kind: RequestKind,
    ) -> Receiver<anyhow::Result<MathTask>> {
        let (tx, rx) = mpsc::channel();
        let client = self.clone();

        std::thread::spawn(move || {
            let result = client.request_task_blocking(difficulty, topic.as_deref(), kind);
            let _ = tx.send(result);
        });

        rx
    }

    fn request_task_blocking(
        &self,
        difficulty: u32,
        topic: Option<&str>,
        kind: RequestKind,
    ) -> anyhow::Result<MathTask> {
        if let Some(delay) = self.fake_delay {
            return Ok(self.fake_task(difficulty, topic, kind, delay));
        }
        let (system, user) = task_gen::build_prompt(difficulty, topic);
        let content = self.request_tagged_blocking(kind, &system, &user)?;

        let json_slice = task_gen::extract_json_object(&content)
            .ok_or_else(|| anyhow::anyhow!("no JSON object in model response: {content}"))?;

        let mut task: MathTask = serde_json::from_str(json_slice)?;
        // Nothing the model writes reaches a child with an answer key that
        // doesn't match its own sum: a failure here falls back to the bank.
        expr::verify(&task)
            .map_err(|why| anyhow::anyhow!("rejected the model's problem: {why:?}"))?;
        if let Some(topic) = topic {
            task.topic = topic.to_string();
        }
        Ok(task)
    }

    /// Lower-level call shared by live math-task generation and the
    /// build-time dialogue batch generator (`src/bin/gen_content.rs`).
    pub fn request_raw_blocking(&self, system: &str, user: &str) -> anyhow::Result<String> {
        self.request_tagged_blocking(RequestKind::Question, system, user)
    }

    /// `request_raw_blocking`, with the call tagged for the token panel's
    /// split by kind (FR21, FR32).
    pub fn request_tagged_blocking(
        &self,
        kind: RequestKind,
        system: &str,
        user: &str,
    ) -> anyhow::Result<String> {
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
                self.stats().record_failure(kind);
                return Err(e);
            }
        };
        if let Some(usage) = RequestUsage::from_response(&resp, started.elapsed(), kind) {
            self.stats().record(usage);
        }

        resp["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("unexpected response shape: {resp}"))
    }

    /// The stand-in model: waits, then answers from the offline bank with a
    /// made-up token count, so the panel has something to show.
    fn fake_task(
        &self,
        difficulty: u32,
        topic: Option<&str>,
        kind: RequestKind,
        delay: Duration,
    ) -> MathTask {
        let started = Instant::now();
        std::thread::sleep(delay);
        let bank = fallback_bank();
        let mut task = bank
            .choose(&mut rand::thread_rng())
            .cloned()
            .expect("fallback task bank must not be empty");
        task.difficulty = difficulty;
        if let Some(topic) = topic {
            task.topic = topic.to_string();
        }
        self.record_fake(kind, 140, 60, started.elapsed());
        task
    }

    /// Books a made-up call into the token stats (the stand-in model has no
    /// `usage` block to read), so the panel and the studio's budget see it.
    pub fn record_fake(
        &self,
        kind: RequestKind,
        prompt_tokens: u64,
        completion_tokens: u64,
        round_trip: Duration,
    ) {
        let secs = round_trip.as_secs_f64().max(0.001);
        self.stats().record(RequestUsage {
            kind,
            prompt_tokens,
            completion_tokens,
            tokens_per_sec: completion_tokens as f64 / secs,
            provider_timed: false,
            round_trip,
        });
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
