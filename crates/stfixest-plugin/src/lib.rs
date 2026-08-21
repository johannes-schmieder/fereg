#![doc = "Narrow Stata FFI and safe action dispatcher for StFixest."]

mod dispatch;
mod input;
mod output;

#[cfg(feature = "ffi")]
mod ffi;

pub use dispatch::{dispatch_args, ACTION_DOCTOR, ACTION_SELFTEST, ACTION_VERSION};
pub use input::{prepare_ols_test, SpiApi, ACTION_PREPARE_OLS_TEST};
pub use output::{BufferSink, OutputSink};

#[cfg(feature = "ffi")]
pub use ffi::stfixest_dispatch;
