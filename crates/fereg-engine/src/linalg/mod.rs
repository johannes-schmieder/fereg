//! Shared scaled rank-revealing algebra, independent of estimator and FE routing.
mod qr;
mod scaling;
pub(crate) use qr::WeightedQr;
pub(crate) use scaling::{ColumnScaling, ScaledDesign};
