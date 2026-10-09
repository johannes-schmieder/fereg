//! Certified Frisch-Waugh-Lovell OLS core.

mod canonical_products;
mod dof;
mod finite_operations;
mod finite_wls_kernel;
pub(crate) use canonical_products::canonical_three_workspace_bytes;
mod implicit_centered;
mod linear_session;
pub(crate) use linear_session::coefficient_accuracy_scale;
pub(crate) use linear_session::{ppml_coefficient_lifetime_bytes, ppml_component_checkpoint_bytes};
mod prepared;
mod prepared_categorical_clusters;
mod prepared_clustered;
mod prepared_clustered_view;
mod prepared_clusters;
mod prepared_dof;
mod prepared_fit;
mod prepared_inference_view;
mod prepared_rhs;
pub(crate) use prepared_rhs::retained_rhs_stride;
mod prepared_products;
pub use prepared_products::{CertifiedPreparedOlsInference, PreparedOlsInferenceBuffers};
mod prepared_robust;
mod prepared_scores;
mod prepared_scratch;
mod prepared_unadjusted;
mod prepared_wald;
mod rank;
mod vce;

pub use dof::{compute_dof, DofConfig, DofMethod, DofReport};
pub(crate) use finite_wls_kernel::retained_scratch_moment_envelope;
pub(crate) use finite_wls_kernel::{solve_finite_direction, FiniteDirectionWorkspace};
pub(crate) use implicit_centered::ImplicitCenteredDesign;
#[cfg(test)]
pub(crate) use linear_session::{
    begin_frozen_qr_fallback_observation, finish_frozen_qr_fallback_observation,
    inject_frozen_linear_failure, CertifiedFrozenLinearOutput, FrozenLinearFailureInjection,
};
#[allow(unused_imports)]
pub(crate) use linear_session::{
    classify_frozen_linear_fallback, CertifiedLinearSession, CertifiedLinearSessionOutput,
    CertifiedLinearSessionProducts, CertifiedLinearWithinDesign, FrozenLinearBasis,
    FrozenLinearBasisSlot, FrozenLinearFallbackKind, FrozenLinearSolveCertificate,
    LinearConstantRepresentation, LinearReconstructionCertificate, LinearSessionGenerations,
    LinearSessionPhaseAllocationObservation, LinearSessionReuseReport, LinearSessionSolveKind,
    LinearSessionWorkspaceCapacities, LinearWithinDesignCertificate, LinearWithinMemoryReport,
    PpmlOrdinaryLinearOutput, StrictLinearCandidateTransaction,
};
#[doc(hidden)]
pub use linear_session::{
    LinearSessionAllocationPhase, LinearSessionPhaseAllocationEvent,
    LinearSessionPhaseAllocatorProbe,
};
pub(crate) use prepared::{
    factorize_preparation_sources, prepare_categorical_numeric_sample,
    prepare_categorical_numeric_sample_owned, prepare_ols_numeric_sample, FeTopologyReservation,
    SystemFeTopologyReservation,
};
pub use prepared::{
    prepare_ols_data, prepare_ols_data_owned, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding,
    OlsInputRole, OlsRequestV1, PreparedCategoricalSource, PreparedOlsCategoricalComponent,
    PreparedOlsCoefficient, PreparedOlsCoefficientStatus, PreparedOlsColumn, PreparedOlsColumnKind,
    PreparedOlsData, PreparedOlsFeSingletonPolicy, PreparedOlsModel, PreparedOlsSingletons,
};
pub(crate) use prepared_categorical_clusters::prepare_ppml_categorical_ols_clustered_data;
pub use prepared_categorical_clusters::{
    prepare_categorical_ols_clustered_data, prepare_categorical_ols_clustered_data_owned,
    PreparedCategoricalOlsClusteredData,
};
pub use prepared_clustered::{
    fit_prepared_categorical_ols_clustered_map,
    fit_prepared_categorical_ols_clustered_map_with_loaded_moments,
    fit_prepared_model_clustered_with_loaded_moments, fit_prepared_ols_clustered_map,
    fit_prepared_ols_clustered_map_with_loaded_moments, CertifiedPreparedCategoricalOlsClustered,
    CertifiedPreparedOlsClustered, PreparedOlsClusterSubsetCertificate,
    PreparedOlsClusteredCertificate, PreparedOlsClusteredConfig, PreparedOlsClusteredCovariance,
    PreparedOlsClusteredDofReport, PreparedOlsClusteredPsdReport, PreparedOlsClusteredPsdState,
};
pub use prepared_clusters::{
    compile_prepared_model, prepare_model_owned, prepare_ols_clustered_data,
    prepare_ols_clustered_data_owned, OlsClusterTermV1, OlsClusteredRequestV1,
    PreparedOlsClusterDesign, PreparedOlsClusterDimension, PreparedOlsClusteredData,
};
pub use prepared_dof::{
    compute_prepared_ols_absorbed_dof, PreparedOlsAbsorbedDofEffect,
    PreparedOlsAbsorbedDofEffectIdentity, PreparedOlsAbsorbedDofPolicy,
    PreparedOlsAbsorbedDofReport,
};
pub use prepared_fit::{
    fit_prepared_categorical_ols_coefficients_map, fit_prepared_ols_coefficients_map,
    CertifiedPreparedOlsFit, LoadedOlsRows, PreparedOlsBread, PreparedOlsBreadCertificate,
    PreparedOlsBreadCoordinate, PreparedOlsBreadCoordinateKind, PreparedOlsBreadWeightScale,
    PreparedOlsExecutionConfig, PreparedOlsExecutionMode, PreparedOlsExecutionReport,
    PreparedOlsFitConfig, PreparedOlsFitStatus, PreparedOlsFittedCoefficient,
    PreparedOlsRankCertificate, PreparedOlsScoreCertificate,
};
#[allow(unused_imports)]
pub(crate) use prepared_fit::{
    fit_prepared_weighted_linear_projection_once, CertifiedPreparedLinearProjection,
    LinearSolveFrame, PreparedLinearProjectionReconstructionCertificate,
};
pub use prepared_rhs::{
    absorb_prepared_ols_rhs_map, add_numeric_ols_predictor, build_prepared_ols_rhs, PreparedOlsRhs,
};
pub(crate) use prepared_rhs::{
    build_prepared_ols_rhs_with_loaded_rows_and_reuse, build_prepared_ols_rhs_with_response,
    numeric_sources_are_finite, validate_prepared_model_metadata, NumericOlsColumns,
    MAX_BOUND_NUMERIC_COLUMNS,
};
pub use prepared_robust::{
    fit_prepared_categorical_ols_robust_map,
    fit_prepared_categorical_ols_robust_map_with_loaded_moments,
    fit_prepared_model_robust_with_loaded_moments, fit_prepared_ols_robust_map,
    fit_prepared_ols_robust_map_with_loaded_moments, CertifiedPreparedCategoricalOlsRobust,
    CertifiedPreparedOlsRobust, PreparedOlsRobustCertificate, PreparedOlsRobustConfig,
    PreparedOlsRobustCovariance,
};
pub(crate) use prepared_scratch::{conservative_scratch_io_bytes, ols_scratch_transpose_reserve};
pub use prepared_unadjusted::{
    fit_prepared_categorical_ols_unadjusted_map,
    fit_prepared_categorical_ols_unadjusted_map_with_loaded_moments,
    fit_prepared_model_unadjusted_with_loaded_moments, fit_prepared_ols_unadjusted_map,
    fit_prepared_ols_unadjusted_map_with_loaded_moments,
    fit_prepared_ols_unadjusted_map_with_loaded_rows, CertifiedPreparedCategoricalOlsUnadjusted,
    CertifiedPreparedOlsUnadjusted, PreparedOlsFitStatistics, PreparedOlsReferenceWeightReport,
    PreparedOlsResidualDofReport, PreparedOlsUnadjustedCertificate, PreparedOlsUnadjustedConfig,
    PreparedOlsUnadjustedCovariance,
};
pub use rank::{solve_rank_revealing, FiniteSolver, RankSolve};
pub use vce::{ClusterSubsetReport, VceKind, VceReport};

/// Status of one canonical finite-dimensional model column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressorStatus {
    Kept,
    BaseLevel,
    SyntaxOmitted,
    CollinearWithFixedEffects,
    CollinearAfterResidualization,
    NumericallyRankDeficient,
}
