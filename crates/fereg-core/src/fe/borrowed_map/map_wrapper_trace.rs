//! Phase slots of one borrowed MAP wrapper call (`FEREG_PHASE|map_wrapper`):
//! trace-only, with no arithmetic or decision effect.
use crate::diagnostics::phase_ledger::PhaseLedger;

pub(super) const SCAN: usize = 0;
pub(super) const NORMS: usize = 1;
pub(super) const INITIAL: usize = 2;
pub(super) const SWEEP: usize = 3;
pub(super) const CERTIFY: usize = 4;

pub(super) fn ledger(rows: usize, rhs: usize) -> PhaseLedger<5> {
    let mut ledger = PhaseLedger::new(
        "map_wrapper",
        ["scan", "norms", "initial_certificate", "sweep", "certify"],
    );
    ledger.facts(&[("rows", rows as u64), ("rhs", rhs as u64)]);
    ledger
}
