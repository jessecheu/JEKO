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
}
