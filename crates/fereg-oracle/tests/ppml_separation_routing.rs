#![deny(clippy::too_many_lines)]
//! Hidden `inject=ppml_separation_auto` against the public detector solver.
//!
//! The hidden arm solves admitted two-intercept detector projections with the
//! OLS two-FE Auto route instead of retained-product CG; every detector rule
//! is unchanged. Decision identity requires equal separated sets, drop
//! reasons, generation hashes, method order, statuses, iterations, certificate
//! verdicts and published fit bits; certificate magnitudes are solver
//! diagnostics. On this small seeded corpus (weakly connected chains, sparse
//! AKM graphs and induced finite-regressor separation at the public
//! `septolerance(1e-8)`) identity does NOT hold on every case: the census
//! below pins each divergence class so the evidence is reproducible and any
//! change to either arm is seen (VALIDATION, PPML separation routing).
#[allow(dead_code)]
mod support;

use fereg_core::ppml::{fit_prepared_ppml, CertifiedPpmlResult};
use fereg_core::{
    DropReason, FeregError, MethodStatus, SeparationConfig, SeparationMethod, SeparationMethods,
    SeparationReport, ThreadConfig, ThreadTeam,
};
use fereg_oracle::designs::{generate, Design, DesignSpec, Graph, WeightKind};

fn fit(
    design: &Design,
    config: SeparationConfig,
    injected: bool,
    threads: usize,
) -> Result<CertifiedPpmlResult, FeregError> {
    let (mut request, loaded) = support::ppml::request(design, None, false, config);
    let team = ThreadConfig::from_requested(threads, false).expect("threads");
    request.linear.execution.threads = team;
    request.inject_separation_auto = injected;
    ThreadTeam::new(team)
        .install(|| fit_prepared_ppml(request, loaded))
        .expect("team")
}

/// Every discrete separation decision, without certificate magnitudes or time.
fn decisions(report: &SeparationReport) -> String {
    let methods: Vec<_> = report
        .methods
        .iter()
        .map(|entry| {
            (
                entry.generation,
                entry.method,
                entry.status,
                entry.iterations,
                entry.dropped_observations.clone(),
                entry.singleton_drops.clone(),
                entry.certificate.as_ref().map(|c| (c.proof, c.passed)),
                entry.exact_fe_certificate.clone(),
            )
        })
        .collect();
    format!(
        "{:?}",
        (
            &report.retained_observations,
            &report.dropped_observations,
            &report.drop_reasons,
            (report.singleton_count, report.separated_count),
            (report.generations, &report.generation_hashes),
            methods,
        )
    )
}

/// Separation decisions, final sample and every published coefficient and
/// covariance bit (the fit after separation does not use detector state).
fn published(fit: &CertifiedPpmlResult) -> (String, Vec<DropReason>, Vec<u64>, Vec<u64>) {
    (
        decisions(fit.separation()),
        fit.sample().drop_reasons().to_vec(),
        fit.coefficients()
            .values()
            .iter()
            .map(|v| v.to_bits())
            .collect(),
        fit.inference()
            .vce
            .covariance
            .iter()
            .map(|v| v.to_bits())
            .collect(),
    )
}

/// The public `feppml` separation tolerances: `septolerance(1e-8)`,
/// `sepiterate(10000)` (`crates/fereg-plugin/src/ppml_estimate.rs`).
fn public_separation(methods: SeparationMethods) -> SeparationConfig {
    let tolerance: f64 = 1e-8;
    SeparationConfig {
        methods,
        tolerance,
        certificate_tolerance: tolerance.min(1e-7),
        projection_tolerance: tolerance.min(1e-10),
        max_ir_iterations: 10_000,
        ..SeparationConfig::default()
    }
}

fn configurations() -> [SeparationConfig; 3] {
    let methods = |simplex, fixed_effects| SeparationMethods {
        fixed_effects,
        simplex,
        iterated_rectifier: true,
        mu: false,
    };
    [
        public_separation(methods(true, true)),
        public_separation(methods(false, true)),
        public_separation(methods(false, false)),
    ]
}

/// Seeded two-FE Poisson designs with zeros; odd seeds add a finite regressor
/// that is positive only on some zero rows, which only a numerical detector
/// (simplex or the rectifier's projection) can separate.
fn designs() -> Vec<(String, Design)> {
    let mut designs = Vec::new();
    for seed in 0..18_u64 {
        let (graph, rows, levels) = match seed % 3 {
            0 => (Graph::DensePanel, 240, vec![12, 10]),
            1 => (Graph::AkmCampaign, 1_000, vec![200, 40]),
            _ => (Graph::Chain, 300, vec![30, 20]),
        };
        let spec = DesignSpec {
            rows,
            levels,
            regressors: 2,
            graph,
            weights: [
                WeightKind::Unit,
                WeightKind::Frequency,
                WeightKind::Probability,
            ][(seed as usize / 3) % 3],
            poisson: true,
            zero_fraction: [0.2, 0.45, 0.7][(seed as usize / 2) % 3],
            ..DesignSpec::default()
        };
        let mut design = generate(seed, &spec).expect("design");
        if seed % 2 == 1 {
            let mut zeros = 0;
            for row in 0..design.outcome.len() {
                design.regressors[1][row] = 0.0;
                if design.outcome[row] == 0.0 {
                    zeros += 1;
                    if zeros % 5 == 0 {
                        design.regressors[1][row] = 1.0 + (row % 3) as f64;
                    }
                }
            }
        }
        designs.push((format!("seed {seed} {graph:?}"), design));
    }
    designs
}

