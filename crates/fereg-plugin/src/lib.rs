#![doc = "Narrow Stata FFI and safe action dispatcher for fereg."]

mod action_memory;
mod memory_report;
mod memory_forecast;
mod input_memory;
mod metadata_memory;
mod dispatch;
mod estimate;
mod input;
mod numeric_pipeline;
mod instrumentation;
mod ols_frame_memory;
mod ols_preparation_memory;
mod ols_publication_memory;
mod output;
mod ppml_estimate;
mod ppml_result;
mod result;
mod result_view;

#[cfg(feature = "ffi")]
mod ffi;

pub use dispatch::{dispatch_args, ACTION_DOCTOR, ACTION_SELFTEST, ACTION_VERSION};
pub use estimate::{estimate_ols_test, ACTION_ESTIMATE_OLS};
pub use input::{prepare_ols_test, SpiApi, ACTION_PREPARE_OLS_TEST};
pub use output::{BufferSink, OutputSink};
pub use ppml_estimate::{estimate_ppml_test, ACTION_ESTIMATE_PPML};

#[cfg(feature = "ffi")]
pub use ffi::fereg_dispatch;
