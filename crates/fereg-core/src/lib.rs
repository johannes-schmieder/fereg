#![forbid(unsafe_code)]
#![doc = "Safe, Stata-independent numerical core for fereg."]

pub mod data;
pub mod diagnostics;
pub mod error;
pub mod factorize;
pub mod fe;
pub mod interrupt;
pub mod kernels;
pub mod linalg;
pub mod memory;
pub mod metadata;
pub mod ols;
pub mod options;
pub mod parallel;
pub mod ppml;
pub mod ppml_separation;
#[path = "ols/prepared_categorical_data.rs"]
mod prepared_categorical_data;
#[path = "ols/prepared_categorical_rebase.rs"]
mod prepared_categorical_rebase;
#[path = "ols/prepared_fit_view.rs"]
pub(crate) mod prepared_fit_view;
pub mod protocol;
mod sandwich;
pub mod selftest;
#[path = "ols/topology_dof.rs"]
pub mod topology_dof;

pub use data::{
    prepare_numeric_sample, prepare_numeric_sample_with_source_uses, AdjustmentInput, DenseMatrix,
    DomainDecision, DropReason, NumericColumnView, NumericDomainPolicy, NumericPreparationInput,
    NumericSourceUse, PreparedAdjustment, PreparedWeights, WeightInput, WeightKind,
};
pub use diagnostics::{BuildInfo, DoctorInput, DoctorReport};
pub use error::{ErrorCode, FeregError, Result};
pub use factorize::factorize_f64;
pub use fe::{
    absorb_borrowed_intercept_map_certified, absorb_lsmr, absorb_map, certify_residuals,
    connected_components, factorize_code_interaction, AbsorbConfig, BorrowedMapWeights,
    CategoricalCodeView, EffectTerm, FeDesign, FeSolverKind, LsmrConfig, LsmrPreconditioner,
    MapConfig, SolveReport, SolveStopReason,
};
pub use memory::{
    estimate_phase2_memory, plan_ols_memory, FeSolverMemoryBudget, MemoryEstimate, MemoryPlan,
};
pub use metadata::{
    AbsorbComponentRole, AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledModelSpecification,
    CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm, LevelState, MetadataFlag,
    ModelWireV2, NumericWireTable, RawTermKind, SourceEntry, SourceId, TermComponent, TermId,
    TermOrder, TimeSeriesState, ABSORB_COMPONENT_ROW_WIDTH, ABSORB_TERM_ROW_WIDTH,
    COMPONENT_ROW_WIDTH, SOURCE_ROW_WIDTH, TERM_ROW_WIDTH,
};
pub use ols::{
    absorb_prepared_ols_rhs_map, build_prepared_ols_rhs,
    fit_prepared_model_clustered_with_loaded_moments, fit_prepared_model_coefficients,
    fit_prepared_model_robust_with_loaded_moments,
    fit_prepared_model_unadjusted_with_loaded_moments, prepare_model, prepare_model_owned,
    CertifiedPreparedCategoricalOlsClustered, CertifiedPreparedOlsClustered,
    CertifiedPreparedOlsFit, CertifiedPreparedOlsRobust, CertifiedPreparedOlsUnadjusted,
    LinearSessionAllocationPhase, LinearSessionPhaseAllocationEvent,
    LinearSessionPhaseAllocatorProbe, LoadedNumericColumn, LoadedOlsInput, OlsClusterTermV1,
    OlsClusteredRequestV1, OlsInputBinding, OlsInputRole, OlsRequestV1,
    PreparedCategoricalOlsClusteredData, PreparedOlsAbsorbedDofPolicy, PreparedOlsBreadCoordinate,
    PreparedOlsBreadCoordinateKind, PreparedOlsCategoricalComponent,
    PreparedOlsClusterSubsetCertificate, PreparedOlsClusteredCertificate,
    PreparedOlsClusteredConfig, PreparedOlsClusteredCovariance, PreparedOlsClusteredData,
    PreparedOlsClusteredDofReport, PreparedOlsClusteredPsdState, PreparedOlsCoefficient,
    PreparedOlsCoefficientStatus, PreparedOlsColumn, PreparedOlsColumnKind, PreparedOlsData,
    PreparedOlsExecutionConfig, PreparedOlsExecutionMode, PreparedOlsExecutionReport,
    PreparedOlsFitConfig, PreparedOlsFitStatistics, PreparedOlsFitStatus, PreparedOlsModel,
    PreparedOlsReferenceWeightReport, PreparedOlsResidualDofReport, PreparedOlsRobustCertificate,
    PreparedOlsRobustConfig, PreparedOlsUnadjustedCertificate, PreparedOlsUnadjustedConfig,
};
pub use options::{MemoryMode, ThreadConfig};
pub use parallel::ThreadTeam;
pub use ppml::{
    fit_prepared_ppml, LoadedPpmlInput, PpmlConfig, PpmlSolverFamily, PreparedPpmlOutputRequest,
    PreparedPpmlRequestV1,
};
pub use ppml_separation::{
    MethodStatus, SeparationCertificate, SeparationConfig, SeparationDropReason, SeparationMethod,
    SeparationMethodReport, SeparationMethods, SeparationReport,
};
pub use prepared_categorical_data::PreparedCategoricalOlsData;
pub use prepared_categorical_rebase::PreparedCategoricalOlsReportPlan;
pub use protocol::{
    Handshake, PpmlAdjustmentV1, PpmlResultReceiptV1, PpmlResultVceV1, ACTION_PROTOCOL_VERSION,
    ADO_VERSION, MODEL_WIRE_VERSION, OLS_RESULT_WIRE_VERSION, PLUGIN_VERSION,
    PPML_REQUEST_WIRE_VERSION, PPML_RESULT_MAGIC, PPML_RESULT_RECEIPT_COLUMNS,
    PPML_RESULT_RECEIPT_COMPLETE_COLUMN, PPML_RESULT_RECEIPT_COMPLETE_VALUE,
    PPML_RESULT_WIRE_VERSION,
};
pub use selftest::run_native_selftest;
pub use topology_dof::{
    CategoricalTopologyDofEffectIdentity, CategoricalTopologyDofPolicy,
    CategoricalTopologyDofReport,
};
