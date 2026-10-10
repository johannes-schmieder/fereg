//! PPML-owned fit-lifetime memory planning, reusable IRLS buffers, and
//! observational performance telemetry.
//!
//! This module deliberately does not participate in OLS planning.  PPML has
//! changing Hessian weights, a separation stage, and final likelihood
//! products that need a different peak model and independently qualified
//! automatic policy.  The selected `Fast`/`Low` execution mode can be passed
//! to the shared weighted-linear engine only after this preflight succeeds.

use std::mem::size_of;
use std::time::{Duration, Instant};

#[cfg(test)]
use super::family::{fill_poisson_working_state, poisson_objective_from_eta};
use super::family::{means_from_eta, objective_partial_cells, PreparedPoissonInputs};
use super::{PoissonBounds, PoissonObjective, PpmlCoefficientStatus, PpmlConfig};
use crate::data::DropReason;
use crate::error::{allocation, invalid_argument, ErrorCode, FeregError, Result};
use crate::fe::{CategoricalCodeView, FeSolverKind, LevelIndex};
#[cfg(test)]
use crate::fe::{FUSED_PARALLEL_MAX_RHS, PARALLEL_DOT_CHUNK_ROWS};
use crate::memory::matrix_bytes;
use crate::metadata::CoefficientSlot;
use crate::ols::{
    FrozenLinearBasisSlot, PreparedOlsBreadCoordinate, PreparedOlsCoefficient, PreparedOlsColumn,
    PreparedOlsFitStatus, PreparedOlsFittedCoefficient,
};
use crate::options::MemoryMode;
use crate::ppml_separation::{ExactFeLevelIdentity, SeparationDropReason, SeparationMethodReport};

const MINIMUM_HEADROOM_BYTES: u128 = 512 * 1024 * 1024;
const ITERATION_VECTOR_COUNT: u128 = 8;
const COEFFICIENT_VECTOR_COUNT: u128 = 3;
/// Simultaneously retained finite-session square workspaces. This includes
/// the moment/certificate bank plus the frozen Gram, Cholesky factor,
/// lower-triangular inverse, and final symmetric bread. The inverse-lower
/// buffer is distinct: the exact canonical inverse path cannot overwrite the
/// factor before it has formed and certified the final bread.
const LINEAR_SQUARE_WORKSPACE_COUNT: u128 = 7;
/// The frozen finite solve takes one observation-length residual buffer while
/// the accepted-step residual is still retained.  Eight fallible 32-byte
/// solver records are allocated at the same boundary before either residual
/// can be released.
const FINITE_SOLVE_FIXED_RECORD_COUNT: u128 = 8;
const FINITE_SOLVE_FIXED_RECORD_BYTES: u128 = 32;
/// Stable-objective bookkeeping retained while the finite-solve residual is
/// still live.
const OBJECTIVE_LINE_SEARCH_RECORD_BYTES: u128 = 64;
/// Canonical strict recertification overlaps four observation-length response,
/// residual, and comparison vectors with the retained reusable session.
const STRICT_FINAL_OVERLAP_VECTOR_COUNT: u128 = 4;
/// Outcome, prior weights, and the one mean copy retained only through
/// composite objective replay. Requested observation products are accounted
/// separately by `saved_output_bytes`.
const PUBLICATION_RESULT_NUMERIC_VECTOR_COUNT: u128 = 3;
// Sample and requested outputs share one immutable final-row ledger.
const PUBLICATION_RESULT_ROW_MAP_COUNT: u128 = 1;
const PREPARED_DROP_REASON_COPY_COUNT: u128 = 2;
const _: () = assert!(size_of::<DropReason>() == 1);
const _: () = assert!(size_of::<Option<SeparationDropReason>>() == 1);
const _: () = assert!(size_of::<bool>() == 1);
const _: () = assert!(size_of::<PpmlCoefficientStatus>() == 1);
const _: () = assert!(size_of::<CoefficientSlot>() == 4);
const _: () = assert!(size_of::<PreparedOlsCoefficient>() == 24);
const _: () = assert!(size_of::<PreparedOlsColumn>() == 48);
const _: () = assert!(size_of::<PreparedOlsBreadCoordinate>() == 16);
const _: () = assert!(size_of::<FrozenLinearBasisSlot>() == 24);
const _: () = assert!(size_of::<PreparedOlsFitStatus>() == 1);
const _: () = assert!(size_of::<PreparedOlsFittedCoefficient>() == 40);
const _: () = assert!(size_of::<Option<f64>>() == 16);
const _: () = assert!(size_of::<ExactFeLevelIdentity>() == 16);
// `CertifiedPpmlResult::new` independently validates the identified
// covariance after sandwich temporaries have been released.  The optimized
// PSD validator retains at most the extracted matrix, one current/output
// matrix, and the Jacobi work/eigenvector pair together.  The eigenvalue
// vector is live at that same boundary.
const PUBLICATION_PSD_SQUARE_WORKSPACE_COUNT: u128 = 4;
const DEFAULT_DIAGNOSTIC_CAPACITY: usize = 128;
/// Registered heap allocation for the boxed, inline-certified Mu exploratory
/// state. Its moved mean vector is modeled separately at exact N-sized cost.
const MU_EXPLORATION_RECORD_BYTES: u128 = 128;
pub const PPML_STEADY_STATE_AUDIT_ITERATIONS: usize = 16;
/// Registered maximum modeled size for one bounded diagnostic record.  New
/// report types must prove they fit this slot in a focused test.
pub(super) const DIAGNOSTIC_RECORD_BYTES: u128 = 288;

/// User request and bounded-workspace policy for one PPML fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlMemoryConfig {
    pub requested: MemoryMode,
    /// Require the dense fast execution route while preserving the user's
    /// `memory(auto)` request identity. Public PPML LSMR and CMG currently
    /// use this because their retained ordinary-frame engines have no
    /// certified low-memory implementation.
    pub require_fast_route: bool,
    /// Process/application memory visible to the caller before the fit.  A
    /// missing value retains the requested mode; `Auto` then prefers fast.
    pub available_bytes: Option<u128>,
    /// Allocator-observed bytes already live between the embedding action's
    /// baseline and the core entry boundary. The plugin supplies this after
    /// request parsing and input loading so every phase peak includes the
    /// still-live action, raw-input, and transaction buffers. Core-only
    /// callers without an embedding allocator leave it at zero.
    pub embedding_resident_bytes: u128,
    /// Maximum generated-column pool retained by the low-memory route.
    pub poolsize: usize,
    /// Fixed capacity of the iteration diagnostic ring.  This is independent
    /// of N, K, FE levels, cluster groups, and the number of IRLS iterations.
    pub diagnostic_capacity: usize,
}

impl PpmlMemoryConfig {
    /// Shared admission limit for embedding construction and core execution.
    #[must_use]
    pub fn usable_bytes(self) -> Option<u128> {
        crate::memory::usable_memory_bytes(self.available_bytes)
    }
}

impl Default for PpmlMemoryConfig {
    fn default() -> Self {
        Self {
            requested: MemoryMode::Auto,
            require_fast_route: false,
            available_bytes: None,
            embedding_resident_bytes: 0,
            poolsize: 10,
            diagnostic_capacity: DEFAULT_DIAGNOSTIC_CAPACITY,
        }
    }
}

/// Complete dimensions required to preflight a PPML fit before allocating
/// its large workspaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlMemoryDimensions {
    pub metadata: super::PpmlMetadataMemory,
    pub observations: usize,
    /// Full observation axis on which numeric/domain and separation ledgers
    /// remain live. This can exceed `observations` after if/in or missing-data
    /// exclusions and is invariant across separation generations.
    pub original_observations: usize,
    /// Rows admitted by if/in before numeric-domain and missing-value
    /// exclusions. Canonical compaction reserves observation vectors and the
    /// first retained-row map at this capacity, even when their final logical
    /// length is only `observations`.
    pub candidate_observations: usize,
    /// Retained numeric source columns, prior to generated finite terms.
    pub loaded_numeric_columns: usize,
    /// Candidate-capacity canonical offset retained by the raw prepared-model
    /// owner across all separation generations.  Exposure is logged exactly
    /// once into this same storage.  Zero means no offset/exposure was
    /// supplied; otherwise this equals `candidate_observations` even when
    /// numeric/domain preparation shrinks the generation-zero active sample.
    pub retained_base_adjustment_capacity_elements: usize,
    /// Elements in the generation-zero canonical-sample raw pweight vector
    /// retained by the PPML owner. Rows rejected for missing or invalid
    /// numeric data have no reusable raw weight and are not stored. Every
    /// separation generation is normalized from these supplied values rather
    /// than from an earlier rounded normalization. This stays at the initial
    /// active length after separation shrinks N, and is zero for unweighted
    /// and frequency-weighted fits.
    pub retained_raw_preseparation_probability_weight_elements: usize,
    /// Canonical generated finite columns, including the structural constant.
    pub finite_columns: usize,
    /// Compact coefficient-slot axis in the final prepared generation.  It
    /// includes that generation's omitted/base slots but excludes candidate
    /// factor slots that vanished during final-sample rebasing.
    pub coefficient_slots: usize,
    /// Original candidate/publication coefficient axis.  This can exceed the
    /// compact solve axis but never creates an N-by-reported-slot design.
    pub reported_coefficient_slots: usize,
    pub fe_dimensions: usize,
    pub fe_total_levels: usize,
    pub fe_max_levels: usize,
    /// Sum of intercept-plus-slope channels across every FE effect.
    pub fe_total_channels: usize,
    /// Largest intercept-plus-slope channel count in one FE effect.
    pub fe_max_channels: usize,
    /// Largest number of observations in one FE level.  Reusable MAP uses
    /// this, rather than the number of levels, for its certificate basis.
    pub fe_max_level_members: usize,
    /// Sum of level-by-channel FE coefficient directions.  Slope-bearing
    /// effects can make this larger than `fe_total_levels`.
    pub fe_coefficient_directions: usize,
    pub fe_max_effect_directions: usize,
    pub cluster_dimensions: usize,
    /// Sum of final groups across every nonempty cluster subset used by CGM.
    pub cluster_subset_groups: usize,
    /// Maximum observation-vector count required by any enabled detector.
    pub separation_workspace_vectors: usize,
    /// Whether the exact singleton fixed-point policy is requested. It has a
    /// row queue, byte masks, and exact per-level u128 multiplicities that
    /// cannot be inferred from the numerical detector's f64-vector count.
    pub separation_singleton_enabled: bool,
    /// Frozen count of requested singleton/FE/simplex/IR/Mu policies (0..=5).
    /// The exact retained execution-record ceiling is this count times the
    /// admitted generation capacity.
    pub separation_method_count: usize,
    /// Initial plus committed generation capacity. All-positive outcomes need
    /// at most two generations (initial singleton fixed point and replay);
    /// samples with any zero retain the observation-count envelope.
    pub separation_generation_capacity: usize,
    /// Whether any requested singleton or separation method can accept rows
    /// and therefore construct an atomic old-plus-new prepared generation.
    /// This policy is frozen at generation-zero admission.
    pub separation_rebase_enabled: bool,
    /// Whether exact zero-only absorbed-intercept level certificates can be
    /// produced and retained across separation generations.
    pub separation_exact_fe_enabled: bool,
    /// Whether fitted-mean Mu exploration is enabled for this fit.
    pub separation_mu_enabled: bool,
    /// Registered hard ceiling for the simultaneous explicit-design plus
    /// aggregate simplex-kernel workspace; zero disables simplex storage.
    pub simplex_tableau_cells: usize,
    /// Observation-sized result columns retained for publication.  This
    /// includes requested predictions and saved FE channels, but not private
    /// shadows represented by `publication_matrix_cells`.
    pub requested_output_columns: usize,
    /// Requested mu reuses the mean vector that publication already retains
    /// transiently for independent objective replay. Other requested outputs
    /// require distinct observation vectors.
    pub requested_mean_output: bool,
    /// Number of declared FE channels explicitly requested for saving.  When
    /// this is nonzero, recovery must materialize the total absorbed effect
    /// and every declared channel to certify the full decomposition, even if
    /// only one channel will survive publication.
    pub requested_saved_fe_channels: usize,
    /// Total cells across coefficient, VCE, status, fit, DoF, certificate,
    /// runtime, receipt, and other private publication matrices.
    pub publication_matrix_cells: usize,
    pub threads: usize,
    /// Whether the qualification-only warmed allocator audit is requested.
    /// Its independent canonical-equivalence solve has one explicitly
    /// modeled response-refresh lifetime that normal production fits omit.
    pub allocation_audit_enabled: bool,
}

impl PpmlMemoryDimensions {
    fn validate(self, config: PpmlMemoryConfig) -> Result<()> {
        if self.observations == 0 {
            return Err(invalid_argument(
                "PPML memory preflight requires at least one observation",
            ));
        }
        if self.candidate_observations < self.observations
            || self.original_observations < self.candidate_observations
        {
            return Err(invalid_argument(
                "PPML observation counts must satisfy prepared <= candidate <= original",
            ));
        }
        if self.retained_base_adjustment_capacity_elements != 0
            && self.retained_base_adjustment_capacity_elements != self.candidate_observations
        {
            return Err(invalid_argument(
                "PPML retained base adjustment must be absent or reserve the immutable candidate capacity",
            ));
        }
        if self.separation_generation_capacity == 0
            || self.separation_generation_capacity > self.observations
        {
            return Err(invalid_argument(
                "PPML separation generation capacity must lie in 1..=N",
            ));
        }
        if self.separation_method_count > 5 {
            return Err(invalid_argument(
                "PPML separation method count exceeds the five registered policies",
            ));
        }
        if self.finite_columns > 0 && config.poolsize == 0 {
            return Err(invalid_argument(
                "PPML low-memory poolsize must be positive when finite columns exist",
            ));
        }
        if self.coefficient_slots < self.finite_columns {
            return Err(invalid_argument(
                "PPML coefficient-slot count is smaller than the generated finite design",
            ));
        }
        if self.reported_coefficient_slots < self.coefficient_slots {
            return Err(invalid_argument(
                "PPML reported coefficient-slot count is smaller than the final prepared axis",
            ));
        }
        if self.threads == 0 {
            return Err(invalid_argument(
                "PPML memory preflight requires at least one worker thread",
            ));
        }
        if self.fe_dimensions == 0
            && (self.fe_total_levels != 0
                || self.fe_max_levels != 0
                || self.fe_total_channels != 0
                || self.fe_max_channels != 0
                || self.fe_max_level_members != 0
                || self.fe_coefficient_directions != 0
                || self.fe_max_effect_directions != 0)
        {
            return Err(invalid_argument(
                "PPML FE level counts require at least one FE dimension",
            ));
        }
        if self.fe_max_levels > self.fe_total_levels {
            return Err(invalid_argument(
                "PPML maximum FE level count exceeds the total level count",
            ));
        }
        if self.fe_dimensions > 0
            && (self.fe_total_channels < self.fe_dimensions
                || self.fe_max_channels == 0
                || self.fe_max_channels > self.fe_total_channels
                || self.fe_max_level_members == 0)
        {
            return Err(invalid_argument(
                "PPML FE dimensions require positive channel and largest-level sizes",
            ));
        }
        if self.fe_max_level_members > self.observations {
            return Err(invalid_argument(
                "PPML largest FE level exceeds the observation count",
            ));
        }
        if self.fe_dimensions > 0 && self.fe_coefficient_directions < self.fe_total_levels {
            return Err(invalid_argument(
                "PPML FE coefficient-direction count is smaller than its level count",
            ));
        }
        if self.requested_saved_fe_channels > self.fe_total_channels {
            return Err(invalid_argument(
                "PPML requested saved FE channels exceed declared FE channels",
            ));
        }
        if self
            .requested_saved_fe_channels
            .checked_add(usize::from(self.requested_mean_output))
            .is_none_or(|minimum| minimum > self.requested_output_columns)
        {
            return Err(invalid_argument(
                "PPML requested output count is smaller than its saved-FE/mu products",
            ));
        }
        if self.fe_max_effect_directions > self.fe_coefficient_directions {
            return Err(invalid_argument(
                "PPML maximum FE effect directions exceed total FE directions",
            ));
        }
        if self.cluster_dimensions > 5 {
            return Err(invalid_argument(
                "PPML memory preflight supports at most five cluster dimensions",
            ));
        }
        if self.cluster_dimensions == 0 && self.cluster_subset_groups != 0 {
            return Err(invalid_argument(
                "PPML cluster subset groups require a cluster dimension",
            ));
        }
        Ok(())
    }
}

