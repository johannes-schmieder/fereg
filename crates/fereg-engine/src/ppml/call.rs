//! One metadata/preloading admission, consumed by the complete PPML controller.
use super::{bias, separation};
pub use crate::memory::Resources;
use crate::{
    data::{CompiledColumns, Loaded, LoadedSource, NumericColumn, SourcePlan, WeightKind},
    memory::{self, Admitted, Inputs, Layout, Plan},
    outputs::PpmlRequest,
    spec::{Estimator, Options},
    threads::ThreadTeam,
    vce, ErrorCode, FeregError, ModelSpec, Result, SourceId,
};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiasUnitSource {
    Model(SourceId),
    Outcome,
    Weight,
    Adjustment,
}

#[derive(Debug, Default)]
pub struct Request {
    /// Omission chooses clustered when declared, otherwise robust.
    pub vce: Option<vce::Kind>,
    pub outputs: PpmlRequest,
    pub separation: separation::Methods,
    pub separation_controls: separation::RunControls,
    pub bias: Option<bias::Request>,
    /// Reuse an original loaded bank; None requires a separate explicit bank.
    pub bias_unit_source: Option<BiasUnitSource>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Loaded,
    Prepared,
    /// Repeated caller-thread checkpoints in separation and IRLS.
    Numerical(super::InterruptPoint),
    Separated,
    Estimated,
    Inferred,
    Outputs,
    /// Repeated caller-thread boundaries inside the optional correction.
    BiasCorrection,
    Corrected,
}
pub struct Call<'a> {
    pub(super) model: &'a ModelSpec,
    pub(super) request: &'a Request,
    pub(super) options: Options,
    pub(super) columns: CompiledColumns,
    pub(super) sources: SourcePlan,
    pub(super) kind: vce::Kind,
    pub(super) admitted: Admitted,
    inputs: Inputs,
    weight_kind: Option<WeightKind>,
    pub(super) started: Instant,
}
impl<'a> Call<'a> {
    pub fn new(
        model: &'a ModelSpec,
        options: &Options,
        request: &'a Request,
        mut inputs: Inputs,
        weight_kind: Option<WeightKind>,
        resources: Resources,
    ) -> Result<Self> {
        let started = Instant::now();
        options.validate()?;
        request.separation_controls.validate()?;
        let columns = CompiledColumns::new(model, Estimator::Ppml)?;
        let sources = SourcePlan::new(model)?;
        if request.bias_unit_source.is_some_and(|source| {
            request.bias.is_none()
                || match source {
                    BiasUnitSource::Model(id) => id.get() as usize > model.model.sources.len(),
                    BiasUnitSource::Weight => !inputs.weighted,
                    BiasUnitSource::Adjustment => !inputs.adjustment,
                    BiasUnitSource::Outcome => false,
                }
        }) {
            return Err(invalid(
                "bias-unit source is absent from the input or correction is off",
            ));
        }
        if inputs.weighted != weight_kind.is_some()
            || inputs.auxiliary_numeric_sources
                != usize::from(request.bias.is_some() && request.bias_unit_source.is_none())
        {
            return Err(invalid(
                "PPML weight or bias-unit declaration disagrees with input bounds",
            ));
        }
        if matches!(
            weight_kind,
            Some(WeightKind::Analytic | WeightKind::Importance)
        ) {
            return Err(FeregError::new(
                ErrorCode::UnsupportedWeight,
                "PPML supports only frequency and probability weights",
            ));
        }
        let kind = request.vce.unwrap_or(if model.clusters.is_empty() {
            vce::Kind::Robust
        } else {
            vce::Kind::Cluster
        });
        if kind == vce::Kind::Iid {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "PPML IID covariance is unsupported",
            ));
        }
        if model.clusters.len() > 5 || (kind == vce::Kind::Cluster) == model.clusters.is_empty() {
            return Err(invalid("PPML VCE and cluster declarations disagree"));
        }
        let products = request.outputs.common.kinds(request.outputs.mean)?;
        if request.outputs.residual_type.is_some() && !request.outputs.common.residuals {
            return Err(invalid("PPML residual type requires a residual output"));
        }
        for &(effect, channel) in &request.outputs.common.saved {
            if model.absorbed_terms.get(effect).is_none_or(|term| {
                channel >= term.slope_sources.len() + usize::from(term.intercept)
            }) {
                return Err(invalid(
                    "saved FE channel does not exist in the declared model",
                ));
            }
        }
        if let Some(bias) = request.bias {
            bias.method.validate()?;
            bias::validate_model(model)?;
        }
        inputs = inputs.with_model(
            model,
            request.outputs.common.saved.capacity() as u128 * 2 * size_of::<usize>() as u128,
        )?;
        let mut options = *options;
        if options.threads == 0 {
            options.threads =
                std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        }
        let layout = Layout::new(model, inputs, Estimator::Ppml)?;
        let mut storage = layout.request(options.solver);
        storage.observation_outputs = products.len();
        storage.publication_matrix_cells = resources.publication_matrix_cells;
        storage.worker_threads = options.threads;
        storage.bias = request.bias;
        if request.separation.contains(separation::Method::Simplex) {
            storage.simplex_cells = request.separation_controls.simplex.max_cells;
        }
        let admitted = Plan::new(storage)?.admit(resources.usable_bytes)?;
        Ok(Self {
            model,
            request,
            options,
            columns,
            sources,
            kind,
            admitted,
            inputs,
            weight_kind,
            started,
        })
    }
    #[must_use]
    pub fn memory(&self) -> &memory::Estimate {
        self.admitted.estimate()
    }
    pub fn run(self, loaded: &Loaded, units: Option<&NumericColumn>) -> Result<super::PpmlFit> {
        self.run_with(loaded, units, &mut |_| Ok(()))
    }
    /// Callbacks run on the caller through separation/replay, IRLS and bias;
    /// individual FE solves and dense kernels still run to their next boundary.
    pub fn run_with(
        self,
        loaded: &Loaded,
        units: Option<&NumericColumn>,
        interrupt: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<super::PpmlFit> {
        self.bind(loaded, units)?;
        interrupt(Phase::Loaded)?;
        let team = ThreadTeam::new(self.options.threads)?;
        super::fit::execute(self, loaded, units, &team, interrupt)
    }
    pub fn run_with_team(
        self,
        loaded: &Loaded,
        units: Option<&NumericColumn>,
        team: &ThreadTeam,
        interrupt: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<super::PpmlFit> {
        self.bind(loaded, units)?;
        if team.threads() != self.options.threads {
            return Err(invalid(
                "worker team differs from the preflight declaration",
            ));
        }
        interrupt(Phase::Loaded)?;
        super::fit::execute(self, loaded, units, team, interrupt)
    }
    fn bind(&self, loaded: &Loaded, units: Option<&NumericColumn>) -> Result<()> {
        self.inputs.bind_loaded_with_auxiliary(loaded, units)?;
        self.units(loaded, units)?;
        if loaded.weights.as_ref().map(|(kind, _)| *kind) != self.weight_kind {
            return Err(invalid("loaded weight kind differs from preflight"));
        }
        Ok(())
    }
    pub(super) fn units<'input>(
        &self,
        loaded: &'input Loaded,
        auxiliary: Option<&'input NumericColumn>,
    ) -> Result<Option<&'input NumericColumn>> {
        if let Some(source) = self.request.bias_unit_source {
            if auxiliary.is_some() {
                return Err(invalid(
                    "a bound bias-unit source cannot also have an auxiliary bank",
                ));
            }
            return match source {
                BiasUnitSource::Model(id) => match &loaded.sources[id.get() as usize - 1] {
                    LoadedSource::Numeric(column) => Ok(Some(column)),
                    LoadedSource::Strings(_) => Err(FeregError::new(
                        ErrorCode::Unsupported,
                        "biasunit requires an explicit numeric source",
                    )),
                },
                BiasUnitSource::Outcome => Ok(Some(&loaded.outcome)),
                BiasUnitSource::Weight => Ok(loaded.weights.as_ref().map(|(_, column)| column)),
                BiasUnitSource::Adjustment => match &loaded.adjustment {
                    crate::data::LoadedAdjustment::None => Ok(None),
                    crate::data::LoadedAdjustment::Offset(column)
                    | crate::data::LoadedAdjustment::Exposure(column) => Ok(Some(column)),
                },
            };
        }
        if auxiliary.is_some_and(|column| {
            std::ptr::eq(column, &loaded.outcome)
                || loaded.weights.as_ref().is_some_and(|(_, existing)| std::ptr::eq(column, existing))
                || matches!(&loaded.adjustment, crate::data::LoadedAdjustment::Offset(existing) |
                    crate::data::LoadedAdjustment::Exposure(existing) if std::ptr::eq(column, existing))
                || loaded.sources.iter().any(|source|
                    matches!(source, LoadedSource::Numeric(existing) if std::ptr::eq(column, existing)))
        }) {
            return Err(invalid(
                "an existing bank must use the declared bias-unit source binding",
            ));
        }
        Ok(auxiliary)
    }
}
pub(super) fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
