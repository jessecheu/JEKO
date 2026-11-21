use std::env;
use std::path::Path;

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

/// Which checkout this binary is, so two copies of the repo run side by side can be
/// told apart: `JEKO_INSTANCE` if set (environment or `.env`), else the name of the
/// folder the binary was built from (`JEKO`, `JEKO2`, ...).
pub fn instance_name() -> String {
    let _ = dotenvy::dotenv();
    env::var("JEKO_INSTANCE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "jeko".to_string())
        })
}
