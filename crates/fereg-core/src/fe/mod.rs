//! Fixed-effect preprocessing, projection, and certification.

mod accelerated_map;
mod auto;
mod borrowed_map;
pub(crate) use borrowed_map::three_fe_schur;
mod categorical_request;
mod certificate;
mod cmg;
mod components;
mod design;
mod interaction;
mod lsmr;
mod lsmr_batch;
mod map;
mod norms;
mod partition;
mod partition_registry;
mod partition_singleton;
mod partition_validation;
mod projection;
mod workspace_capacity;
pub(crate) use workspace_capacity::{
    categorical_parallel_counts, CategoricalAcceleratorCapacity, MapCapacity,
};
mod report;
pub(crate) mod session;
pub(crate) use session::FeSession;
mod singleton;
pub(crate) mod slope_coefficients;
mod topology_adapter;
mod topology_axis;
mod topology_finalize;
mod topology_map;
mod topology_registry;
mod topology_rhs;
mod topology_slope_map;
#[cfg(test)]
pub(crate) use topology_slope_map::CategoricalRecoveryMethod;
mod topology_source;

pub use accelerated_map::absorb_accelerated_map;
pub use auto::{absorb_fixed_effects, AbsorbConfig};
pub use borrowed_map::{
    absorb_borrowed_intercept_auto_certified, absorb_borrowed_intercept_cg_certified,
    absorb_borrowed_intercept_map_certified, BorrowedMapWeights, CertifiedBorrowedMap,
    FeCoefficientBatch,
};
pub(crate) use borrowed_map::{
    absorb_borrowed_intercept_cg_retaining_certified,
    absorb_borrowed_intercept_cg_with_structural_constant_certified,
    absorb_borrowed_intercept_irons_tuck_certified, absorb_borrowed_intercept_lsmr_certified,
    absorb_borrowed_intercept_map_retaining_certified,
    absorb_borrowed_intercept_map_with_structural_constant_certified,
    absorb_borrowed_intercept_stable_cg_retaining_certified, borrowed_cg_recovery_workspace_bytes,
    project_borrowed_intercept_structural, BorrowedLsmrAbsorber, ReusableCategoricalMapAbsorber,
};
#[cfg(test)]
pub(crate) use borrowed_map::{
    FUSED_PARALLEL_MAX_RHS, PARALLEL_DOT_CHUNK_ROWS, PARALLEL_PROJECTION_MIN_ROWS,
};
pub(crate) use categorical_request::bind_categorical_effect_recipes;
pub use categorical_request::{plan_categorical_absorbed_request, CategoricalAbsorbRequestPlan};
pub use certificate::{certify_residuals, FeCertificate};
pub(crate) use certificate::{certify_residuals_with_reference_norms, weighted_column_norms};
pub(crate) use cmg::{
    explicit_cmg_components, CmgAbsorber, CmgConfig, CmgControlSeed, CmgTopologyProfile,
    CmgUnpolishedProjectionReport, PreparedCmgTopology,
    MAX_NUISANCE_COLUMNS as CMG_MAX_NUISANCE_COLUMNS,
};
pub(crate) use components::connected_components_all_fallible;
pub use components::{connected_components, ComponentInfo};
pub(crate) use design::{retained_effect_order_by_levels, ValidatedBorrowedInterceptSource};
pub use design::{
    BorrowedInterceptEffect, BorrowedInterceptFeDesign, EffectTerm, FeDesign, LevelIndex,
};
pub use interaction::{factorize_code_interaction, CategoricalCodeView, EncodedInteraction};
pub(crate) use interaction::{try_factorize_dense_code_interaction, InteractionReservation};
pub(crate) use lsmr::resolve_preconditioner_kind;
pub use lsmr::{absorb_lsmr, LsmrConfig};
pub use map::{absorb_map, MapConfig};
pub(crate) use map::{MapGeometryGeneration, ReusableMapAbsorber, ReusableMapCapacities};
pub use partition::CategoricalPartition;
pub use partition_registry::CategoricalPartitionRegistry;
pub use partition_singleton::{
    prune_categorical_partition_singletons, CategoricalPartitionSingletonResult,
};
pub use partition_validation::validate_categorical_partition;
pub(crate) use projection::ParallelProjectionPlan;
pub use projection::{project_effect_in_place, ProjectionWorkspace};
pub use report::{FeSolverKind, LsmrPreconditioner, SolveReport, SolveStopReason};
pub use singleton::{
    prune_singletons, prune_singletons_with_multiplicity, SingletonMultiplicity, SingletonResult,
};
pub use topology_adapter::borrow_categorical_topology_design;
pub use topology_finalize::{
    finalize_categorical_topology, CompactedCategoricalSource, FinalizedCategoricalTopology,
};
pub(crate) use topology_finalize::{ValidatedFinalizedTopology, ValidatedSingleTopology};
pub use topology_map::{
    absorb_finalized_categorical_topology_auto_certified,
    absorb_finalized_categorical_topology_cg_certified,
    absorb_finalized_categorical_topology_map_certified,
};
pub use topology_registry::CategoricalTopologyRegistry;
pub use topology_rhs::{
    compact_categorical_rhs, CandidateCategoricalWeights, CompactedCategoricalRhs,
};
pub use topology_slope_map::{
    absorb_finalized_categorical_intercept_slope_map_certified,
    absorb_finalized_categorical_slope_map_certified,
    recover_finalized_categorical_effects_map_certified, CategoricalAbsorbedEffectView,
    RecoveredCategoricalEffects,
};
pub(crate) use topology_slope_map::{
    recover_finalized_model_effects_certified, slope_map_workspace_bytes,
};
pub use topology_source::CategoricalTopologySource;

#[cfg(test)]
pub(crate) use borrowed_map::certify_borrowed_intercept_candidate;

#[cfg(test)]
pub(crate) use topology_slope_map::{
    single_slope_recovery_extra_bytes, single_slope_recovery_geometry_bytes,
};
