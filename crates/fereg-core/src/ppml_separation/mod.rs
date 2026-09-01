//! Statistical-separation detection and exact retained-sample accounting.
//!
//! The prepared production sequence runs exact FE levels and comprehensive
//! iterated rectifier. The prepared simplex materializes a bounded explicit
//! finite-plus-FE design (including absorbed slope channels) and refuses its
//! design or tableau before allocation when the configured cell cap is
//! exceeded. Every numerical drop carries a full-model certificate.

#[cfg(test)]
mod fe;
#[cfg(test)]
mod ir;
#[cfg(test)]
mod ledger;
#[cfg(test)]
mod mu;
mod prepared_coordinator;
#[cfg(test)]
mod projection;
#[cfg(test)]
mod runner;
mod simplex;
pub(crate) use simplex::{model_simplex_interruptible, simplex_workspace_peak_cells};
#[cfg(test)]
mod subset;

#[allow(unused_imports)]
pub(crate) use prepared_coordinator::{
    PreparedGenerationCommit, PreparedSeparationCascade, PreparedSeparationCoordinator,
    PreparedSeparationGeneration, PreparedSeparationOutcome, PreparedSeparationRebuilder,
};
// The dense `FeDesign` detector runner is retained only as a scientific oracle
// for the module's regression tests. Production PPML enters through
// `ppml::fit_prepared_ppml` and the immutable prepared-generation coordinator.
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use runner::{detect_separation, detect_separation_with_multiplicity};

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
    /// divided by the direction scale.
    pub interior_zero_relative: f64,
    /// Largest positive value on a zero-outcome observation, divided by the
    /// direction scale.  The canonical recession direction is nonpositive on
    /// the boundary.
    pub boundary_violation_relative: f64,
    pub span_residual_relative: f64,
    /// Smallest `-z_i / ||z||_inf` among proposed rows.
    pub negative_margin_relative: f64,
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
    /// The iterated-rectifier recurrence reached its certified zero fixed
    /// point under the registered recurrence and projection tolerances.
    IteratedRectifierFixedPoint,
}

/// Exact evidence for zero-only absorbed-intercept levels.  Level identifiers
/// refer to the immutable input design, not to detector-local compact order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactFeSeparationCertificate {
    pub checked_intercept_effects: usize,
    pub zero_only_levels: Vec<(usize, u32)>,
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
    pub note: String,
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
    pub note: String,
}

impl DetectionProposal {
    pub(crate) fn empty(
        method: SeparationMethod,
        status: MethodStatus,
        note: impl Into<String>,
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
    use super::subset::subset_fixed_effects;
    use super::*;
    use crate::data::DenseMatrix;
    use crate::fe::{EffectTerm, FeDesign};
    use crate::ppml::{PpmlConfig, PpmlStart};

    #[test]
    fn default_sequence_excludes_heuristic_mu() {
        let methods = SeparationMethods::default();
        assert!(methods.fixed_effects && methods.simplex && methods.iterated_rectifier);
        assert!(!methods.mu);
    }

    #[test]
    fn exact_fe_method_preserves_original_drop_identities() {
        let outcome = [0.0, 0.0, 0.0, 1.0, 2.0, 3.0];
        let regressors = DenseMatrix::zeros(outcome.len(), 0).expect("matrix");
        let effect = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("effect");
        let design = FeDesign::new(outcome.len(), vec![effect]).expect("design");
        let report = detect_separation(
            &outcome,
            &regressors,
            Some(&design),
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: true,
                    simplex: false,
                    iterated_rectifier: false,
                    mu: false,
                },
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            None,
        )
        .expect("separation");
        assert_eq!(report.dropped_observations, vec![0, 1]);
        assert_eq!(report.retained_observations, vec![2, 3, 4, 5]);
        assert_eq!(
            report.drop_reasons[0],
            Some(SeparationDropReason::FixedEffectZeroLevel)
        );
    }

