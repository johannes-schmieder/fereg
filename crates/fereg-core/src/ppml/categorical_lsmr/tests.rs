use super::*;
use crate::fe::EffectTerm;

fn test_team(threads: usize) -> InstalledThreadTeam {
    ThreadTeam::new(ThreadConfig::from_requested(threads, true).expect("thread config"))
        .acquire()
        .expect("installed thread team")
}

/// The engine on `design` (its codes) with `active` controls, as
/// `new_with_threads` builds it from a prepared fit.
fn engine(
    design: &FeDesign,
    active: DenseMatrix,
    threads: usize,
    preconditioner: LsmrPreconditioner,
) -> ReusableCategoricalJointLsmr<'_> {
    let effects: Vec<_> = design
        .effects()
        .iter()
        .map(|effect| fedemean::Effect::new(effect.codes(), effect.n_levels()))
        .collect();
    let view = fedemean::Design::new(design.nobs(), effects).expect("fedemean design");
    let slots = (0..active.ncols()).collect();
    ReusableCategoricalJointLsmr::assemble(
        test_team(threads),
        design,
        active,
        slots,
        view,
        preconditioner,
    )
    .expect("engine")
}

/// An independent dense weighted least-squares fit of `target` on the
/// controls and every level dummy (Gaussian elimination with complete
/// pivoting on the normal equations, dropping null pivots): the fitted
/// values and the controls' coefficients, both unique for an identified
/// design.
fn dense_oracle(
    design: &FeDesign,
    active: &DenseMatrix,
    weights: &[f64],
    target: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let rows = design.nobs();
    let k = active.ncols();
    let width = k + design.total_coefficients();
    let row = |i: usize| {
        let mut values = vec![0.0; width];
        values[..k].copy_from_slice(&active.as_slice()[i * k..(i + 1) * k]);
        let mut offset = k;
        for effect in design.effects() {
            values[offset + effect.codes()[i] as usize] = 1.0;
            offset += effect.n_levels();
        }
        values
    };
    let mut normal = vec![vec![0.0; width + 1]; width];
    for i in 0..rows {
        let values = row(i);
        for a in 0..width {
            for b in 0..width {
                normal[a][b] += weights[i] * values[a] * values[b];
            }
            normal[a][width] += weights[i] * values[a] * target[i];
        }
    }
    let scale = normal
        .iter()
        .enumerate()
        .map(|(a, r)| r[a].abs())
        .fold(0.0, f64::max);
    let mut order: Vec<usize> = (0..width).collect();
    let mut rank = 0;
    for pivot in 0..width {
        let (mut best, mut value) = (pivot, 0.0);
        for c in pivot..width {
            let v = normal[order[c]][order[c]].abs();
            if v > value {
                (best, value) = (c, v);
            }
        }
        if value <= 1e-11 * scale {
            break;
        }
        order.swap(pivot, best);
        let p = order[pivot];
        for &r in &order[pivot + 1..] {
            let factor = normal[r][p] / normal[p][p];
            for c in 0..=width {
                normal[r][c] -= factor * normal[p][c];
            }
        }
        rank += 1;
    }
    let mut theta = vec![0.0; width];
    for pivot in (0..rank).rev() {
        let p = order[pivot];
        let mut value = normal[p][width];
        for &c in &order[pivot + 1..rank] {
            value -= normal[p][c] * theta[c];
        }
        theta[p] = value / normal[p][p];
    }
    let fitted = (0..rows)
        .map(|i| row(i).iter().zip(&theta).map(|(x, t)| x * t).sum())
        .collect();
    (fitted, theta[..k].to_vec())
}

/// A connected three-effect design with two controls and a Poisson frame
/// whose correction is not in the design's span.
struct Fixture {
    design: FeDesign,
    active: DenseMatrix,
    outcome: Vec<f64>,
}

