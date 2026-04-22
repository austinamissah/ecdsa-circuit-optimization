//! Small targeted notes and tests for the current phase-bug hypothesis.
//!
//! Current evidence:
//! - local specialized-step equivalence holds on reachable states,
//! - local forward+backward equivalence holds for the first 3 steps,
//! - `with_kal_inv_raw(..., body=[])` also shows no phase divergence for small
//!   tested prefix lengths,
//! - yet the full point-add circuit exhibits phase garbage for many larger
//!   integrated prefix lengths.
//!
//! So the most likely remaining cause is not the isolated Kaliski inverse
//! itself, but an interaction with the surrounding point-add scaffold.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseHypothesis {
    LocalStepPhaseBugUnlikely,
    LocalInvIdentityPhaseBugUnlikely,
    FullScaffoldInteractionLikely,
}

pub fn current_phase_hypothesis() -> PhaseHypothesis {
    PhaseHypothesis::FullScaffoldInteractionLikely
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_hypothesis_marker() {
        eprintln!("=== current phase-bug hypothesis ===");
        eprintln!("{:?}", current_phase_hypothesis());
        eprintln!("====================================");
        assert_eq!(current_phase_hypothesis(), PhaseHypothesis::FullScaffoldInteractionLikely);
    }
}
