//! Fixed-effect preprocessing, projection, and certification.

mod certificate;
mod components;
mod design;
mod map;
mod projection;
mod singleton;

pub use certificate::{certify_residuals, FeCertificate};
pub use components::{connected_components, ComponentInfo};
pub use design::{EffectTerm, FeDesign, LevelIndex};
pub use map::{absorb_map, MapConfig, SolveReport, SolveStopReason};
pub use projection::{project_effect_in_place, ProjectionWorkspace};
pub use singleton::{prune_singletons, SingletonResult};
