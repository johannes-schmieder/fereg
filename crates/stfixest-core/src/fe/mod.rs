//! Fixed-effect preprocessing, projection, and certification.

mod auto;
mod certificate;
mod components;
mod design;
mod lsmr;
mod map;
mod projection;
mod report;
mod singleton;

pub use auto::{absorb_fixed_effects, AbsorbConfig};
pub use certificate::{certify_residuals, FeCertificate};
pub(crate) use certificate::{certify_residuals_with_reference_norms, weighted_column_norms};
pub use components::{connected_components, ComponentInfo};
pub use design::{
    BorrowedInterceptEffect, BorrowedInterceptFeDesign, EffectTerm, FeDesign, LevelIndex,
};
pub use lsmr::{absorb_lsmr, LsmrConfig, LsmrPreconditioner};
pub use map::{absorb_map, MapConfig};
pub use projection::{project_effect_in_place, ProjectionWorkspace};
pub use report::{FeSolverKind, SolveReport, SolveStopReason};
pub use singleton::{
    prune_singletons, prune_singletons_with_multiplicity, SingletonMultiplicity, SingletonResult,
};