fn validate_final_dimensions_within_preflight(
    preflight: PpmlMemoryDimensions,
    final_dimensions: PpmlMemoryDimensions,
) -> Result<()> {
    let bounded = final_dimensions.observations <= preflight.observations
        && final_dimensions.separation_generation_capacity
            <= preflight.separation_generation_capacity
        && final_dimensions.loaded_numeric_columns <= preflight.loaded_numeric_columns
        && final_dimensions.finite_columns <= preflight.finite_columns
        && final_dimensions.coefficient_slots <= preflight.coefficient_slots
        && final_dimensions.reported_coefficient_slots <= preflight.reported_coefficient_slots
        && final_dimensions.fe_dimensions <= preflight.fe_dimensions
        && final_dimensions.fe_total_levels <= preflight.fe_total_levels
        && final_dimensions.fe_max_levels <= preflight.fe_max_levels
        && final_dimensions.fe_total_channels <= preflight.fe_total_channels
        && final_dimensions.fe_max_channels <= preflight.fe_max_channels
        && final_dimensions.fe_max_level_members <= preflight.fe_max_level_members
        && final_dimensions.fe_coefficient_directions <= preflight.fe_coefficient_directions
        && final_dimensions.fe_max_effect_directions <= preflight.fe_max_effect_directions
        && final_dimensions.cluster_subset_groups <= preflight.cluster_subset_groups
        && final_dimensions.publication_matrix_cells <= preflight.publication_matrix_cells;
    let stable = final_dimensions.metadata == preflight.metadata
        && final_dimensions.original_observations == preflight.original_observations
        && final_dimensions.candidate_observations == preflight.candidate_observations
        && final_dimensions.retained_base_adjustment_capacity_elements
            == preflight.retained_base_adjustment_capacity_elements
        && final_dimensions.retained_raw_preseparation_probability_weight_elements
            == preflight.retained_raw_preseparation_probability_weight_elements
        && final_dimensions.cluster_dimensions == preflight.cluster_dimensions
        && final_dimensions.separation_workspace_vectors == preflight.separation_workspace_vectors
        && final_dimensions.separation_singleton_enabled == preflight.separation_singleton_enabled
        && final_dimensions.separation_method_count == preflight.separation_method_count
        && final_dimensions.separation_rebase_enabled == preflight.separation_rebase_enabled
        && final_dimensions.separation_exact_fe_enabled == preflight.separation_exact_fe_enabled
        && final_dimensions.separation_mu_enabled == preflight.separation_mu_enabled
        && final_dimensions.simplex_tableau_cells == preflight.simplex_tableau_cells
        && final_dimensions.requested_output_columns == preflight.requested_output_columns
        && final_dimensions.requested_mean_output == preflight.requested_mean_output
        && final_dimensions.requested_saved_fe_channels == preflight.requested_saved_fe_channels
        && final_dimensions.threads == preflight.threads
        && final_dimensions.allocation_audit_enabled == preflight.allocation_audit_enabled;
    if !bounded || !stable {
        return Err(invalid_argument(
            "PPML final prepared dimensions exceed or disagree with the generation-zero preflight envelope",
        ));
    }
    Ok(())
}

/// PPML execution mode after `Auto` has been resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlMemoryRoute {
    Fast,
    Low,
}

/// Exact decision emitted at the fit-lifetime memory-planner boundary.
///
/// The decision event is sent before the selected route constructs any large
/// workspace and, on rejection, before the planner formats its allocation
/// error.  Embeddings may use this narrow observer to capture qualification
/// evidence; ordinary fits leave it disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlMemoryPreflightObservation {
    pub requested: MemoryMode,
    pub selected: Option<PpmlMemoryRoute>,
    pub fast_peak_bytes: u128,
    pub low_peak_bytes: u128,
    pub available_bytes: Option<u128>,
    pub embedding_resident_bytes: u128,
    pub safety_headroom_bytes: u128,
    pub usable_bytes: Option<u128>,
    pub workspace_allocation_started: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlMemoryPreflightEvent {
    Begin,
    Decision(PpmlMemoryPreflightObservation),
}

pub type PpmlMemoryPreflightProbe = fn(PpmlMemoryPreflightEvent) -> Result<()>;

/// Itemized peak model for one concrete PPML route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlMemoryInventory {
    /// Nested candidate model retained while rebuilding compact recipes.
    pub preparation_metadata_transient_bytes: u128,
    /// Boxed IRLS parent and copied coefficient/FE/cluster publication metadata.
    pub result_metadata_bytes: u128,
    /// Still-live embedding/action storage measured relative to the action
    /// allocator baseline immediately before entering the core fit.
    pub embedding_resident_bytes: u128,
    /// Transferred raw-owner capacity not already included in the embedding.
    pub input_owner_supplement_bytes: u128,
    pub canonical_numeric_bytes: u128,
    /// Owner-side candidate-capacity offset/logged-exposure storage.  The
    /// generation-local compact offset remains separately included in
    /// `canonical_numeric_bytes`.
    pub retained_base_adjustment_bytes: u128,
    /// Generation-zero canonical-row raw pweights retained by the prepared
    /// owner across all separation generations. Missing/domain-excluded
    /// if/in candidates are absent. This storage is distinct from both the
    /// loaded weight column and each generation's canonical prior weights.
    pub retained_raw_preseparation_probability_weight_bytes: u128,
    pub topology_bytes: u128,
    /// Fixed-topology copy owned by the reusable FE solver, including its
    /// construction envelope (one count/cursor pair plus all retained effects).
    /// Charged on every route, including MAP and Mu before specialized admission.
    /// Distinct from canonical topology and never rebuilt per iteration.
    pub fe_solver_topology_bytes: u128,
    pub cluster_topology_bytes: u128,
    /// Candidate and current prepared-generation compact row maps.
    pub steady_row_map_bytes: u128,
    /// Immutable numeric and current prepared-generation original-axis
    /// `DropReason` payloads.
    pub prepared_drop_reason_bytes: u128,
    /// Original-axis `Vec<bool>` owned by separation. Rust `Vec<bool>` stores
    /// one `bool` element per byte; it is not the packed C++ specialization.
    pub separation_activity_bytes: u128,
    /// Original-axis `Option<SeparationDropReason>` payload.
    pub separation_reason_bytes: u128,
    pub row_ledger_bytes: u128,
    /// Control blocks for immutable candidate rows and initial drop reasons,
    /// plus the distinct current retained-row axis whenever rebasing is
    /// enabled: strong/weak counts and one moved Vec header per allocation.
    /// Their Vec payloads are charged in `row_ledger_bytes` without a slice
    /// copy; the no-rebase path shares candidate and current row authority.
    pub row_axis_arc_header_bytes: u128,
    /// Retained coefficient/column/report-plan metadata prepared before the
    /// numerical fit. This is K/slot-sized, never N-by-K.
    pub prepared_model_metadata_bytes: u128,
    /// Complete retained prepared generation, excluding shared raw-owner Arcs.
    /// This independently bounds capacities, including the original slot axis.
    pub prepared_generation_bytes: u128,
    /// Increment above the legacy retained-generation fields. It accompanies
    /// both the current generation and a simultaneous replacement generation.
    pub prepared_generation_supplement_bytes: u128,
    /// Complete replacement-generation payload held beside the old prepared
    /// generation during an accepted atomic rebase: canonical numerics,
    /// categorical/cluster topology, model metadata, its original-axis drop
    /// ledger, and its compact prepared row map.
    pub separation_replacement_generation_bytes: u128,
    /// Incremental duplicate of the current primary proposal during commit.
    /// Certified prepared execution moves the exact-capacity primary directly
    /// into the commit, so this is zero; the explicit field makes that
    /// no-copy invariant visible in telemetry.
    pub separation_rebase_proposal_bytes: u128,
    /// Numerical recession direction retained into the accepted replacement
    /// build. Independent certification completes first and releases it before
    /// cascade/rebuild, so certified execution reports zero here.
    pub separation_rebase_direction_bytes: u128,
    /// Transactional payload beyond the replacement generation: cloned
    /// original-axis activity/reason ledgers, the staged retained-row map, the
    /// owner's temporary candidate mask, probability-weight source positions,
    /// and (for a numerical method) the validated recession direction. This
    /// is the maximum of the build and post-build lifetimes; ordinary steady
    /// owners remain in `row_ledger_bytes`.
    pub separation_rebase_staging_bytes: u128,
    /// Total accepted-rebase overlap beyond the ordinary steady `common`
    /// payload. This equals replacement generation plus its simultaneously
    /// live ledger/staging/direction bound. Detector-only workspaces are a
    /// separate, mutually exclusive phase and are not summed into this value.
    pub separation_rebase_bytes: u128,
    /// Arc<Vec<_>> control block of the newly allocated retained-row ledger
    /// used by an accepted atomic rebase. Rebuild clones share it.
    pub separation_rebase_arc_header_bytes: u128,
    /// Final merged original-axis `DropReason` payload, materialized after
    /// IRLS and retained throughout inference, FE recovery, and publication.
    pub final_drop_reason_bytes: u128,
    /// Retained and dropped candidate-index vectors in `SeparationReport`.
    /// Both reserve the full candidate count even when one is empty.
    pub separation_report_row_bytes: u128,
    /// Candidate-axis `Option<SeparationDropReason>` report payload.
    pub separation_report_reason_bytes: u128,
    /// Stable original-row identities retained across accepted method reports.
    /// Each deleted row is published exactly once across primary, singleton,
    /// and exact-FE follow-up vectors, so exact-capacity report vectors are
    /// jointly bounded by the generation-zero prepared sample.
    pub separation_method_report_row_bytes: u128,
    /// Exact-capacity accepted term/original-row level witnesses across every
    /// persisted exact-FE report. Once a zero-only level is accepted all of
    /// its active rows leave the ledger, so the identities are disjoint across
    /// generations and jointly bounded by the initial total FE levels.
    pub separation_method_report_level_bytes: u128,
    /// Pre-reserved `SeparationMethodReport` record bank. Payloads owned by
    /// those records (accepted rows/levels) are charged separately above.
    pub separation_method_report_record_bytes: u128,
    /// Initial plus every committed generation hash. Since a valid commit
    /// leaves at least one row, at most generation-zero N hashes persist;
    /// all-positive outcomes use the tighter admitted generation capacity.
    pub separation_generation_hash_bytes: u128,
    /// Incremental detector/cascade level capacity beyond the retained report
    /// envelope. An outer cascade accumulator, an inner full-level scan, and
    /// the inner exact-capacity copy peak at three total-level banks including
    /// the one bank already charged in `common`.
    pub separation_exact_level_transient_bytes: u128,
    /// Initial/replay singleton fixed-point workspace beyond retained report
    /// payloads: working/selection bool masks plus dropped/pass row banks.
    pub separation_singleton_workspace_bytes: u128,
    /// Fixed-point cascade workspace beyond retained report payloads after the
    /// row-reason compaction: outer active/reason bytes plus the larger of one
    /// singleton or exact-FE inner scan.
    pub separation_cascade_workspace_bytes: u128,
    pub iteration_buffer_bytes: u128,
    pub coefficient_buffer_bytes: u128,
    pub retained_within_resident_bytes: u128,
    /// Resident byte buffer owned by the low route's row-major scratch file.
    /// It is byte-addressed storage, not an f64 element count, and is large
    /// enough for either the bounded I/O tile or one complete retained row.
    pub retained_within_io_buffer_bytes: u128,
    pub retained_within_scratch_bytes: u128,
    pub linear_vector_bytes: u128,
    pub fe_workspace_bytes: u128,
    pub linear_workspace_bytes: u128,
    /// Retained rank/bread/frozen-basis metadata, accepted fitted coefficient
    /// records, and the bounded reported-coefficient workspace owned by the
    /// shared linear session.
    pub linear_result_metadata_bytes: u128,
    /// Maximum incremental detector/replay workspace before an accepted
    /// transaction. Explicit simplex storage and shared-linear numerical
    /// validation plus the registered f64 detector-vector bank are alternative
    /// method peaks. The proposal/certificate allowance is added once to
    /// their maximum; accepted-rebase ownership is modeled separately by
    /// `separation_rebase_bytes`.
    pub separation_workspace_bytes: u128,
    /// Joint ceiling for the live explicit N-by-P design plus nullspace,
    /// cone, tableau, FE-code, and solver scratch. The detector checks their
    /// sum before materializing the design.
    pub simplex_combined_bytes: u128,
    /// Mean moved out of exploratory IRLS plus its bounded boxed certificate.
    /// Eta and all other IRLS vectors are dropped before Mu projection.
    pub mu_exploration_retained_bytes: u128,
    pub inference_workspace_bytes: u128,
    /// Published-axis covariance, coefficient statistics and cluster/Wald
    /// identities retained after the sandwich temporaries are released.
    /// These remain live through saved-effect recovery and publication.
    pub inference_result_bytes: u128,
    /// Incremental complete-channel recovery envelope. The final fit and
    /// inference result are parents charged separately in the phase peak.
    pub fe_recovery_workspace_bytes: u128,
    /// Distinct requested observation vectors live at publication. Requested
    /// mu is excluded because it reuses the mandatory composite-replay mean;
    /// saved channels, xb, residual, absorbed, and eta are included exactly.
    pub saved_output_bytes: u128,
    /// Plugin matrix payloads and one borrowed-slice descriptor per declared
    /// FE channel, including channels not requested for saved publication.
    pub publication_bytes: u128,
    /// Immutable final-sample/output ownership created while the accepted fit
    /// is still live: outcome, weights, the composite-replay mean, and two
    /// compact row maps. Other retained vectors are request-sized above.
    pub publication_result_bytes: u128,
    /// Private scratch used when `CertifiedPpmlResult` independently
    /// revalidates PSD and Wald certificates.  This is not part of the
    /// plugin-owned publication matrices and is live only in Publication.
    pub publication_validator_workspace_bytes: u128,
    pub diagnostic_bytes: u128,
    pub preparation_peak_bytes: u128,
    /// Largest pre-fit detector phase (FE, simplex, or iterated rectifier).
    pub separation_peak_bytes: u128,
    /// Fitted-start Mu retains the complete generation fit while its detector
    /// workspace is live.
    pub separation_mu_peak_bytes: u128,
    pub iteration_peak_bytes: u128,
    /// One retained N-length canonical response/result that overlaps the
    /// post-equivalence working-state refresh only when allocation auditing
    /// is explicitly enabled.
    pub audit_equivalence_refresh_transient_bytes: u128,
    pub weight_response_refresh_peak_bytes: u128,
    /// One extra N-length residual plus the fixed finite-solve and objective
    /// records retained from the frozen solve through candidate line search.
    pub post_solve_transient_bytes: u128,
    pub post_solve_peak_bytes: u128,
    /// Candidate line-search peak after the qualification equivalence solve;
    /// normal fits equal `post_solve_peak_bytes` exactly.
    pub objective_line_search_peak_bytes: u128,
    /// Total strict-only transient: four N-length residual/predictor vectors
    /// plus `strict_final_metadata_transient_bytes`.
    pub strict_final_transient_bytes: u128,
    /// Candidate fit/basis records and the second frozen coefficient vector
    /// staged beside the accepted fit, basis, and rollback identities.
    /// Candidate bread is already one of the modeled square banks; this field
    /// is the exact additional K/slot-sized phase maximum.
    pub strict_final_metadata_transient_bytes: u128,
    pub strict_final_peak_bytes: u128,
    pub inference_peak_bytes: u128,
    /// Peak while certifying saved FE outputs.  This is distinct from
    /// sandwich inference because partial save masks still recover every
    /// declared FE channel plus their total.
    pub fe_recovery_peak_bytes: u128,
    pub publication_peak_bytes: u128,
    /// Maximum resident incremental bytes across every fit stage.
    pub peak_bytes: u128,
}

/// Both PPML routes are always modeled before one is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlMemoryEstimate {
    pub fast: PpmlMemoryInventory,
    pub low: PpmlMemoryInventory,
    pub recommended_poolsize: usize,
}

/// Immutable result of PPML memory preflight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlMemoryPlan {
    pub requested: MemoryMode,
    pub selected: PpmlMemoryRoute,
    /// Exact dimensions of the final prepared generation consumed by IRLS
    /// and publication.
    pub dimensions: PpmlMemoryDimensions,
    /// Generation-zero capacity envelope used for the one fit-lifetime
    /// preflight. Separation may only reduce these dimensions; its detector
    /// workspace and every later fit phase remain charged to this envelope.
    pub preflight_dimensions: PpmlMemoryDimensions,
    pub estimate: PpmlMemoryEstimate,
    pub available_bytes: Option<u128>,
    pub embedding_resident_bytes: u128,
    pub safety_headroom_bytes: u128,
    pub usable_bytes: Option<u128>,
    /// Retained workspace for the selected PPML-specific ordinary-frame
    /// engine. MAP is already included in the base route inventory.
    pub solver_workspace_bytes: u128,
    /// Separately admitted graph/CG detector state, released before IRLS.
    pub(crate) separation_projection_bytes: u128,
    /// Optional generic MAP projection bank, retained from preparation through
    /// the final Hessian solve and released with the linear session.
    pub reusable_projection_bytes: u128,
    /// Optional current-call certificate geometry; capacity shares the linear session lifetime.
    pub reusable_certificate_bytes: u128,
    /// Optional coefficient-space slope workspace, sharing the linear-session lifetime.
    pub reusable_slope_bytes: u128,
}

impl PpmlMemoryPlan {
    /// A warmed MAP audit measures ordinary directions, separately from an
    /// admitted low three-FE canonical LSMR checkpoint outside that window.
    #[must_use]
    pub fn audit_route_matches_final(
        self,
        audited: FeSolverKind,
        final_route: FeSolverKind,
    ) -> bool {
        audited == final_route
            || (audited == FeSolverKind::Map
                && final_route == FeSolverKind::Lsmr
                && self.selected == PpmlMemoryRoute::Low
                && self.dimensions.fe_dimensions == 3
                && self.dimensions.fe_total_channels == 3
                && self.solver_workspace_bytes > 0)
    }

    /// Bind the already-selected fit-lifetime plan to a smaller final
    /// generation without re-running route selection after separation.
    pub(crate) fn with_final_dimensions(
        mut self,
        dimensions: PpmlMemoryDimensions,
    ) -> Result<Self> {
        let diagnostic_capacity = self.estimate_diagnostic_capacity();
        dimensions.validate(PpmlMemoryConfig {
            requested: self.requested,
            require_fast_route: self.selected == PpmlMemoryRoute::Fast,
            available_bytes: self.available_bytes,
            embedding_resident_bytes: self.embedding_resident_bytes,
            poolsize: self.estimate.recommended_poolsize.max(1),
            diagnostic_capacity,
        })?;
        validate_final_dimensions_within_preflight(self.preflight_dimensions, dimensions)?;
        self.dimensions = dimensions;
        Ok(self)
    }

