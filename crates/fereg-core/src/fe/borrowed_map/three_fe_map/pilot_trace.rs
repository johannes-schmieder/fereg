//! Phase slots of one bounded three-FE MAP pilot (`FEREG_PHASE|three_fe_pilot`):
//! trace-only, with no arithmetic or decision effect.
use crate::diagnostics::phase_ledger::PhaseLedger;

pub(super) const SETUP: usize = 0;
pub(super) const INITIAL: usize = 1;
pub(super) const SWEEP: usize = 2;
pub(super) const CERTIFY: usize = 3;
pub(super) const FINISH: usize = 4;

pub(super) fn ledger(rows: usize, rhs: usize, predicted: usize) -> PhaseLedger<5> {
    let mut ledger = PhaseLedger::new(
        "three_fe_pilot",
        ["setup", "initial_certificate", "sweep", "certify", "finish"],
    );
    ledger.facts(&[
        ("rows", rows as u64),
        ("rhs", rhs as u64),
        ("predicted_sweeps", predicted as u64),
    ]);
    ledger
}

/// The slot of the certificate after `iteration` sweeps.
pub(super) fn certificate_slot(iteration: usize) -> usize {
    if iteration == 0 {
        INITIAL
    } else {
        CERTIFY
    }
}