    #[test]
    fn simplex_reproduces_the_two_regressor_primer() {
        let outcome = [0.0, 0.0, 0.0, 1.0, 2.0, 3.0];
        let regressors = DenseMatrix::from_columns(&[
            vec![2.0, -1.0, 0.0, 0.0, 5.0, 6.0],
            vec![-1.0, 2.0, 0.0, 0.0, -10.0, -12.0],
        ])
        .expect("matrix");
        let report = detect_separation(
            &outcome,
            &regressors,
            None,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: false,
                    simplex: true,
                    iterated_rectifier: false,
                    mu: false,
                },
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            None,
        )
        .expect("separation");
        assert_eq!(report.dropped_observations, vec![0]);
        assert!(report.methods[0]
            .certificate
            .as_ref()
            .is_some_and(|certificate| certificate.passed));
    }

    #[test]
    fn iterated_rectifier_finds_combined_two_way_fe_separation() {
        let outcome = [0.0, 1.0, 0.0, 0.0, 1.0];
        let regressors = DenseMatrix::zeros(outcome.len(), 0).expect("matrix");
        let first = EffectTerm::new(vec![0, 0, 1, 1, 1], 2, true, vec![]).expect("first");
        let second = EffectTerm::new(vec![0, 0, 0, 1, 1], 2, true, vec![]).expect("second");
        let design = FeDesign::new(outcome.len(), vec![first, second]).expect("design");
        let report = detect_separation(
            &outcome,
            &regressors,
            Some(&design),
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: true,
                    simplex: false,
                    iterated_rectifier: true,
                    mu: false,
                },
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            None,
        )
        .expect("separation");
        assert_eq!(report.dropped_observations, vec![2]);
        assert_eq!(
            report.drop_reasons[2],
            Some(SeparationDropReason::IteratedRectifier)
        );
    }

    #[test]
    fn subsetting_reencodes_effect_levels_contiguously() {
        let effect = EffectTerm::new(vec![0, 2, 2, 4], 5, true, vec![]).expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        let subset = subset_fixed_effects(&design, &[1, 2, 3]).expect("subset");
        assert_eq!(subset.effects()[0].codes(), &[0, 0, 1]);
        assert_eq!(subset.effects()[0].n_levels(), 2);
    }

    #[test]
    fn numerical_drop_singletons_and_exact_fe_followup_are_one_generation() {
        // Row 0 is separated by the finite direction.  Its deletion makes row
        // 1 a B singleton.  Removing row 1 then leaves A=1 with two zero-only
        // rows, which exact FE deletes in the same ledger transaction.
        let outcome = [0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let regressors =
            DenseMatrix::from_columns(&[vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]]).expect("matrix");
        let effect_a = EffectTerm::new(vec![0, 1, 1, 1, 0, 0, 0], 2, true, vec![]).expect("A");
        let effect_b = EffectTerm::new(vec![0, 0, 1, 1, 1, 2, 2], 3, true, vec![]).expect("B");
        let design = FeDesign::new(outcome.len(), vec![effect_a, effect_b]).expect("design");
        let report = detect_separation(
            &outcome,
            &regressors,
            Some(&design),
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: true,
                    simplex: true,
                    iterated_rectifier: false,
                    mu: false,
                },
                ..SeparationConfig::default()
            },
            None,
        )
        .expect("fixed point");
        assert_eq!(report.generations, 1);
        assert_eq!(report.generation_hashes.len(), 2);
        assert_eq!(report.dropped_observations, vec![0, 1, 2, 3, 4]);
        assert_eq!(report.singleton_count, 2);
        assert_eq!(report.separated_count, 3);
        assert_eq!(report.drop_reasons[0], Some(SeparationDropReason::Simplex));
        assert_eq!(
            report.drop_reasons[1],
            Some(SeparationDropReason::SingletonCascade)
        );
        assert_eq!(
            &report.drop_reasons[2..4],
            &[
                Some(SeparationDropReason::FixedEffectZeroLevel),
                Some(SeparationDropReason::FixedEffectZeroLevel),
            ]
        );
        assert_eq!(
            report.drop_reasons[4],
            Some(SeparationDropReason::SingletonCascade)
        );
        let simplex = report
            .methods
            .iter()
            .find(|entry| {
                entry.method == SeparationMethod::Simplex && !entry.dropped_observations.is_empty()
            })
            .expect("simplex report");
        assert_eq!(simplex.generation, 1);
        assert_eq!(simplex.dropped_observations, vec![0]);
        assert_eq!(simplex.singleton_drops, vec![1, 4]);
        let exact_followup = report
            .methods
            .iter()
            .find(|entry| {
                entry.method == SeparationMethod::FixedEffects
                    && entry.generation == 1
                    && !entry.dropped_observations.is_empty()
            })
            .expect("exact follow-up");
        assert_eq!(exact_followup.dropped_observations, vec![2, 3]);
        assert!(exact_followup
            .exact_fe_certificate
            .as_ref()
            .is_some_and(|certificate| certificate.passed));
    }

    #[test]
    fn no_separation_is_identity_with_a_stable_generation_hash() {
        let outcome = [0.0, 1.0, 2.0];
        let regressors = DenseMatrix::from_columns(&[vec![1.0, 1.0, 1.0]]).expect("matrix");
        let config = SeparationConfig {
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        let first = detect_separation(
            &outcome,
            &regressors,
            None,
            PpmlConfig::default(),
            config,
            None,
        )
        .expect("first");
        let second = detect_separation(
            &outcome,
            &regressors,
            None,
            PpmlConfig::default(),
            config,
            None,
        )
        .expect("second");
        assert_eq!(first.active, vec![true, true, true]);
        assert_eq!(first.retained_observations, vec![0, 1, 2]);
        assert!(first.dropped_observations.is_empty());
        assert_eq!(first.generations, 0);
        assert_eq!(first.generation_hashes, second.generation_hashes);
    }

    #[test]
    fn all_zero_outcomes_fail_with_insufficient_observations() {
        let outcome = [0.0, 0.0];
        let regressors = DenseMatrix::zeros(2, 0).expect("matrix");
        let error = detect_separation(
            &outcome,
            &regressors,
            None,
            PpmlConfig::default(),
            SeparationConfig::default(),
            None,
        )
        .expect_err("all-zero outcome");
        assert_eq!(error.code, crate::ErrorCode::InsufficientObservations);
        assert_eq!(error.return_code(), 2001);
    }

    #[test]
    fn simplex_memory_skip_requires_comprehensive_ir() {
        let outcome = [0.0, 0.0, 1.0];
        let regressors = DenseMatrix::from_columns(&[vec![2.0, -1.0, 0.0], vec![-1.0, 2.0, 0.0]])
            .expect("matrix");
        let simplex_only = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: true,
                iterated_rectifier: false,
                mu: false,
            },
            simplex_max_tableau_cells: 1,
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        let error = detect_separation(
            &outcome,
            &regressors,
            None,
            PpmlConfig::default(),
            simplex_only,
            None,
        )
        .expect_err("simplex-only memory failure");
        assert_eq!(error.code, crate::ErrorCode::Allocation);

        let with_ir = SeparationConfig {
            methods: SeparationMethods {
                iterated_rectifier: true,
                ..simplex_only.methods
            },
            ..simplex_only
        };
        let report = detect_separation(
            &outcome,
            &regressors,
            None,
            PpmlConfig::default(),
            with_ir,
            None,
        )
        .expect("IR covers bounded simplex");
        assert!(report.methods.iter().any(|entry| {
            entry.method == SeparationMethod::Simplex && entry.status == MethodStatus::SkippedMemory
        }));
        assert_eq!(report.dropped_observations, vec![0, 1]);
    }

    #[test]
    fn optional_mu_drop_restarts_on_original_row_identity() {
        let outcome = [0.0, 1.0, 1.0];
        let regressors = DenseMatrix::from_columns(&[vec![1.0, 0.0, 0.0]]).expect("matrix");
        let start = PpmlStart {
            coefficients: vec![0.0],
            linear_predictor: vec![-100.0, 0.0, 0.0],
        };
        let report = detect_separation(
            &outcome,
            &regressors,
            None,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: false,
                    simplex: false,
                    iterated_rectifier: false,
                    mu: true,
                },
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            Some(&start),
        )
        .expect("mu separation");
        assert_eq!(report.dropped_observations, vec![0]);
        assert_eq!(report.generations, 1);
        assert_eq!(report.drop_reasons[0], Some(SeparationDropReason::Mu));
        assert!(report.methods.iter().any(|entry| {
            entry.method == SeparationMethod::Mu
                && entry.status == MethodStatus::Certified
                && entry.dropped_observations == vec![0]
        }));
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
