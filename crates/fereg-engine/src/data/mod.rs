//! Shared source preparation and finite-column generation for both estimators.
mod clusters;
pub use clusters::ClusterFrame;
mod columns;
pub use columns::{ColumnSlot, CompiledColumns, SyntaxStatus};
mod loaded;
mod model_matrix;
mod prepare;
mod prepared;
pub(crate) use prepared::retain_rows;
mod reference_weights;
mod sample;
mod source_plan;
pub use source_plan::SourcePlan;
mod weights;
pub(crate) use reference_weights::exact_count;
pub use reference_weights::ReferenceWeights;

use crate::error::Result;
pub use loaded::{Loaded, LoadedAdjustment, LoadedSource, NumericColumn};
pub use model_matrix::{ColumnRecipe, DenseMatrix, ModelMatrixBuilder};
pub use sample::{
    AdjustmentInput, DomainDecision, DropReason, NumericColumnView, NumericDomainPolicy,
    NumericPreparationInput, NumericSourceUse, Prepared, PreparedAdjustment, PreparedWeights,
    WeightInput,
};
pub use weights::{WeightKind, Weights};

pub fn prepare_numeric_sample<P: NumericDomainPolicy>(
    input: &NumericPreparationInput<'_>,
    policy: &P,
) -> Result<Prepared> {
    prepare::prepare_sample(input, None, policy)
}

pub fn prepare_numeric_sample_with_source_uses<P: NumericDomainPolicy>(
    input: &NumericPreparationInput<'_>,
    uses: &[NumericSourceUse],
    policy: &P,
) -> Result<Prepared> {
    prepare::prepare_sample(input, Some(uses), policy)
}
