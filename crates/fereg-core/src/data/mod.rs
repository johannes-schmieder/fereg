//! Native data preparation and finite model-matrix construction.

mod axis;
mod model_matrix;
mod sample;
mod weights;

pub use axis::FinalizedAxis;
pub use model_matrix::DenseMatrix;
pub(crate) use sample::{
    classify_numeric_sample_into_mask, classify_numeric_sample_with_source_uses,
    NumericSampleClassification,
};
pub use sample::{
    prepare_numeric_sample, prepare_numeric_sample_with_source_uses, AdjustmentInput,
    DomainDecision, DropReason, NumericColumnView, NumericDomainPolicy, NumericPreparationInput,
    NumericSourceUse, PreparedAdjustment, PreparedNumericSample, PreparedWeights, WeightInput,
};
pub use weights::WeightKind;
