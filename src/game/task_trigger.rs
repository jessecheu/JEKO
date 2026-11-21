use crate::llm::MathTask;
use rand::seq::SliceRandom;
use std::sync::mpsc::{Receiver, TryRecvError};

pub enum TriggerState {
    Idle,
    Loading(Receiver<anyhow::Result<MathTask>>),
    Active(MathTask),
    Resolved { correct: bool, cooldown: f32 },
}

/// How a `Loading` request ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// The model's answer came back.
    Live,
    /// The call failed; a question from the local bank stepped in.
    Fallback,
}

impl TriggerState {
    /// Checks the in-flight request, if any, and promotes it to `Active`
    /// once the Cerebras call lands — or falls back to the local task bank
    /// immediately on error so a flaky connection never blocks play.
    /// Says how it ended when it did.
    pub fn poll(&mut self, fallback: &[MathTask]) -> Option<Arrival> {
        let TriggerState::Loading(rx) = self else {
            return None;
        };
        match rx.try_recv() {
            Ok(Ok(task)) => {
                *self = TriggerState::Active(task);
                Some(Arrival::Live)
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                *self = TriggerState::Active(pick_fallback(fallback));
                Some(Arrival::Fallback)
            }
            Err(TryRecvError::Empty) => None,
        }
    }
}

pub fn pick_fallback(bank: &[MathTask]) -> MathTask {
    bank.choose(&mut rand::thread_rng())
        .cloned()
        .expect("fallback task bank must not be empty")
}