fn fixture(rows: usize, levels: [usize; 3], controls: usize) -> Fixture {
    let codes: Vec<Vec<u32>> = levels
        .iter()
        .enumerate()
        .map(|(e, &count)| {
            (0..rows)
                .map(|i| ((i * (7 + 4 * e) + i / (3 + e)) % count) as u32)
                .collect()
        })
        .collect();
    let design = FeDesign::new(
        rows,
        codes
            .iter()
            .zip(levels)
            .map(|(c, l)| EffectTerm::new(c.clone(), l, true, Vec::new()).expect("effect"))
            .collect(),
    )
    .expect("design");
    let values = (0..rows)
        .flat_map(|i| (0..controls).map(move |c| ((i * (c + 2) + 3 * c) as f64 * 0.37).sin()))
        .collect();
    let active = DenseMatrix::from_row_major(rows, controls, values).expect("controls");
    let outcome = (0..rows)
        .map(|i| {
            if i % 9 == 4 {
                0.0
            } else {
                1.0 + ((i * 13) % 7) as f64 / 3.0
            }
        })
        .collect();
    Fixture {
        design,
        active,
        outcome,
    }
}

fn frame_weights(rows: usize, phase: f64) -> Vec<f64> {
    (0..rows)
        .map(|i| 0.6 + 0.5 * ((i as f64) * 0.11 + phase).cos().abs())
        .collect()
}

fn max_abs_difference(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max)
}

#[test]
fn frames_match_the_dense_joint_oracle_under_every_preconditioner() {
    let fixture = fixture(240, [17, 11, 5], 2);
    for preconditioner in [
        LsmrPreconditioner::Auto,
        LsmrPreconditioner::Off,
        LsmrPreconditioner::Diagonal,
        LsmrPreconditioner::Schwarz,
    ] {
        let mut solver = engine(&fixture.design, fixture.active.clone(), 2, preconditioner);
        for (frame, phase) in [0.0, 0.4, 0.9].into_iter().enumerate() {
            let weights = frame_weights(240, phase);
            let mean = weights.clone();
            let report = solver
                .solve(
                    &fixture.design,
                    &fixture.outcome,
                    &mean,
                    &weights,
                    1e-11,
                    10_000,
                )
                .expect("certified frame");
            assert!(report.certificate.passed);
            assert!(report.certificate.relative_infinity_norm <= 1e-11);
            assert_eq!(report.warm_start_attempted, frame > 0);
            assert_eq!(
                report.formulation,
                if preconditioner == LsmrPreconditioner::Schwarz {
                    Formulation::Projections
                } else {
                    Formulation::Joint
                }
            );
            assert_eq!(report.route.reduction, crate::fe::FeReduction::Full);
            assert_eq!(report.route.method, crate::fe::FeMethod::Lsmr);
            assert!(report.iterations > 0);
            let target: Vec<f64> = fixture
                .outcome
                .iter()
                .zip(&mean)
                .map(|(&y, &mu)| if y == 0.0 { -1.0 } else { y / mu - 1.0 })
                .collect();
            let (fitted, coefficients) =
                dense_oracle(&fixture.design, &fixture.active, &weights, &target);
            assert!(max_abs_difference(&solver.delta_eta, &fitted) <= 1e-9);
            assert!(max_abs_difference(&solver.step, &coefficients) <= 1e-9);
            // The next frame starts from this one (the joint state at the
            // accepted step).
            solver.accept_step(1.0);
        }
        solver.reset_warm_start();
        assert!(!solver.joint.warm_start_valid);
        assert!(solver
            .projections
            .as_ref()
            .is_none_or(|p| !p.warm_start_valid));
    }
}

