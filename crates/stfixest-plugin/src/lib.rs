#![doc = "Narrow Stata FFI and safe action dispatcher for StFixest."]

mod dispatch;
mod output;

#[cfg(feature = "ffi")]
mod ffi;

pub use dispatch::{dispatch_args, ACTION_DOCTOR, ACTION_SELFTEST, ACTION_VERSION};
pub use output::{BufferSink, OutputSink};

#[cfg(feature = "ffi")]
pub use ffi::stfixest_dispatch;