    #[must_use]
    pub const fn selected_inventory(self) -> PpmlMemoryInventory {
        match self.selected {
            PpmlMemoryRoute::Fast => self.estimate.fast,
            PpmlMemoryRoute::Low => self.estimate.low,
        }
    }

    #[must_use]
    pub const fn selected_bytes(self) -> u128 {
        let ordinary = self
            .solver_workspace_bytes
            .saturating_add(self.reusable_projection_bytes)
            .saturating_add(self.reusable_certificate_bytes)
            .saturating_add(self.reusable_slope_bytes);
        let separate = self.separation_projection_bytes;
        self.selected_inventory()
            .peak_bytes
            .saturating_add(if ordinary > separate {
                ordinary
            } else {
                separate
            })
    }

    // Charges contain the other two optional buffers and the proposed replacement.
    fn reusable_workspaces_fit(&self, charges: [u128; 3]) -> bool {
        let Some(base) = self
            .selected_inventory()
            .peak_bytes
            .checked_add(self.solver_workspace_bytes)
        else {
            return false;
        };
        charges
            .into_iter()
            .try_fold(base, u128::checked_add)
            .is_some_and(|peak| self.usable_bytes.is_none_or(|usable| peak <= usable))
    }

    /// Optional storage must never make an otherwise affordable fit fail.
    pub(crate) fn admit_reusable_projection(&mut self, bytes: u128) -> bool {
        if !self.reusable_workspaces_fit([
            self.reusable_certificate_bytes,
            self.reusable_slope_bytes,
            bytes,
        ]) {
            return false;
        }
        self.reusable_projection_bytes = bytes;
        true
    }

    pub(crate) fn admit_reusable_certificate(&mut self, bytes: u128) -> bool {
        if !self.reusable_workspaces_fit([
            self.reusable_projection_bytes,
            self.reusable_slope_bytes,
            bytes,
        ]) {
            return false;
        }
        self.reusable_certificate_bytes = bytes;
        true
    }

    pub(crate) fn admit_reusable_slope(&mut self, bytes: u128) -> bool {
        if !self.reusable_workspaces_fit([
            self.reusable_projection_bytes,
            self.reusable_certificate_bytes,
            bytes,
        ]) {
            return false;
        }
        self.reusable_slope_bytes = bytes;
        true
    }

    pub(crate) fn admit_separation_projection(&mut self, bytes: u128) -> bool {
        let Some(peak) = self.selected_inventory().peak_bytes.checked_add(bytes) else {
            return false;
        };
        if self.usable_bytes.is_some_and(|usable| peak > usable) {
            return false;
        }
        self.separation_projection_bytes = bytes;
        true
    }

    pub(crate) fn with_solver_workspace(mut self, bytes: u128) -> Result<Self> {
        let peak = self
            .selected_inventory()
            .peak_bytes
            .checked_add(bytes)
            .and_then(|n| n.checked_add(self.reusable_projection_bytes))
            .and_then(|n| n.checked_add(self.reusable_certificate_bytes))
            .and_then(|n| n.checked_add(self.reusable_slope_bytes))
            .ok_or_else(|| allocation("PPML solver workspace peak overflows"))?;
        if self.usable_bytes.is_some_and(|usable| peak > usable) {
            return Err(allocation(format!(
                "public PPML FE solver workspace does not fit the selected memory plan: solver_workspace={bytes}, execution={}, usable={}",
                self.selected_inventory().peak_bytes,
                self.usable_bytes.unwrap_or(0),
            )));
        }
        self.solver_workspace_bytes = bytes;
        Ok(self)
    }

    /// Fit-lifetime resident peak assigned to one stable timing phase.  A
    /// detector-specific separation phase shares the registered largest
    /// separation peak; this is deliberately conservative until a narrower
    /// detector workspace is selected before preflight.
    #[must_use]
    pub const fn modeled_phase_peak_bytes(self, phase: PpmlPhase) -> u128 {
        let inventory = self.selected_inventory();
        let base = match phase {
            PpmlPhase::Preparation => inventory.preparation_peak_bytes,
            PpmlPhase::SeparationFe | PpmlPhase::SeparationSimplex | PpmlPhase::SeparationIr => {
                inventory.separation_peak_bytes
            }
            PpmlPhase::SeparationMu => inventory.separation_mu_peak_bytes,
            PpmlPhase::WeightResponseRefresh => inventory.weight_response_refresh_peak_bytes,
            // Projection and finite-solve banks are nested inside the strict
            // canonical solve as well as ordinary IRLS. Their action-wide
            // phase maximum must therefore include the outer strict lifetime.
            PpmlPhase::FeProjection | PpmlPhase::FiniteSolve => inventory.strict_final_peak_bytes,
            PpmlPhase::ObjectiveLineSearch => inventory.objective_line_search_peak_bytes,
            PpmlPhase::StrictFinalSolve => inventory.strict_final_peak_bytes,
            PpmlPhase::Inference => inventory.inference_peak_bytes,
            PpmlPhase::FeRecovery => inventory.fe_recovery_peak_bytes,
            PpmlPhase::Publication => inventory.publication_peak_bytes,
        };
        match phase {
            PpmlPhase::WeightResponseRefresh
            | PpmlPhase::FeProjection
            | PpmlPhase::FiniteSolve
            | PpmlPhase::ObjectiveLineSearch
            | PpmlPhase::StrictFinalSolve => base
                .saturating_add(self.solver_workspace_bytes)
                .saturating_add(self.reusable_projection_bytes)
                .saturating_add(self.reusable_certificate_bytes)
                .saturating_add(self.reusable_slope_bytes),
            PpmlPhase::Preparation => base
                .saturating_add(self.reusable_projection_bytes)
                .saturating_add(self.reusable_certificate_bytes)
                .saturating_add(self.reusable_slope_bytes),
            PpmlPhase::SeparationFe
            | PpmlPhase::SeparationSimplex
            | PpmlPhase::SeparationIr
            | PpmlPhase::SeparationMu => base.saturating_add(self.separation_projection_bytes),
            PpmlPhase::Inference | PpmlPhase::FeRecovery | PpmlPhase::Publication => base,
        }
    }
}

/// Build the complete PPML fit-lifetime peak model.
pub fn estimate_ppml_memory(
    dimensions: PpmlMemoryDimensions,
    config: PpmlMemoryConfig,
) -> Result<PpmlMemoryEstimate> {
    dimensions.validate(config)?;
    let fast = estimate_route(dimensions, config, PpmlMemoryRoute::Fast)?;
    let low = estimate_route(dimensions, config, PpmlMemoryRoute::Low)?;
    Ok(PpmlMemoryEstimate {
        fast,
        low,
        recommended_poolsize: config.poolsize.min(dimensions.finite_columns),
    })
}

/// Resolve `memory(fast|low|auto)` before constructing any large PPML
/// workspace.  An unavailable route fails with `ErrorCode::Allocation`, which
/// maps to Stata `r(909)` at the plugin boundary.
pub fn plan_ppml_memory(
    dimensions: PpmlMemoryDimensions,
    config: PpmlMemoryConfig,
) -> Result<PpmlMemoryPlan> {
    plan_ppml_memory_with_probe(dimensions, config, None)
}

pub(crate) fn plan_ppml_memory_with_probe(
    dimensions: PpmlMemoryDimensions,
    config: PpmlMemoryConfig,
    probe: Option<PpmlMemoryPreflightProbe>,
) -> Result<PpmlMemoryPlan> {
    let estimate = estimate_ppml_memory(dimensions, config)?;
    // Model and validate first so a malformed request cannot leave an
    // embedding's begin/decision observation window half-open. Estimation is
    // scalar arithmetic and does not construct a PPML workspace.
    if let Some(probe) = probe {
        probe(PpmlMemoryPreflightEvent::Begin)?;
    }
    let safety_headroom_bytes = config
        .available_bytes
        .map(|available| MINIMUM_HEADROOM_BYTES.max(available / 5))
        .unwrap_or(0);
    let usable_bytes = config.usable_bytes();
    let fits = |bytes: u128| usable_bytes.is_none_or(|usable| bytes <= usable);
    let selected = match (config.requested, config.require_fast_route) {
        (MemoryMode::Fast, _) if fits(estimate.fast.peak_bytes) => Some(PpmlMemoryRoute::Fast),
        (MemoryMode::Low, false) if fits(estimate.low.peak_bytes) => Some(PpmlMemoryRoute::Low),
        (MemoryMode::Auto, _) if fits(estimate.fast.peak_bytes) => Some(PpmlMemoryRoute::Fast),
        (MemoryMode::Auto, false) if fits(estimate.low.peak_bytes) => Some(PpmlMemoryRoute::Low),
        (MemoryMode::Fast | MemoryMode::Low | MemoryMode::Auto, _) => None,
    };
    if let Some(probe) = probe {
        probe(PpmlMemoryPreflightEvent::Decision(
            PpmlMemoryPreflightObservation {
                requested: config.requested,
                selected,
                fast_peak_bytes: estimate.fast.peak_bytes,
                low_peak_bytes: estimate.low.peak_bytes,
                available_bytes: config.available_bytes,
                embedding_resident_bytes: config.embedding_resident_bytes,
                safety_headroom_bytes,
                usable_bytes,
                workspace_allocation_started: false,
            },
        ))?;
    }
    // Emit outside the begin/decision allocator observation window. This
    // private diagnostic adds no retained state and never changes admission.
    if std::env::var_os("FEREG_PPML_MEMORY_TRACE").is_some() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_PPML_MEMORY_TRACE|dimensions={dimensions:?}|fast={:?}|low={:?}",
            estimate.fast, estimate.low,
        ));
    }
    let selected = match selected {
        Some(selected) => selected,
        None => {
            let available = config.available_bytes.map_or_else(
                || "unavailable".to_owned(),
                |value| format!("{value} bytes"),
            );
            return Err(allocation(format!(
                "feppml memory preflight failed before workspace allocation: fast={} bytes, low={} bytes, available={}, safety_headroom={} bytes",
                estimate.fast.peak_bytes,
                estimate.low.peak_bytes,
                available,
                safety_headroom_bytes,
            )));
        }
    };
    Ok(PpmlMemoryPlan {
        requested: config.requested,
        selected,
        dimensions,
        preflight_dimensions: dimensions,
        estimate,
        available_bytes: config.available_bytes,
        embedding_resident_bytes: config.embedding_resident_bytes,
        safety_headroom_bytes,
        usable_bytes,
        solver_workspace_bytes: 0,
        separation_projection_bytes: 0,
        reusable_projection_bytes: 0,
        reusable_certificate_bytes: 0,
        reusable_slope_bytes: 0,
    })
}

// Resident buffers and scratch-file capacity have distinct owners.
struct PpmlStorageContribution {
    resident_bytes: u128,
    io_buffer_bytes: u128,
    scratch_file_bytes: u128,
    projection_rhs: u128,
}

fn storage_contribution(
    n: u128,
    k: u128,
    pool: u128,
    route: PpmlMemoryRoute,
) -> Result<PpmlStorageContribution> {
    match route {
        PpmlMemoryRoute::Fast => {
            // Compaction changes length, not the original N-by-(K+1) capacity.
            let projection_rhs = crate::ols::retained_rhs_stride(k)?;
            Ok(PpmlStorageContribution {
                resident_bytes: matrix_bytes::<f64>(
                    n,
                    projection_rhs,
                    "PPML fast retained within design capacity",
                )?,
                io_buffer_bytes: 0,
                scratch_file_bytes: 0,
                projection_rhs,
            })
        }
        PpmlMemoryRoute::Low => {
            let resident_bytes = matrix_bytes::<f64>(n, pool, "PPML low within-design pool")?;
            let scratch_file_bytes = matrix_bytes::<f64>(n, k, "PPML low scratch cells")?;
            let io_buffer_bytes = crate::ols::conservative_scratch_io_bytes(scratch_file_bytes, k)?;
            Ok(PpmlStorageContribution {
                resident_bytes,
                io_buffer_bytes,
                scratch_file_bytes,
                projection_rhs: pool.max(1),
            })
        }
    }
}

