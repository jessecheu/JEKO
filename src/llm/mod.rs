pub mod buffer;
pub mod cerebras_client;
pub mod expr;
pub mod task_gen;
pub mod usage;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fallback_bank_loads_and_every_answer_checks_out() {
        let bank = fallback_bank();
        assert!(bank.len() >= 8);
        for t in &bank {
            assert_eq!(expr::verify(t), Ok(()), "{}", t.problem);
            assert!(t.explanation.len() >= 2, "{}", t.problem);
        }
    }

    #[test]
    fn a_task_without_explanation_or_expression_still_loads() {
        let t: MathTask = serde_json::from_str(
            r#"{"topic":"t","problem":"p","answer":1,"hint":"h","difficulty":1}"#,
        )
        .unwrap();
        assert!(t.explanation.is_empty() && t.expression.is_none());
    }
}
