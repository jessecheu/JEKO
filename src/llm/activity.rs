//! What the model is being asked to make right now (FR33), for the creation
//! bar at the top centre (`ui/creation_bar.rs`).
//!
//! `GenActivity` is a read-only mirror: `Game` opens a job where it starts a
//! model request and closes it where the answer lands. Nothing here gates
//! gameplay, changes XP or timing, or waits on a network call (invariants 1, 6
//! and 7). The ring never shows a made-up percentage: it is a stage ring. It
//! sweeps while a job is `Writing`, and closes when the job ends.

use super::usage::RequestKind;

/// How long a finished job stays on the bar, in seconds.
pub const LINGER_S: f32 = 2.2;
/// A job still `Writing` after this long is dropped: the request was let go of
/// without anyone telling the bar. The client's own timeout is 8 s.
pub const STALE_S: f32 = 12.0;
/// The bar shows at most this many circles; the rest count as "+N".
pub const MAX_CIRCLES: usize = 3;
/// Seconds the pop and the spark of a finished circle take.
pub const POP_S: f32 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    /// The request is out and the answer isn't back.
    Writing,
    /// The model's answer came back.
    Ready,
    /// The call failed or was too slow, and a bank question stepped in.
    Fallback,
}

impl JobState {
    pub fn is_done(self) -> bool {
        self != JobState::Writing
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenJob {
    pub id: u32,
    pub kind: RequestKind,
    /// What it is for, e.g. "flask" or "battle".
    pub label: String,
    pub state: JobState,
    /// `GenActivity` clock when it started.
    pub started: f32,
    /// Clock when it ended.
    pub ended: Option<f32>,
    /// A dev job ends by itself at this clock reading, in this state.
    script: Option<(f32, JobState)>,
}

impl GenJob {
    /// Seconds from start to end, once it has ended.
    pub fn took(&self) -> Option<f32> {
        self.ended.map(|e| e - self.started)
    }
}

/// One circle on the bar, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct CircleView {
    pub kind: RequestKind,
    pub label: String,
    pub state: JobState,
    /// Seconds since it started, for the sweeping arc.
    pub age: f32,
    /// 0 while writing; 0..1 over `POP_S` after it ends, then 1.
    pub pop: f32,
    /// Seconds since it ended, if it has (for the spark).
    pub since_end: Option<f32>,
    /// How long it took, once it has ended.
    pub took: Option<f32>,
    /// 1 while shown, fading to 0 over its last half second.
    pub alpha: f32,
}

/// What is ready ahead of need (FR33 milestone B): the real stock, never a
/// guess. Set by `Game` from the question buffer and the roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stock {
    /// Look-ahead questions waiting in the buffer.
    pub questions: usize,
    /// Generated monsters not met yet, for the tier coming up.
    pub monsters: usize,
}

impl Stock {
    pub fn is_empty(&self) -> bool {
        self.questions == 0 && self.monsters == 0
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BarView {
    pub circles: Vec<CircleView>,
    /// Jobs beyond `MAX_CIRCLES`.
    pub more: usize,
    /// What is ready ahead of need, shown under the circles.
    pub stock: Stock,
}

#[derive(Debug, Default)]
pub struct GenActivity {
    jobs: Vec<GenJob>,
    next_id: u32,
    clock: f32,
    stock: Stock,
}

impl GenActivity {
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a job that is being written now.
    pub fn start(&mut self, kind: RequestKind, label: &str) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.jobs.push(GenJob {
            id,
            kind,
            label: label.to_string(),
            state: JobState::Writing,
            started: self.clock,
            ended: None,
            script: None,
        });
        id
    }

    /// Opens a job that has already ended, for a question the bank answered
    /// because there is no model to ask.
    pub fn instant(&mut self, kind: RequestKind, label: &str, state: JobState) -> u32 {
        let id = self.start(kind, label);
        self.finish(id, state);
        id
    }

    /// Opens a dev job that writes for `secs` and then ends in `end`, so each
    /// state can be checked with no model (`JEKO_DEV=gen:<kind>`).
    pub fn scripted(&mut self, kind: RequestKind, label: &str, secs: f32, end: JobState) -> u32 {
        let id = self.start(kind, label);
        if let Some(j) = self.jobs.last_mut() {
            j.script = Some((self.clock + secs, end));
        }
        id
    }

    /// Renames a job, for a creation whose name is only known once it is made
    /// (a monster's, a lesson's title).
    pub fn rename(&mut self, id: u32, label: &str) {
        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
            j.label = label.to_string();
        }
    }