fn estimate_route(
    dimensions: PpmlMemoryDimensions,
    config: PpmlMemoryConfig,
    route: PpmlMemoryRoute,
) -> Result<PpmlMemoryInventory> {
    let n = dimensions.observations as u128;
    let original_n = dimensions.original_observations as u128;
    let candidate_n = dimensions.candidate_observations as u128;
    let loaded = dimensions.loaded_numeric_columns as u128;
    let k = dimensions.finite_columns as u128;
    let coefficient_slots = dimensions.coefficient_slots as u128;
    let reported_coefficient_slots = dimensions.reported_coefficient_slots as u128;
    let fe = dimensions.fe_dimensions as u128;
    let levels = dimensions.fe_total_levels as u128;
    let total_fe_channels = dimensions.fe_total_channels as u128;
    let maximum_fe_channels = dimensions.fe_max_channels as u128;
    let maximum_level_members = dimensions.fe_max_level_members as u128;
    let fe_directions = dimensions.fe_coefficient_directions as u128;
    let clusters = dimensions.cluster_dimensions as u128;
    let subset_groups = dimensions.cluster_subset_groups as u128;
    let threads = dimensions.threads as u128;
    let pool = config.poolsize.min(dimensions.finite_columns) as u128;
    let word = size_of::<usize>() as u128;

    let canonical_numeric_bytes = checked_sum(&[
        matrix_bytes::<f64>(n, loaded, "canonical numeric source cells")?,
        // Outcome, prior weight, and canonical offset.
        matrix_bytes::<f64>(n, 3, "canonical PPML scalar columns")?,
        bytes_f64(k)?,
    ])?;
    let retained_base_adjustment_bytes =
        bytes_f64(dimensions.retained_base_adjustment_capacity_elements as u128)?;
    let retained_raw_preseparation_probability_weight_bytes =
        bytes_f64(dimensions.retained_raw_preseparation_probability_weight_elements as u128)?;
    let topology_bytes = checked_sum(&[
        checked_mul(
            checked_mul(n, fe, "PPML observation-FE codes")?,
            4,
            "PPML FE code bytes",
        )?,
        checked_mul(
            checked_mul(n, fe, "PPML observation-FE memberships")?,
            word,
            "PPML FE membership bytes",
        )?,
        checked_mul(levels, 3 * word, "PPML FE level offsets and permutations")?,
        matrix_bytes::<f64>(fe_directions, 2, "PPML FE coefficient work")?,
    ])?;
    let fe_solver_topology_bytes = if fe == 0 {
        0
    } else {
        let slope_channels = total_fe_channels
            .checked_sub(fe)
            .ok_or_else(|| allocation("PPML FE channel count is smaller than its effect count"))?;
        let legacy_envelope = checked_sum(&[
            checked_mul(
                checked_mul(n, fe, "PPML reusable FE codes")?,
                word,
                "PPML reusable FE code bytes",
            )?,
            checked_mul(
                checked_mul(n, fe, "PPML reusable FE memberships")?,
                word,
                "PPML reusable FE membership bytes",
            )?,
            checked_mul(
                levels,
                checked_mul(2, word, "PPML reusable FE level metadata")?,
                "PPML reusable FE level metadata bytes",
            )?,
            matrix_bytes::<f64>(n, slope_channels, "PPML reusable FE slope values")?,
        ])?;
        let construction = crate::memory::OwnedFeDesignMemoryEstimate::from_summary(
            dimensions.observations,
            dimensions.fe_dimensions,
            dimensions.fe_total_levels,
            dimensions.fe_max_levels,
            usize::try_from(slope_channels)
                .map_err(|_| allocation("PPML slope channel count is not addressable"))?,
        )?;
        // Public LSMR/CMG have a second topology admission check. MAP and Mu
        // do not: this base preflight must cover EffectTerm construction too.
        // Keep the existing conservative envelope where it is larger.
        legacy_envelope.max(construction.construction_bytes)
    };
    let cluster_topology_bytes = checked_sum(&[
        checked_mul(
            checked_mul(n, clusters, "PPML observation-cluster codes")?,
            4,
            "PPML cluster code bytes",
        )?,
        checked_mul(subset_groups, 2 * word, "PPML cluster group topology")?,
    ])?;
    // The immutable candidate axis keeps the capacity reserved during
    // numeric compaction, while the current prepared-generation map keeps
    // only active rows. Original-axis ledgers use their concrete enum widths,
    // including the byte-wide Vec<bool> activity mask.
    let steady_row_map_bytes = checked_mul(
        checked_sum(&[
            candidate_n,
            checked_mul(n, 2, "PPML retained and prepared generation row maps")?,
        ])?,
        word,
        "PPML candidate, retained, and prepared row-map capacity bytes",
    )?;
    let prepared_drop_reason_bytes = checked_mul(
        checked_mul(
            original_n,
            PREPARED_DROP_REASON_COPY_COUNT,
            "PPML prepared drop-reason elements",
        )?,
        size_of::<DropReason>() as u128,
        "PPML prepared drop-reason bytes",
    )?;
    let separation_activity_bytes = checked_mul(
        original_n,
        size_of::<bool>() as u128,
        "PPML separation activity",
    )?;
    let separation_reason_bytes = checked_mul(
        original_n,
        size_of::<Option<SeparationDropReason>>() as u128,
        "PPML original-axis separation reasons",
    )?;
    let row_ledger_bytes = checked_sum(&[
        steady_row_map_bytes,
        prepared_drop_reason_bytes,
        separation_activity_bytes,
        separation_reason_bytes,
    ])?;
    let arc_vec_control_block_bytes = checked_sum(&[
        checked_mul(2, word, "PPML Arc strong/weak counters")?,
        size_of::<Vec<usize>>() as u128,
    ])?;
    // Immutable candidate rows and initial numeric drop reasons each own one
    // Arc allocation. After the first accepted rebase, the current retained
    // row map owns a third allocation; without any drop-capable policy it
    // continues to share the candidate Arc.
    let steady_arc_count = 2 + u128::from(dimensions.separation_rebase_enabled);
    let row_axis_arc_header_bytes = checked_mul(
        steady_arc_count,
        arc_vec_control_block_bytes,
        "PPML steady Arc<Vec> row-axis control blocks",
    )?;
    let final_drop_reason_bytes = checked_mul(
        original_n,
        size_of::<DropReason>() as u128,
        "PPML final merged drop reasons",
    )?;
    let separation_report_row_bytes = checked_mul(
        checked_mul(n, 2, "PPML separation report row vectors")?,
        word,
        "PPML separation report row bytes",
    )?;
    let separation_report_reason_bytes = checked_mul(
        n,
        size_of::<Option<SeparationDropReason>>() as u128,
        "PPML separation report reasons",
    )?;
    let separation_method_report_row_bytes =
        checked_mul(n, word, "PPML accepted separation method row identities")?;
    let separation_method_report_level_bytes = if dimensions.separation_exact_fe_enabled {
        checked_mul(
            levels,
            size_of::<ExactFeLevelIdentity>() as u128,
            "PPML accepted exact-FE level identities",
        )?
    } else {
        0
    };
    let separation_method_report_record_bytes = checked_mul(
        checked_mul(
            dimensions
                .separation_generation_capacity
                .min(crate::ppml_separation::PREFLIGHT_REPORT_GENERATIONS) as u128,
            dimensions.separation_method_count as u128,
            "PPML separation method execution records",
        )?,
        size_of::<SeparationMethodReport>() as u128,
        "PPML separation method execution-record bytes",
    )?;
    let separation_generation_hash_bytes = checked_mul(
        dimensions.separation_generation_capacity as u128,
        size_of::<u64>() as u128,
        "PPML separation generation-hash bytes",
    )?;
    let separation_exact_level_transient_bytes = checked_mul(
        separation_method_report_level_bytes,
        2,
        "PPML exact-FE cascade level workspaces",
    )?;
    let separation_singleton_workspace_bytes = if dimensions.separation_singleton_enabled {
        checked_sum(&[
            checked_mul(
                levels,
                size_of::<u128>() as u128,
                "PPML singleton exact multiplicity bytes",
            )?,
            checked_mul(
                checked_sum(&[fe, 1])?,
                word,
                "PPML singleton source offset bytes",
            )?,
            checked_mul(
                fe,
                size_of::<(&[u32], &LevelIndex)>() as u128,
                "PPML singleton borrowed source views",
            )?,
            checked_mul(
                loaded,
                size_of::<CategoricalCodeView<'_>>() as u128,
                "PPML singleton compact categorical views",
            )?,
            checked_mul(n, word, "PPML singleton queue and sorted output bytes")?,
            checked_mul(
                n,
                checked_mul(
                    2,
                    size_of::<bool>() as u128,
                    "PPML singleton bool-mask width",
                )?,
                "PPML singleton bool-mask bytes",
            )?,
        ])?
    } else {
        0
    };
    let separation_cascade_workspace_bytes = if dimensions.separation_rebase_enabled
        && (dimensions.separation_singleton_enabled || dimensions.separation_exact_fe_enabled)
    {
        let outer_active_and_reason = checked_mul(
            n,
            checked_mul(
                2,
                size_of::<bool>() as u128,
                "PPML cascade outer byte-mask width",
            )?,
            "PPML cascade outer active/reason bytes",
        )?;
        let singleton_inner = if dimensions.separation_singleton_enabled {
            separation_singleton_workspace_bytes
        } else {
            0
        };
        let exact_inner = if dimensions.separation_exact_fe_enabled {
            checked_sum(&[
                checked_mul(n, word, "PPML cascade exact-FE row workspace")?,
                checked_mul(
                    n,
                    size_of::<bool>() as u128,
                    "PPML cascade exact-FE selection mask",
                )?,
                separation_exact_level_transient_bytes,
            ])?
        } else {
            0
        };
        checked_sum(&[outer_active_and_reason, singleton_inner.max(exact_inner)])?
    } else {
        0
    };
    let prepared_model_metadata_bytes = checked_sum(&[
        dimensions.metadata.prepared_nested_bytes,
        checked_mul(
            reported_coefficient_slots,
            size_of::<PpmlCoefficientStatus>() as u128,
            "PPML original omission statuses",
        )?,
        checked_mul(
            coefficient_slots,
            checked_sum(&[
                size_of::<PreparedOlsCoefficient>() as u128,
                size_of::<CoefficientSlot>() as u128,
            ])?,
            "PPML prepared coefficient and report slots",
        )?,
        checked_mul(
            k,
            size_of::<PreparedOlsColumn>() as u128,
            "PPML prepared finite-column metadata",
        )?,
    ])?;
    // Constructors reserve coefficients, columns and reporting slots at the
    // original reporting axis even if rebasing removes most logical slots.
    // The row-scaled metadata includes categorical sources that are absent
    // from the absorbed-effect count (finite factors and cluster-only sources).
    let prepared_generation_bytes = checked_sum(&[
        matrix_bytes::<f64>(n, checked_sum(&[loaded, 3])?, "PPML generation numerics")?,
        checked_mul(
            loaded,
            size_of::<Vec<f64>>() as u128,
            "PPML numeric descriptors",
        )?,
        checked_mul(n, word, "PPML generation sample row map")?,
        original_n,
        dimensions.metadata.prepared_nested_bytes,
        dimensions.metadata.generation_fixed_bytes,
        checked_mul(
            n,
            dimensions.metadata.generation_bytes_per_row,
            "PPML generation topology rows",
        )?,
        checked_mul(levels, word, "PPML generation membership offsets")?,
        checked_mul(
            reported_coefficient_slots,
            (size_of::<PreparedOlsCoefficient>()
                + size_of::<PreparedOlsColumn>()
                + size_of::<CoefficientSlot>()) as u128,
            "PPML generation reserved coefficient axis",
        )?,
    ])?;
    let generation_row_allowance = checked_mul(n, word, "PPML generation row allowance")?;
    // Both current and replacement generations own this payload; row ledgers
    // and staging remain separate charges at their actual overlap boundaries.
    let prepared_model_payload = checked_sum(&[
        canonical_numeric_bytes,
        topology_bytes,
        cluster_topology_bytes,
        prepared_model_metadata_bytes,
    ])?;
    let legacy_generation_bytes =
        checked_sum(&[prepared_model_payload, generation_row_allowance, original_n])?;
    let prepared_generation_supplement_bytes =
        prepared_generation_bytes.saturating_sub(legacy_generation_bytes);
    // Atomic rebase is a true old-plus-new transaction.  The ordinary
    // `common` payload below already owns the raw owner, current generation,
    // and steady ledgers.  A replacement generation is built in full before
    // any of those old owners can be released.
    let replacement_drop_reason_bytes = checked_mul(
        original_n,
        size_of::<DropReason>() as u128,
        "PPML replacement generation drop reasons",
    )?;
    let replacement_row_map_bytes = checked_mul(n, word, "PPML replacement generation row map")?;
    let separation_replacement_generation_bytes = if dimensions.separation_rebase_enabled {
        checked_sum(&[
            prepared_model_payload,
            prepared_generation_supplement_bytes,
            dimensions.metadata.preparation_transient_bytes,
            replacement_drop_reason_bytes,
            replacement_row_map_bytes,
        ])?
    } else {
        0
    };
    // Historical accepted rows (D) and current primary/singleton/exact rows
    // (M) are disjoint, so D+M<=N and fit the ordinary method-report row
    // envelope. The validated numerical primary is moved directly into the
    // commit, and its certified direction is released before cascade/rebuild.
    // Consequently neither payload needs a second transaction allowance.
    let separation_rebase_proposal_bytes = 0;
    let separation_rebase_direction_bytes = 0;
    let staged_ledger_bytes = if dimensions.separation_rebase_enabled {
        checked_sum(&[
            checked_mul(
                original_n,
                size_of::<bool>() as u128,
                "PPML staged separation activity",
            )?,
            checked_mul(
                original_n,
                size_of::<Option<SeparationDropReason>>() as u128,
                "PPML staged separation reasons",
            )?,
            checked_mul(n, word, "PPML staged retained row map")?,
            separation_rebase_proposal_bytes,
        ])?
    } else {
        0
    };
    let staged_candidate_mask_bytes = if dimensions.separation_rebase_enabled {
        checked_mul(
            original_n,
            size_of::<bool>() as u128,
            "PPML staged loaded candidate mask",
        )?
    } else {
        0
    };
    let staged_probability_position_bytes = if !dimensions.separation_rebase_enabled
        || dimensions.retained_raw_preseparation_probability_weight_elements == 0
    {
        0
    } else {
        checked_mul(n, word, "PPML staged raw pweight positions")?
    };
    let staged_probability_value_bytes = if !dimensions.separation_rebase_enabled
        || dimensions.retained_raw_preseparation_probability_weight_elements == 0
    {
        0
    } else {
        bytes_f64(n)?
    };
    // Probability staging first owns positions+values without a replacement.
    // During the new model build the values have moved into the loaded column,
    // while positions remain for exact restoration.  The completed-generation
    // lifetime follows after the temporary candidate mask/positions are gone.
    // Every generation allocates its compact offset after that restoration, so
    // the build-phase generation is bounded by the completed payload minus one
    // guaranteed N-vector.
    let generation_offset_bytes = bytes_f64(n)?;
    let replacement_during_build_bytes = if dimensions.separation_rebase_enabled {
        separation_replacement_generation_bytes
            .checked_sub(generation_offset_bytes)
            .ok_or_else(|| allocation("PPML replacement generation build bound underflows"))?
    } else {
        0
    };
    let probability_stage_only_bytes = checked_sum(&[
        staged_candidate_mask_bytes,
        staged_probability_position_bytes,
        staged_probability_value_bytes,
    ])?;
    let replacement_build_bytes = checked_sum(&[
        replacement_during_build_bytes,
        staged_candidate_mask_bytes,
        staged_probability_position_bytes,
    ])?;
    let staged_or_replacement_bytes = probability_stage_only_bytes
        .max(replacement_build_bytes)
        .max(separation_replacement_generation_bytes);
    let separation_rebase_bytes = checked_sum(&[
        staged_ledger_bytes,
        staged_or_replacement_bytes,
        separation_rebase_direction_bytes,
    ])?;
    let separation_rebase_staging_bytes = separation_rebase_bytes
        .checked_sub(separation_replacement_generation_bytes)
        .ok_or_else(|| allocation("PPML rebase staging bound underflows"))?;
    let separation_rebase_arc_header_bytes = if dimensions.separation_rebase_enabled {
        arc_vec_control_block_bytes
    } else {
        0
    };
    let (iteration_cells, coefficient_cells) =
        PpmlIterationLayout::new(dimensions).capacity_counts()?;
    let iteration_buffer_bytes = bytes_f64(checked_sum(&[
        iteration_cells,
        checked_mul(n, 2, "PPML immutable outcome terms")?,
    ])?)?;
    let coefficient_buffer_bytes = bytes_f64(coefficient_cells)?;
    let storage = storage_contribution(n, k, pool, route)?;

    let square = checked_mul(k, k, "PPML finite square workspace")?;
    let projection_rhs = storage.projection_rhs;
    let effective_maximum_channels = maximum_fe_channels.max(1);
    let effective_maximum_level_members = maximum_level_members.max(1);
    let low_projection_block_cells = match route {
        PpmlMemoryRoute::Fast => 0,
        PpmlMemoryRoute::Low => checked_mul(n, projection_rhs, "PPML low projection block")?,
    };
    // Categorical MAP retains group reductions and certified acceleration in
    // both modes. Low memory sizes every bank by the admitted projection pool;
    // its retained design remains on scratch and its response is separate.
    let low_categorical_map = matches!(dimensions.fe_dimensions, 1..=3)
        && dimensions.fe_total_channels == dimensions.fe_dimensions;
    let categorical_parallel_cells = if route == PpmlMemoryRoute::Fast || low_categorical_map {
        // Fast's historical reserve also covers noncategorical designs.
        checked_sum(&crate::fe::categorical_parallel_counts(
            dimensions.fe_max_levels as u128,
            n,
            projection_rhs,
            threads,
        )?)?
    } else {
        0
    };
    let categorical_accelerator_cells = crate::fe::CategoricalAcceleratorCapacity::new(
        dimensions.fe_dimensions,
        dimensions.fe_total_channels == dimensions.fe_dimensions,
        n,
        projection_rhs,
        levels,
        dimensions.fe_max_levels as u128,
    )?
    .cells(projection_rhs)?;
    let fe_workspace_cells = checked_sum(&[
        crate::fe::MapCapacity::new(
            effective_maximum_channels,
            effective_maximum_level_members,
            projection_rhs,
        )?
        .cells()?,
        low_projection_block_cells,
        categorical_parallel_cells,
        categorical_accelerator_cells,
        if low_categorical_map {
            checked_mul(projection_rhs, 7, "PPML coexisting projection scalar banks")?
        } else {
            0
        },
    ])?;
    let fe_workspace_bytes = bytes_f64(fe_workspace_cells)?;
    // Session response, owned weights, two predictors, frozen response,
    // normalized reusable-MAP weights, and one residual bank. Canonical solve
    // moves that residual into the accepted fit; frozen solve recycles it.
    // A strict transaction's rollback residual is charged in its overlap below.
    // The rank-vector bank is separately exact: retained reference norms,
    // reference means, and within norms consume three K vectors; refreshed
    // mean corrections consume one more; rhs/row/solve/scores/scales and the
    // accepted frozen-step coefficients consume six; the session row consumes
    // one. Strict's second coefficient bank is charged in its transient below.
    let linear_vector_bytes = checked_sum(&[
        matrix_bytes::<f64>(n, 7, "PPML linear observation vectors")?,
        matrix_bytes::<f64>(k, 11, "PPML linear rank vectors")?,
        checked_mul(
            coefficient_slots,
            checked_mul(4, word, "PPML linear slot bookkeeping width")?,
            "PPML linear slot bookkeeping",
        )?,
    ])?;
    let retained_moment_partial_cells = match route {
        PpmlMemoryRoute::Fast => 0,
        PpmlMemoryRoute::Low => crate::ols::retained_scratch_moment_envelope(n, k, threads)?,
    };
    let linear_workspace_bytes = checked_sum(&[
        matrix_bytes::<f64>(
            square,
            LINEAR_SQUARE_WORKSPACE_COUNT,
            "PPML moment/factor/inverse-lower/bread workspaces",
        )?,
        bytes_f64(retained_moment_partial_cells)?,
    ])?;
    let reported_coefficient_workspace_slots = if coefficient_slots == 0 {
        0
    } else {
        coefficient_slots.max(4)
    };
    let linear_result_metadata_bytes = checked_sum(&[
        checked_mul(
            checked_mul(k, 4, "PPML retained linear rank indices")?,
            word,
            "PPML retained linear rank-index bytes",
        )?,
        bytes_f64(square)?,
        checked_mul(
            coefficient_slots,
            size_of::<PreparedOlsFittedCoefficient>() as u128,
            "PPML accepted fitted coefficient records",
        )?,
        checked_mul(
            coefficient_slots,
            size_of::<PreparedOlsBreadCoordinate>() as u128,
            "PPML reported bread coordinates",
        )?,
        checked_mul(
            coefficient_slots,
            size_of::<FrozenLinearBasisSlot>() as u128,
            "PPML frozen linear basis slots",
        )?,
        checked_mul(
            reported_coefficient_workspace_slots,
            size_of::<Option<f64>>() as u128,
            "PPML frozen reported-coefficient workspace",
        )?,
    ])?;

    let detector_vector_cells = checked_mul(
        n,
        dimensions.separation_workspace_vectors as u128,
        "PPML separation detector vectors",
    )?;
    let simplex_combined_bytes = bytes_f64(dimensions.simplex_tableau_cells as u128)?;
    let detector_vector_bytes = bytes_f64(detector_vector_cells)?;
    let has_detector =
        dimensions.separation_workspace_vectors > 0 || dimensions.simplex_tableau_cells > 0;
    let has_numerical_detector =
        dimensions.separation_workspace_vectors >= 8 || dimensions.simplex_tableau_cells > 0;
    let detector_proposal_bytes = if has_detector {
        checked_mul(
            n,
            checked_sum(&[word, 1])?,
            "PPML separation detector proposal",
        )?
    } else {
        0
    };
    let detector_certificate_bytes = if has_numerical_detector {
        matrix_bytes::<f64>(square, 2, "PPML separation certificates")?
    } else {
        0
    };
    let separation_linear_projection_bytes = if has_numerical_detector {
        checked_sum(&[
            coefficient_buffer_bytes,
            storage.resident_bytes,
            storage.io_buffer_bytes,
            fe_solver_topology_bytes,
            linear_vector_bytes,
            fe_workspace_bytes,
            linear_workspace_bytes,
            linear_result_metadata_bytes,
        ])?
    } else {
        0
    };
    // Detectors run sequentially. The bounded explicit simplex design/tableau
    // is released before numerical direction validation enters the shared
    // linear projection, and IR/Mu use that projection while their registered
    // vector bank and two-square span certificate are live. Thus the exact
    // method envelope is max(simplex, shared-linear + detector-vectors +
    // numerical-certificate), not the sum of those alternatives. The simplex
    // ceiling already includes its own certificate/result vectors. A detector
    // may return one N-sized row proposal plus its activity mask outside that
    // method maximum.
    let numerical_detector_workspace = checked_sum(&[
        separation_linear_projection_bytes,
        detector_vector_bytes,
        detector_certificate_bytes,
    ])?;
    let separation_workspace_bytes = checked_sum(&[
        simplex_combined_bytes
            .max(numerical_detector_workspace)
            .max(separation_exact_level_transient_bytes),
        detector_proposal_bytes,
    ])?;
    let mu_exploration_retained_bytes = if dimensions.separation_mu_enabled {
        checked_sum(&[bytes_f64(n)?, MU_EXPLORATION_RECORD_BYTES])?
    } else {
        0
    };
    let cluster_subsets = if clusters == 0 {
        0
    } else {
        (1_u128 << dimensions.cluster_dimensions) - 1
    };
    let sandwich_inference_workspace_bytes = checked_sum(&[
        if clusters == 0 {
            matrix_bytes::<f64>(square, 6, "PPML sandwich square workspaces")?
        } else {
            super::vce::clustered_score_workspace_bound(
                dimensions.observations,
                dimensions.finite_columns,
            )?
            .max(matrix_bytes::<f64>(
                square,
                6,
                "PPML sandwich square workspaces",
            )?)
        },
        matrix_bytes::<f64>(subset_groups, k, "PPML cluster subset scores")?,
        checked_mul(
            cluster_subsets,
            size_of::<crate::sandwich::ClusterSubsetReduction>() as u128,
            "PPML reducer subset reports",
        )?,
        matrix_bytes::<f64>(k, threads.max(1), "PPML sandwich reductions")?,
        // Retain the prior square envelope and conservatively add the complete
        // robust partial bank, including any smaller final numerical rank.
        if clusters == 0 {
            super::vce::dense_score_partials_bound(
                dimensions.observations,
                dimensions.finite_columns,
            )?
        } else {
            0
        },
    ])?;
    // A clustered zero-dimensional sandwich does not construct score/meat
    // workspaces. It counts exact joint groups by sorting one fallibly owned
    // row ordering, with every subset processed sequentially. Numerical rank
    // is known only after preflight, so the inference ceiling must dominate
    // either mutually exclusive branch rather than summing both.
    let rank_zero_cluster_inventory_bytes = if clusters == 0 {
        0
    } else {
        checked_mul(n, word, "PPML rank-zero cluster subset ordering")?
    };
    let inference_workspace_bytes =
        sandwich_inference_workspace_bytes.max(rank_zero_cluster_inventory_bytes);
    let inference_slots = coefficient_slots.max(reported_coefficient_slots);
    // During factor verification this full-axis covariance allowance instead
    // holds the old compact covariance; expansion follows that decision.
    // The full-axis VCE survives the compact sandwich workspaces, then
    // coexists with FE recovery and the private publication matrices. Count
    // each subset's dimension vector at its declared subset width ceiling.
    let inference_result_bytes = checked_sum(&[
        matrix_bytes::<f64>(
            inference_slots,
            inference_slots,
            "PPML retained full covariance",
        )?,
        matrix_bytes::<f64>(
            inference_slots,
            2,
            "PPML retained standard errors and z statistics",
        )?,
        checked_mul(k, word, "PPML retained Wald slot identities")?,
        checked_mul(clusters, word, "PPML retained declared cluster groups")?,
        checked_mul(
            cluster_subsets,
            checked_sum(&[
                size_of::<super::PpmlClusterSubsetReport>() as u128,
                checked_mul(clusters, word, "PPML cluster subset dimension capacity")?,
            ])?,
            "PPML retained cluster subset reports",
        )?,
    ])?;
    let separately_allocated_output_columns = dimensions
        .requested_output_columns
        .checked_sub(usize::from(dimensions.requested_mean_output))
        .ok_or_else(|| allocation("PPML requested mean exceeds observation-output count"))?;
    let saved_output_bytes = matrix_bytes::<f64>(
        n,
        separately_allocated_output_columns as u128,
        "PPML separately allocated requested observation outputs",
    )?;
    let fe_recovery_workspace_bytes = owned_fe_recovery_workspace_bytes(dimensions)?;
    let publication_bytes = checked_sum(&[
        bytes_f64(dimensions.publication_matrix_cells as u128)?,
        checked_mul(
            dimensions.fe_total_channels as u128,
            size_of::<&[f64]>() as u128,
            "PPML publication saved-channel descriptors",
        )?,
    ])?;
    let publication_result_bytes = checked_sum(&[
        arc_vec_control_block_bytes,
        matrix_bytes::<f64>(
            n,
            PUBLICATION_RESULT_NUMERIC_VECTOR_COUNT,
            "PPML publication result numeric vectors",
        )?,
        checked_mul(
            checked_mul(
                n,
                PUBLICATION_RESULT_ROW_MAP_COUNT,
                "PPML publication row maps",
            )?,
            word,
            "PPML publication row-map bytes",
        )?,
    ])?;
    // Publication validation runs after sandwich inference has released its
    // six K-by-K workspaces.  Rank is bounded by the generated finite axis;
    // omitted/base reporting slots never grow the identified covariance.
    //
    // PSD peak (after dropping the raw eigensystem before the final one):
    //   4 * rank^2 f64 + rank f64.
    // Wald peak:
    //   2 * rank^2 f64 + 2 * rank f64 + rank usize
    //   + one bool for every reporting slot.
    // The final slot-state validation needs the same bool vector but is
    // sequential, so it does not add to either peak.
    let publication_psd_validator_bytes = bytes_f64(checked_sum(&[
        checked_mul(
            square,
            PUBLICATION_PSD_SQUARE_WORKSPACE_COUNT,
            "PPML publication PSD square workspaces",
        )?,
        k,
    ])?)?;
    let publication_wald_validator_bytes = checked_sum(&[
        bytes_f64(checked_sum(&[
            checked_mul(square, 2, "PPML publication Wald square workspaces")?,
            checked_mul(k, 2, "PPML publication Wald vectors")?,
        ])?)?,
        checked_mul(k, word, "PPML publication Wald tested slots")?,
        reported_coefficient_slots,
    ])?;
    // The original-axis generation mask, candidate-axis separation replay
    // and inference-slot/saved-channel masks belong to sequential validators.
    // Method-attribution storage is released before hash replay. Candidate
    // capacity, rather than final N, bounds the replay after separation drops.
    let publication_separation_replay_bytes = checked_mul(
        dimensions.candidate_observations as u128,
        (size_of::<bool>() + size_of::<Option<SeparationDropReason>>()) as u128,
        "PPML publication separation replay masks",
    )?;
    let publication_metadata_mask_bytes = checked_sum(&[
        reported_coefficient_slots,
        dimensions.requested_saved_fe_channels as u128,
    ])?;
    let publication_validator_workspace_bytes = publication_psd_validator_bytes
        .max(publication_wald_validator_bytes)
        .max(dimensions.original_observations as u128)
        .max(publication_separation_replay_bytes)
        .max(publication_metadata_mask_bytes)
        .max(k);
    let diagnostic_bytes = checked_mul(
        config.diagnostic_capacity as u128,
        DIAGNOSTIC_RECORD_BYTES,
        "PPML bounded diagnostic record",
    )?;

    let input_owner_supplement_bytes = dimensions
        .metadata
        .input_owner_bytes
        .saturating_sub(config.embedding_resident_bytes);
    let common = checked_sum(&[
        dimensions.metadata.bias_workspace_bytes,
        config.embedding_resident_bytes,
        input_owner_supplement_bytes,
        prepared_model_payload,
        retained_base_adjustment_bytes,
        retained_raw_preseparation_probability_weight_bytes,
        row_ledger_bytes,
        row_axis_arc_header_bytes,
        prepared_generation_supplement_bytes,
        separation_method_report_row_bytes,
        separation_method_report_level_bytes,
        separation_method_report_record_bytes,
        separation_generation_hash_bytes,
        diagnostic_bytes,
    ])?;
    let input_preparation_peak_bytes = checked_sum(&[
        common,
        dimensions.metadata.preparation_transient_bytes,
        matrix_bytes::<f64>(
            candidate_n,
            checked_sum(&[loaded, 3])?,
            "PPML candidate-capacity preparation staging",
        )?,
    ])?;
    let result_metadata_bytes = checked_sum(&[
        dimensions.metadata.result_bytes,
        size_of::<super::prepared_irls::PreparedPpmlIrlsFit>() as u128,
    ])?;
    let fitted_common = checked_sum(&[
        common,
        result_metadata_bytes,
        iteration_buffer_bytes,
        coefficient_buffer_bytes,
        storage.resident_bytes,
        storage.io_buffer_bytes,
        linear_result_metadata_bytes,
    ])?;
    let iteration_peak_bytes = checked_sum(&[
        fitted_common,
        fe_solver_topology_bytes,
        linear_vector_bytes,
        fe_workspace_bytes,
        linear_workspace_bytes,
    ])?;
    let audit_equivalence_refresh_transient_bytes = if dimensions.allocation_audit_enabled {
        bytes_f64(n)?
    } else {
        0
    };
    let weight_response_refresh_peak_bytes = checked_sum(&[
        iteration_peak_bytes,
        audit_equivalence_refresh_transient_bytes,
    ])?;
    let post_solve_transient_bytes = checked_sum(&[
        bytes_f64(n)?,
        checked_mul(
            FINITE_SOLVE_FIXED_RECORD_COUNT,
            FINITE_SOLVE_FIXED_RECORD_BYTES,
            "PPML frozen finite-solve records",
        )?,
        OBJECTIVE_LINE_SEARCH_RECORD_BYTES,
    ])?;
    let post_solve_peak_bytes = checked_sum(&[iteration_peak_bytes, post_solve_transient_bytes])?;
    let objective_line_search_peak_bytes = checked_sum(&[
        post_solve_peak_bytes,
        audit_equivalence_refresh_transient_bytes,
    ])?;
    // `Preparation` is also the boundary under which IRLS constructs its
    // reusable PPML and shared-linear workspaces. Those buffers become the
    // ordinary iteration profile after initialization, but their allocation
    // peak must remain attributed to the same phase rather than appearing as
    // unexplained preparation growth.
    let preparation_peak_bytes = input_preparation_peak_bytes.max(iteration_peak_bytes);
    // A detector completes before its accepted atomic transaction begins.
    // Historical report rows D and current primary/cascade rows M are
    // disjoint (D+M<=N) and share `separation_method_report_row_bytes` in
    // `common`.  During commit, only one exact-capacity primary copy and (for
    // numerical methods) its one direction survive beside the staged ledgers
    // and replacement generation.  Taking the phase maximum prevents the
    // detector's N-sized proposal and shared-linear validation workspaces from
    // being charged again during the old-plus-new generation transaction.
    let separation_detection_peak_bytes = checked_sum(&[
        common,
        separation_workspace_bytes.max(separation_singleton_workspace_bytes),
    ])?;
    let separation_cascade_peak_bytes = checked_sum(&[common, separation_cascade_workspace_bytes])?;
    let separation_transaction_peak_bytes = checked_sum(&[
        common,
        separation_rebase_bytes,
        separation_rebase_arc_header_bytes,
    ])?;
    let separation_peak_bytes = separation_detection_peak_bytes
        .max(separation_cascade_peak_bytes)
        .max(separation_transaction_peak_bytes);
    // Mu's exploratory IRLS fit is sequential with its detector. The moved
    // mean plus inline certificate remain live only through the Mu numerical
    // detector and, for an accepted proposal, its rebase transaction. They do
    // not overlap an unrelated simplex tableau that may dominate the global
    // non-Mu detector maximum.
    let separation_mu_peak_bytes = if dimensions.separation_mu_enabled {
        let mu_detection_peak = checked_sum(&[
            common,
            numerical_detector_workspace,
            detector_proposal_bytes,
            mu_exploration_retained_bytes,
        ])?;
        let mu_transaction_peak = checked_sum(&[
            separation_transaction_peak_bytes,
            mu_exploration_retained_bytes,
        ])?;
        let mu_cascade_peak =
            checked_sum(&[separation_cascade_peak_bytes, mu_exploration_retained_bytes])?;
        iteration_peak_bytes
            .max(separation_peak_bytes)
            .max(mu_detection_peak)
            .max(mu_cascade_peak)
            .max(mu_transaction_peak)
    } else {
        separation_peak_bytes
    };
    // The strict solve does not rebuild an N-by-K design. During candidate
    // assembly the six working/result squares plus accepted fit and frozen
    // step breads total eight K-by-K banks, already covered by the seven
    // `linear_workspace_bytes` banks plus the one square in
    // `linear_result_metadata_bytes`. Four N-length residual/predictor banks
    // are additionally live, as is one second K-length frozen coefficient
    // bank.
    //
    // Two mutually exclusive non-square candidate boundaries must both fit.
    // The installed/pre-install stage owns candidate fit kept+dropped,
    // candidate basis kept+dropped, candidate active positions, and the
    // outer rollback copy (four K index banks), plus the caller's copied
    // active-column identities when a specialized engine changes basis.
    // That fifth bank survives the transaction through engine refresh. Add
    // one complete set of
    // fitted coefficient, bread-coordinate, and basis-slot records. Earlier,
    // fit assembly instead overlaps final fitted coefficients/coordinates
    // with generated Option/status vectors, reference/score norms, rollback
    // and candidate active positions, selected candidate positions, fit
    // kept+dropped, and canonical candidate identities. Take their maximum;
    // neither boundary may borrow storage from the accepted baseline.
    let strict_final_vector_transient_bytes = matrix_bytes::<f64>(
        n,
        STRICT_FINAL_OVERLAP_VECTOR_COUNT,
        "PPML strict-final canonical vector overlap",
    )?;
    let strict_installed_metadata_bytes = checked_sum(&[
        checked_mul(
            checked_mul(k, 5, "PPML strict installed rank-index banks")?,
            word,
            "PPML strict installed rank-index bytes",
        )?,
        checked_mul(
            coefficient_slots,
            checked_sum(&[
                size_of::<PreparedOlsFittedCoefficient>() as u128,
                size_of::<PreparedOlsBreadCoordinate>() as u128,
                size_of::<FrozenLinearBasisSlot>() as u128,
            ])?,
            "PPML strict candidate coefficient/bread/basis records",
        )?,
    ])?;
    let strict_assembly_per_generated_bytes = checked_sum(&[
        size_of::<Option<f64>>() as u128,
        size_of::<PreparedOlsFitStatus>() as u128,
        checked_mul(
            2,
            size_of::<f64>() as u128,
            "PPML strict assembly norm widths",
        )?,
        checked_mul(5, word, "PPML strict assembly index widths")?,
    ])?;
    let strict_assembly_metadata_bytes = checked_sum(&[
        checked_mul(
            coefficient_slots,
            checked_sum(&[
                size_of::<PreparedOlsFittedCoefficient>() as u128,
                size_of::<PreparedOlsBreadCoordinate>() as u128,
            ])?,
            "PPML strict assembly fitted coefficient/bread records",
        )?,
        checked_mul(
            k,
            strict_assembly_per_generated_bytes,
            "PPML strict assembly generated-column records",
        )?,
    ])?;
    let strict_final_metadata_transient_bytes = checked_sum(&[
        bytes_f64(k)?,
        strict_installed_metadata_bytes.max(strict_assembly_metadata_bytes),
    ])?;
    let strict_final_transient_bytes = checked_sum(&[
        strict_final_vector_transient_bytes,
        strict_final_metadata_transient_bytes,
    ])?;
    let strict_final_peak_bytes = checked_sum(&[
        iteration_peak_bytes,
        strict_final_transient_bytes,
        audit_equivalence_refresh_transient_bytes,
    ])?;
    // Inference results survive recovery and publication; each phase adds
    // only its own transient work or requested publication products.
    let final_result_owners = checked_sum(&[
        fitted_common,
        final_drop_reason_bytes,
        separation_report_row_bytes,
        separation_report_reason_bytes,
        inference_result_bytes,
    ])?;
    let inference_peak_bytes = checked_sum(&[final_result_owners, inference_workspace_bytes])?;
    let fe_recovery_peak_bytes = checked_sum(&[final_result_owners, fe_recovery_workspace_bytes])?;
    let publication_peak_bytes = checked_sum(&[
        final_result_owners,
        saved_output_bytes,
        publication_bytes,
        publication_result_bytes,
        publication_validator_workspace_bytes,
    ])?;
    let peak_bytes = preparation_peak_bytes
        .max(separation_peak_bytes)
        .max(separation_mu_peak_bytes)
        .max(iteration_peak_bytes)
        .max(weight_response_refresh_peak_bytes)
        .max(post_solve_peak_bytes)
        .max(objective_line_search_peak_bytes)
        .max(strict_final_peak_bytes)
        .max(inference_peak_bytes)
        .max(fe_recovery_peak_bytes)
        .max(publication_peak_bytes);

    Ok(PpmlMemoryInventory {
        preparation_metadata_transient_bytes: dimensions.metadata.preparation_transient_bytes,
        result_metadata_bytes,
        embedding_resident_bytes: config.embedding_resident_bytes,
        input_owner_supplement_bytes,
        canonical_numeric_bytes,
        retained_base_adjustment_bytes,
        retained_raw_preseparation_probability_weight_bytes,
        topology_bytes,
        fe_solver_topology_bytes,
        cluster_topology_bytes,
        steady_row_map_bytes,
        prepared_drop_reason_bytes,
        separation_activity_bytes,
        separation_reason_bytes,
        row_ledger_bytes,
        row_axis_arc_header_bytes,
        prepared_model_metadata_bytes,
        prepared_generation_bytes,
        prepared_generation_supplement_bytes,
        separation_replacement_generation_bytes,
        separation_rebase_proposal_bytes,
        separation_rebase_direction_bytes,
        separation_rebase_staging_bytes,
        separation_rebase_bytes,
        separation_rebase_arc_header_bytes,
        final_drop_reason_bytes,
        separation_report_row_bytes,
        separation_report_reason_bytes,
        separation_method_report_row_bytes,
        separation_method_report_level_bytes,
        separation_method_report_record_bytes,
        separation_generation_hash_bytes,
        separation_exact_level_transient_bytes,
        separation_singleton_workspace_bytes,
        separation_cascade_workspace_bytes,
        iteration_buffer_bytes,
        coefficient_buffer_bytes,
        retained_within_resident_bytes: storage.resident_bytes,
        retained_within_io_buffer_bytes: storage.io_buffer_bytes,
        retained_within_scratch_bytes: storage.scratch_file_bytes,
        linear_vector_bytes,
        fe_workspace_bytes,
        linear_workspace_bytes,
        linear_result_metadata_bytes,
        separation_workspace_bytes,
        simplex_combined_bytes,
        mu_exploration_retained_bytes,
        inference_workspace_bytes,
        inference_result_bytes,
        fe_recovery_workspace_bytes,
        saved_output_bytes,
        publication_bytes,
        publication_result_bytes,
        publication_validator_workspace_bytes,
        diagnostic_bytes,
        preparation_peak_bytes,
        separation_peak_bytes,
        separation_mu_peak_bytes,
        iteration_peak_bytes,
        audit_equivalence_refresh_transient_bytes,
        weight_response_refresh_peak_bytes,
        post_solve_transient_bytes,
        post_solve_peak_bytes,
        objective_line_search_peak_bytes,
        strict_final_transient_bytes,
        strict_final_metadata_transient_bytes,
        strict_final_peak_bytes,
        inference_peak_bytes,
        fe_recovery_peak_bytes,
        publication_peak_bytes,
        peak_bytes,
    })
}

