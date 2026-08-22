//! Fixed-effect preprocessing, projection, and certification.

mod auto;
mod borrowed_map;
mod certificate;
mod components;
mod design;
mod interaction;
mod lsmr;
mod map;
mod partition;
mod partition_registry;
mod partition_singleton;
mod partition_validation;
mod projection;
mod report;
mod singleton;
mod topology_adapter;
mod topology_finalize;
mod topology_map;
mod topology_registry;
mod topology_source;

pub use auto::{absorb_fixed_effects, AbsorbConfig};
pub(crate) use borrowed_map::absorb_borrowed_intercept_map_with_structural_constant_certified;
pub use borrowed_map::{
    absorb_borrowed_intercept_map_certified, BorrowedMapWeights, CertifiedBorrowedMap,
};
pub use certificate::{certify_residuals, FeCertificate};
pub(crate) use certificate::{certify_residuals_with_reference_norms, weighted_column_norms};
pub use components::{connected_components, ComponentInfo};
pub(crate) use design::ValidatedBorrowedInterceptSource;
pub use design::{
    BorrowedInterceptEffect, BorrowedInterceptFeDesign, EffectTerm, FeDesign, LevelIndex,
};
pub use interaction::{factorize_code_interaction, CategoricalCodeView, EncodedInteraction};
pub use lsmr::{absorb_lsmr, LsmrConfig, LsmrPreconditioner};
pub use map::{absorb_map, MapConfig};
pub use partition::CategoricalPartition;
pub use partition_registry::CategoricalPartitionRegistry;
pub use partition_singleton::{
    prune_categorical_partition_singletons, CategoricalPartitionSingletonResult,
};
pub use partition_validation::validate_categorical_partition;
pub use projection::{project_effect_in_place, ProjectionWorkspace};
pub use report::{FeSolverKind, SolveReport, SolveStopReason};
pub use singleton::{
    prune_singletons, prune_singletons_with_multiplicity, SingletonMultiplicity, SingletonResult,
};
pub use topology_adapter::borrow_categorical_topology_design;
pub use topology_finalize::{
    finalize_categorical_topology, CompactedCategoricalSource, FinalizedCategoricalTopology,
};
pub use topology_map::absorb_finalized_categorical_topology_map_certified;
pub use topology_registry::CategoricalTopologyRegistry;
pub use topology_source::CategoricalTopologySource;
