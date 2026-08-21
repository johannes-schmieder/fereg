#![forbid(unsafe_code)]
#![doc = "Safe, Stata-independent Phase 0-1 core for StFixest."]

pub mod data;
pub mod diagnostics;
pub mod error;
pub mod factorize;
pub mod fe;
pub mod linalg;
pub mod memory;
pub mod metadata;
pub mod options;
pub mod parallel;
pub mod protocol;
pub mod selftest;

pub use data::{
    prepare_numeric_data, ColumnRecipe, DenseMatrix, DropReason, ModelMatrixBuilder,
    NumericPreparationInput, PreparedData, WeightKind,
};
pub use diagnostics::{BuildInfo, DoctorInput, DoctorReport};
pub use error::{ErrorCode, Result, StFixestError};
pub use factorize::{
    factorize_f64, factorize_i64, factorize_strings, interaction_codes, EncodedFactor,
};
pub use fe::{
    absorb_map, certify_residuals, connected_components, prune_singletons, ComponentInfo,
    EffectTerm, FeCertificate, FeDesign, MapConfig, SingletonResult, SolveReport, SolveStopReason,
};
pub use memory::{estimate_phase2_memory, MemoryEstimate};
pub use metadata::{
    AbsorbTerm, ClusterTerm, ModelSpec, ModelTerm, OperatorType, VariableRole, VariableSpec,
};
pub use options::{MemoryMode, ThreadConfig};
pub use parallel::ThreadTeam;
pub use protocol::{Handshake, ProtocolVersion, ADO_VERSION, PLUGIN_VERSION, PROTOCOL_VERSION};
pub use selftest::{run_native_selftest, SelfTestCase, SelfTestReport};