/// PPML admits only retained-product, repository-owned CG and slope-MAP
/// recovery. Failed attempts release their workspaces before the next route;
/// normalization and finishing are also sequential. No observation-sized
/// state is allocated to calculate this envelope.
fn owned_fe_recovery_workspace_bytes(shape: PpmlMemoryDimensions) -> Result<u128> {
    if shape.requested_saved_fe_channels == 0 {
        return Ok(0);
    }
    let n = shape.observations as u128;
    let effects = shape.fe_dimensions as u128;
    let channels = shape.fe_total_channels as u128;
    let levels = shape.fe_total_levels as u128;
    let max_levels = shape.fe_max_levels as u128;
    let directions = shape.fe_coefficient_directions as u128;
    let word = size_of::<usize>() as u128;
    let source_views = checked_mul(
        shape.loaded_numeric_columns.max(shape.fe_dimensions) as u128,
        size_of::<CategoricalCodeView<'_>>() as u128,
        "PPML recovery categorical source views",
    )?;
    let offsets = checked_mul(effects + 1, word, "PPML recovery effect offsets")?;
    let coefficient = bytes_f64(directions)?;
    let slope_workspace = crate::fe::slope_map_workspace_bytes(shape.fe_max_channels, 1)?;
    let output = checked_sum(&[
        matrix_bytes::<f64>(
            n,
            channels + 2,
            "PPML recovery channels, total and residual",
        )?,
        checked_mul(
            channels,
            size_of::<Option<Vec<f64>>>() as u128,
            "PPML recovery channel registry",
        )?,
    ])?;
    let views = checked_sum(&[
        checked_mul(
            effects,
            size_of::<crate::fe::CategoricalAbsorbedEffectView<'_>>() as u128,
            "PPML recovery effect views",
        )?,
        checked_mul(
            channels,
            size_of::<&[f64]>() as u128,
            "PPML recovery slope views",
        )?,
        // The full decomposition mask remains live throughout every attempt.
        channels,
    ])?;
    // Topology revalidation can re-factor one interaction at a time. Its
    // row ordering and output codes precede recovery numeric construction.
    let validation = checked_sum(&[
        checked_mul(
            n,
            word + size_of::<u32>() as u128,
            "PPML recovery topology validation",
        )?,
        checked_mul(source_views, 2, "PPML recovery validation source tables")?,
    ])?;
    let normalization_metadata = checked_sum(&[
        source_views,
        checked_mul(
            effects,
            4 * word,
            "PPML recovery intercept indices and component views",
        )?,
        offsets,
        checked_mul(
            effects,
            size_of::<Vec<usize>>() as u128,
            "PPML recovery component registry",
        )?,
    ])?;
    // Union-find parents/ranks/observed flags coexist with returned labels.
    // Once the union-find drops, three per-component numeric banks coexist
    // with labels. Component count cannot exceed any participating level axis.
    let components = checked_mul(levels, 2 * word + 2, "PPML recovery component construction")?
        .max(checked_sum(&[
            checked_mul(levels, word, "PPML recovery component labels")?,
            matrix_bytes::<f64>(max_levels, 3, "PPML recovery component means")?,
        ])?);
    let normalization = checked_sum(&[coefficient, normalization_metadata, components])?;
    let row_partials = checked_mul(
        shape.observations.div_ceil(32_768) as u128,
        32,
        "PPML recovery row certificates",
    )?;
    let level_partials = checked_mul(
        shape.fe_max_levels.div_ceil(4_096) as u128,
        32,
        "PPML recovery level certificates",
    )?;
    let finishing = checked_sum(&[
        coefficient,
        offsets,
        source_views,
        checked_mul(
            effects,
            3 * word,
            "PPML recovery finishing code and level views",
        )?,
        output,
        slope_workspace.max(row_partials).max(level_partials),
    ])?;
    let retained = validation.max(normalization).max(finishing);
    let map = validation.max(checked_sum(&[
        coefficient,
        offsets,
        output,
        slope_workspace,
    ])?);
    let cg = if shape.fe_dimensions > 1 && shape.fe_total_channels == shape.fe_dimensions {
        let design = checked_sum(&[
            source_views,
            checked_mul(
                effects,
                size_of::<crate::fe::BorrowedInterceptEffect<'_>>() as u128,
                "PPML recovery borrowed CG design",
            )?,
        ])?;
        let cg_coefficients = checked_sum(&[
            bytes_f64(levels)?,
            checked_mul(effects * 3 + 1, word, "PPML recovery CG coefficient layout")?,
        ])?;
        let recurrence = crate::fe::borrowed_cg_recovery_workspace_bytes(
            shape.observations,
            shape.fe_dimensions,
            shape.fe_total_levels,
            shape.fe_max_levels,
            shape.threads,
        )?;
        validation.max(checked_sum(&[
            design,
            recurrence.max(checked_sum(&[
                cg_coefficients,
                normalization.max(finishing),
            ])?),
        ])?)
    } else {
        0
    };
    checked_sum(&[views, retained.max(map).max(cg)])
}

