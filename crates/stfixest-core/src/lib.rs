#![forbid(unsafe_code)]
#![doc = "Safe, Stata-independent Phase 0-1 core for StFixest."]

pub mod diagnostics;
pub mod error;
pub mod factorize;
pub mod memory;
pub mod metadata;
pub mod options;
pub mod parallel;
pub mod protocol;
pub mod selftest;

pub use diagnostics::{BuildInfo, DoctorInput, DoctorReport};
pub use error::{ErrorCode, Result, StFixestError};
pub use factorize::{
    factorize_f64, factorize_i64, factorize_strings, interaction_codes, EncodedFactor,
};
pub use memory::{estimate_phase2_memory, MemoryEstimate};
pub use metadata::{
    AbsorbTerm, ClusterTerm, ModelSpec, ModelTerm, OperatorType, VariableRole, VariableSpec,
};
pub use options::{MemoryMode, ThreadConfig};
pub use parallel::ThreadTeam;
pub use protocol::{Handshake, ProtocolVersion, ADO_VERSION, PLUGIN_VERSION, PROTOCOL_VERSION};
pub use selftest::{run_native_selftest, SelfTestCase, SelfTestReport};
