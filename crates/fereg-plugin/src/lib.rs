#![doc = "Narrow Stata FFI and safe action dispatcher for fereg."]

mod action_memory;
mod cancellable;
mod developer_injection;
mod dispatch;
mod estimate;
#[cfg_attr(not(feature = "ffi"), allow(dead_code))]
mod fused_kernels;
mod input;
mod input_memory;
mod instrumentation;
mod memory_forecast;
mod memory_report;
mod metadata_memory;
mod model_metadata;
mod numeric_pipeline;
mod ols_frame_memory;
mod ols_preparation_memory;
mod ols_publication_memory;
mod output;
mod ppml_estimate;
mod ppml_execution;
mod ppml_result;
mod publication;
mod result;

#[cfg(feature = "ffi")]
mod ffi;

pub use dispatch::{dispatch_args, ACTION_DOCTOR, ACTION_SELFTEST, ACTION_VERSION};
pub use estimate::{estimate_ols_test, ACTION_ESTIMATE_OLS};
pub use input::{prepare_ols_test, SpiApi, ACTION_PREPARE_OLS_TEST};
pub use output::{BufferSink, OutputSink};
pub use ppml_estimate::{estimate_ppml_test, ACTION_ESTIMATE_PPML};

#[cfg(feature = "ffi")]
pub use ffi::fereg_dispatch;
