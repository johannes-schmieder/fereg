//! Native data preparation and finite model-matrix construction.

mod model_matrix;
mod sample;
mod weights;

pub use model_matrix::{ColumnRecipe, DenseMatrix, ModelMatrixBuilder};
pub use sample::{
    prepare_numeric_sample, prepare_numeric_sample_with_source_uses, AdjustmentInput,
    DomainDecision, DropReason, NumericColumnView, NumericDomainPolicy, NumericPreparationInput,
    NumericSourceUse, PreparedAdjustment, PreparedNumericSample, PreparedWeights, WeightInput,
};
pub use weights::WeightKind;
