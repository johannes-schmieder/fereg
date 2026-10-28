#![deny(clippy::too_many_lines)]
//! The default two-FE Auto detector route against the retained-CG arm.
//!
//! Since beta.10 admitted two-intercept detector projections use the OLS
//! two-FE Auto route by default (a failed Auto step re-executes as retained
//! CG); `inject=ppml_separation_cg` keeps retained-product CG, the earlier
//! public path, as a developer comparison arm. Every detector rule is the
//! same in both. Decision identity requires equal separated sets, drop
//! reasons, generation hashes, method order, statuses, iterations,
//! certificate verdicts and published fit bits; certificate magnitudes are
//! solver diagnostics. On this small seeded corpus (weakly connected chains,
//! sparse AKM graphs and induced finite-regressor separation at the public
//! `septolerance(1e-8)`) the arms agree in sample, drops and fit wherever
//! both complete; the census below pins the cells whose terminal rectifier
//! certifies at a different iteration, and the two seed-3 cells only the Auto
//! arm completes, so any change to either arm is seen (VALIDATION, PPML
//! separation routing).
//!
//! Every arm that completes also agrees with an independent witness: the
//! retained sample of a HiGHS linear program (maximal recession-cone
//! separation, iterated with singleton pruning to a fixed point) on each
//! seeded design, pinned below.
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
    auto_route: bool,
    threads: usize,
) -> Result<CertifiedPpmlResult, FeregError> {
    let (mut request, loaded) = support::ppml::request(design, None, false, config);
    let team = ThreadConfig::from_requested(threads, false).expect("threads");
    request.linear.execution.threads = team;
    request.inject_separation_cg = !auto_route;
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

/// Outcome class of one retained-CG/Auto pair.
fn classify(
    cg: &Result<CertifiedPpmlResult, FeregError>,
    auto: &Result<CertifiedPpmlResult, FeregError>,
) -> &'static str {
    match (cg, auto) {
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
        (Err(_), Ok(_)) => "cg-fails",
        (Ok(_), Err(_)) => "auto-fails",
    }
}

/// Every (seed, configuration) pair whose class is not "same" at one worker.
/// Configuration 0 is the public `fe simplex ir` chain. "ir-iterations":
/// equal sample, drops and fit bits; only a terminal no-separation rectifier
/// execution's iteration count and certificate differ. Where both arms
/// succeed the separated set never differs.
///
/// Before the retained-CG detector re-executed failed steps with LSMR
/// (2026-10-27), nine more cells were "cg-fails": (5, 1), (5, 2), (11, 1),
/// (11, 2), (17, 1) and (17, 2) now "ir-iterations", and (10, 0), (14, 0) and
/// (16, 0) now "same". Before a failed Auto step re-executed as retained CG
/// (2026-10-28), (9, 1) and (9, 2) were "auto-fails" (a two-FE PCG final
/// residual at the categorical certificate's rounding floor; MAP could not
/// move it) and (3, 1) and (3, 2) "both-fail"; all four became "same", the
/// seed-3 cells with the retained-CG error in both arms. Since the Auto route
/// became the default (beta.10) the first arm is `inject=ppml_separation_cg`.
/// Since an Auto detector whose every attempt fails re-checks its two-FE PCG
/// miss about each level's largest-weight member (2026-10-28), (3, 1) and
/// (3, 2) complete in the Auto arm with the LP-witness sample, while the
/// retained-CG arm keeps its error: "cg-fails".
const DIVERGENCES: &[(usize, usize, &str)] = &[
    (2, 0, "ir-iterations"),
    (2, 1, "ir-iterations"),
    (2, 2, "ir-iterations"),
    (3, 1, "cg-fails"),
    (3, 2, "cg-fails"),
    (5, 0, "ir-iterations"),
    (5, 1, "ir-iterations"),
    (5, 2, "ir-iterations"),
    (8, 0, "ir-iterations"),
    (8, 1, "ir-iterations"),
    (8, 2, "ir-iterations"),
    (11, 0, "ir-iterations"),
    (11, 1, "ir-iterations"),
    (11, 2, "ir-iterations"),
    (17, 0, "ir-iterations"),
    (17, 1, "ir-iterations"),
    (17, 2, "ir-iterations"),
];

