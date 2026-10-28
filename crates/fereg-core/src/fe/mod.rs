//! Fixed-effect preprocessing, projection, and certification.

mod accelerated_map;
mod auto;
mod borrowed_map;
mod routing;
pub(crate) mod routing_break;
pub(crate) mod routing_direct;
pub(crate) mod routing_gates;
mod routing_kernel;
pub(crate) mod routing_observer;
pub(crate) mod routing_remaining;
pub(crate) use borrowed_map::three_fe_schur;
mod categorical_request;
mod certificate;
mod cmg;
mod cmg_schur;
mod components;
mod design;
pub(crate) mod exact_sections;
mod interaction;
mod lsmr;
mod lsmr_batch;
mod map;
mod norms;
mod partition;
mod partition_registry;
mod partition_singleton;
mod partition_validation;
pub(crate) mod ppml_topology;
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
mod topology_finalize;
mod topology_map;
mod topology_registry;
mod topology_slope_map;
#[cfg(test)]
pub(crate) use topology_slope_map::CategoricalRecoveryMethod;
mod topology_source;

pub use accelerated_map::absorb_accelerated_map;
pub use auto::{absorb_fixed_effects, AbsorbConfig};
#[cfg(test)]
pub(crate) use borrowed_map::derived_residual::inject_derived_certificate_failure_once;
pub(crate) use borrowed_map::{
    absorb_borrowed_intercept_cg, absorb_borrowed_intercept_irons_tuck_certified,
    absorb_borrowed_intercept_lsmr, absorb_borrowed_intercept_map,
    borrowed_cg_recovery_workspace_bytes, project_borrowed_intercept_structural, BorrowedCgOptions,
    BorrowedLsmrAbsorber, JointCmgBlock, PpmlCmg, PpmlCmgScratch, PpmlCmgView,
    ReusableCategoricalMapAbsorber, PARALLEL_DOT_CHUNK_ROWS,
};
pub use borrowed_map::{
    absorb_borrowed_intercept_map_certified, BorrowedMapWeights, CertifiedBorrowedMap,
    FeCoefficientBatch,
};
#[cfg(test)]
pub(crate) use borrowed_map::{FUSED_PARALLEL_MAX_RHS, PARALLEL_PROJECTION_MIN_ROWS};
pub(crate) use categorical_request::bind_categorical_effect_recipes;
pub use categorical_request::{plan_categorical_absorbed_request, CategoricalAbsorbRequestPlan};
pub use certificate::{certify_residuals, FeCertificate};
pub(crate) use certificate::{certify_residuals_with_reference_norms, weighted_column_norms};
pub(crate) use cmg::{
    explicit_cmg_components, CmgAbsorber, CmgConfig, CmgControlSeed, CmgTopologyProfile,
    CmgUnpolishedProjectionReport, PreparedCmgTopology,
    MAX_NUISANCE_COLUMNS as CMG_MAX_NUISANCE_COLUMNS,
};
pub(crate) use components::connected_components_all;
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
pub use projection::ProjectionWorkspace;
pub use report::{FeSolverKind, LsmrPreconditioner, SolveReport, SolveStopReason};
pub use routing::{CategoricalPrefix, RoutingPreset};
#[doc(hidden)]
pub use routing_break::routing_builder_break_ready;
pub use singleton::{prune_singletons_with_multiplicity, SingletonMultiplicity, SingletonResult};
pub use topology_adapter::borrow_categorical_topology_design;
pub use topology_finalize::{
    finalize_categorical_topology, CompactedCategoricalSource, FinalizedCategoricalTopology,
};
pub(crate) use topology_finalize::{ValidatedFinalizedTopology, ValidatedSingleTopology};
pub use topology_map::absorb_finalized_categorical_topology_map_certified;
pub use topology_registry::CategoricalTopologyRegistry;
pub use topology_slope_map::{
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
