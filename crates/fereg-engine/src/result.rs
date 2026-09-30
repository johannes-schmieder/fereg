use crate::{
    data::{DropReason, WeightKind},
    dof::{DofReport, ResidualDof},
    fe, memory,
    outputs::Outputs,
    spec::Solver,
    statistics::OlsStatistics,
    vce, wls,
};
use std::{ops::Deref, time::Duration};

/// Publication authority is immutable and constructed only by the controller.
/// Detached copies of numerical fields have no authority to publish a fit.
/// ```compile_fail
/// fn change(mut fit: fereg_engine::ols::OlsFit) { fit.coefficients[0] = 0.; }
/// ```
#[derive(Debug)]
pub struct OlsFit(pub(crate) FitData);
impl Deref for OlsFit {
    type Target = FitData;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[derive(Debug)]
pub struct FitData {
    pub stripes: Vec<String>,
    pub coefficients: Vec<f64>,
    pub statuses: Vec<wls::Status>,
    pub coordinates: Vec<usize>,
    pub covariance: vce::Covariance,
    pub t_statistics: Vec<f64>,
    pub wald: vce::Wald,
    pub dof: ResidualDof,
    pub absorbed_dof: DofReport,
    pub statistics: OlsStatistics,
    pub sample: Sample,
    pub weights: WeightSummary,
    pub outputs: Outputs,
    pub diagnostics: Diagnostics,
    pub requested_solver: Solver,
    pub vce: vce::Kind,
    pub workers: usize,
    pub memory: memory::Estimate,
    pub timings: Timings,
}
/// Only row identity survives; input, projected and score banks do not.
#[derive(Debug, PartialEq, Eq)]
pub struct Sample {
    pub original_rows: usize,
    pub candidate_rows: usize,
    pub retained_rows: Vec<usize>,
    pub drop_reasons: Vec<DropReason>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightSummary {
    pub kind: Option<WeightKind>,
    pub physical_rows: usize,
    pub effective_rows: usize,
    pub raw_sum: Option<f64>,
    pub log_raw_sum: Option<f64>,
    pub log_multiplier: f64,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timings {
    pub preparation: Duration,
    pub estimation: Duration,
    pub inference: Duration,
    pub outputs: Duration,
    pub total: Duration,
}
#[derive(Debug, PartialEq)]
pub struct Diagnostics {
    pub work: wls::Work,
    pub projection: fe::Certificate,
    pub model: fe::Certificate,
    pub right_hand_sides: Vec<fe::RhsDiagnostics>,
    pub setup: Vec<fe::SetupDiagnostics>,
    pub recovery: Vec<fe::RhsDiagnostics>,
    pub model_refinements: usize,
    pub residual_reconstruction: f64,
    pub original_finite_score: f64,
    pub finite_score: f64,
    pub inverse_error: f64,
    pub finite_refinements: usize,
    pub finite_method: wls::Method,
    pub constant_formula_error: f64,
}
impl Diagnostics {
    pub(crate) fn new(
        linear: wls::AbsorbedFit,
        work: wls::Work,
        constant_formula_error: f64,
    ) -> Self {
        Self {
            work,
            projection: linear.projection_certificate,
            model: linear.model_certificate,
            right_hand_sides: linear.projection_diagnostics,
            setup: linear.setup_diagnostics,
            recovery: linear.recovery_diagnostics,
            model_refinements: linear.model_refinements,
            residual_reconstruction: linear.relative_residual_reconstruction,
            original_finite_score: linear.relative_original_finite_score,
            finite_score: linear.finite.relative_score,
            inverse_error: linear.finite.inverse_error,
            finite_refinements: linear.finite.refinements,
            finite_method: linear.finite.method,
            constant_formula_error,
        }
    }
}

impl From<crate::data::ReferenceWeights<'_>> for WeightSummary {
    fn from(weights: crate::data::ReferenceWeights<'_>) -> Self {
        Self {
            kind: weights.kind(),
            physical_rows: weights.nobs(),
            effective_rows: weights.effective(),
            raw_sum: weights.raw_sum,
            log_raw_sum: weights.log_raw_sum,
            log_multiplier: weights.log_multiplier(),
        }
    }
}
impl From<crate::data::Prepared> for Sample {
    fn from(sample: crate::data::Prepared) -> Self {
        Self {
            original_rows: sample.n_original,
            candidate_rows: sample.n_candidate,
            retained_rows: sample.original_row_by_active,
            drop_reasons: sample.drop_reasons,
        }
    }
}