/// Independent witness per seeded design: the number of retained rows and the
/// FNV-1a fold (`h ^= row; h *= 0x100000001b3`) of their sorted indices from a
/// HiGHS LP (scipy 1.13.1) that maximizes the number of zero-outcome rows a
/// span direction makes strictly negative while it is zero on positive
/// outcomes and nonpositive on zeros, alternated with singleton pruning until
/// neither drops a row. It shares no code with fereg.
const LP_WITNESS: [(usize, u64); 18] = [
    (240, 0xf2f9_72d8_a87c_5ad5),
    (962, 0x6422_fcd7_2286_4b54),
    (300, 0xf90b_c441_25dc_3a99),
    (220, 0x7146_8d5c_595e_2390),
    (856, 0x7bfe_8ec9_0305_b28f),
    (209, 0x03dc_7ce4_053c_61a7),
    (240, 0xf2f9_72d8_a87c_5ad5),
    (960, 0x4d9b_eeb3_32a9_6287),
    (300, 0xf90b_c441_25dc_3a99),
    (219, 0xca95_914c_4b52_a12a),
    (805, 0x65b9_c4b9_4448_fad7),
    (194, 0x921b_823a_1a63_f600),
    (240, 0xf2f9_72d8_a87c_5ad5),
    (960, 0xcc38_130b_dae1_4a1f),
    (295, 0x6684_2172_f171_6f24),
    (220, 0x88a9_92be_76d6_15ba),
    (806, 0x778a_8c73_5c7d_dd84),
    (228, 0xaf35_96f7_b415_6b10),
];

fn retained_witness(fit: &CertifiedPpmlResult) -> (usize, u64) {
    let mut count = 0;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for (row, reason) in fit.sample().drop_reasons().iter().enumerate() {
        if *reason == DropReason::Retained {
            count += 1;
            hash = (hash ^ row as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    (count, hash)
}

#[test]
fn auto_projection_divergence_census_is_pinned() {
    let mut observed = Vec::new();
    for (seed, (name, design)) in designs().into_iter().enumerate() {
        for (index, config) in configurations().into_iter().enumerate() {
            let (cg, auto) = (
                fit(&design, config, false, 1),
                fit(&design, config, true, 1),
            );
            for arm in [&cg, &auto].into_iter().flatten() {
                let context = format!("{name} config {index}");
                assert_eq!(retained_witness(arm), LP_WITNESS[seed], "{context}");
            }
            let class = classify(&cg, &auto);
            if class != "same" {
                observed.push((seed, index, class));
            }
        }
    }
    assert_eq!(observed, DIVERGENCES);
}

/// Both arms, including fits that re-execute a failed Auto step as retained
/// CG, a failed retained-CG step with LSMR, or a failed Auto chain with the
/// largest-weight re-check, repeat bit for bit at a fixed worker count and keep their
/// decisions and sample across 1, 2, 4 and 8 workers.
#[test]
fn both_arms_repeat_and_agree_across_workers() {
    for (name, design) in designs() {
        for ((index, config), injected) in configurations()
            .into_iter()
            .enumerate()
            .flat_map(|cell| [(cell, false), (cell, true)])
        {
            let Ok(reference) = fit(&design, config, injected, 1) else {
                continue;
            };
            let reference = published(&reference);
            for threads in [2, 4, 8] {
                let first = fit(&design, config, injected, threads).expect("repeated fit");
                let second = fit(&design, config, injected, threads).expect("repeated fit");
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
                let context = format!("{name} config {index} auto {injected} threads {threads}");
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

/// One census cell's outcome in both arms, for diagnosis:
/// `FEREG_SEPARATION_CENSUS_CELL=<seed>:<configuration> cargo test -p
/// fereg-oracle --test ppml_separation_routing census_cell -- --ignored
/// --nocapture` (add `FEREG_PHASE_TIMING=1` for the rectifier and re-execution
/// laps).
#[test]
#[ignore = "diagnostic; selects one census cell from FEREG_SEPARATION_CENSUS_CELL"]
fn census_cell() {
    let cell = std::env::var("FEREG_SEPARATION_CENSUS_CELL").expect("<seed>:<configuration>");
    let (seed, index) = cell.split_once(':').expect("<seed>:<configuration>");
    let (seed, index): (usize, usize) =
        (seed.parse().expect("seed"), index.parse().expect("index"));
    let (name, design) = designs().swap_remove(seed);
    for injected in [false, true] {
        match fit(&design, configurations()[index], injected, 1) {
            Ok(fit) => eprintln!(
                "{name} config {index} auto {injected}: retained {:?}, witness {:?}, {}",
                retained_witness(&fit),
                LP_WITNESS[seed],
                published(&fit).0
            ),
            Err(error) => eprintln!(
                "{name} config {index} auto {injected}: {:?} {}",
                error.code, error.message
            ),
        }
    }
}