/// Capacity fingerprint proving that the PPML-owned steady-state buffers did
/// not grow.  It intentionally makes no claim about the shared linear session
/// or the global allocator; those have separate telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlWorkspaceCapacityReport {
    pub observation_capacity_sum: usize,
    pub coefficient_capacity_sum: usize,
    /// The workspace owns no diagnostic records; this stays zero, as the
    /// runtime result wire and the telemetry validator require.
    pub diagnostic_capacity: usize,
}

/// Compact public summary of the shared linear session's full private
/// capacity fingerprint.  Start/end equality of these totals is reported in
/// addition to the exact private-struct equality bit, so qualification
/// evidence can preserve both boundaries without exporting OLS internals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PpmlLinearWorkspaceCapacityReport {
    pub element_capacity_sum: u128,
    pub retained_resident_elements: usize,
    pub retained_scratch_bytes: u64,
    pub fe_topology_resident_bytes: u128,
}

/// Allocation counters sampled by an embedding allocator without consulting
/// RSS or allocating diagnostic storage.  The event counter must increase on
/// every successful `alloc`/`alloc_zeroed` and every successful growing
/// `realloc`; deallocation and shrinking reallocations do not count as new
/// allocation events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PpmlAllocatorSnapshot {
    pub current_bytes: u64,
    pub peak_bytes: u64,
    pub allocation_events: u64,
}

/// Optional no-context probe used only by PPML allocation qualification.
///
/// A plain function pointer keeps the normal estimator and all OLS paths free
/// of dynamic dispatch, closure storage, and allocator-observer overhead.  The
/// prepared PPML driver calls it at complete-iteration boundaries, never from
/// an observation or fixed-effect row loop.
pub type PpmlAllocatorSnapshotProbe = fn() -> PpmlAllocatorSnapshot;

/// Boundary event for qualification-only, phase-local allocator peaks.  The
/// embedding allocator keeps a secondary peak counter that is independent of
/// the action-wide tracker.  `End` returns the maximum tracked resident bytes
/// for this phase minus the action-start baseline, saturating at zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlPhaseAllocatorEvent {
    Begin,
    End,
}

pub type PpmlPhaseAllocatorProbe = fn(PpmlPhase, PpmlPhaseAllocatorEvent) -> u64;

pub(crate) fn begin_phase_allocation(probe: Option<PpmlPhaseAllocatorProbe>, phase: PpmlPhase) {
    if let Some(probe) = probe {
        let _ = probe(phase, PpmlPhaseAllocatorEvent::Begin);
    }
}

pub(crate) fn end_phase_allocation(
    probe: Option<PpmlPhaseAllocatorProbe>,
    phase: PpmlPhase,
) -> Option<u64> {
    probe.map(|probe| probe(phase, PpmlPhaseAllocatorEvent::End))
}

/// External observation window for the steady-state allocation gate.  The
/// embedding allocator owns event counting; core capacity fingerprints cover
/// the PPML workspace, while the shared session supplies its own boolean and
/// scratch evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlSteadyStateAllocationObservation {
    pub measured_iterations: usize,
    pub allocator_events_start: u64,
    pub allocator_events_end: u64,
    pub allocator_current_start: u64,
    pub allocator_current_end: u64,
    pub allocator_peak_start: u64,
    pub allocator_peak_end: u64,
    pub workspace_start: PpmlWorkspaceCapacityReport,
    pub workspace_end: PpmlWorkspaceCapacityReport,
    pub linear_workspace_start: PpmlLinearWorkspaceCapacityReport,
    pub linear_workspace_end: PpmlLinearWorkspaceCapacityReport,
    /// Equality of the complete shared-linear-session capacity fingerprints
    /// captured at the same two iteration boundaries.
    pub linear_workspace_capacity_stable: bool,
    pub scratch_bytes_start: u64,
    pub scratch_bytes_end: u64,
    /// Actual final FE solver route for the measured iteration window.  Only
    /// MAP is qualified for allocation-free changing-weight PPML today.
    pub linear_route: FeSolverKind,
    pub linear_session_allocation_free: bool,
    pub fresh_warm_equivalent: bool,
    /// Median duration across the complete fixed-size warmed iteration
    /// window.  Collection and sorting use stack storage only.
    pub steady_iteration_median_nanoseconds: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlSteadyStateAllocationCertificate {
    /// Original allocator and exact-capacity evidence retained for fresh validation.
    pub observation: PpmlSteadyStateAllocationObservation,
    pub allocation_events: u64,
    pub allocator_peak_growth_bytes: u64,
    pub workspace_capacity_stable: bool,
    pub linear_workspace_capacity_stable: bool,
    pub scratch_stable: bool,
    pub route_allocation_qualified: bool,
    pub passed: bool,
}

impl PpmlSteadyStateAllocationCertificate {
    pub fn allocator_growth_bytes(&self) -> i128 {
        i128::from(self.observation.allocator_current_end)
            - i128::from(self.observation.allocator_current_start)
    }
}

/// Validate (but never create) an allocation observation.  This is an
/// operational/performance certificate and cannot replace scientific result
/// certification.
pub fn certify_ppml_steady_state_allocation(
    observation: PpmlSteadyStateAllocationObservation,
) -> Result<PpmlSteadyStateAllocationCertificate> {
    if observation.measured_iterations < PPML_STEADY_STATE_AUDIT_ITERATIONS {
        return Err(invalid_argument(
            "PPML steady-state allocation audit requires at least 16 measured iterations",
        ));
    }
    if observation.allocator_events_end < observation.allocator_events_start
        || observation.allocator_peak_end < observation.allocator_peak_start
        || observation.allocator_peak_start < observation.allocator_current_start
        || observation.allocator_peak_end < observation.allocator_current_end
    {
        return Err(invalid_argument(
            "PPML steady-state allocator counters are not monotone",
        ));
    }
    let allocation_events = observation.allocator_events_end - observation.allocator_events_start;
    let allocator_growth_bytes = i128::from(observation.allocator_current_end)
        - i128::from(observation.allocator_current_start);
    let allocator_peak_growth_bytes = observation
        .allocator_peak_end
        .checked_sub(observation.allocator_peak_start)
        .expect("allocator peak monotonicity was validated");
    let workspace_capacity_stable = observation.workspace_start == observation.workspace_end;
    let linear_workspace_capacity_stable = observation.linear_workspace_capacity_stable
        && observation.linear_workspace_start == observation.linear_workspace_end;
    let scratch_stable = observation.scratch_bytes_start == observation.scratch_bytes_end;
    let route_allocation_qualified = observation.linear_route == FeSolverKind::Map;
    let passed = allocation_events == 0
        && allocator_growth_bytes == 0
        && allocator_peak_growth_bytes == 0
        && workspace_capacity_stable
        && linear_workspace_capacity_stable
        && scratch_stable
        && route_allocation_qualified
        && observation.linear_session_allocation_free
        && observation.fresh_warm_equivalent;
    Ok(PpmlSteadyStateAllocationCertificate {
        observation,
        allocation_events,
        allocator_peak_growth_bytes,
        workspace_capacity_stable,
        linear_workspace_capacity_stable,
        scratch_stable,
        route_allocation_qualified,
        passed,
    })
}

/// Fit-lifetime PPML state reused by every IRLS and line-search iteration.
/// Counts owned by the mutable iteration banks. Immutable outcome terms are
/// separate parents in the phase inventory, never part of the capacity proof.
struct PpmlIterationLayout {
    rows: usize,
    slots: usize,
    objective: usize,
}
impl PpmlIterationLayout {
    fn new(dimensions: PpmlMemoryDimensions) -> Self {
        Self {
            rows: dimensions.observations,
            slots: dimensions.coefficient_slots,
            objective: objective_partial_cells(dimensions.observations, dimensions.threads),
        }
    }
    fn capacity_counts(&self) -> Result<(u128, u128)> {
        Ok((
            checked_sum(&[
                checked_mul(
                    self.rows as u128,
                    ITERATION_VECTOR_COUNT,
                    "PPML reusable iteration vectors",
                )?,
                self.objective as u128,
            ])?,
            checked_mul(
                self.slots as u128,
                COEFFICIENT_VECTOR_COUNT,
                "PPML reusable coefficient vectors",
            )?,
        ))
    }
}

