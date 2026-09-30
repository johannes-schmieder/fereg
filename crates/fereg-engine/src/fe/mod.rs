//! Shared FE design, plan and coefficient/residual contract for all backends.
mod blocks;
mod certificate;
mod certificate_basis;
mod cmg;
mod cmg_core;
mod design;
mod diagnostics;
mod direct;
mod engine;
mod lsmr;
mod lsmr_refine;
mod monitor;
mod normalize;
mod plan;
mod refinement;
mod schur_pcg;
mod schur_refine;
use crate::data::DenseMatrix;
pub use crate::data::Weights;
use crate::spec::Solver;
pub use cmg::CmgSession;
pub use cmg_core::{CmgCore, CmgCoreCandidate};
pub use design::{CoefficientLayout, Design, EffectLayout};
pub use diagnostics::SetupDiagnostics;
pub use direct::DirectCandidate;
pub use engine::Engine;
pub use lsmr::{LsmrCandidate, LsmrSession};
pub use monitor::MonitorControl;
pub use plan::{Backend, Plan, PlanOverride};
pub use schur_pcg::{SchurCandidate, SchurSession};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    NoEffects,
    Direct,
    Converged,
    Stagnation,
    ProjectedExhaustion,
    BudgetExhausted,
    NumericalBreakdown,
    CertificateFailure,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    MeasuredStagnation,
    ProjectedExhaustion,
    NumericalNonConvergence,
    CertificateFailure,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    /// Residual corrections within this backend; their work is included in iterations.
    pub refinements: usize,
    pub backend: Backend,
    pub iterations: usize,
    pub stop: StopReason,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RhsDiagnostics {
    pub requested: Solver,
    pub selected: Backend,
    pub used: Backend,
    pub checkpoint_backend: Backend,
    pub attempts: Vec<Attempt>,
    pub iterations: usize,
    pub stop: StopReason,
    pub fallback: Option<FallbackReason>,
}
/// Scientific quantities recomputed from original RHSs and returned coefficients.
#[derive(Debug, Clone, PartialEq)]
pub struct Certificate {
    pub relative_score: Vec<f64>,
    pub relative_score_rms: Vec<f64>,
    pub raw_score_max: Vec<f64>,
    pub weighted_rhs_norm: Vec<f64>,
    pub weighted_residual_norm: Vec<f64>,
    pub relative_reconstruction: Vec<f64>,
    pub tolerance: f64,
}
/// Every backend has the same layout; no estimator may consume an uncertified result.
#[derive(Debug, PartialEq)]
pub struct Projection {
    pub coefficients: DenseMatrix,
    pub residuals: DenseMatrix,
    pub certificate: Certificate,
    pub diagnostics: Vec<RhsDiagnostics>,
    /// Work performed for numerical setup during this call, excluding reused factors.
    pub setup_diagnostics: Vec<SetupDiagnostics>,
}
