use crate::llm::MathTask;
use rand::seq::SliceRandom;
use std::sync::mpsc::{Receiver, TryRecvError};

pub enum TriggerState {
    Idle,
    Loading(Receiver<anyhow::Result<MathTask>>),
    Active(MathTask),
    Resolved { correct: bool, cooldown: f32 },
}

impl TriggerState {
    /// Checks the in-flight request, if any, and promotes it to `Active`
    /// once the Cerebras call lands — or falls back to the local task bank
    /// immediately on error so a flaky connection never blocks play.
    pub fn poll(&mut self, fallback: &[MathTask]) {
        if let TriggerState::Loading(rx) = self {
            match rx.try_recv() {
                Ok(Ok(task)) => *self = TriggerState::Active(task),
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                    *self = TriggerState::Active(pick_fallback(fallback));
                }
                Err(TryRecvError::Empty) => {}
            }
        }
    }
}

pub fn pick_fallback(bank: &[MathTask]) -> MathTask {
    bank.choose(&mut rand::thread_rng())
        .cloned()
        .expect("fallback task bank must not be empty")
}