#[test]
fn frames_repeat_at_fixed_workers_and_agree_across_worker_counts() {
    let fixture = fixture(300, [23, 13, 7], 3);
    let run = |threads: usize, preconditioner| {
        let ambient = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .expect("ambient pool");
        let mut solver = engine(
            &fixture.design,
            fixture.active.clone(),
            threads,
            preconditioner,
        );
        let mut frames = Vec::new();
        for phase in [0.0, 0.3] {
            let weights = frame_weights(300, phase);
            let report = ambient
                .install(|| {
                    solver.solve(
                        &fixture.design,
                        &fixture.outcome,
                        &weights,
                        &weights,
                        1e-10,
                        10_000,
                    )
                })
                .expect("certified frame");
            assert_eq!(report.executing_threads, threads);
            frames.push((
                solver.delta_eta.clone(),
                solver.step.clone(),
                report.certificate,
            ));
            solver.accept_step(1.0);
        }
        frames
    };
    for preconditioner in [LsmrPreconditioner::Auto, LsmrPreconditioner::Schwarz] {
        let reference = run(1, preconditioner);
        assert_eq!(
            run(1, preconditioner),
            reference,
            "repeatable at one worker"
        );
        for threads in [4, 1] {
            let frames = run(threads, preconditioner);
            if preconditioner == LsmrPreconditioner::Schwarz {
                // fedemean's projections do not depend on the worker count.
                assert_eq!(frames, reference);
            }
            for (frame, expected) in frames.iter().zip(&reference) {
                assert!(max_abs_difference(&frame.0, &expected.0) <= 1e-11);
                assert!(max_abs_difference(&frame.1, &expected.1) <= 1e-11);
            }
        }
    }
}

#[test]
fn projections_support_zero_finite_columns_with_three_effects() {
    let fixture = fixture(120, [9, 6, 4], 0);
    let mut solver = engine(
        &fixture.design,
        fixture.active.clone(),
        4,
        LsmrPreconditioner::Schwarz,
    );
    let weights = frame_weights(120, 0.2);
    let report = solver
        .solve(
            &fixture.design,
            &fixture.outcome,
            &weights,
            &weights,
            1e-10,
            10_000,
        )
        .expect("FE-only correction");
    assert!(report.certificate.passed);
    assert!(matches!(
        report.route.preconditioner,
        crate::fe::FePreconditioner::Schwarz(_)
    ));
    assert_eq!(solver.active_rank(), 0);
    let target: Vec<f64> = fixture
        .outcome
        .iter()
        .zip(&weights)
        .map(|(&y, &mu)| if y == 0.0 { -1.0 } else { y / mu - 1.0 })
        .collect();
    let (fitted, _) = dense_oracle(&fixture.design, &fixture.active, &weights, &target);
    assert!(max_abs_difference(&solver.delta_eta, &fitted) <= 1e-9);
}

#[test]
fn a_missed_joint_certificate_reprojects_once_from_the_solved_coefficients() {
    let fixture = fixture(200, [15, 8, 5], 2);
    let weights = frame_weights(200, 0.5);
    let mut solver = engine(
        &fixture.design,
        fixture.active.clone(),
        2,
        LsmrPreconditioner::Schwarz,
    );
    FORCE_CERTIFICATE_MISSES.set(1);
    let report = solver
        .solve(
            &fixture.design,
            &fixture.outcome,
            &weights,
            &weights,
            1e-9,
            10_000,
        )
        .expect("retried frame");
    assert_eq!(report.retries, 1);
    assert!(report.certificate.passed);
    assert_eq!(report.effective_tolerance, 1e-9 * RETRY_FACTOR);
    // Later frames target the tighter scale the miss demonstrated.
    assert_eq!(
        solver.projections.as_ref().unwrap().tolerance_scale,
        SCALE_STEP
    );
    let target: Vec<f64> = fixture
        .outcome
        .iter()
        .zip(&weights)
        .map(|(&y, &mu)| if y == 0.0 { -1.0 } else { y / mu - 1.0 })
        .collect();
    let (fitted, coefficients) = dense_oracle(&fixture.design, &fixture.active, &weights, &target);
    assert!(max_abs_difference(&solver.delta_eta, &fitted) <= 1e-9);
    assert!(max_abs_difference(&solver.step, &coefficients) <= 1e-9);

    // Two misses exhaust the frame: a numerical refusal that forgets the
    // warm starts.
    FORCE_CERTIFICATE_MISSES.set(2);
    let error = solver
        .solve(
            &fixture.design,
            &fixture.outcome,
            &weights,
            &weights,
            1e-9,
            10_000,
        )
        .expect_err("exhausted frame");
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert!(!solver.projections.as_ref().unwrap().warm_start_valid);
    assert_eq!(FORCE_CERTIFICATE_MISSES.get(), 0);
}

