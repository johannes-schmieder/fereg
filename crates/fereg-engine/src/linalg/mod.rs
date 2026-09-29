//! Shared scaled rank-revealing algebra, independent of estimator and FE routing.
mod qr;
mod scaling;
pub(crate) use qr::{scale_ratio, WeightedQr};
pub(crate) use scaling::{ColumnScaling, ScaledDesign};
