//! Statistical-separation detection and exact retained-sample accounting.
//!
//! The default sequence mirrors ppmlhdfe's public default: exact FE levels,
//! finite-regressor simplex, then the comprehensive iterated rectifier.  Every
//! numerical method must return a column-space/sign certificate before any row
//! is dropped; nonconvergence therefore fails closed.

mod fe;
mod ir;
mod mu;
mod projection;
mod runner;
mod simplex;
mod subset;

use super::PpmlResult;

pub use runner::{detect_separation, fit_ppml_separated, fit_ppml_separated_with_start};

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
    pub interior_zero_relative: f64,
    pub boundary_violation_relative: f64,
    pub span_residual_relative: f64,
    pub positive_maximum: f64,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeparationMethodReport {
    pub method: SeparationMethod,
    pub status: MethodStatus,
    pub iterations: usize,
    pub dropped_observations: Vec<usize>,
    pub singleton_drops: Vec<usize>,
    pub certificate: Option<SeparationCertificate>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeparationReport {
    pub active: Vec<bool>,
    pub retained_observations: Vec<usize>,
    pub dropped_observations: Vec<usize>,
    pub drop_reasons: Vec<Option<SeparationDropReason>>,
    pub methods: Vec<SeparationMethodReport>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeparatedPpmlResult {
    pub estimation: PpmlResult,
    pub separation: SeparationReport,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DetectionResult {
    pub dropped: Vec<usize>,
    pub iterations: usize,
    pub status: MethodStatus,
    pub certificate: Option<SeparationCertificate>,
    pub note: String,
}

impl DetectionResult {
    pub(crate) fn empty(status: MethodStatus, note: impl Into<String>) -> Self {
        Self {
            dropped: Vec::new(),
            iterations: 0,
            status,
            certificate: None,
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
    use crate::ppml::PpmlConfig;

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
}
