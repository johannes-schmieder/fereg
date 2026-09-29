//! Shared FE design, plan and coefficient/residual contract for all backends.
mod blocks;
mod certificate;
mod certificate_basis;
mod design;
mod direct;
mod normalize;
mod plan;
mod schur_pcg;
use crate::data::DenseMatrix;
use crate::spec::Solver;
pub use design::{CoefficientLayout, Design, EffectLayout, Weights};
pub use direct::DirectCandidate;
pub use plan::{Backend, Plan, PlanOverride};
pub use schur_pcg::{SchurCandidate, SchurSession};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    NoEffects,
    Direct,
    Converged,
    Stagnation,
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
}