#[test]
fn a_stalled_automatic_joint_recurrence_hands_the_fit_to_the_projections() {
    let fixture = fixture(200, [15, 8, 5], 2);
    let mut solver = engine(
        &fixture.design,
        fixture.active.clone(),
        2,
        LsmrPreconditioner::Auto,
    );
    let weights = frame_weights(200, 0.5);
    let joint = solver
        .solve(
            &fixture.design,
            &fixture.outcome,
            &weights,
            &weights,
            1e-10,
            10_000,
        )
        .expect("joint frame");
    assert_eq!(joint.formulation, Formulation::Joint);
    assert_eq!(joint.retries, 0);
    assert!(joint.recurrence_residual.is_some());
    solver.accept_step(1.0);
    FORCE_JOINT_STALL.set(true);
    let weights = frame_weights(200, 0.8);
    let handed = solver
        .solve(
            &fixture.design,
            &fixture.outcome,
            &weights,
            &weights,
            1e-10,
            10_000,
        )
        .expect("projected frame");
    assert_eq!(handed.formulation, Formulation::Projections);
    assert_eq!(handed.retries, 1);
    assert!(handed.certificate.passed);
    assert!(handed.recurrence_residual.is_none());
    let target: Vec<f64> = fixture
        .outcome
        .iter()
        .zip(&weights)
        .map(|(&y, &mu)| if y == 0.0 { -1.0 } else { y / mu - 1.0 })
        .collect();
    let (fitted, coefficients) = dense_oracle(&fixture.design, &fixture.active, &weights, &target);
    assert!(max_abs_difference(&solver.delta_eta, &fitted) <= 1e-9);
    assert!(max_abs_difference(&solver.step, &coefficients) <= 1e-9);
    // Later frames of the fit start on the projections, warm.
    let weights = frame_weights(200, 0.9);
    let later = solver
        .solve(
            &fixture.design,
            &fixture.outcome,
            &weights,
            &weights,
            1e-10,
            10_000,
        )
        .expect("later frame");
    assert_eq!(later.formulation, Formulation::Projections);
    assert_eq!(later.retries, 0);
    assert!(later.warm_start_attempted);
    // Explicit Jacobi keeps the joint recurrence: a stall that its
    // continuation cannot repair is the frame's numerical refusal.
    let mut explicit = engine(
        &fixture.design,
        fixture.active.clone(),
        2,
        LsmrPreconditioner::Diagonal,
    );
    FORCE_JOINT_STALL.set(true);
    let error = explicit
        .solve(
            &fixture.design,
            &fixture.outcome,
            &weights,
            &weights,
            1e-10,
            10_000,
        )
        .expect_err("explicit Jacobi does not hand off");
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert!(explicit.projections.is_none());
}

