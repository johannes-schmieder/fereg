//! Statistical-separation detection and exact retained-sample accounting.
//!
//! The prepared production sequence runs exact FE levels and comprehensive
//! iterated rectifier. The prepared simplex materializes a bounded explicit
//! finite-plus-FE design (including absorbed slope channels) and refuses its
//! design or tableau before allocation when the configured cell cap is
//! exceeded. Every numerical drop carries a full-model certificate.

mod prepared_coordinator;
mod simplex;
pub(crate) use simplex::{model_simplex_interruptible, simplex_workspace_peak_cells};

pub(crate) use prepared_coordinator::{
    PreparedSeparationCascade, PreparedSeparationCoordinator, PreparedSeparationRebuilder,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparationMethod {
    Singletons,
    FixedEffects,
    Simplex,
    IteratedRectifier,
    Mu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeparationMethods {
    pub fixed_effects: bool,
    pub simplex: bool,
    pub iterated_rectifier: bool,
    pub mu: bool,
}

impl Default for SeparationMethods {
    fn default() -> Self {
        Self {
            fixed_effects: true,
            simplex: true,
            iterated_rectifier: true,
            mu: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparationConfig {
    pub methods: SeparationMethods,
    pub tolerance: f64,
    pub certificate_tolerance: f64,
    pub projection_tolerance: f64,
    pub max_ir_iterations: usize,
    pub simplex_max_iterations: usize,
    pub simplex_max_tableau_cells: usize,
    pub mu_tolerance: f64,
    pub drop_singletons: bool,
    pub strict: bool,
}

impl Default for SeparationConfig {
    fn default() -> Self {
        Self {
            methods: SeparationMethods::default(),
            tolerance: 1e-5,
            certificate_tolerance: 1e-7,
            projection_tolerance: 1e-10,
            max_ir_iterations: 100,
            simplex_max_iterations: 10_000,
            simplex_max_tableau_cells: 8_000_000,
            mu_tolerance: 1e-8,
            drop_singletons: true,
            strict: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodStatus {
    NoCandidates,
    NoSeparation,
    Certified,
    /// A bounded detector was not allocated.  The coordinator accepts this
    /// only when a later comprehensive method certifies the same generation.
    SkippedMemory,
    SkippedUnsupported,
    MaximumIterations,
    Uncertified,
}

/// Allocation-free diagnostic identity attached to one detector execution.
/// `code` is always a static registered message; `detail` carries at most one
/// bounded integer such as an exhausted iteration limit. Diagnostic prose is
/// deliberately excluded from certification digests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeparationNote {
    pub code: &'static str,
    pub detail: usize,
}

impl SeparationNote {
    #[must_use]
    pub const fn new(code: &'static str) -> Self {
        Self { code, detail: 0 }
    }

    #[must_use]
    pub const fn with_detail(code: &'static str, detail: usize) -> Self {
        Self { code, detail }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.code.is_empty()
    }
}

impl From<&'static str> for SeparationNote {
    fn from(code: &'static str) -> Self {
        Self::new(code)
    }
}

impl std::fmt::Display for SeparationNote {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.detail == 0 {
            formatter.write_str(self.code)
        } else {
            write!(formatter, "{} ({})", self.code, self.detail)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparationDropReason {
    InitialSingleton,
    FixedEffectZeroLevel,
    Simplex,
    IteratedRectifier,
    Mu,
    SingletonCascade,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeparationCertificate {
    pub proof: SeparationCertificateProof,
    /// Maximum absolute direction value on positive-outcome observations,
    /// divided by the direction scale.  For a simplex dual proof, this is the
    /// normalized free-coordinate stationarity residual `||A'u||_inf`.
    pub interior_zero_relative: f64,
    /// Largest positive value on a zero-outcome observation, divided by the
    /// direction scale.  The canonical recession direction is nonpositive on
    /// the boundary.  For a simplex dual proof, this is the normalized
    /// nonnegative-variable dual-inequality violation.
    pub boundary_violation_relative: f64,
    pub span_residual_relative: f64,
    /// Smallest `-z_i / ||z||_inf` among proposed rows, or the normalized
    /// strict Farkas objective margin for a simplex dual proof.
    pub negative_margin_relative: f64,
    /// Infinity norm of the recession direction or dual multiplier.
    pub direction_scale: f64,
    pub positive_maximum: f64,
    pub passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparationCertificateProof {
    RecessionDirection,
    /// The positive-outcome finite design has zero nullity, so no nonzero
    /// direction can be zero on every positive-outcome observation.
    SimplexFullPositiveRank {
        columns: usize,
    },
    /// Phase-I proved that the nonnegative boundary cone with unit slack
    /// normalization is infeasible.  The raw dual stays detector-private;
    /// these dimensions and the scalar residuals bind the independently
    /// verified Farkas proof carried by the report.
    SimplexDualInfeasibility {
        boundary_rows: usize,
        nullity: usize,
    },
    /// The iterated-rectifier recurrence reached its certified zero fixed
    /// point under the registered recurrence and projection tolerances.
    IteratedRectifierFixedPoint,
}

/// Collision-free identity of one accepted zero-only absorbed-intercept
/// level.  The term id is invariant across prepared generations and the
/// representative is the least immutable original-row id among the active
/// members of that level.  The representative row is deleted by the same
/// atomic transaction, so an accepted identity cannot recur after rebasing,
/// even when a different source level reuses the same compact code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExactFeLevelIdentity {
    pub effect: crate::metadata::AbsorbTermId,
    pub representative_original_row: usize,
}

/// Exact evidence for zero-only absorbed-intercept levels.  Identities bind
/// immutable input rows and requested absorbed terms, never detector-local
/// compact level codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactFeSeparationCertificate {
    pub checked_intercept_effects: usize,
    pub zero_only_levels: Vec<ExactFeLevelIdentity>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeparationMethodReport {
    /// Sample-ledger generation on which this result became authoritative.
    /// Reports that accept no rows use the generation they inspected.
    pub generation: usize,
    pub method: SeparationMethod,
    pub status: MethodStatus,
    pub iterations: usize,
    pub dropped_observations: Vec<usize>,
    pub singleton_drops: Vec<usize>,
    pub certificate: Option<SeparationCertificate>,
    pub exact_fe_certificate: Option<ExactFeSeparationCertificate>,
    /// Wall-clock seconds spent in this detector execution, including its
    /// independent certificate and atomic ledger commit. Observational only.
    pub elapsed_seconds: f64,
    /// Registered maximum number of observation-length floating-point
    /// workspace vectors used by this method. This is available before the
    /// detector runs and feeds PPML memory preflight.
    pub workspace_observation_vectors: usize,
    /// Registered simplex-tableau cell ceiling for this execution; zero for
    /// non-simplex methods or when simplex is disabled.
    pub simplex_tableau_cells: usize,
    pub note: SeparationNote,
}

/// Allocation-free detector workspace requirements for PPML preflight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeparationWorkspaceRequirements {
    pub observation_vectors: usize,
    pub simplex_tableau_cells: usize,
}

/// Return the largest registered detector workspace implied by `config`.
/// This function performs no allocation and must run before detector scratch
/// is constructed.
#[must_use]
pub const fn separation_workspace_requirements(
    config: SeparationConfig,
) -> SeparationWorkspaceRequirements {
    let mut observation_vectors = 0_usize;
    // The exact FE scan uses one compact-axis selection mask so rows that
    // belong to zero-only levels in multiple absorbed effects are emitted
    // once without allowing an unreserved proposal vector to grow.
    if config.methods.fixed_effects && observation_vectors < 1 {
        observation_vectors = 1;
    }
    if config.methods.iterated_rectifier && observation_vectors < 8 {
        observation_vectors = 8;
    }
    if config.methods.mu && observation_vectors < 8 {
        observation_vectors = 8;
    }
    SeparationWorkspaceRequirements {
        observation_vectors,
        simplex_tableau_cells: if config.methods.simplex {
            config.simplex_max_tableau_cells
        } else {
            0
        },
    }
}

/// All-positive Poisson outcomes admit no separation deletion. Recursive
/// singleton pruning reaches its fixed point in one initial commit, and the
/// remaining generation only replays the enabled policies. Samples containing
/// any zero retain the general one-generation-per-row envelope.
pub(crate) const fn separation_generation_capacity(
    observations: usize,
    all_positive: bool,
    drop_singletons: bool,
) -> usize {
    if all_positive {
        let bound = 1 + drop_singletons as usize;
        if observations < bound {
            observations
        } else {
            bound
        }
    } else {
        observations
    }
}

/// Number of independently requested separation policies. Each immutable
/// generation executes a policy at most once before either committing a
/// deletion or reaching the terminal stable generation. This count freezes
/// the `policies * admitted_generations` report-record envelope.
#[must_use]
pub const fn separation_method_count(config: SeparationConfig) -> usize {
    config.drop_singletons as usize
        + config.methods.fixed_effects as usize
        + config.methods.simplex as usize
        + config.methods.iterated_rectifier as usize
        + config.methods.mu as usize
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeparationReport {
    pub candidate_observations: usize,
    pub active: Vec<bool>,
    pub retained_observations: Vec<usize>,
    pub dropped_observations: Vec<usize>,
    pub drop_reasons: Vec<Option<SeparationDropReason>>,
    pub singleton_count: usize,
    pub separated_count: usize,
    pub generations: usize,
    /// Deterministic FNV-1a hashes of the initial and every committed active
    /// mask/reason generation.  These are diagnostics, not cryptographic IDs.
    pub generation_hashes: Vec<u64>,
    pub methods: Vec<SeparationMethodReport>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DetectionProposal {
    pub method: SeparationMethod,
    /// Detector-local row numbers until the proposal is mapped to stable
    /// original observation identifiers by the coordinator.
    pub dropped: Vec<usize>,
    pub iterations: usize,
    pub status: MethodStatus,
    pub certificate: Option<SeparationCertificate>,
    pub exact_fe_certificate: Option<ExactFeSeparationCertificate>,
    /// Canonically signed observation-space direction on the immutable
    /// detector view.  It is required for every numerical deletion proposal.
    pub direction: Option<Vec<f64>>,
    pub note: SeparationNote,
}

impl DetectionProposal {
    pub(crate) fn empty(
        method: SeparationMethod,
        status: MethodStatus,
        note: impl Into<SeparationNote>,
    ) -> Self {
        Self {
            method,
            dropped: Vec::new(),
            iterations: 0,
            status,
            certificate: None,
            exact_fe_certificate: None,
            direction: None,
            note: note.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sequence_excludes_heuristic_mu() {
        let methods = SeparationMethods::default();
        assert!(methods.fixed_effects && methods.simplex && methods.iterated_rectifier);
        assert!(!methods.mu);
    }
}
#[test]
fn prepared_simplex_request_registers_its_bounded_tableau() {
    let simplex_only = separation_workspace_requirements(SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: false,
            mu: false,
        },
        ..SeparationConfig::default()
    });
    assert_eq!(simplex_only.observation_vectors, 0);
    assert_eq!(simplex_only.simplex_tableau_cells, 8_000_000);

    let with_ir = separation_workspace_requirements(SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: true,
            mu: false,
        },
        ..SeparationConfig::default()
    });
    assert_eq!(with_ir.observation_vectors, 8);
    assert_eq!(with_ir.simplex_tableau_cells, 8_000_000);

    let fe_only = separation_workspace_requirements(SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: true,
            simplex: false,
            iterated_rectifier: false,
            mu: false,
        },
        ..SeparationConfig::default()
    });
    assert_eq!(fe_only.observation_vectors, 1);
    assert_eq!(fe_only.simplex_tableau_cells, 0);
}
