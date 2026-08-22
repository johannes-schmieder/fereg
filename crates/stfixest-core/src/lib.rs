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
pub mod ppml_separation;
pub mod protocol;
pub mod selftest;

pub use data::{
    prepare_numeric_sample, prepare_numeric_sample_with_source_uses, AdjustmentInput, ColumnRecipe,
    DenseMatrix, DomainDecision, DropReason, ModelMatrixBuilder, NumericColumnView,
    NumericDomainPolicy, NumericPreparationInput, NumericSourceUse, PreparedAdjustment,
    PreparedNumericSample, PreparedWeights, WeightInput, WeightKind,
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
    AbsorbComponent, AbsorbComponentRole, AbsorbTermId, AbsorbedTerm, CoefficientSlot,
    CompiledModelSpecification, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm,
    LevelState, MetadataFlag, ModelWireV2, NumericWireTable, RawTermKind, SourceEntry, SourceId,
    TermComponent, TermId, TermOrder, TimeSeriesState, ABSORB_COMPONENT_ROW_WIDTH,
    ABSORB_TERM_ROW_WIDTH, COMPONENT_ROW_WIDTH, SOURCE_ROW_WIDTH, TERM_ROW_WIDTH,
};
pub use ols::{
    compute_dof, fit_ols, prepare_ols_data, solve_rank_revealing, ClusterSubsetReport, DofConfig,
    DofMethod, DofReport, FiniteSolver, FitStatistics, LoadedNumericColumn, LoadedOlsInput,
    OlsConfig, OlsInputBinding, OlsInputRole, OlsRequestV1, OlsResult, PreparedCategoricalSource,
    PreparedOlsCategoricalComponent, PreparedOlsCoefficient, PreparedOlsCoefficientStatus,
    PreparedOlsColumn, PreparedOlsColumnKind, PreparedOlsData, PreparedOlsEffect, PreparedOlsModel,
    RankSolve, RegressorStatus, VceKind, VceReport,
};
pub use options::{MemoryMode, ThreadConfig};
pub use parallel::ThreadTeam;
pub use ppml::{
    fit_ppml, fit_ppml_with_start, means_from_eta, offset_from_exposure, poisson_objective,
    PoissonBounds, PoissonObjective, PpmlCertificate, PpmlConfig, PpmlDiagnostics,
    PpmlFitStatistics, PpmlInitialization, PpmlIterationReport, PpmlResult, PpmlStart,
};
pub use ppml_separation::{
    detect_separation, fit_ppml_separated, fit_ppml_separated_with_start, MethodStatus,
    SeparatedPpmlResult, SeparationCertificate, SeparationConfig, SeparationDropReason,
    SeparationMethod, SeparationMethodReport, SeparationMethods, SeparationReport,
};
pub use protocol::{
    Handshake, ProtocolVersion, ACTION_PROTOCOL_VERSION, ADO_VERSION, MODEL_WIRE_VERSION,
    PLUGIN_VERSION,
};
pub use selftest::{run_native_selftest, SelfTestCase, SelfTestReport};
