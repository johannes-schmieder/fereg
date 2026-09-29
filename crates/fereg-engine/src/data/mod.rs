//! Shared source preparation and finite-column generation for both estimators.
mod loaded;
mod model_matrix;
mod prepare;
mod prepared;
mod sample;
mod weights;

use crate::error::Result;
pub use loaded::{Loaded, LoadedAdjustment, LoadedSource, NumericColumn};
pub use model_matrix::{ColumnRecipe, DenseMatrix, ModelMatrixBuilder};
pub use sample::{
    AdjustmentInput, DomainDecision, DropReason, NumericColumnView, NumericDomainPolicy,
    NumericPreparationInput, NumericSourceUse, Prepared, PreparedAdjustment, PreparedWeights,
    WeightInput,
};
pub use weights::WeightKind;

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