#[derive(Debug)]
pub struct PpmlIterationWorkspace {
    eta: Vec<f64>,
    mean: Vec<f64>,
    hessian_weights: Vec<f64>,
    working_response: Vec<f64>,
    candidate_eta: Vec<f64>,
    trial_eta: Vec<f64>,
    trial_mean: Vec<f64>,
    /// Bounds under which `mean` was computed from the current `eta` with the
    /// deterministic per-row Poisson mean, or `None` when unknown. Lets the
    /// working-state refresh reuse the accepted line-search mean instead of
    /// recomputing identical `exp` values.
    mean_bounds: Option<PoissonBounds>,
    /// The same record for `trial_mean` against `trial_eta`.
    trial_mean_bounds: Option<PoissonBounds>,
    /// Advances whenever `mean` may change, so a certificate computed from
    /// the mean can be reused while the generation is unchanged.
    mean_generation: u64,
    score_residuals: Vec<f64>,
    objective_partials: Vec<f64>,
    coefficients: Vec<f64>,
    candidate_coefficients: Vec<f64>,
    trial_coefficients: Vec<f64>,
    steady_state_capacity: PpmlWorkspaceCapacityReport,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlLineSearchAcceptance {
    pub objective: PoissonObjective,
    pub step: f64,
    pub step_halvings: usize,
}

impl PpmlIterationWorkspace {
    /// Adopt initializer-owned eta and coefficient vectors, avoiding a second
    /// observation-sized allocation and copy at the fit boundary.
    /// The prepared driver retains its own bounded iteration reports, so the
    /// numerical workspace holds no diagnostic records.
    pub(crate) fn from_initial_state(
        plan: PpmlMemoryPlan,
        eta: Vec<f64>,
        coefficients: Vec<f64>,
    ) -> Result<Self> {
        if eta.len() != plan.dimensions.observations {
            return Err(invalid_argument(
                "PPML initial predictor does not match the preflight sample",
            ));
        }
        if coefficients.len() != plan.dimensions.coefficient_slots {
            return Err(invalid_argument(
                "PPML initial coefficients do not match the preflight slot axis",
            ));
        }
        if eta.iter().any(|value| !value.is_finite())
            || coefficients.iter().any(|value| !value.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "PPML initial predictor and coefficients must be finite",
            ));
        }
        let layout = PpmlIterationLayout::new(plan.dimensions);
        let n = layout.rows;
        let k = layout.slots;
        let mut workspace = Self {
            eta,
            mean: fallible_filled(n, 0.0, "PPML mean buffer")?,
            hessian_weights: fallible_filled(n, 0.0, "PPML Hessian-weight buffer")?,
            working_response: fallible_filled(n, 0.0, "PPML working-response buffer")?,
            candidate_eta: fallible_filled(n, 0.0, "PPML candidate-eta buffer")?,
            trial_eta: fallible_filled(n, 0.0, "PPML line-search eta buffer")?,
            trial_mean: fallible_filled(n, 0.0, "PPML line-search mean buffer")?,
            mean_bounds: None,
            trial_mean_bounds: None,
            mean_generation: 0,
            score_residuals: fallible_filled(n, 0.0, "PPML score-residual buffer")?,
            objective_partials: fallible_filled(
                layout.objective,
                0.0,
                "PPML objective partial sums",
            )?,
            coefficients,
            candidate_coefficients: fallible_filled(k, 0.0, "PPML candidate-coefficient buffer")?,
            trial_coefficients: fallible_filled(k, 0.0, "PPML trial-coefficient buffer")?,
            steady_state_capacity: PpmlWorkspaceCapacityReport {
                observation_capacity_sum: 0,
                coefficient_capacity_sum: 0,
                diagnostic_capacity: 0,
            },
        };
        workspace.steady_state_capacity = workspace.capacity_report();
        Ok(workspace)
    }

    #[must_use]
    pub fn eta(&self) -> &[f64] {
        &self.eta
    }

    #[must_use]
    /// Identifies the current `mean` contents; see the field documentation.
    pub(crate) const fn mean_generation(&self) -> u64 {
        self.mean_generation
    }

    pub fn mean(&self) -> &[f64] {
        &self.mean
    }

    #[must_use]
    pub fn hessian_weights(&self) -> &[f64] {
        &self.hessian_weights
    }

    #[must_use]
    pub fn working_response(&self) -> &[f64] {
        &self.working_response
    }

    #[must_use]
    pub fn candidate_eta(&self) -> &[f64] {
        &self.candidate_eta
    }

    pub(crate) fn candidate_eta_mut(&mut self) -> &mut [f64] {
        &mut self.candidate_eta
    }

    #[must_use]
    pub fn trial_eta(&self) -> &[f64] {
        &self.trial_eta
    }

    #[must_use]
    pub fn trial_mean(&self) -> &[f64] {
        &self.trial_mean
    }

    #[must_use]
    pub fn score_residuals(&self) -> &[f64] {
        &self.score_residuals
    }

    #[must_use]
    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }

    #[must_use]
    pub fn candidate_coefficients(&self) -> &[f64] {
        &self.candidate_coefficients
    }

    pub(crate) fn candidate_coefficients_mut(&mut self) -> &mut [f64] {
        &mut self.candidate_coefficients
    }

    /// Form an absolute PPML candidate from a certified incremental solve.
    ///
    /// The joint LSMR engine owns corrections on its compact active-control
    /// axis.  Keeping the scatter here lets it update eta and beta without
    /// allocating or borrowing accepted and candidate fields separately.
    pub(crate) fn fill_incremental_candidate(
        &mut self,
        delta_eta: &[f64],
        active_slots: &[usize],
        delta_coefficients: &[f64],
    ) -> Result<()> {
        if delta_eta.len() != self.eta.len()
            || active_slots.len() != delta_coefficients.len()
            || active_slots
                .iter()
                .any(|slot| *slot >= self.coefficients.len())
        {
            return Err(invalid_argument(
                "PPML incremental candidate dimensions are inconsistent",
            ));
        }
        for ((candidate, &eta), &delta) in
            self.candidate_eta.iter_mut().zip(&self.eta).zip(delta_eta)
        {
            *candidate = eta + delta;
            if !candidate.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML incremental candidate predictor is nonfinite",
                ));
            }
        }
        self.candidate_coefficients
            .copy_from_slice(&self.coefficients);
        for (slot, delta) in active_slots.iter().zip(delta_coefficients) {
            self.candidate_coefficients[*slot] += *delta;
            if !self.candidate_coefficients[*slot].is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML incremental candidate coefficient is nonfinite",
                ));
            }
        }
        self.certify_steady_state_capacity()
    }

    /// Form an absolute candidate from a fused Frisch-Waugh-Lovell solve.
    pub(crate) fn fill_absolute_candidate(
        &mut self,
        fitted_without_offset: &[f64],
        offset: &[f64],
        active_slots: &[usize],
        active_coefficients: &[f64],
    ) -> Result<()> {
        if fitted_without_offset.len() != self.eta.len()
            || offset.len() != self.eta.len()
            || active_slots.len() != active_coefficients.len()
            || active_slots
                .iter()
                .any(|slot| *slot >= self.coefficients.len())
        {
            return Err(invalid_argument(
                "PPML absolute candidate dimensions are inconsistent",
            ));
        }
        for row in 0..self.eta.len() {
            self.candidate_eta[row] = offset[row] + fitted_without_offset[row];
            if !self.candidate_eta[row].is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML absolute candidate predictor is nonfinite",
                ));
            }
        }
        self.candidate_coefficients
            .copy_from_slice(&self.coefficients);
        for (slot, value) in active_slots.iter().zip(active_coefficients) {
            self.candidate_coefficients[*slot] = *value;
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML absolute candidate coefficient is nonfinite",
                ));
            }
        }
        self.certify_steady_state_capacity()
    }

    #[must_use]
    pub fn trial_coefficients(&self) -> &[f64] {
        &self.trial_coefficients
    }

    pub fn set_candidate_eta(&mut self, values: &[f64]) -> Result<()> {
        copy_equal_len(&mut self.candidate_eta, values, "PPML candidate eta")
    }

    pub(crate) fn refresh_score_residuals(&mut self, outcome: &[f64]) -> Result<()> {
        require_observation_lengths(self.mean.len(), &[outcome.len()], "PPML score residual")?;
        for (target, (value, mean)) in self
            .score_residuals
            .iter_mut()
            .zip(outcome.iter().zip(&self.mean))
        {
            *target = value - mean;
            if !target.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML score residual is not finitely representable",
                ));
            }
        }
        self.certify_steady_state_capacity()
    }

    /// Fill mean, Hessian weights, working response, and raw public score
    /// residuals without allocating.
    #[cfg(test)]
    pub(crate) fn refresh_working_state(
        &mut self,
        outcome: &[f64],
        prior_weights: &[f64],
        offset: &[f64],
        bounds: PoissonBounds,
    ) -> Result<()> {
        require_observation_lengths(
            self.eta.len(),
            &[outcome.len(), prior_weights.len(), offset.len()],
            "PPML working state",
        )?;
        self.mean_bounds = None;
        self.mean_generation = self.mean_generation.wrapping_add(1);
        means_from_eta(&self.eta, bounds, &mut self.mean)?;
        self.mean_bounds = Some(bounds);
        fill_poisson_working_state(
            outcome,
            &self.mean,
            prior_weights,
            &self.eta,
            offset,
            &mut self.hessian_weights,
            &mut self.working_response,
        )?;
        for ((residual, outcome), mean) in
            self.score_residuals.iter_mut().zip(outcome).zip(&self.mean)
        {
            *residual = *outcome - *mean;
            if !residual.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML working-state score residual is not finitely representable",
                ));
            }
        }
        self.certify_steady_state_capacity()
    }

    pub(crate) fn refresh_prepared_working_state(
        &mut self,
        inputs: &PreparedPoissonInputs<'_>,
        bounds: PoissonBounds,
    ) -> Result<()> {
        require_observation_lengths(
            self.eta.len(),
            &[inputs.outcome().len()],
            "PPML working state",
        )?;
        if self.mean_bounds != Some(bounds) {
            self.mean_bounds = None;
            self.mean_generation = self.mean_generation.wrapping_add(1);
            inputs.fill_means(&self.eta, bounds, &mut self.mean)?;
            self.mean_bounds = Some(bounds);
        }
        inputs.fill_working_state_and_scores(
            &self.mean,
            &self.eta,
            &mut self.hessian_weights,
            &mut self.working_response,
            &mut self.score_residuals,
        )?;
        self.certify_steady_state_capacity()
    }

    /// Reuse the working-response buffer for a Hessian-only final
    /// recertification. The accepted predictor is already in the model span;
    /// projecting `eta - offset` under the accepted `q * mu` weights rebuilds
    /// the exact final within design and bread without taking another Newton
    /// step or moving the accepted PPML point.
    pub(crate) fn prepare_final_hessian_response(&mut self, offset: &[f64]) -> Result<()> {
        require_observation_lengths(
            self.eta.len(),
            &[offset.len()],
            "PPML final-Hessian response",
        )?;
        for ((response, eta), offset) in self.working_response.iter_mut().zip(&self.eta).zip(offset)
        {
            *response = *eta - *offset;
            if !response.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "PPML final-Hessian response is not finite",
                ));
            }
        }
        self.certify_steady_state_capacity()
    }

    /// Materialize one deterministic line-search trial in caller-owned
    /// buffers.  `trial_mean` is overwritten only after all eta values are
    /// finite; representability remains governed by `PoissonBounds`.
    #[cfg(test)]
    pub(crate) fn fill_line_search_trial(
        &mut self,
        step: f64,
        bounds: PoissonBounds,
    ) -> Result<()> {
        self.fill_line_search_trial_with(step, bounds, |_| Ok(()))
    }

    #[cfg(test)]
    pub(super) fn fill_line_search_trial_with(
        &mut self,
        step: f64,
        bounds: PoissonBounds,
        transform: impl FnOnce(&mut [f64]) -> Result<()>,
    ) -> Result<()> {
        self.fill_line_search_trial_core(step, bounds, None, transform)
    }

    pub(super) fn fill_prepared_line_search_trial_with(
        &mut self,
        step: f64,
        bounds: PoissonBounds,
        inputs: &PreparedPoissonInputs<'_>,
        transform: impl FnOnce(&mut [f64]) -> Result<()>,
    ) -> Result<()> {
        self.fill_line_search_trial_core(step, bounds, Some(inputs), transform)
    }

    fn fill_line_search_trial_core(
        &mut self,
        step: f64,
        bounds: PoissonBounds,
        inputs: Option<&PreparedPoissonInputs<'_>>,
        transform: impl FnOnce(&mut [f64]) -> Result<()>,
    ) -> Result<()> {
        if !step.is_finite() || !(0.0..=1.0).contains(&step) {
            return Err(invalid_argument(
                "PPML line-search step must be finite and lie in [0, 1]",
            ));
        }
        self.trial_mean_bounds = None;
        for row in 0..self.eta.len() {
            let value = self.eta[row] + step * (self.candidate_eta[row] - self.eta[row]);
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    format!("PPML line-search eta is nonfinite at compact row {row}"),
                ));
            }
            self.trial_eta[row] = value;
        }
        for column in 0..self.coefficients.len() {
            let value = self.coefficients[column]
                + step * (self.candidate_coefficients[column] - self.coefficients[column]);
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    format!("PPML line-search coefficient is nonfinite at column {column}"),
                ));
            }
            self.trial_coefficients[column] = value;
        }
        transform(&mut self.trial_eta)?;
        if let Some(inputs) = inputs {
            inputs.fill_means(&self.trial_eta, bounds, &mut self.trial_mean)?;
        } else {
            means_from_eta(&self.trial_eta, bounds, &mut self.trial_mean)?;
        }
        self.trial_mean_bounds = Some(bounds);
        self.certify_steady_state_capacity()
    }

    /// Evaluate the strict candidate directly in the reusable mean buffer
    /// without changing the accepted eta or coefficient state.
    pub(crate) fn evaluate_candidate_objective(
        &mut self,
        inputs: &PreparedPoissonInputs<'_>,
        bounds: PoissonBounds,
    ) -> Result<PoissonObjective> {
        self.trial_mean_bounds = None;
        inputs.fill_means(&self.candidate_eta, bounds, &mut self.trial_mean)?;
        let objective = inputs.evaluate_tiled(
            &self.candidate_eta,
            &self.trial_mean,
            &mut self.objective_partials,
        )?;
        self.certify_steady_state_capacity()?;
        Ok(objective)
    }

    pub(crate) fn evaluate_current_objective(
        &mut self,
        inputs: &PreparedPoissonInputs<'_>,
    ) -> Result<PoissonObjective> {
        inputs.evaluate_tiled(&self.eta, &self.mean, &mut self.objective_partials)
    }

    pub(super) fn evaluate_trial_objective(
        &mut self,
        inputs: &PreparedPoissonInputs<'_>,
    ) -> Result<PoissonObjective> {
        inputs.evaluate_tiled(
            &self.trial_eta,
            &self.trial_mean,
            &mut self.objective_partials,
        )
    }

    pub(super) fn evaluate_external_objective(
        &mut self,
        inputs: &PreparedPoissonInputs<'_>,
        eta: &[f64],
        mean: &[f64],
    ) -> Result<PoissonObjective> {
        inputs.evaluate_tiled(eta, mean, &mut self.objective_partials)
    }

    /// Accept the already-certified trial by swapping observation buffers and
    /// copying K coefficients into their fixed-capacity destination.
    pub fn accept_trial(&mut self) -> Result<()> {
        std::mem::swap(&mut self.eta, &mut self.trial_eta);
        std::mem::swap(&mut self.mean, &mut self.trial_mean);
        self.mean_generation = self.mean_generation.wrapping_add(1);
        std::mem::swap(&mut self.mean_bounds, &mut self.trial_mean_bounds);
        self.coefficients.copy_from_slice(&self.trial_coefficients);
        self.certify_steady_state_capacity()
    }

    /// Deterministic stable-objective backtracking entirely within the
    /// preallocated candidate/trial buffers.  The accepted trial becomes the
    /// current state; a rejected search leaves the current state unchanged.
    #[cfg(test)]
    pub(crate) fn accept_candidate(
        &mut self,
        outcome: &[f64],
        prior_weights: &[f64],
        current_objective: PoissonObjective,
        config: PpmlConfig,
    ) -> Result<PpmlLineSearchAcceptance> {
        let offset = vec![0.; outcome.len()];
        let inputs = PreparedPoissonInputs::new(outcome, prior_weights, &offset)?;
        self.accept_candidate_interruptible(&inputs, current_objective, config, &mut || Ok(()))
    }

    /// Interruptible counterpart used only by the PPML native caller-thread
    /// entry. The callback is invoked once before every backtracking trial.
    pub(crate) fn accept_candidate_interruptible(
        &mut self,
        inputs: &PreparedPoissonInputs<'_>,
        current_objective: PoissonObjective,
        config: PpmlConfig,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<PpmlLineSearchAcceptance> {
        let allowed = current_objective.deviance
            + config.objective_slack * current_objective.deviance.abs().max(1.0);
        let mut step = 1.0_f64;
        for step_halvings in 0..=config.max_step_halvings {
            interrupt()?;
            if step < config.minimum_step {
                break;
            }
            match self.fill_prepared_line_search_trial_with(step, config.bounds, inputs, |_| Ok(()))
            {
                Ok(()) => {}
                Err(error) if error.code == ErrorCode::NonConvergence => {
                    step *= 0.5;
                    continue;
                }
                Err(error) => return Err(error),
            }
            match self.evaluate_trial_objective(inputs) {
                Ok(objective) if objective.deviance <= allowed => {
                    self.accept_trial()?;
                    return Ok(PpmlLineSearchAcceptance {
                        objective,
                        step,
                        step_halvings,
                    });
                }
                Ok(_) => {}
                Err(error) if error.code == ErrorCode::NonConvergence => {}
                Err(error) => return Err(error),
            }
            step *= 0.5;
        }
        Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!(
                "PPML step-halving found no finite deviance-improving step after {} halvings",
                config.max_step_halvings
            ),
        ))
    }

    pub(crate) fn into_final_buffers(self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        (self.coefficients, self.eta, self.mean, self.score_residuals)
    }

    #[must_use]
    pub fn capacity_report(&self) -> PpmlWorkspaceCapacityReport {
        PpmlWorkspaceCapacityReport {
            observation_capacity_sum: self.eta.capacity()
                + self.mean.capacity()
                + self.hessian_weights.capacity()
                + self.working_response.capacity()
                + self.candidate_eta.capacity()
                + self.trial_eta.capacity()
                + self.trial_mean.capacity()
                + self.score_residuals.capacity()
                + self.objective_partials.capacity(),
            coefficient_capacity_sum: self.coefficients.capacity()
                + self.candidate_coefficients.capacity()
                + self.trial_coefficients.capacity(),
            diagnostic_capacity: 0,
        }
    }

    pub fn certify_steady_state_capacity(&self) -> Result<()> {
        let current = self.capacity_report();
        if current == self.steady_state_capacity {
            Ok(())
        } else {
            Err(FeregError::new(
                ErrorCode::Internal,
                format!(
                    "PPML steady-state workspace capacity changed: expected {:?}, observed {:?}",
                    self.steady_state_capacity, current
                ),
            ))
        }
    }
}

impl PpmlMemoryPlan {
    fn estimate_diagnostic_capacity(self) -> usize {
        (self.estimate.fast.diagnostic_bytes / DIAGNOSTIC_RECORD_BYTES).min(usize::MAX as u128)
            as usize
    }
}

/// Stable timing axes for private PPML diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PpmlPhase {
    Preparation = 0,
    SeparationFe = 1,
    SeparationSimplex = 2,
    SeparationIr = 3,
    SeparationMu = 4,
    WeightResponseRefresh = 5,
    FeProjection = 6,
    FiniteSolve = 7,
    ObjectiveLineSearch = 8,
    StrictFinalSolve = 9,
    Inference = 10,
    FeRecovery = 11,
    Publication = 12,
}

pub const PPML_PHASE_COUNT: usize = 13;
pub const PPML_PHASES: [PpmlPhase; PPML_PHASE_COUNT] = [
    PpmlPhase::Preparation,
    PpmlPhase::SeparationFe,
    PpmlPhase::SeparationSimplex,
    PpmlPhase::SeparationIr,
    PpmlPhase::SeparationMu,
    PpmlPhase::WeightResponseRefresh,
    PpmlPhase::FeProjection,
    PpmlPhase::FiniteSolve,
    PpmlPhase::ObjectiveLineSearch,
    PpmlPhase::StrictFinalSolve,
    PpmlPhase::Inference,
    PpmlPhase::FeRecovery,
    PpmlPhase::Publication,
];

