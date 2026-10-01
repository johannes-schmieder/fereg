//! Metadata-only bounds available before any observation-scale allocation.
use super::{checked::add_count, Effect, Inputs, Request};
use crate::{
    data::{CompiledColumns, Loaded},
    error::{reserved, Result},
    execution,
    spec::{Estimator, ModelSpec, Solver},
    ErrorCode, FeregError,
};

#[derive(Debug)]
pub struct Layout {
    inputs: Inputs,
    effects: Vec<Effect>,
    finite_columns: usize,
    coefficient_slots: usize,
    estimator: Estimator,
    clusters: usize,
}
impl Layout {
    /// Every categorical interaction has at most one level per candidate row.
    /// These bounds do not use factor values to decide scientific eligibility.
    pub fn new(model: &ModelSpec, inputs: Inputs, estimator: Estimator) -> Result<Self> {
        let columns = CompiledColumns::new(model, estimator)?;
        Self::from_columns_polled(model, inputs, estimator, &columns, &mut || Ok(()))
    }
    // Call preflight lends its validated columns rather than compiling a second owner.
    pub(crate) fn from_columns_polled(
        model: &ModelSpec,
        inputs: Inputs,
        estimator: Estimator,
        columns: &CompiledColumns,
        poll: &mut impl FnMut() -> Result<()>,
    ) -> Result<Self> {
        poll()?;
        if inputs.sources != model.model.sources.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "storage and model source counts differ",
            ));
        }
        let mut effects = reserved(add_count(model.absorbed_terms.len(), 1)?)?;
        for (index, term) in model.absorbed_terms.iter().enumerate() {
            execution::check_chunk_with(index, poll)?;
            effects.push(Effect {
                levels: inputs.active_rows,
                channels: add_count(term.slope_sources.len(), usize::from(term.intercept))?,
            });
        }
        if columns.implicit_constant {
            effects.push(Effect {
                levels: 1,
                channels: 1,
            });
        }
        poll()?;
        Ok(Self {
            inputs,
            effects,
            finite_columns: columns.recipes.len(),
            coefficient_slots: columns.slots.len(),
            estimator,
            clusters: model.clusters.len(),
        })
    }
    /// Already-loaded native callers use the same layout. The host should use
    /// preloading bounds instead; this scan does not allocate a second row bank.
    pub fn from_loaded(
        model: &ModelSpec,
        loaded: &Loaded,
        estimator: Estimator,
        metadata_bytes: u128,
        parent_bytes: u128,
    ) -> Result<Self> {
        loaded.validate_alignment()?;
        Self::new(
            model,
            super::loaded::inputs(loaded, metadata_bytes, parent_bytes)?,
            estimator,
        )
    }
    /// The caller adds requested publication products to this single request.
    #[must_use]
    pub fn request(&self, solver: Solver) -> Request<'_> {
        Request {
            inputs: self.inputs,
            effects: &self.effects,
            finite_columns: self.finite_columns,
            coefficient_slots: self.coefficient_slots,
            estimator: self.estimator,
            solver,
            clusters: self.clusters,
            worker_threads: 0,
            observation_outputs: 0,
            publication_matrix_cells: 0,
            bias: None,
            simplex_cells: 0,
        }
    }
}