    /// Sets what is ready ahead of need.
    pub fn set_stock(&mut self, stock: Stock) {
        self.stock = stock;
    }

    /// Ends a job. Ending one twice, or one that's gone, does nothing.
    pub fn finish(&mut self, id: u32, state: JobState) {
        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
            if j.ended.is_none() {
                j.state = state;
                j.ended = Some(self.clock);
            }
        }
    }

    /// Takes a job off the bar with no ending, for a request that was let go
    /// of (its answer goes to the buffer, not to anyone waiting).
    pub fn cancel(&mut self, id: u32) {
        self.jobs.retain(|j| j.id != id);
    }

    /// Drops every job, for a new run.
    pub fn clear(&mut self) {
        self.jobs.clear();
    }

    /// Advances the clock, ends dev jobs that are due, and drops what has
    /// stayed long enough.
    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
        let now = self.clock;
        for j in &mut self.jobs {
            if let (Some((at, state)), None) = (j.script, j.ended) {
                if now >= at {
                    j.state = state;
                    j.ended = Some(now);
                }
            }
        }
        self.jobs.retain(|j| match j.ended {
            Some(e) => now - e < LINGER_S,
            None => now - j.started < STALE_S,
        });
    }

    pub fn jobs(&self) -> &[GenJob] {
        &self.jobs
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// Jobs still being written.
    pub fn running(&self) -> usize {
        self.jobs.iter().filter(|j| !j.state.is_done()).count()
    }

    /// What the bar draws: the oldest `MAX_CIRCLES` jobs, then the count of
    /// the rest.
    pub fn view(&self) -> BarView {
        self.view_where(|_| true)
    }

    /// The same, for the jobs `keep` lets through (in a battle only the
    /// battle's own question shows).
    pub fn view_where(&self, keep: impl Fn(&GenJob) -> bool) -> BarView {
        let shown: Vec<&GenJob> = self.jobs.iter().filter(|j| keep(j)).collect();
        let circles = shown
            .iter()
            .take(MAX_CIRCLES)
            .map(|j| {
                let since_end = j.ended.map(|e| self.clock - e);
                CircleView {
                    kind: j.kind,
                    label: j.label.clone(),
                    state: j.state,
                    age: self.clock - j.started,
                    pop: since_end.map_or(0.0, |s| (s / POP_S).clamp(0.0, 1.0)),
                    since_end,
                    took: j.took(),
                    alpha: since_end.map_or(1.0, |s| ((LINGER_S - s) / 0.5).clamp(0.0, 1.0)),
                }
            })
            .collect();
        BarView {
            circles,
            more: shown.len().saturating_sub(MAX_CIRCLES),
            stock: self.stock,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q: RequestKind = RequestKind::Question;

    #[test]
    fn a_job_writes_then_is_ready_and_remembers_how_long_it_took() {
        let mut a = GenActivity::new();
        let id = a.start(Q, "flask");
        assert_eq!(a.running(), 1);
        a.tick(0.5);
        a.finish(id, JobState::Ready);
        let j = &a.jobs()[0];
        assert_eq!(j.state, JobState::Ready);
        assert!((j.took().unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(a.running(), 0);
    }

    #[test]
    fn a_failed_call_shows_as_fallback() {
        let mut a = GenActivity::new();
        let id = a.start(Q, "battle");
        a.finish(id, JobState::Fallback);
        assert_eq!(a.jobs()[0].state, JobState::Fallback);
    }

    #[test]
    fn ending_a_job_twice_keeps_the_first_ending() {
        let mut a = GenActivity::new();
        let id = a.start(Q, "flask");
        a.finish(id, JobState::Fallback);
        a.tick(1.0);
        a.finish(id, JobState::Ready);
        assert_eq!(a.jobs()[0].state, JobState::Fallback);
        assert_eq!(a.jobs()[0].took(), Some(0.0));
    }

    #[test]
    fn a_finished_job_leaves_after_it_has_lingered() {
        let mut a = GenActivity::new();
        let id = a.start(Q, "flask");
        a.finish(id, JobState::Ready);
        a.tick(LINGER_S - 0.1);
        assert_eq!(a.jobs().len(), 1);
        a.tick(0.2);
        assert!(a.is_empty());
    }

    #[test]
    fn a_job_nobody_ended_is_dropped_when_stale() {
        let mut a = GenActivity::new();
        a.start(Q, "flask");
        a.tick(STALE_S - 0.1);
        assert_eq!(a.jobs().len(), 1);
        a.tick(0.2);
        assert!(a.is_empty());
    }

    #[test]
    fn cancel_removes_a_job_without_an_ending() {
        let mut a = GenActivity::new();
        let id = a.start(Q, "flask");
        a.cancel(id);
        assert!(a.is_empty());
        // Ending a job that is gone is harmless.
        a.finish(id, JobState::Ready);
    }

    #[test]
    fn the_bar_shows_three_circles_and_counts_the_rest() {
        let mut a = GenActivity::new();
        for _ in 0..5 {
            a.start(Q, "flask");
        }
        let v = a.view();
        assert_eq!(v.circles.len(), MAX_CIRCLES);
        assert_eq!(v.more, 2);
        // Oldest first.
        assert!(v.circles[0].age >= v.circles[2].age);
    }

    #[test]
    fn a_circle_pops_once_it_has_ended_and_fades_before_it_goes() {
        let mut a = GenActivity::new();
        let id = a.start(Q, "flask");
        assert_eq!(a.view().circles[0].pop, 0.0);
        a.finish(id, JobState::Ready);
        a.tick(POP_S / 2.0);
        let half = a.view().circles[0].pop;
        assert!(half > 0.0 && half < 1.0);
        a.tick(POP_S);
        assert_eq!(a.view().circles[0].pop, 1.0);
        assert_eq!(a.view().circles[0].alpha, 1.0);
        a.tick(LINGER_S - POP_S * 1.5 - 0.2);
        assert!(a.view().circles[0].alpha < 1.0);
    }

    #[test]
    fn a_job_can_be_renamed_once_its_name_is_known() {
        let mut a = GenActivity::new();
        let id = a.start(RequestKind::Monster, "tier 2");
        a.rename(id, "Bramblehog");
        assert_eq!(a.view().circles[0].label, "Bramblehog");
        // A job that's gone is harmless.
        a.cancel(id);
        a.rename(id, "x");
    }

    #[test]
    fn the_view_can_leave_jobs_out_and_counts_only_the_rest() {
        let mut a = GenActivity::new();
        a.start(RequestKind::Prefetch, "ahead");
        a.start(Q, "battle");
        for _ in 0..4 {
            a.start(RequestKind::Prefetch, "ahead");
        }
        let v = a.view_where(|j| j.label == "battle");
        assert_eq!(v.circles.len(), 1);
        assert_eq!(v.more, 0);
        assert_eq!(a.view().more, 3);
    }

    #[test]
    fn the_view_carries_the_stock() {
        let mut a = GenActivity::new();
        assert!(a.view().stock.is_empty());
        a.set_stock(Stock {
            questions: 4,
            monsters: 1,
        });
        assert_eq!(a.view().stock.questions, 4);
        assert!(!a.view().stock.is_empty());
    }

    #[test]
    fn an_instant_job_is_already_over() {
        let mut a = GenActivity::new();
        a.instant(Q, "flask", JobState::Fallback);
        assert_eq!(a.running(), 0);
        assert_eq!(a.view().circles[0].took, Some(0.0));
    }

    #[test]
    fn a_scripted_job_ends_by_itself() {
        let mut a = GenActivity::new();
        a.scripted(Q, "dev", 1.0, JobState::Ready);
        a.tick(0.9);
        assert_eq!(a.running(), 1);
        a.tick(0.2);
        assert_eq!(a.running(), 0);
        assert_eq!(a.jobs()[0].state, JobState::Ready);
    }
}
