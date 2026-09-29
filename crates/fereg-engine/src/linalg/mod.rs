//! Shared scaled rank-revealing algebra, independent of estimator and FE routing.
mod qr;
pub(crate) use qr::WeightedQr;
