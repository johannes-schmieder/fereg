//! Extreme scales: no vector leaves the double range unless the answer itself does.
use super::super::*;
use crate::lsmr::fixtures::*;
use crate::lsmr::Operator;

#[derive(Clone, Copy, Debug)]
enum Metric {
    None,
    Identity,
    Diagonal,
}

fn solve<A: Operator>(
    a: &A,
    b: &[f64],
    metric: Metric,
    local_size: Option<usize>,
    warm_start: Option<&[f64]>,
) -> Result<LsmrResult, SolveError> {
    let n = a.ncols();
    let diagonal = DiagOp(
        (0..n)
            .map(|j| if j % 2 == 0 { 4.0 } else { 0.25 })
            .collect(),
    );
    let options = MlsmrOptions {
        warm_start,
        local_size,
        ..Default::default()
    };
    match metric {
        Metric::None => lsmr(a, b, 1e-10, 50, local_size),
        Metric::Identity => mlsmr(a, b, &IdentityOp { n }, 1e-10, 50, options),
        Metric::Diagonal => mlsmr(a, b, &diagonal, 1e-10, 50, options),
    }
}

/// `α₁ = ‖Aᵀb‖ / ‖b‖ ≈ 1e-310` is subnormal although `‖A‖` and `‖b‖` are not, so `1/α₁` is ∞.
#[test]
fn a_subnormal_initial_gradient_normalizes_its_basis_vector() {
    a_subnormal_initial_gradient_normalizes_its_basis_vector_case(Metric::None, None);
    a_subnormal_initial_gradient_normalizes_its_basis_vector_case(Metric::None, Some(2));
    a_subnormal_initial_gradient_normalizes_its_basis_vector_case(Metric::Identity, None);
    a_subnormal_initial_gradient_normalizes_its_basis_vector_case(Metric::Identity, Some(2));
    a_subnormal_initial_gradient_normalizes_its_basis_vector_case(Metric::Diagonal, None);
    a_subnormal_initial_gradient_normalizes_its_basis_vector_case(Metric::Diagonal, Some(2));
}

fn a_subnormal_initial_gradient_normalizes_its_basis_vector_case(
    metric: Metric,
    local_size: Option<usize>,
) {
    let r = solve(
        &DiagOp(vec![0.0, 1.0]),
        &[1e200, 1e-110],
        metric,
        local_size,
        None,
    )
    .expect("subnormal-gradient solve");

    assert!(r.converged, "{:?}", r.stop_reason);
    assert_eq!(r.x[0], 0.0);
    assert!((r.x[1] / 1e-110 - 1.0).abs() < 1e-9, "{:?}", r.x);
}

/// A subnormal `‖b‖` has no representable reciprocal either.
#[test]
fn a_subnormal_rhs_normalizes_its_first_vector() {
    a_subnormal_rhs_normalizes_its_first_vector_case(Metric::None);
    a_subnormal_rhs_normalizes_its_first_vector_case(Metric::Identity);
    a_subnormal_rhs_normalizes_its_first_vector_case(Metric::Diagonal);
}

fn a_subnormal_rhs_normalizes_its_first_vector_case(metric: Metric) {
    let r = solve(&IdentityOp { n: 2 }, &[1e-310, 0.0], metric, None, None)
        .expect("subnormal-rhs solve");

    assert!(r.converged, "{:?}", r.stop_reason);
    assert!((r.x[0] / 1e-310 - 1.0).abs() < 1e-9, "{:?}", r.x);
    assert_eq!(r.x[1], 0.0);
}

