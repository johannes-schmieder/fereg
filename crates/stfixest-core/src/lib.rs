#![forbid(unsafe_code)]
#![doc = "Safe, Stata-independent numerical core for StFixest."]

pub mod data;
pub mod diagnostics;
pub mod error;
pub mod factorize;
pub mod fe;
pub mod linalg;
pub mod memory;
pub mod metadata;
pub mod ols;
pub mod options;
pub mod parallel;
pub mod ppml;
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
    absorb_fixed_effects, absorb_lsmr, absorb_map, certify_residuals, connected_components,
    prune_singletons, AbsorbConfig, ComponentInfo, EffectTerm, FeCertificate, FeDesign,
    FeSolverKind, LsmrConfig, LsmrPreconditioner, MapConfig, SingletonResult, SolveReport,
    SolveStopReason,
};
pub use memory::{estimate_phase2_memory, MemoryEstimate};
pub use metadata::{
    AbsorbTerm, ClusterTerm, ModelSpec, ModelTerm, OperatorType, VariableRole, VariableSpec,
};
pub use ols::{
    compute_dof, fit_ols, solve_rank_revealing, ClusterSubsetReport, DofConfig, DofMethod,
    DofReport, FiniteSolver, FitStatistics, OlsConfig, OlsResult, RankSolve, RegressorStatus,
    VceKind, VceReport,
};
pub use options::{MemoryMode, ThreadConfig};
pub use parallel::ThreadTeam;
pub use ppml::{
    fit_ppml, fit_ppml_with_start, means_from_eta, offset_from_exposure, poisson_objective,
    PoissonBounds, PoissonObjective, PpmlCertificate, PpmlConfig, PpmlDiagnostics,
    PpmlFitStatistics, PpmlInitialization, PpmlIterationReport, PpmlResult, PpmlStart,
};
pub use protocol::{Handshake, ProtocolVersion, ADO_VERSION, PLUGIN_VERSION, PROTOCOL_VERSION};
pub use selftest::{run_native_selftest, SelfTestCase, SelfTestReport};
