//! Native data preparation and finite model-matrix construction.

mod model_matrix;
mod sample;
mod weights;

pub use model_matrix::{ColumnRecipe, DenseMatrix, ModelMatrixBuilder};
pub use sample::{prepare_numeric_data, DropReason, NumericPreparationInput, PreparedData};
pub use weights::{validate_weight, WeightKind};
