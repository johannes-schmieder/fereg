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
#[path = "ols/topology_dof.rs"]
pub mod topology_dof;

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
    absorb_borrowed_intercept_map_certified, absorb_finalized_categorical_topology_map_certified,
    absorb_fixed_effects, absorb_lsmr, absorb_map, borrow_categorical_topology_design,
    certify_residuals, connected_components, factorize_code_interaction,
    finalize_categorical_topology, prune_categorical_partition_singletons, prune_singletons,
    prune_singletons_with_multiplicity, validate_categorical_partition, AbsorbConfig,
    BorrowedInterceptEffect, BorrowedInterceptFeDesign, BorrowedMapWeights,
    CategoricalCodeView, CategoricalPartition, CategoricalPartitionRegistry,
    CategoricalPartitionSingletonResult, CategoricalTopologyRegistry, CategoricalTopologySource,
    CertifiedBorrowedMap, CompactedCategoricalSource, ComponentInfo, EffectTerm, EncodedInteraction,
    FeCertificate, FeDesign, FeSolverKind, FinalizedCategoricalTopology, LsmrConfig,
    LsmrPreconditioner, MapConfig, SingletonMultiplicity, SingletonResult, SolveReport,
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
    absorb_prepared_ols_rhs_map, build_prepared_ols_rhs, compute_dof,
    compute_prepared_ols_absorbed_dof, fit_ols, fit_prepared_ols_clustered_map,
    fit_prepared_ols_coefficients_map, fit_prepared_ols_robust_map,
    fit_prepared_ols_unadjusted_map, prepare_ols_clustered_data, prepare_ols_data,
    solve_rank_revealing, CertifiedPreparedOlsFit, CertifiedPreparedOlsRobust,
    CertifiedPreparedOlsUnadjusted, ClusterSubsetReport, DofConfig, DofMethod, DofReport,
    FiniteSolver, FitStatistics, LoadedNumericColumn, LoadedOlsInput, OlsClusterTermV1,
    OlsClusteredRequestV1, OlsConfig, OlsInputBinding, OlsInputRole, OlsRequestV1, OlsResult,
    PreparedCategoricalSource, PreparedOlsAbsorbedDofEffect, PreparedOlsAbsorbedDofEffectIdentity,
    PreparedOlsAbsorbedDofPolicy, PreparedOlsAbsorbedDofReport, PreparedOlsBread,
    PreparedOlsBreadCertificate, PreparedOlsBreadCoordinate, PreparedOlsBreadCoordinateKind,
    PreparedOlsBreadWeightScale, PreparedOlsCategoricalComponent, PreparedOlsClusterDesign,
    PreparedOlsClusterDimension, PreparedOlsClusterSubsetCertificate,
    PreparedOlsClusteredCertificate, PreparedOlsClusteredConfig, PreparedOlsClusteredCovariance,
    PreparedOlsClusteredData, PreparedOlsClusteredDofReport, PreparedOlsClusteredPsdReport,
    PreparedOlsClusteredPsdState, PreparedOlsCoefficient, PreparedOlsCoefficientStatus,
    PreparedOlsColumn, PreparedOlsColumnKind, PreparedOlsData, PreparedOlsEffect,
    PreparedOlsFeSingletonPolicy, PreparedOlsFeTopology, PreparedOlsFeTopologySource,
    PreparedOlsFitConfig, PreparedOlsFitStatistics, PreparedOlsFitStatus,
    PreparedOlsFittedCoefficient, PreparedOlsModel, PreparedOlsRankCertificate,
    PreparedOlsReferenceWeightReport, PreparedOlsResidualDofReport, PreparedOlsRhs,
    PreparedOlsRobustCertificate, PreparedOlsRobustConfig, PreparedOlsRobustCovariance,
    PreparedOlsScoreCertificate, PreparedOlsSingletons, PreparedOlsUnadjustedCertificate,
    PreparedOlsUnadjustedConfig, PreparedOlsUnadjustedCovariance, RankSolve, RegressorStatus,
    VceKind, VceReport,
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
    OLS_RESULT_WIRE_VERSION, PLUGIN_VERSION,
};
pub use selftest::{run_native_selftest, SelfTestCase, SelfTestReport};
pub use topology_dof::{
    compute_categorical_topology_absorbed_dof, CategoricalTopologyDofEffect,
    CategoricalTopologyDofEffectIdentity, CategoricalTopologyDofEffectSpec,
    CategoricalTopologyDofPolicy, CategoricalTopologyDofReport,
};
