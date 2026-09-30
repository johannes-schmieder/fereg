use super::{bias, family, irls, separation};
use crate::{
    data::DenseMatrix,
    dof::{DofReport, ResidualDof},
    memory,
    outputs::Outputs,
    result::{Diagnostics as LinearDiagnostics, Sample, WeightSummary},
    spec::Solver,
    statistics::PpmlStatistics,
    vce, wls, Result,
};
use std::{ops::Deref, time::Duration};

/// Only the completed controller constructs publication authority.
/// ```compile_fail
/// fn change(mut fit: fereg_engine::ppml::PpmlFit) { fit.coefficients[0] = 0.; }
/// ```
#[derive(Debug)]
pub struct PpmlFit(pub(super) FitData);
impl Deref for PpmlFit {
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
    pub z_statistics: Vec<f64>,
    pub wald: vce::Wald,
    pub dof: ResidualDof,
    pub absorbed_dof: DofReport,
    pub statistics: PpmlStatistics,
    pub sample: Sample,
    pub weights: WeightSummary,
    pub outputs: Outputs,
    pub diagnostics: Diagnostics,
    pub separation: separation::Report,
    pub bias: Option<BiasResult>,
    pub requested_solver: Solver,
    pub vce: vce::Kind,
    pub workers: usize,
    pub memory: memory::Estimate,
    pub timings: Timings,
}
#[derive(Debug, PartialEq)]
pub struct Diagnostics {
    pub irls: irls::Diagnostics,
    pub scores: family::ScoreCertificate,
    pub state: family::StateCertificate,
    pub linear: LinearDiagnostics,
}
impl From<irls::FittedState> for Diagnostics {
    fn from(state: irls::FittedState) -> Self {
        let linear = LinearDiagnostics::new(state.linear, state.diagnostics.work, 0.);
        Self {
            irls: state.diagnostics,
            scores: state.scores,
            state: state.state,
            linear,
        }
    }
}
/// Publication products only: full FE coefficient/calculation banks are freed.
#[derive(Debug, PartialEq)]
pub struct BiasResult {
    pub request: bias::Request,
    pub ordinary: DenseMatrix,
    pub saved: Outputs,
    pub support: [bias::Summary; 2],
    pub first_levels: usize,
    pub second_levels: usize,
    pub finite_slots: Vec<usize>,
    pub diagnostics: bias::Diagnostics,
    pub table: bias::DiagnosticTable,
}
impl BiasResult {
    pub(super) fn new(correction: bias::Correction, workspace: u128) -> Result<Self> {
        let table = correction.diagnostic_table(workspace)?;
        Ok(Self {
            request: correction.request,
            ordinary: correction.ordinary,
            saved: correction.saved,
            support: correction.support,
            first_levels: correction.first_levels,
            second_levels: correction.second_levels,
            finite_slots: correction.finite_slots,
            diagnostics: correction.diagnostics,
            table,
        })
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timings {
    pub preparation: Duration,
    pub separation: Duration,
    pub estimation: Duration,
    pub inference: Duration,
    pub outputs: Duration,
    pub bias: Duration,
    pub total: Duration,
}