#[test]
fn a_workspace_beyond_the_admitted_limit_is_an_allocation_refusal() {
    let fixture = fixture(120, [9, 6, 4], 1);
    let weights = frame_weights(120, 0.1);
    let mut solver = engine(
        &fixture.design,
        fixture.active.clone(),
        1,
        LsmrPreconditioner::Schwarz,
    );
    solver.ensure_projections().expect("projections");
    solver.projections.as_mut().unwrap().memory_limit = 64;
    let error = solver
        .solve(
            &fixture.design,
            &fixture.outcome,
            &weights,
            &weights,
            1e-9,
            10_000,
        )
        .expect_err("refused workspace");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn cancellation_stops_a_frame_as_an_interruption() {
    const CHILD: &str = "FEREG_PPML_FWL_LSMR_CANCEL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([
                "ppml::categorical_lsmr::tests::cancellation_stops_a_frame_as_an_interruption",
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let fixture = fixture(120, [9, 6, 4], 1);
    let weights = frame_weights(120, 0.1);
    for preconditioner in [LsmrPreconditioner::Diagonal, LsmrPreconditioner::Schwarz] {
        let mut solver = engine(&fixture.design, fixture.active.clone(), 1, preconditioner);
        crate::interrupt::request_cancellation();
        let error = solver
            .solve(
                &fixture.design,
                &fixture.outcome,
                &weights,
                &weights,
                1e-9,
                10_000,
            )
            .expect_err("cancelled frame");
        assert_eq!(error.code, ErrorCode::Interrupted);
        crate::interrupt::reset_cancellation();
        solver
            .solve(
                &fixture.design,
                &fixture.outcome,
                &weights,
                &weights,
                1e-9,
                10_000,
            )
            .expect("a later frame runs normally");
    }
}

#[test]
fn exact_certificate_is_thread_count_deterministic() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let rows = 2_048;
    let level_counts = [131, 137, 149];
    let effects = level_counts
        .iter()
        .enumerate()
        .map(|(effect, &levels)| {
            let codes = (0..rows)
                .map(|row| ((row * (37 + effect * 12) + 5 * effect) % levels) as u32)
                .collect::<Vec<_>>();
            EffectTerm::new(codes, levels, true, Vec::new()).expect("effect")
        })
        .collect::<Vec<_>>();
    let design = FeDesign::new(rows, effects).expect("design");
    let active_values = (0..rows)
        .flat_map(|row| {
            (0..4).map(move |column| ((row * (column + 3) + column * 11) % 97) as f64 / 53.0)
        })
        .collect::<Vec<_>>();
    let active = DenseMatrix::from_row_major(rows, 4, active_values).expect("active");
    let weights = (0..rows)
        .map(|row| 0.5 + (row % 29) as f64 / 17.0)
        .collect::<Vec<_>>();
    let target = (0..rows)
        .map(|row| (row % 41) as f64 / 19.0 - 0.75)
        .collect::<Vec<_>>();
    let residual = (0..rows)
        .map(|row| (row % 31) as f64 / 23.0 - 0.5)
        .collect::<Vec<_>>();

    let evaluate = |threads| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool");
        let mut finite_scratch = vec![DirectionCertificate::default(); active.ncols()];
        let mut fe_scratch = vec![DirectionCertificate::default(); design.total_coefficients()];
        let certificate = pool
            .install(|| {
                certify_joint_with_scratch(
                    &design,
                    &active,
                    &weights,
                    &target,
                    &residual,
                    1e-8,
                    &mut finite_scratch,
                    &mut fe_scratch,
                )
            })
            .expect("certificate");
        (certificate, finite_scratch, fe_scratch)
    };

    assert_eq!(evaluate(1), evaluate(4));
}

#[test]
fn exact_certificate_rejects_an_uncorrected_target() {
    let codes = vec![0, 0, 1, 1];
    let design = FeDesign::new(
        4,
        vec![EffectTerm::new(codes, 2, true, Vec::new()).expect("effect")],
    )
    .expect("design");
    let active = DenseMatrix::from_row_major(4, 0, Vec::new()).expect("active");
    let certificate = certify_joint(
        &design,
        &active,
        &[1.0; 4],
        &[1.0, 1.0, -1.0, -1.0],
        &[1.0, 1.0, -1.0, -1.0],
        1e-8,
    )
    .expect("certificate");
    assert!(!certificate.passed);
    assert!(certificate.relative_infinity_norm > 0.7);
}

#[test]
fn exact_certificate_does_not_turn_overflowing_squares_into_a_false_pass() {
    let design = FeDesign::new(
        2,
        vec![EffectTerm::new(vec![0, 0], 1, true, Vec::new()).expect("effect")],
    )
    .expect("design");
    let active = DenseMatrix::from_row_major(2, 0, Vec::new()).expect("active");
    let values = [1e200, 1e200];
    let certificate = certify_joint(&design, &active, &[1.0; 2], &values, &values, 1e-8)
        .expect("finite scaled certificate");
    assert!(!certificate.passed);
    assert!((certificate.relative_infinity_norm - 1.0).abs() <= 1e-15);
}

