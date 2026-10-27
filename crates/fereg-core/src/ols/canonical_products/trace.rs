//! Phase slots of one canonical products call (`FEREG_PHASE|canonical_products`)
//! and of one shared-CMG component solve: trace-only.
use crate::diagnostics::phase_ledger::PhaseLedger;

pub(super) type ProductsLedger = PhaseLedger<6>;

pub(super) const SETUP: usize = 0;
/// Scales, means and retained cross products of every column.
pub(super) const RHS: usize = 1;
/// The plain probe of column 0 on the largest component.
pub(super) const PROBE: usize = 2;
/// Probe plus any hierarchy construction it triggers.
pub(super) const CMG_PLAN: usize = 3;
pub(super) const SOLVE: usize = 4;
pub(super) const CERTIFICATE: usize = 5;

pub(super) fn ledger() -> ProductsLedger {
    PhaseLedger::new(
        "canonical_products",
        ["setup", "rhs", "probe", "cmg_plan", "solve", "certificate"],
    )
}

/// Charge the certificate and record the call's shape (rows, width, reduced
/// levels, free coordinates, implicit) and its largest iteration count.
pub(super) fn finish(
    ledger: &mut ProductsLedger,
    shape: [usize; 5],
    iterations: &std::sync::atomic::AtomicUsize,
) {
    ledger.lap(CERTIFICATE);
    let names = ["rows", "width", "q", "d", "implicit"];
    for (name, value) in names.into_iter().zip(shape) {
        ledger.fact(name, value as u64);
    }
    let iterations = iterations.load(std::sync::atomic::Ordering::Relaxed);
    ledger.fact("iterations", iterations as u64);
}

/// One preconditioned component solve of one column: CMG cycle seconds.
pub(super) fn column_ledger() -> PhaseLedger<1> {
    PhaseLedger::new("canonical_cmg_column", ["cycle"])
}
