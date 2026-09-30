use fereg_engine::{
    data::{DenseMatrix, Prepared},
    ppml::separation::*,
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::Role,
    ErrorCode,
};
mod support;
fn sample(y: &[f64]) -> Prepared {
    let mut s = support::slope_sample(true, &vec![0; y.len()], &[]);
    s.outcome = y.to_vec();
    s
}
fn ledger(s: Prepared, methods: Methods) -> Ledger {
    Ledger::new(
        s,
        Policy {
            methods,
            drop_singletons: false,
        },
    )
    .unwrap()
}
fn model<'a>(x: &'a DenseMatrix, roles: &'a [Role]) -> Model<'a> {
    Model {
        raw: x,
        roles,
        options: Options::for_estimator(Estimator::Ppml),
        controls: Controls::default(),
    }
}
#[test]
fn retained_primer_ray_is_certified_on_the_full_model_before_atomic_deletion() {
    let y = [0., 0., 0., 1., 2., 3.];
    let raw = DenseMatrix::from_columns(&[
        vec![2., -1., 0., 0., 5., 6.],
        vec![-1., 2., 0., 0., -10., -12.],
    ])
    .unwrap();
    let m = model(&raw, &[Role::Ordinary; 2]);
    let mut l = ledger(sample(&y), Methods::only(Method::Simplex));
    let r = l
        .view()
        .model_simplex(&m, SimplexLimits::default())
        .unwrap();
    assert_eq!(r.status, Status::Certified);
    assert_eq!(r.direction.as_ref().unwrap().rows, [0]);
    assert_eq!(r.capacity.finite_columns, 2);
    assert_eq!(r.capacity.fe_columns, 1);
    let before = l.trace_digest();
    let accepted = l
        .accept_direction(
            r.direction.as_ref().unwrap(),
            &m,
            &ThreadTeam::new(1).unwrap(),
        )
        .unwrap();
    assert_eq!(accepted.commit.primary, 1);
    assert_ne!(before, l.trace_digest());
    assert_eq!(l.view().sample().retained_rows(), [1, 2, 3, 4, 5]);
}
#[test]
fn phase_one_combines_directions_and_preserves_small_full_model_contrasts() {
    let one = ThreadTeam::new(1).unwrap();
    for contrast in [1., 2_f64.powi(-18)] {
        let raw = DenseMatrix::from_columns(&[
            vec![1., 1. + 2. * contrast, 1. - contrast],
            vec![1., 1. - contrast, 1. + 2. * contrast],
        ])
        .unwrap();
        let l = ledger(sample(&[1., 0., 0.]), Methods::only(Method::Simplex));
        let m = model(&raw, &[Role::Ordinary; 2]);
        let r = l
            .view()
            .model_simplex(&m, SimplexLimits::default())
            .unwrap();
        assert_eq!(r.status, Status::Certified);
        assert!(r.pivots > 0 && r.pivots <= SimplexLimits::default().max_pivots);
        let proposed = &r.direction.as_ref().unwrap().rows;
        assert!(!proposed.is_empty() && proposed.iter().all(|row| [1, 2].contains(row)));
        // x1+x2-2 is strictly positive on both boundary rows. Phase-I needs
        // only one nonempty valid ray; later ledger rounds recover its union.
        assert_eq!(raw[(0, 0)] + raw[(0, 1)] - 2., 0.);
        for row in [1, 2] {
            assert!(raw[(row, 0)] + raw[(row, 1)] - 2. > 0.);
        }
        l.view()
            .certify_direction(r.direction.as_ref().unwrap(), &m, &one)
            .unwrap();
        let err = l
            .view()
            .model_simplex(
                &m,
                SimplexLimits {
                    max_pivots: 1,
                    ..SimplexLimits::default()
                },
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NonConvergence);
    }
}
#[test]
fn mixed_sign_and_duplicate_constraints_have_independent_duals_and_fresh_replay() {
    let raw = DenseMatrix::from_columns(&[vec![0., 1., -1., 1., -1.]]).unwrap();
    let l = ledger(
        sample(&[1., 0., 0., 0., 0.]),
        Methods::only(Method::Simplex),
    );
    let m = model(&raw, &[Role::Ordinary]);
    let r = l
        .view()
        .model_simplex(&m, SimplexLimits::default())
        .unwrap();
    assert_eq!(r.status, Status::NoSeparation);
    let Some(SimplexEvidence::DualInfeasibility { certificate, .. }) = &r.evidence else {
        panic!("independent dual required");
    };
    assert_eq!(certificate.boundary_rows, 4);
    assert_eq!(certificate.nullity, 1);
    assert!(certificate.infeasibility_margin_relative > 0.);
    let terminal = l
        .view()
        .replay_simplex(&r, &m, SimplexLimits::default())
        .unwrap();
    assert_eq!(terminal.generation(), 0);
    assert_eq!(terminal.status(), Status::NoSeparation);
    let mut forged = r.clone();
    let Some(SimplexEvidence::DualInfeasibility { dual, .. }) = &mut forged.evidence else {
        unreachable!()
    };
    *dual.last_mut().unwrap() = 1.;
    assert_eq!(
        l.view()
            .replay_simplex(&forged, &m, SimplexLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    forged = r.clone();
    forged.generation += 1;
    assert_eq!(
        l.view()
            .replay_simplex(&forged, &m, SimplexLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    forged = r.clone();
    forged.capacity.required_cells += 1;
    assert!(l
        .view()
        .replay_simplex(&forged, &m, SimplexLimits::default())
        .is_err());
}
#[test]
fn capacity_counts_all_observed_fe_levels_and_slope_channels_before_dense_work() {
    let s = support::prepared(&[7, 5, 3], true, 10, 0);
    let n = s.n_active();
    let mut s = s;
    s.outcome[0] = 0.;
    let l = ledger(s, Methods::default());
    let raw = DenseMatrix::zeros(n, 2).unwrap();
    let m = model(&raw, &[Role::Ordinary; 2]);
    let cap = l.view().simplex_capacity(&m).unwrap();
    assert_eq!(cap.fe_columns, 2 * (7 + 5 + 3));
    assert_eq!(cap.explicit_width, cap.fe_columns + 2);
    assert!(cap.topology_cells > 0 && cap.certificate_cells > 0);
    assert_eq!(
        cap.required_cells,
        cap.kernel_cells + cap.topology_cells + cap.certificate_cells
    );
    let skipped = l
        .view()
        .model_simplex(
            &m,
            SimplexLimits {
                max_cells: cap.required_cells - 1,
                ..SimplexLimits::default()
            },
        )
        .unwrap();
    assert_eq!(skipped.status, Status::SkippedCapacity);
    assert_eq!(skipped.pivots, 0);
    assert!(skipped.direction.is_none() && skipped.evidence.is_none());
    assert!(l
        .view()
        .replay_simplex(&skipped, &m, skipped.limits)
        .is_err());
    let overflow = simplex_kernel_cells(usize::MAX, usize::MAX - 1, 1, 2).unwrap_err();
    assert_eq!(overflow.return_code(), 909);
    assert_eq!(simplex_kernel_cells(10, 6, 4, 3).unwrap(), 185);
}
#[test]
fn only_fresh_requested_same_generation_full_model_ir_can_close_capacity_skip() {
    let raw = DenseMatrix::from_columns(&[vec![0., 1., -1.]]).unwrap();
    let m = model(&raw, &[Role::Ordinary]);
    let methods = Methods::parse("simplex ir").unwrap();
    let l = ledger(sample(&[1., 0., 0.]), methods);
    let limits = SimplexLimits {
        max_cells: 1,
        ..SimplexLimits::default()
    };
    let skip = l.view().model_simplex(&m, limits).unwrap();
    assert_eq!(skip.status, Status::SkippedCapacity);
    let one = ThreadTeam::new(1).unwrap();
    let ir = l.view().iterated_rectifier(&m, 100, &one).unwrap();
    assert_eq!(ir.status, Status::NoSeparation);
    let (simplex, terminal) = l
        .resolve_simplex_skip(&skip, &ir, &m, limits, 100, &one)
        .unwrap();
    assert_eq!(simplex.status(), Status::SkippedCapacity);
    assert_eq!(terminal.generation(), simplex.generation());
    let simplex_only = ledger(sample(&[1., 0., 0.]), Methods::only(Method::Simplex));
    assert_eq!(
        simplex_only
            .resolve_simplex_skip(&skip, &ir, &m, limits, 100, &one)
            .unwrap_err()
            .return_code(),
        909
    );
    // Same rows and width, different full model: an IR seal for the mixed
    // sign design cannot cover the separating span of a changed raw column.
    let changed = DenseMatrix::from_columns(&[vec![0., -1., -1.]]).unwrap();
    let changed_model = model(&changed, &[Role::Ordinary]);
    assert!(l
        .resolve_simplex_skip(&skip, &ir, &changed_model, limits, 100, &one)
        .is_err());
    let separating_ir = l
        .view()
        .iterated_rectifier(&changed_model, 100, &one)
        .unwrap();
    assert_eq!(separating_ir.status, Status::Certified);
    assert_eq!(
        l.resolve_simplex_skip(&skip, &separating_ir, &changed_model, limits, 100, &one)
            .unwrap_err()
            .return_code(),
        909
    );
    let mut restarted = ledger(sample(&[1., 0., 0.]), methods);
    restarted
        .accept_direction(
            separating_ir.direction.as_ref().unwrap(),
            &changed_model,
            &one,
        )
        .unwrap();
    let current_raw = DenseMatrix::zeros(1, 1).unwrap();
    let current_model = model(&current_raw, &[Role::Ordinary]);
    assert_eq!(
        restarted
            .resolve_simplex_skip(&skip, &ir, &current_model, limits, 100, &one)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    let mut forged = ir.clone();
    forged.generation += 1;
    assert!(l
        .resolve_simplex_skip(&skip, &forged, &m, limits, 100, &one)
        .is_err());
}
#[test]
fn full_slope_channels_and_declared_omissions_define_the_cone() {
    let one = ThreadTeam::new(1).unwrap();
    let mut s = support::slope_sample(false, &[0, 0, 1, 1], &[vec![0., -1., 0., -2.]]);
    s.outcome = vec![1., 0., 1., 0.];
    let l = ledger(s, Methods::only(Method::Simplex));
    let raw = DenseMatrix::zeros(4, 0).unwrap();
    let m = model(&raw, &[]);
    let r = l
        .view()
        .model_simplex(&m, SimplexLimits::default())
        .unwrap();
    assert_eq!(r.status, Status::Certified);
    assert_eq!(r.capacity.fe_columns, 2);
    l.view()
        .certify_direction(r.direction.as_ref().unwrap(), &m, &one)
        .unwrap();
    let l = ledger(sample(&[1., 0., 0.]), Methods::only(Method::Simplex));
    let raw = DenseMatrix::from_columns(&[vec![0., -1., -2.]]).unwrap();
    for role in [Role::Base, Role::ExplicitlyOmitted] {
        let roles = [role];
        let m = model(&raw, &roles);
        let r = l
            .view()
            .model_simplex(&m, SimplexLimits::default())
            .unwrap();
        assert_eq!(r.status, Status::NoSeparation);
        assert_eq!(r.capacity.explicit_width, 2);
        l.view()
            .replay_simplex(&r, &m, SimplexLimits::default())
            .unwrap();
    }
}
#[test]
fn column_range_changes_preserve_the_ray_and_original_model_span() {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    let l = ledger(sample(&[1., 0., 0.]), Methods::only(Method::Simplex));
    for scale in [1e-200, 1e-100, 1., 1e100, 1e200] {
        let raw = DenseMatrix::from_columns(&[vec![0., -scale, -2. * scale]]).unwrap();
        let m = model(&raw, &[Role::Ordinary]);
        let r = l
            .view()
            .model_simplex(&m, SimplexLimits::default())
            .unwrap();
        assert_eq!(r.status, Status::Certified);
        let p = r.direction.as_ref().unwrap();
        assert_eq!(p.rows, [1, 2]);
        assert_eq!(p.direction, [0., -0.5, -1.]);
        if scale == 1e-200 {
            // The literal coefficient 1/scale proves the observation ray's
            // span, but its original-coordinate bread overflows at this
            // scale. The shared frame must reject publication.
            assert_eq!(
                l.view().certify_direction(p, &m, &one).unwrap_err().code,
                ErrorCode::NonConvergence
            );
        } else {
            assert_eq!(
                l.view().certify_direction(p, &m, &one).unwrap(),
                l.view().certify_direction(p, &m, &eight).unwrap()
            );
        }
    }
    let raw = DenseMatrix::from_columns(&[vec![0., -1e-300, -1e300]]).unwrap();
    let m = model(&raw, &[Role::Ordinary]);
    let err = l
        .view()
        .model_simplex(&m, SimplexLimits::default())
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NonConvergence);
    assert!(err.message.contains("underflow"));
}
#[test]
fn no_candidate_and_capacity_paths_preserve_explicit_solver_support_and_alignment() {
    let s = support::slope_sample(false, &[0, 0, 1, 1], &[vec![0., 1., 0., 1.]]);
    let l = ledger(s, Methods::default());
    let raw = DenseMatrix::zeros(4, 0).unwrap();
    let mut m = model(&raw, &[]);
    m.options.solver = Solver::Cmg;
    assert_eq!(
        l.view()
            .model_simplex(&m, SimplexLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    m.options.solver = Solver::Auto;
    let r = l
        .view()
        .model_simplex(&m, SimplexLimits::default())
        .unwrap();
    assert_eq!(r.status, Status::NoCandidates);
    assert_eq!(r.pivots, 0);
    assert!(l
        .view()
        .replay_simplex(&r, &m, SimplexLimits::default())
        .is_ok());
    assert_eq!(
        l.view()
            .model_simplex(
                &m,
                SimplexLimits {
                    max_pivots: 0,
                    ..SimplexLimits::default()
                }
            )
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    m.roles = &[Role::Ordinary];
    assert_eq!(
        l.view()
            .model_simplex(&m, SimplexLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}