#[test]
fn exact_certificate_does_not_erase_a_finite_moment_through_split_underflow() {
    let design = FeDesign::new(
        2,
        vec![EffectTerm::new(vec![0, 0], 1, true, Vec::new()).expect("effect")],
    )
    .expect("design");
    let active = DenseMatrix::from_row_major(2, 1, vec![1e300, 1e300]).expect("active");
    let values = [1e-200, 1e-200];
    let error = certify_joint(&design, &active, &[1e-300; 2], &values, &values, 1e-8)
        .expect_err("unrepresentable reference norm must fail closed");
    assert_eq!(error.code, ErrorCode::NonConvergence);
}

#[test]
fn weighted_moment_reorders_a_nonzero_subnormal_split_factor() {
    let weight = 1e-300_f64;
    let residual = 6e-174_f64;
    let basis = 1e308_f64;
    let root_weight = weight.sqrt();
    let split = (root_weight * residual) * (root_weight * basis);
    let expected = finite_product3(weight, residual, basis).expect("finite reordered product");
    assert!(root_weight.mul_add(residual, 0.0).is_subnormal());
    assert!((split - expected).abs() / expected.abs() > 0.1);

    let (moment, weighted_basis) = finite_weighted_moment(weight, residual, basis, root_weight)
        .expect("robust weighted moment");
    assert_eq!(moment, expected);
    assert_eq!(weighted_basis, root_weight * basis);
}

#[test]
fn topology_identity_rejects_an_equal_shaped_replacement() {
    let first = FeDesign::new(
        4,
        vec![EffectTerm::new(vec![0, 0, 1, 1], 2, true, Vec::new()).expect("first")],
    )
    .expect("first design");
    let replacement = FeDesign::new(
        4,
        vec![EffectTerm::new(vec![0, 1, 0, 1], 2, true, Vec::new()).expect("replacement")],
    )
    .expect("replacement design");
    let identity = FeTopologyIdentity::capture(&first);
    assert!(identity.matches(&first));
    assert!(!identity.matches(&replacement));

    let empty = DenseMatrix::from_row_major(4, 0, Vec::new()).expect("active");
    let mut solver = engine(&first, empty, 1, LsmrPreconditioner::Auto);
    let error = solver
        .solve(&replacement, &[1.0; 4], &[1.0; 4], &[1.0; 4], 1e-8, 100)
        .expect_err("replacement topology must fail before projection");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
}

#[test]
fn topology_identity_rejects_a_different_effect_partition_with_the_same_total_levels() {
    let first = FeDesign::new(
        4,
        vec![
            EffectTerm::new(vec![0, 0, 0, 0], 1, true, Vec::new()).expect("first effect"),
            EffectTerm::new(vec![0, 1, 2, 0], 3, true, Vec::new()).expect("second effect"),
        ],
    )
    .expect("first design");
    let replacement = FeDesign::new(
        4,
        vec![
            EffectTerm::new(vec![0, 1, 0, 1], 2, true, Vec::new()).expect("first effect"),
            EffectTerm::new(vec![0, 0, 1, 1], 2, true, Vec::new()).expect("second effect"),
        ],
    )
    .expect("replacement design");
    assert_eq!(first.total_coefficients(), replacement.total_coefficients());
    let identity = FeTopologyIdentity::capture(&first);
    assert!(!identity.matches(&replacement));
}

#[test]
fn joint_memory_estimate_scales_with_retained_design_and_thread_team() {
    let estimate = |columns, levels: &[usize], threads| {
        estimate_joint_lsmr_workspace_bytes(
            10_000,
            columns,
            levels,
            threads,
            LsmrPreconditioner::Auto,
        )
        .expect("estimate")
    };
    let base = estimate(2, &[2_000, 1_000], 1);
    let wider = estimate(8, &[2_000, 1_000], 1);
    // fedemean's workspace does not depend on the worker count.
    let parallel = estimate(8, &[2_000, 1_000], 8);
    let more_levels = estimate(8, &[2_001, 1_001], 8);
    assert!(wider > base);
    assert_eq!(parallel, wider);
    assert!(more_levels > parallel);
    // At least the retained controls, the projected matrix and the
    // coefficients of every column.
    let floor = 8 * (10_000 * 8 + 10_000 * 9 + 2 * 9 * 3_000);
    assert!(wider > floor);
    // A preconditioner that keeps the joint recurrence never projects.
    let joint = estimate_joint_lsmr_workspace_bytes(
        10_000,
        8,
        &[2_000, 1_000],
        1,
        LsmrPreconditioner::Diagonal,
    )
    .unwrap();
    assert!(joint < wider);
}