/// Outcome class of one public/hidden pair.
fn classify(
    public: &Result<CertifiedPpmlResult, FeregError>,
    hidden: &Result<CertifiedPpmlResult, FeregError>,
) -> &'static str {
    match (public, hidden) {
        (Ok(p), Ok(h)) if published(p) == published(h) => "same",
        (Ok(p), Ok(h)) => {
            // Equal sample and fit; only a terminal no-separation rectifier
            // execution's iteration count and certificate differ.
            let without_terminal_ir = |fit: &CertifiedPpmlResult| {
                let mut report = fit.separation().clone();
                for entry in &mut report.methods {
                    if entry.method == SeparationMethod::IteratedRectifier
                        && entry.status == MethodStatus::NoSeparation
                    {
                        entry.iterations = 0;
                        entry.certificate = None;
                    }
                }
                decisions(&report)
            };
            let (_, p_reasons, p_b, p_v) = published(p);
            let (_, h_reasons, h_b, h_v) = published(h);
            if (p_reasons, p_b, p_v) == (h_reasons, h_b, h_v)
                && without_terminal_ir(p) == without_terminal_ir(h)
            {
                "ir-iterations"
            } else {
                "other"
            }
        }
        (Err(p), Err(h)) if p.code == h.code && p.message == h.message => "same",
        (Err(_), Err(_)) => "both-fail",
        (Err(_), Ok(_)) => "public-fails",
        (Ok(_), Err(_)) => "hidden-fails",
    }
}

/// Every (seed, configuration) pair whose class is not "same" at one worker,
/// observed 2026-10-27. Configuration 0 is the public `fe simplex ir` chain.
/// "ir-iterations": equal sample, drops and fit bits; the terminal rectifier
/// certifies at iteration 1 instead of CG's 2. "public-fails": retained CG's
/// projection fails a certificate and the public fit errors. "hidden-fails":
/// the Auto route exhausts its iteration budget. Where both arms succeed the
/// separated set never differs.
const DIVERGENCES: &[(usize, usize, &str)] = &[
    (2, 0, "ir-iterations"),
    (2, 1, "ir-iterations"),
    (2, 2, "ir-iterations"),
    (3, 1, "both-fail"),
    (3, 2, "both-fail"),
    (5, 0, "ir-iterations"),
    (5, 1, "public-fails"),
    (5, 2, "public-fails"),
    (8, 0, "ir-iterations"),
    (8, 1, "ir-iterations"),
    (8, 2, "ir-iterations"),
    (9, 1, "hidden-fails"),
    (9, 2, "hidden-fails"),
    (10, 0, "public-fails"),
    (11, 0, "ir-iterations"),
    (11, 1, "public-fails"),
    (11, 2, "public-fails"),
    (14, 0, "public-fails"),
    (16, 0, "public-fails"),
    (17, 0, "ir-iterations"),
    (17, 1, "public-fails"),
    (17, 2, "public-fails"),
];

#[test]
fn hidden_auto_projection_divergence_census_is_pinned() {
    let mut observed = Vec::new();
    for (seed, (_, design)) in designs().into_iter().enumerate() {
        for (index, config) in configurations().into_iter().enumerate() {
            let class = classify(
                &fit(&design, config, false, 1),
                &fit(&design, config, true, 1),
            );
            if class != "same" {
                observed.push((seed, index, class));
            }
        }
    }
    assert_eq!(observed, DIVERGENCES);
}

#[test]
fn hidden_auto_projection_repeats_and_agrees_across_workers() {
    for (name, design) in designs() {
        for (index, config) in configurations().into_iter().enumerate() {
            let Ok(reference) = fit(&design, config, true, 1) else {
                continue;
            };
            let reference = published(&reference);
            for threads in [2, 4, 8] {
                let first = fit(&design, config, true, threads).expect("hidden fit");
                let second = fit(&design, config, true, threads).expect("hidden fit");
                let certificates = |fit: &CertifiedPpmlResult| {
                    format!(
                        "{:?}",
                        fit.separation()
                            .methods
                            .iter()
                            .map(|entry| &entry.certificate)
                            .collect::<Vec<_>>()
                    )
                };
                let context = format!("{name} config {index} threads {threads}");
                // Fixed workers: every bit, certificate magnitudes included.
                assert_eq!(certificates(&first), certificates(&second), "{context}");
                assert_eq!(published(&first), published(&second), "{context}");
                // Across workers: identical decisions and final sample.
                let (decisions, reasons, _, _) = published(&first);
                assert_eq!(decisions, reference.0, "{context}");
                assert_eq!(reasons, reference.1, "{context}");
            }
        }
    }
}
