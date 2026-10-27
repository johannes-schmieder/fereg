//! Phase slots of one joint frame (`FEREG_PHASE|joint_frame`): trace-only.
use crate::diagnostics::phase_ledger::PhaseLedger;

pub(crate) type JointLedger = PhaseLedger<9>;

pub(crate) const SETUP: usize = 0;
/// The retained-block product, merge included.
pub(crate) const OPERATOR: usize = 1;
/// Serial or parallel merge of helper partial products.
pub(crate) const MERGE: usize = 2;
/// The finite cross block of each step.
pub(crate) const CROSS: usize = 3;
/// Curvature and its observation-space fallback.
pub(crate) const CURVATURE: usize = 4;
/// Retained-block and finite preconditioning.
pub(crate) const PRECONDITION: usize = 5;
/// Coefficient, residual and direction updates.
pub(crate) const STEP: usize = 6;
/// Original-row reconstruction and its acceptance.
pub(crate) const RECONSTRUCT: usize = 7;
/// The block checkpoint (hierarchy install or refresh).
pub(crate) const CHECKPOINT: usize = 8;

pub(crate) fn ledger() -> JointLedger {
    PhaseLedger::new(
        "joint_frame",
        [
            "setup",
            "operator",
            "merge",
            "cross",
            "curvature",
            "precondition",
            "step",
            "reconstruct",
            "checkpoint",
        ],
    )
}
