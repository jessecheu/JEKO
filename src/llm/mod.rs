pub mod buffer;
pub mod cerebras_client;
pub mod task_gen;
pub mod usage;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MathTask {
    pub topic: String,
    pub problem: String,
    pub answer: f64,
    pub hint: String,
    pub difficulty: u32,
    /// Two to four short steps that show how to get the answer (FR22). Empty
    /// for the fallback bank and for questions that came without any.
    #[serde(default)]
    pub explanation: Vec<String>,
    /// The sum behind `answer`, e.g. `(12 + 8) / 4` (FR22). Present on
    /// generated lesson packs, where it's evaluated to check the answer key.
    #[serde(default)]
    pub expression: Option<String>,
}

/// The offline question bank compiled into the binary: used when there's no
/// key, when a call fails or runs late, and by the `fakemodel` dev hook.
pub fn fallback_bank() -> Vec<MathTask> {
    serde_json::from_str(include_str!("../../assets/tasks_fallback.json"))
        .expect("assets/tasks_fallback.json must be valid and match MathTask")
}
