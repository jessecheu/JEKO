use std::env;

pub struct Config {
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: String,
}

impl Config {
    pub fn load() -> Self {
        let _ = dotenvy::dotenv();

        Config {
            api_key: env::var("CEREBRAS_API_KEY").ok().filter(|s| !s.is_empty()),
            model: env::var("CEREBRAS_MODEL").unwrap_or_else(|_| "kimi-k3.1".to_string()),
            base_url: env::var("CEREBRAS_BASE_URL")
                .unwrap_or_else(|_| "https://api.cerebras.ai/v1".to_string()),
        }
    }
}
