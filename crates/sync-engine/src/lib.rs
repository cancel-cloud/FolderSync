//! Rust-owned filesystem synchronization and persistent state.
mod automation;
mod deletion;
mod engine;
pub mod fs;
mod index;
mod lineage;
mod logging;
mod model;
mod parallel;
mod progress;
mod recovery;
mod renames;
mod review;
pub mod scheduler;
mod transfer;
pub mod watch;
pub use engine::Engine;
pub use model::*;
pub use progress::{ApplyOutcome, Cancelled, ProgressPhase, ProgressState, RunProgress};
pub fn classify(mode: Mode, a: Option<&str>, b: Option<&str>, base: Option<&str>) -> Action {
    use Action::*;
    if a == b {
        return if mode == Mode::Move && a.is_some() {
            MoveToB
        } else {
            Unchanged
        };
    }
    if a.is_some() && b.is_some() && base.is_none() {
        return Conflict;
    }
    if mode != Mode::TwoWay {
        return match (a, b) {
            (None, Some(_)) if mode == Mode::Mirror => DeleteB,
            (None, _) => Unchanged,
            (Some(_), None) if mode == Mode::Move => MoveToB,
            (Some(_), None) => CopyToB,
            (Some(_), Some(_)) if b == base => {
                if mode == Mode::Move {
                    MoveToB
                } else {
                    CopyToB
                }
            }
            _ => Conflict,
        };
    }
    match (a, b, base) {
        (Some(_), None, None) => CopyToB,
        (None, Some(_), None) => CopyToA,
        (None, Some(_), Some(_)) if b == base => DeleteB,
        (Some(_), None, Some(_)) if a == base => DeleteA,
        (Some(_), Some(_), Some(_)) if a == base => CopyToA,
        (Some(_), Some(_), Some(_)) if b == base => CopyToB,
        _ => Conflict,
    }
}