/// Auditing `x = 0` meets a residual `2^(p−q)` times the correction's, `2^1040` at the corner.
#[test]
fn a_warm_start_far_below_the_rhs_scale_certifies() {
    a_warm_start_far_below_the_rhs_scale_certifies_case(-1000, -40, 0.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(-1000, -40, 0.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(-1000, -40, 1.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(-1000, -40, 1.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(-1000, 0, 0.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(-1000, 0, 0.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(-1000, 0, 1.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(-1000, 0, 1.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(0, -40, 0.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(0, -40, 0.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(0, -40, 1.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(0, -40, 1.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(0, 0, 0.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(0, 0, 0.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(0, 0, 1.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(0, 0, 1.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(1000, -40, 0.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(1000, -40, 0.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(1000, -40, 1.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(1000, -40, 1.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(1000, 0, 0.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(1000, 0, 0.0, Metric::Diagonal);
    a_warm_start_far_below_the_rhs_scale_certifies_case(1000, 0, 1.0, Metric::Identity);
    a_warm_start_far_below_the_rhs_scale_certifies_case(1000, 0, 1.0, Metric::Diagonal);
}

fn a_warm_start_far_below_the_rhs_scale_certifies_case(p: i32, q: i32, rho: f64, metric: Metric) {
    let a = DenseOp {
        rows: 3,
        cols: 2,
        data: vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };
    let (big, small) = (2f64.powi(p), 2f64.powi(q));
    let b = [big, small, rho * small];
    let r = solve(&a, &b, metric, None, Some(&[big, 0.0])).expect("warm solve");

    assert!(r.converged, "{:?}", r.stop_reason);
    assert!((r.x[0] / big - 1.0).abs() < 1e-12, "{:?}", r.x);
    assert!((r.x[1] / small - 1.0).abs() < 1e-9, "{:?}", r.x);
    assert!((r.residual_norm - rho * small).abs() <= 1e-9 * small);
}

/// `⟨v, M⁻¹v⟩` is positive, but its raw terms overflow to `+∞` and `−∞`, which sum to NaN.
#[test]
fn a_definite_metric_whose_raw_dot_product_overflows_is_rescaled() {
    let m = DenseOp {
        rows: 2,
        cols: 2,
        data: vec![1.0, -1.0, -1.0, 1.5],
    };
    let r = mlsmr(
        &DiagOp(vec![2e200, 1e200]),
        &[1.0, 1.0],
        &m,
        1e-10,
        50,
        MlsmrOptions::default(),
    )
    .expect("overflowing-dot solve");

    assert!(r.converged, "{:?}", r.stop_reason);
    assert!((r.x[0] / 5e-201 - 1.0).abs() < 1e-8, "{:?}", r.x);
    assert!((r.x[1] / 1e-200 - 1.0).abs() < 1e-8, "{:?}", r.x);
}

/// `‖A‖ = f64::MAX` leaves `1/α₁` subnormal; a stop dropping column 2 must hold its backward error.
#[test]
fn an_operator_at_the_top_of_the_range_solves_to_its_backward_error() {
    let a = DiagOp(vec![f64::MAX, 1.0]);
    let b = [1.0, 1.0];
    let r = lsmr(&a, &b, 1e-10, 50, None).expect("top-of-range solve");

    assert!(r.converged, "{:?}", r.stop_reason);
    assert!((r.x[0] * f64::MAX - 1.0).abs() < 1e-9, "{:?}", r.x);
    let normr = vec_norm(&[b[0] - f64::MAX * r.x[0], b[1] - r.x[1]]);
    let backward_error = normal_equation_residual(&a, &r.x, &b) / f64::MAX / normr;
    assert!(backward_error <= 1e-10, "{backward_error:e}");
}

/// Past raw `‖A‖ ≈ 1e244` the unnormalized-`u` products overflow, which the solve reports.
#[test]
fn an_operator_past_the_unnormalized_headroom_fails_loudly() {
    an_operator_past_the_unnormalized_headroom_fails_loudly_case(&[1e300, 0.0, 1e50, 1e300]); // adjoint_product
    an_operator_past_the_unnormalized_headroom_fails_loudly_case(&[1.0, 0.0, 1e-50, 1e300]);
    // next_step_coefficient
}

fn an_operator_past_the_unnormalized_headroom_fails_loudly_case(data: &[f64]) {
    let a = DenseOp {
        rows: 2,
        cols: 2,
        data: data.to_vec(),
    };
    let r = mlsmr(
        &a,
        &[1.0, 0.0],
        &IdentityOp { n: 2 },
        0.0,
        2,
        MlsmrOptions::default(),
    );
    assert!(r.is_err(), "{:?}", r.map(|r| r.x));
}

/// Past Krylov exhaustion `h̄` grows by `~1/ε` a step; once it overflows, `x += t_x·h̄` is `0·∞`.
#[test]
fn a_run_past_krylov_exhaustion_never_returns_a_non_finite_x() {
    a_run_past_krylov_exhaustion_never_returns_a_non_finite_x_case(true, 1e244); // budget_stop
    a_run_past_krylov_exhaustion_never_returns_a_non_finite_x_case(false, 1e300);
    // refuted_stop
}

fn a_run_past_krylov_exhaustion_never_returns_a_non_finite_x_case(metric: bool, scale: f64) {
    let a = DenseOp {
        rows: 2,
        cols: 2,
        data: vec![1.0, 0.0, 1e-36, scale],
    };
    let b = [1.0, 0.0];
    let r = if metric {
        mlsmr(
            &a,
            &b,
            &IdentityOp { n: 2 },
            0.0,
            5,
            MlsmrOptions::default(),
        )
    } else {
        lsmr(&a, &b, 0.0, 5, None)
    };
    if let Ok(r) = r {
        assert!(
            r.x.iter().all(|x| x.is_finite()),
            "{:?}: {:?}",
            r.stop_reason,
            r.x
        );
    }
}

/// The metric-blind audit bounds `‖A‖` by `Aᵀ(b/‖b‖)`, which a clamped subnormal `‖b‖` deflates.
#[test]
fn a_subnormal_rhs_bounds_the_operator_by_its_own_norm() {
    let a = DenseOp {
        rows: 3,
        cols: 2,
        data: vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };
    let r = mlsmr(
        &a,
        &[1e-310, 0.0, 1e-310],
        &DiagOp(vec![1.0, 0.0]),
        1e-10,
        50,
        MlsmrOptions {
            warm_start: Some(&[1e-310, 1e-320]),
            ..Default::default()
        },
    )
    .expect("subnormal-rhs warm solve");

    assert!(r.converged, "{:?}", r.stop_reason);
    assert_eq!(
        r.stop_reason,
        LsmrStopReason::InitialNormalEquationResidualZero
    );
}

/// A warm start's reference `‖Aᵀb‖` leaves the double range while `‖A‖` and `‖b‖` do not.
#[test]
fn a_warm_reference_past_the_double_range_reports_against_b() {
    let solve = |s: f64, t: f64| {
        mlsmr(
            &DiagOp(vec![s, 2.0 * s]),
            &[t, t],
            &IdentityOp { n: 2 },
            1e-10,
            1,
            MlsmrOptions {
                warm_start: Some(&[0.5 * t / s, 0.0]),
                ..Default::default()
            },
        )
        .expect("warm solve")
    };
    let (unit, scaled) = (solve(1.0, 1.0), solve(2f64.powi(600), 2f64.powi(500)));
    assert!(
        (scaled.normal_eq_residual / unit.normal_eq_residual - 1.0).abs() < 1e-12,
        "{:e} vs {:e}",
        scaled.normal_eq_residual,
        unit.normal_eq_residual
    );
}

/// `b / ‖b‖` flushes `b₂ = 2^-500`, whose `2^300` column carries the metric reference.
#[test]
fn a_warm_reference_keeps_an_rhs_entry_far_below_its_norm() {
    let r = mlsmr(
        &DiagOp(vec![1.0, 2f64.powi(300)]),
        &[2f64.powi(600), 2f64.powi(-500)],
        &DiagOp(vec![2f64.powi(-1000), 2f64.powi(700)]),
        1e-10,
        0,
        MlsmrOptions {
            warm_start: Some(&[2f64.powi(600), 0.0]),
            ..Default::default()
        },
    )
    .expect("warm solve");
    assert!(
        (r.normal_eq_residual - 1.0).abs() < 1e-12,
        "{:e}",
        r.normal_eq_residual
    );
}

/// A subnormal `Aᵀb` made unit meets a metric near `f64::MAX`, overflowing `M⁻¹g` the raw one fits.
#[test]
fn a_subnormal_warm_gradient_stays_raw_under_a_huge_metric() {
    let r = mlsmr(
        &DiagOp(vec![1e-310, 1e-310]),
        &[1.0, 1.0],
        &DenseOp {
            rows: 2,
            cols: 2,
            data: vec![1.6e308, 8e307, 8e307, 1.6e308],
        },
        1e-10,
        0,
        MlsmrOptions {
            warm_start: Some(&[1.0, 1.0]),
            ..Default::default()
        },
    )
    .expect("warm solve");
    assert!(
        (r.normal_eq_residual - 1.0).abs() < 1e-9,
        "{:e}",
        r.normal_eq_residual
    );
}

/// `‖[ε, ε]‖` rounds to `ε`, so `b / ‖b‖` is `[1, 1]` and would bound `‖A‖ = 1` by `1.118`.
#[test]
fn a_rounded_subnormal_rhs_norm_does_not_overstate_the_operator() {
    let eps = f64::from_bits(1);
    let r = mlsmr(
        &DiagOp(vec![1.0, 0.5]),
        &[eps, eps],
        &DiagOp(vec![eps, eps]),
        0.0046,
        50,
        MlsmrOptions {
            warm_start: Some(&[0.0, 2.0]),
            ..Default::default()
        },
    )
    .expect("warm solve");
    assert!(!r.converged, "{:?}", r.stop_reason);
    assert_eq!(r.stop_reason, LsmrStopReason::FalseConvergence);
}