#[test]
fn checkpoint_memory_charges_the_projected_and_candidate_matrices() {
    let estimate = |rhs, threads| {
        estimate_checkpoint_lsmr_workspace_bytes(50_000, rhs, &[8_000, 7_000, 6_000], threads)
            .expect("checkpoint estimate")
    };
    let one_thread = estimate(11, 1);
    let seven_threads = estimate(11, 7);
    let wider = estimate(17, 7);
    assert_eq!(seven_threads, one_thread);
    assert!(wider > seven_threads);
    let two_dense_batches = 2_u128 * 50_000 * 11 * std::mem::size_of::<f64>() as u128;
    assert!(one_thread > two_dense_batches);
    assert!(estimate_checkpoint_lsmr_workspace_bytes(50_000, 0, &[8_000, 7_000], 1).is_err());
}

#[test]
fn joint_memory_estimate_rejects_invalid_and_overflowing_shapes() {
    let invalid =
        estimate_joint_lsmr_workspace_bytes(100, 2, &[20, 0], 4, LsmrPreconditioner::Auto)
            .expect_err("zero levels must fail");
    assert_eq!(invalid.code, ErrorCode::InvalidArgument);
    let overflow = estimate_joint_lsmr_workspace_bytes(
        usize::MAX,
        usize::MAX,
        &[usize::MAX, usize::MAX],
        usize::MAX,
        LsmrPreconditioner::Auto,
    )
    .expect_err("adversarial dimensions must fail closed");
    assert_eq!(overflow.code, ErrorCode::Allocation);
}

#[test]
fn schwarz_grant_covers_the_star_pairs_at_their_largest_cross_tabulation() {
    use crate::fe::block_schur::schwarz_grant_bytes;
    // One star pair (the largest effect, 2,000 levels, with 1,000 levels):
    // E = min(10,000, 2,000 x 1,000) entries.
    let pair = |entries: u128, big: u128, other: u128| {
        24 * entries + 20 * (big + other) + 48 * (entries + other) + 16 * (big + other)
    };
    assert_eq!(
        schwarz_grant_bytes(10_000, &[1_000, 2_000]).unwrap(),
        pair(10_000, 2_000, 1_000)
    );
    // The star of three effects pairs the largest with each other one; a
    // small cross-tabulation is bounded by its cells.
    assert_eq!(
        schwarz_grant_bytes(10_000, &[2_000, 3, 1_000]).unwrap(),
        pair(6_000, 2_000, 3) + pair(10_000, 2_000, 1_000)
    );
    assert_eq!(schwarz_grant_bytes(10_000, &[2_000]).unwrap(), 0);
    assert!(schwarz_grant_bytes(u128::MAX, &[usize::MAX, usize::MAX]).is_err());
    // The LSMR workspace charges the grant beside fedemean's base workspace.
    let base = crate::fe::block_schur::lsmr_workspace_bytes(10_000, &[2_000], 1).unwrap();
    let with = crate::fe::block_schur::lsmr_workspace_bytes(10_000, &[2_000, 1_000], 1).unwrap();
    assert!(with > base + pair(10_000, 2_000, 1_000));
}

#[test]
fn finite_buffer_estimate_matches_their_allocation() {
    for k in [0_usize, 1, 4, 9] {
        let buffers = FiniteBuffers::new(k).expect("finite buffers");
        let values = [
            &buffers.gram,
            &buffers.rhs,
            &buffers.factor,
            &buffers.forward,
            &buffers.solution,
            &buffers.scales,
            &buffers.step,
        ]
        .iter()
        .map(|v| v.capacity() as u128)
        .sum::<u128>();
        assert_eq!(values, FiniteBuffers::values(k as u128).unwrap());
        assert_eq!(buffers.permutation.capacity(), k);
    }
}