impl PpmlPhase {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Preparation => "preparation",
            Self::SeparationFe => "separation_fe",
            Self::SeparationSimplex => "separation_simplex",
            Self::SeparationIr => "separation_ir",
            Self::SeparationMu => "separation_mu",
            Self::WeightResponseRefresh => "weight_response_refresh",
            Self::FeProjection => "fe_projection",
            Self::FiniteSolve => "finite_solve",
            Self::ObjectiveLineSearch => "objective_line_search",
            Self::StrictFinalSolve => "strict_final_solve",
            Self::Inference => "inference",
            Self::FeRecovery => "fe_recovery",
            Self::Publication => "publication",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ObservedDuration {
    elapsed: Duration,
    observed: bool,
}

/// Timer accumulator that distinguishes an unobserved phase from a measured
/// zero-duration phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpmlPhaseTimings {
    phases: [ObservedDuration; PPML_PHASE_COUNT],
}

impl Default for PpmlPhaseTimings {
    fn default() -> Self {
        Self {
            phases: [ObservedDuration::default(); PPML_PHASE_COUNT],
        }
    }
}

impl PpmlPhaseTimings {
    pub fn add(&mut self, phase: PpmlPhase, elapsed: Duration) {
        let slot = &mut self.phases[phase as usize];
        slot.elapsed = slot.elapsed.saturating_add(elapsed);
        slot.observed = true;
    }

    #[must_use]
    pub fn get(&self, phase: PpmlPhase) -> Option<Duration> {
        let slot = self.phases[phase as usize];
        slot.observed.then_some(slot.elapsed)
    }

    #[must_use]
    pub fn seconds(&self, phase: PpmlPhase) -> Option<f64> {
        self.get(phase).map(|elapsed| elapsed.as_secs_f64())
    }
}

/// Allocation fields are optional because the core allocator is not the
/// plugin's global tracking allocator.  Only the embedding layer may populate
/// these observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PpmlAllocationTelemetry {
    pub start_bytes: Option<u64>,
    pub current_bytes: Option<u64>,
    pub peak_bytes: Option<u64>,
    pub previous_baseline_bytes: Option<u64>,
    pub allocation_events_start: Option<u64>,
    pub allocation_events_end: Option<u64>,
}

impl PpmlAllocationTelemetry {
    #[must_use]
    pub fn peak_delta_bytes(self) -> Option<u64> {
        Some(self.peak_bytes?.saturating_sub(self.start_bytes?))
    }

    #[must_use]
    pub fn growth_bytes(self) -> Option<i128> {
        Some(i128::from(self.current_bytes?) - i128::from(self.start_bytes?))
    }

    #[must_use]
    pub fn allocation_events(self) -> Option<u64> {
        self.allocation_events_end?
            .checked_sub(self.allocation_events_start?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PpmlFallbackKind {
    WarmProjection = 0,
    Moment = 1,
    Rank = 2,
    Bread = 3,
    Route = 4,
}

/// PPML-owned fit-lifetime observational report.  Scientific certification
/// remains independent of these values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpmlMemoryTelemetry {
    pub plan: PpmlMemoryPlan,
    /// Optional expected-heap override for the solver inventory. Admission and
    /// validation keep using plan.solver_workspace_bytes. An unmodeled override
    /// suppresses the forecast; None preserves the ordinary route's model.
    pub expected_solver_workspace_peak: Option<crate::memory::SolverWorkspacePeak>,
    /// Requested application threads; zero preserves the public "native
    /// default" sentinel while `effective_threads` reports the resolved team.
    pub requested_threads: usize,
    pub effective_threads: usize,
    pub timings: PpmlPhaseTimings,
    /// Optional allocator-observed resident peak per stable phase.  Missing
    /// means unobserved and is never converted to zero.
    pub observed_phase_peak_bytes: [Option<u64>; PPML_PHASE_COUNT],
    pub allocations: PpmlAllocationTelemetry,
    /// Actual fixed-effect solver that produced the final certified linear
    /// state. `None` means the route has not been observed; it must never be
    /// serialized as a guessed memory/execution mode.
    pub linear_route: Option<FeSolverKind>,
    /// `None` means scratch instrumentation was unavailable; `Some(0)` is an
    /// observed zero.
    pub scratch_peak_bytes: Option<u64>,
    pub fallback_mask: u32,
    pub fallback_count: usize,
    /// Number of bounded iteration reports actually retained, not the
    /// configured capacity.
    pub diagnostic_records_retained: usize,
    pub workspace_capacity: Option<PpmlWorkspaceCapacityReport>,
    pub ppml_workspace_allocation_free: bool,
    /// This stronger claim stays false until both PPML and the shared linear
    /// session have proven stable capacities under allocator observation.
    pub whole_iteration_allocation_free: bool,
    pub steady_state_allocation_certificate: Option<PpmlSteadyStateAllocationCertificate>,
}

impl PpmlMemoryTelemetry {
    pub fn new(
        plan: PpmlMemoryPlan,
        requested_threads: usize,
        effective_threads: usize,
    ) -> Result<Self> {
        if effective_threads == 0 {
            return Err(invalid_argument(
                "PPML telemetry requires positive effective threads",
            ));
        }
        Ok(Self {
            plan,
            expected_solver_workspace_peak: None,
            requested_threads,
            effective_threads,
            timings: PpmlPhaseTimings::default(),
            observed_phase_peak_bytes: [None; PPML_PHASE_COUNT],
            allocations: PpmlAllocationTelemetry::default(),
            linear_route: None,
            scratch_peak_bytes: None,
            fallback_mask: 0,
            fallback_count: 0,
            diagnostic_records_retained: 0,
            workspace_capacity: None,
            ppml_workspace_allocation_free: false,
            whole_iteration_allocation_free: false,
            steady_state_allocation_certificate: None,
        })
    }

    pub fn record_phase(&mut self, phase: PpmlPhase, elapsed: Duration) {
        self.timings.add(phase, elapsed);
    }

    pub fn observe_phase_peak(&mut self, phase: PpmlPhase, bytes: u64) {
        let slot = &mut self.observed_phase_peak_bytes[phase as usize];
        *slot = Some(slot.map_or(bytes, |observed| observed.max(bytes)));
    }

    #[must_use]
    pub fn observed_phase_peak(&self, phase: PpmlPhase) -> Option<u64> {
        self.observed_phase_peak_bytes[phase as usize]
    }

    pub fn record_fallback(&mut self, kind: PpmlFallbackKind) {
        self.fallback_mask |= 1_u32 << kind as u8;
        self.fallback_count = self.fallback_count.saturating_add(1);
    }

    pub fn observe_allocator(&mut self, observation: PpmlAllocationTelemetry) {
        self.allocations = observation;
    }

    pub fn observe_linear_route(&mut self, route: FeSolverKind) {
        self.linear_route = Some(route);
    }

    pub fn observe_scratch(&mut self, bytes: u64) {
        self.scratch_peak_bytes = Some(
            self.scratch_peak_bytes
                .map_or(bytes, |observed| observed.max(bytes)),
        );
    }

    pub fn observe_diagnostic_records(&mut self, retained: usize) -> Result<()> {
        let capacity = self.plan.estimate_diagnostic_capacity();
        if retained > capacity {
            return Err(FeregError::new(
                ErrorCode::Internal,
                format!(
                    "PPML retained {retained} diagnostic records beyond modeled capacity {capacity}",
                ),
            ));
        }
        self.diagnostic_records_retained = retained;
        Ok(())
    }

    #[must_use]
    pub fn allocation_model_ratio(&self) -> Option<f64> {
        let selected = self.plan.selected_bytes();
        if selected == 0 {
            None
        } else {
            self.allocations
                .peak_delta_bytes()
                .map(|peak| peak as f64 / selected as f64)
        }
    }

    pub fn certify_workspace(&mut self, workspace: &PpmlIterationWorkspace) -> Result<()> {
        workspace.certify_steady_state_capacity()?;
        self.workspace_capacity = Some(workspace.capacity_report());
        self.ppml_workspace_allocation_free = true;
        self.whole_iteration_allocation_free = self
            .steady_state_allocation_certificate
            .is_some_and(|certificate| certificate.passed);
        Ok(())
    }

    pub fn observe_steady_state_allocation(
        &mut self,
        observation: PpmlSteadyStateAllocationObservation,
    ) -> Result<()> {
        if self.linear_route.is_some_and(|route| {
            !self
                .plan
                .audit_route_matches_final(observation.linear_route, route)
        }) {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML steady-state allocation route differs from the final linear route",
            ));
        }
        self.linear_route = Some(observation.linear_route);
        let certificate = certify_ppml_steady_state_allocation(observation)?;
        self.whole_iteration_allocation_free =
            self.ppml_workspace_allocation_free && certificate.passed;
        self.steady_state_allocation_certificate = Some(certificate);
        Ok(())
    }

    /// Recompute every modeled and derived memory claim before it can enter a
    /// certified public result.  Observations may remain unavailable where
    /// the embedding allocator cannot supply them, but present values must be
    /// internally complete and consistent.
    /// Fresh host-boundary validation uses the same capacity and allocation
    /// proof as core certification, with publication's mandatory fingerprint.
    #[doc(hidden)]
    pub fn validate_publication(&self) -> Result<()> {
        self.validate()?;
        if self.workspace_capacity.is_none() {
            return Err(invalid_argument(
                "PPML memory telemetry lacks its workspace capacity",
            ));
        }
        if self
            .steady_state_allocation_certificate
            .is_some_and(|c| c.passed != self.whole_iteration_allocation_free)
        {
            return Err(invalid_argument(
                "PPML warmed allocation certificate is inconsistent",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.effective_threads == 0 || self.plan.dimensions.threads != self.effective_threads {
            return Err(invalid_argument(
                "PPML memory telemetry has inconsistent effective threads",
            ));
        }
        let diagnostic_capacity = self.plan.estimate_diagnostic_capacity();
        let mut reconstructed = plan_ppml_memory(
            self.plan.preflight_dimensions,
            PpmlMemoryConfig {
                requested: self.plan.requested,
                require_fast_route: self.plan.selected == PpmlMemoryRoute::Fast,
                available_bytes: self.plan.available_bytes,
                embedding_resident_bytes: self.plan.embedding_resident_bytes,
                poolsize: self.plan.estimate.recommended_poolsize.max(1),
                diagnostic_capacity,
            },
        )?
        .with_final_dimensions(self.plan.dimensions)?
        .with_solver_workspace(self.plan.solver_workspace_bytes)?;
        if !reconstructed.admit_separation_projection(self.plan.separation_projection_bytes)
            || !reconstructed.admit_reusable_projection(self.plan.reusable_projection_bytes)
            || !reconstructed.admit_reusable_certificate(self.plan.reusable_certificate_bytes)
            || !reconstructed.admit_reusable_slope(self.plan.reusable_slope_bytes)
            || reconstructed != self.plan
        {
            return Err(invalid_argument(
                "PPML memory telemetry plan does not match a fresh preflight reconstruction",
            ));
        }

        const KNOWN_FALLBACK_MASK: u32 = (1_u32 << (PpmlFallbackKind::Route as u8 + 1)) - 1;
        if self.fallback_mask & !KNOWN_FALLBACK_MASK != 0
            || (self.fallback_mask == 0) != (self.fallback_count == 0)
            || self.fallback_count < self.fallback_mask.count_ones() as usize
        {
            return Err(invalid_argument(
                "PPML memory telemetry fallback mask/count is inconsistent",
            ));
        }
        match (
            self.allocations.start_bytes,
            self.allocations.current_bytes,
            self.allocations.peak_bytes,
        ) {
            (None, None, None) => {}
            (Some(start), Some(current), Some(peak)) if peak >= start && peak >= current => {}
            _ => {
                return Err(invalid_argument(
                    "PPML allocator telemetry is partial or has a peak below resident usage",
                ));
            }
        }
        if self.allocations.previous_baseline_bytes.is_some()
            && self.allocations.start_bytes.is_none()
        {
            return Err(invalid_argument(
                "PPML previous allocator baseline lacks an action observation",
            ));
        }
        match (
            self.allocations.allocation_events_start,
            self.allocations.allocation_events_end,
        ) {
            (None, None) if self.allocations.start_bytes.is_none() => {}
            (Some(start), Some(end)) if self.allocations.start_bytes.is_some() && end >= start => {}
            _ => {
                return Err(invalid_argument(
                    "PPML action allocation-event telemetry is partial or nonmonotone",
                ));
            }
        }
        if self.diagnostic_records_retained > diagnostic_capacity {
            return Err(invalid_argument(
                "PPML retained diagnostics exceed the modeled bounded capacity",
            ));
        }
        let route = self.linear_route.ok_or_else(|| {
            invalid_argument("PPML memory telemetry lacks an observed final linear route")
        })?;
        if route == FeSolverKind::Auto {
            return Err(invalid_argument(
                "PPML memory telemetry cannot publish an unresolved automatic linear route",
            ));
        }
        let scratch = self.scratch_peak_bytes.ok_or_else(|| {
            invalid_argument("PPML memory telemetry lacks mandatory observed scratch usage")
        })?;
        if u128::from(scratch) > self.plan.selected_inventory().retained_within_scratch_bytes {
            return Err(invalid_argument(
                "PPML observed scratch usage exceeds its selected-route preflight inventory",
            ));
        }
        for phase in PPML_PHASES {
            if let Some(observed) = self.observed_phase_peak(phase) {
                let modeled = self.plan.modeled_phase_peak_bytes(phase);
                if u128::from(observed) > modeled {
                    return Err(invalid_argument(format!(
                        "PPML observed {:?} phase peak ({observed} bytes) exceeds its preflight model ({modeled} bytes)",
                        phase,
                    )));
                }
            }
        }

        if self.ppml_workspace_allocation_free != self.workspace_capacity.is_some() {
            return Err(invalid_argument(
                "PPML workspace allocation-free claim lacks a capacity certificate",
            ));
        }
        if let Some(capacity) = self.workspace_capacity {
            let (observations, coefficients) =
                PpmlIterationLayout::new(self.plan.dimensions).capacity_counts()?;
            let minimum_observation_capacity = usize::try_from(observations)
                .map_err(|_| allocation("PPML workspace observation capacity overflows"))?;
            let minimum_coefficient_capacity = usize::try_from(coefficients)
                .map_err(|_| allocation("PPML workspace coefficient capacity overflows"))?;
            if capacity.observation_capacity_sum < minimum_observation_capacity
                || capacity.coefficient_capacity_sum < minimum_coefficient_capacity
                || capacity.diagnostic_capacity != 0
            {
                return Err(invalid_argument(
                    "PPML workspace capacity certificate is dimensionally inconsistent",
                ));
            }
        }

        let certified_iteration = match self.steady_state_allocation_certificate {
            None => false,
            Some(certificate) => {
                let recomputed = certify_ppml_steady_state_allocation(certificate.observation)?;
                if recomputed != certificate
                    || !self
                        .plan
                        .audit_route_matches_final(certificate.observation.linear_route, route)
                    || self.workspace_capacity != Some(certificate.observation.workspace_end)
                    || certificate.observation.scratch_bytes_end != scratch
                {
                    return Err(invalid_argument(
                        "PPML warmed-iteration allocation certificate is inconsistent",
                    ));
                }
                certificate.passed
            }
        };
        if self.whole_iteration_allocation_free
            != (self.ppml_workspace_allocation_free && certified_iteration)
        {
            return Err(invalid_argument(
                "PPML whole-iteration allocation claim is inconsistent with its certificates",
            ));
        }
        Ok(())
    }
}

/// Allocation-free stopwatch; elapsed time is recorded explicitly so a phase
/// that exits early cannot be silently counted under a later phase.
#[derive(Debug, Clone, Copy)]
pub struct PpmlPhaseClock {
    started: Instant,
}

impl PpmlPhaseClock {
    #[must_use]
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    #[must_use]
    pub fn elapsed(self) -> Duration {
        self.started.elapsed()
    }
}

fn require_observation_lengths(expected: usize, observed: &[usize], label: &str) -> Result<()> {
    if observed.iter().all(|length| *length == expected) {
        Ok(())
    } else {
        Err(invalid_argument(format!(
            "{label} inputs have inconsistent observation counts",
        )))
    }
}

fn copy_equal_len(target: &mut [f64], source: &[f64], label: &str) -> Result<()> {
    if target.len() != source.len() {
        return Err(invalid_argument(format!(
            "{label} dimensions are inconsistent with preflight",
        )));
    }
    if source.iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            format!("{label} must be finite"),
        ));
    }
    target.copy_from_slice(source);
    Ok(())
}

fn fallible_filled(length: usize, value: f64, label: &str) -> Result<Vec<f64>> {
    crate::memory::Capacity::new(length as u128)
        .filled_with(value, || allocation(format!("unable to reserve {label}")))
}

fn bytes_f64(cells: u128) -> Result<u128> {
    checked_mul(cells, size_of::<f64>() as u128, "PPML f64 storage")
}

fn checked_mul(left: u128, right: u128, label: &str) -> Result<u128> {
    left.checked_mul(right).ok_or_else(|| {
        allocation(format!(
            "PPML memory estimate overflow while sizing {label}",
        ))
    })
}

fn checked_sum(values: &[u128]) -> Result<u128> {
    values.iter().try_fold(0_u128, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| allocation("PPML memory estimate overflow"))
    })
}

#[cfg(test)]
mod tests;
