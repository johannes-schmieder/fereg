//! Preflight binds immutable metadata, products and one admission before loading.
use crate::{
    data::{CompiledColumns, Loaded, SourcePlan, WeightKind},
    dof::DofPolicy,
    memory::{self, Admitted, Inputs, Layout, Plan},
    outputs,
    spec::{Estimator, Options},
    threads::ThreadTeam,
    vce, ErrorCode, FeregError, ModelSpec, Result,
};
use std::time::Instant;

#[derive(Debug, Default)]
pub struct Request {
    /// Omission chooses clustered, probability-weight robust, or otherwise IID.
    pub vce: Option<vce::Kind>,
    pub dof: DofPolicy,
    pub outputs: outputs::Request,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct Resources {
    pub usable_bytes: Option<u128>,
    pub publication_matrix_cells: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Loaded,
    Prepared,
    Estimated,
    Inferred,
    Outputs,
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
        let columns = CompiledColumns::new(model, Estimator::Ols)?;
        let sources = SourcePlan::new(model)?;
        if inputs.weighted != weight_kind.is_some() {
            return Err(invalid(
                "preflight weight declaration disagrees with input bounds",
            ));
        }
        if weight_kind == Some(WeightKind::Importance) {
            return Err(FeregError::new(
                ErrorCode::UnsupportedWeight,
                "OLS does not support importance weights",
            ));
        }
        if inputs.adjustment {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "OLS does not support offset or exposure",
            ));
        }
        let kind = vce::ols_kind(request.vce, weight_kind, model.clusters.len())?;
        let products = request.outputs.kinds(false)?;
        for &(effect, channel) in &request.outputs.saved {
            let channels = if columns.implicit_constant && effect == 0 {
                Some(1)
            } else {
                model
                    .absorbed_terms
                    .get(effect)
                    .map(|e| e.slope_sources.len() + usize::from(e.intercept))
            };
            if channels.is_none_or(|count| channel >= count) {
                return Err(invalid(
                    "saved FE channel does not exist in the declared model",
                ));
            }
        }
        inputs = inputs.with_model(
            model,
            request.outputs.saved.capacity() as u128 * 2 * size_of::<usize>() as u128,
        )?;
        let mut options = *options;
        if options.threads == 0 {
            options.threads =
                std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        }
        let layout = Layout::new(model, inputs, Estimator::Ols)?;
        let mut storage = layout.request(options.solver);
        storage.observation_outputs = products.len();
        storage.publication_matrix_cells = resources.publication_matrix_cells;
        storage.worker_threads = options.threads;
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
    pub fn run(self, loaded: &Loaded) -> Result<super::OlsFit> {
        self.run_with(loaded, &mut |_| Ok(()))
    }
    /// Callbacks execute only on the caller, at complete phase boundaries.
    pub fn run_with(
        self,
        loaded: &Loaded,
        interrupt: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<super::OlsFit> {
        self.bind(loaded)?;
        interrupt(Phase::Loaded)?;
        let team = ThreadTeam::new(self.options.threads)?;
        super::fit::execute(self, loaded, &team, interrupt)
    }
    /// Host-owned teams use the same controller; this entry consumes admission.
    pub fn run_with_team(
        self,
        loaded: &Loaded,
        team: &ThreadTeam,
        interrupt: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<super::OlsFit> {
        self.bind(loaded)?;
        if team.threads() != self.options.threads {
            return Err(invalid(
                "worker team differs from the preflight declaration",
            ));
        }
        interrupt(Phase::Loaded)?;
        super::fit::execute(self, loaded, team, interrupt)
    }
    fn bind(&self, loaded: &Loaded) -> Result<()> {
        self.inputs.bind_loaded(loaded)?;
        if loaded.weights.as_ref().map(|(kind, _)| *kind) != self.weight_kind {
            return Err(invalid("loaded weight kind differs from preflight"));
        }
        Ok(())
    }
}
pub(super) fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
