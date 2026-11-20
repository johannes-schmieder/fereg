use super::*;
use crate::ppml_separation::SeparationCertificateProof as Proof;

#[derive(Clone, Copy)]
enum Terms {
    TwoFe,
    ThreeFe,
    AbsorbedSlope,
    OrdinaryAlias,
}

fn panel(rows: &[[f64; 4]], terms: Terms) -> PreparedPpmlModelOwner {
    let (mut request, _, mut loaded, mut uses) = fixture();
    if matches!(terms, Terms::TwoFe | Terms::ThreeFe) {
        let mut e = request.absorbed_terms[0].clone();
        e.id = AbsorbTermId::new(2).unwrap();
        e.categorical_sources = vec![SourceId::new(3).unwrap()];
        request.absorbed_terms.push(e);
        uses[2] = NumericSourceUse::fixed_effect();
        if matches!(terms, Terms::ThreeFe) {
            let mut e = request.absorbed_terms[0].clone();
            e.id = AbsorbTermId::new(3).unwrap();
            e.categorical_sources = vec![SourceId::new(1).unwrap()];
            request.absorbed_terms.push(e);
            request.model.terms.clear();
            uses[0] = NumericSourceUse::fixed_effect();
        }
    } else if matches!(terms, Terms::AbsorbedSlope) {
        request.absorbed_terms[0]
            .slope_sources
            .push(SourceId::new(3).unwrap());
        uses[2] = NumericSourceUse::fixed_effect();
    } else {
        let mut v = request.model.terms[0].clone();
        v.id = TermId::new(2).unwrap();
        v.order = TermOrder::new(2).unwrap();
        v.coefficient_slot = CoefficientSlot::new(2).unwrap();
        v.components[0].source = SourceId::new(3).unwrap();
        request.model.terms.push(v);
        uses[2] = NumericSourceUse::ordinary();
    }
    loaded.candidate = vec![true; rows.len()].into();
    let values = |j: usize| column(&rows.iter().map(|r| r[j]).collect::<Vec<_>>());
    loaded.outcome = values(0);
    for k in 0..3 {
        loaded.numeric_sources[k].1 = values(k + 1);
    }
    let numeric = prepare_numeric(&loaded, &uses);
    PreparedPpmlModelOwner::new(request, vec![], loaded, numeric).unwrap()
}

fn simplex_only(
    owner: PreparedPpmlModelOwner,
    cells: usize,
) -> Result<PreparedPpmlSeparationOutcome> {
    detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: true,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            simplex_max_tableau_cells: cells,
            ..SeparationConfig::default()
        },
        None,
        None,
    )
}

#[test]
fn two_fe_positive_rank_finds_late_nearly_aliased_contrast_in_either_order() {
    // 48 firms × 2 periods. Period adds one coefficient after absorbing
    // firms. A single late firm supplies the independent slope contrast.
    let mut rows = Vec::new();
    for firm in 0..48 {
        for (period, treatment, outcome) in [(0., 0., 1.), (1., 1., 2.), (0., 0., 1.), (0., 2., 0.)]
        {
            let x = if firm == 47 && period == 1. {
                treatment + 2f64.powi(-43)
            } else {
                treatment
            };
            rows.push([outcome, x, firm as f64, period]);
        }
    }
    for reverse in [false, true] {
        if reverse {
            rows.reverse();
        }
        let owner = panel(&rows, Terms::TwoFe);
        let out = detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig {
                methods: SeparationMethods {
                    fixed_effects: false,
                    simplex: true,
                    iterated_rectifier: false,
                    mu: false,
                },
                simplex_max_iterations: 2,
                simplex_max_tableau_cells: 128,
                drop_singletons: false,
                ..SeparationConfig::default()
            },
            None,
            None,
        )
        .unwrap();
        assert!(out.separation.dropped_observations.is_empty());
        assert!(out
            .separation
            .methods
            .iter()
            .any(|m| m.certificate.as_ref().is_some_and(|c| matches!(
                c.proof,
                Proof::SimplexAnchoredPositiveRank {
                    within_columns: 2,
                    anchor_levels: 48,
                    effects: 2,
                    ..
                }
            ))));
    }
    // Collapse the exceptional positive contrast. x−period then strictly
    // separates every zero with x=2 and period=0.
    for r in &mut rows {
        if r[0] > 0. {
            r[1] = r[3];
        }
    }
    assert_eq!(
        simplex_only(panel(&rows, Terms::TwoFe), 128)
            .err()
            .unwrap()
            .code,
        ErrorCode::Allocation
    );
}

#[test]
fn positive_twins_cover_unbalanced_aliased_rows_but_reject_single_ulp_mismatch() {
    for terms in [Terms::OrdinaryAlias, Terms::AbsorbedSlope] {
        let mut rows = vec![
            [0., -0., 1., 0.],
            [0., 0., 1., -0.],
            [1., 0., 1., 0.],
            [0., 1.5, 2., 3.],
            [2., 1.5, 2., 3.],
        ];
        let out = simplex_only(panel(&rows, terms), 70).unwrap();
        assert!(out
            .separation
            .methods
            .iter()
            .any(|m| m.certificate.as_ref().is_some_and(|c| matches!(
                c.proof,
                Proof::SimplexPositiveRowCoverage { boundary_rows: 3 }
            ))));
        let coordinator = PreparedSeparationCoordinator::new(panel(&rows, terms)).unwrap();
        let generation = coordinator.view().prepared;
        let topology = DetectorTopology::new(generation.prepared()).unwrap();
        let mut dual = vec![0.; rows.len()];
        assert!(positive_rank::positive_twins(
            generation,
            &topology,
            3,
            70,
            560,
            Some(&mut dual),
            &mut |_| Ok(())
        )
        .unwrap());
        assert_eq!(dual, [1., 1., -2., 1., -1.]);
        // The last zero no longer matches on an ordinary or absorbed slope.
        rows[3][3] = f64::from_bits(3f64.to_bits() + 1);
        assert_eq!(
            simplex_only(panel(&rows, terms), 70).err().unwrap().code,
            ErrorCode::Allocation
        );
    }
}

#[test]
fn aliased_without_twins_remains_inconclusive_under_budget() {
    // Constants within each pair are not required for a finite MLE. With
    // x and 2x both explicit, positive x values 0 and 2 span x=1 in each
    // group, but neither a full raw rank nor an identical row is available.
    let mut rows = vec![];
    for g in [1., 2., 3.] {
        for (y, x) in [(1., 0.), (2., 2.), (0., 1.)] {
            rows.push([y, x, g, 2. * x]);
        }
    }
    assert_eq!(
        simplex_only(panel(&rows, Terms::OrdinaryAlias), 64)
            .err()
            .unwrap()
            .code,
        ErrorCode::Allocation
    );
    assert_eq!(
        simplex_only(panel(&rows, Terms::OrdinaryAlias), 1)
            .err()
            .unwrap()
            .code,
        ErrorCode::Allocation
    );
}

#[test]
fn multiple_other_fe_dummy_blocks_have_independent_offsets() {
    // The missing (1,1) corner equals (1,0)+(0,1)−(0,0) on each firm.
    let mut rows = Vec::new();
    for firm in 0..60 {
        for (region, year, y) in [(0., 0., 1.), (1., 0., 1.), (0., 1., 2.), (1., 1., 0.)] {
            rows.push([y, region, firm as f64, year]);
        }
    }
    let out = simplex_only(panel(&rows, Terms::ThreeFe), 64).unwrap();
    assert!(out
        .separation
        .methods
        .iter()
        .any(|m| m.certificate.as_ref().is_some_and(|c| matches!(
            c.proof,
            Proof::SimplexAnchoredPositiveRank {
                within_columns: 2,
                anchor_levels: 60,
                effects: 3,
                ..
            }
        ))));
}

#[test]
fn exact_global_constants_can_be_removed_from_positive_rank_only_after_verification() {
    let mut rows = Vec::new();
    for group in 0..5 {
        for (y, x) in [(1., 0.), (1., 2.), (0., 1.)] {
            rows.push([y, x, group as f64, 7.]);
        }
    }
    let out = simplex_only(panel(&rows, Terms::OrdinaryAlias), 64).unwrap();
    assert!(out
        .separation
        .methods
        .iter()
        .any(|m| m.certificate.as_ref().is_some_and(|c| matches!(
            c.proof,
            Proof::SimplexAnchoredPositiveRank {
                constant_finite: 1,
                within_columns: 1,
                effects: 1,
                ..
            }
        ))));
    rows[2][3] = f64::from_bits(7f64.to_bits() + 1);
    assert_eq!(
        simplex_only(panel(&rows, Terms::OrdinaryAlias), 64)
            .err()
            .unwrap()
            .code,
        ErrorCode::Allocation
    );
}

#[test]
fn packed_rank_proves_an_alias_and_replays_a_one_ulp_boundary_departure() {
    // The positive rows have one slope after anchoring and no twin for the zero.
    // A rounding-scale change on that zero creates a genuine recession direction.
    for shifted in [false, true] {
        let mut rows = Vec::new();
        for g in [1., 2., 3.] {
            for (y, x) in [(1., 0.), (2., 2.), (0., 1.)] {
                let mut z: f64 = 2. * x + g;
                if shifted && y == 0. && g == 2. {
                    z = f64::from_bits(z.to_bits() + 1);
                }
                rows.push([y, x, g, z]);
            }
        }
        let coordinator =
            PreparedSeparationCoordinator::new(panel(&rows, Terms::OrdinaryAlias)).unwrap();
        let generation = coordinator.view().prepared;
        let topology = DetectorTopology::new(generation.prepared()).unwrap();
        let config = SeparationConfig::default();
        let method = SeparationMethod::IteratedRectifier;
        let run = |cells| {
            positive_rank::aliased(method, generation, &topology, 6, cells, config, &mut |_| {
                Ok(())
            })
            .unwrap()
        };
        assert!(run(1).is_none());
        assert_eq!(run(128).is_some(), !shifted);
        let budget = primal::ProofBudget::new(generation, method, config);
        let ray = positive_rank::packed_candidate(generation, budget).unwrap();
        assert_eq!(ray.is_some(), shifted);
        if let Some(ray) = ray {
            let target = primal::direction(generation, &ray).unwrap().unwrap();
            let strict: Vec<_> = target
                .iter()
                .enumerate()
                .filter(|(_, x)| **x < 0.)
                .map(|(i, _)| generation.original_rows()[i])
                .collect();
            assert_eq!(strict, vec![5]);
            assert!(primal::verify(generation, &ray, &strict, budget, &target).unwrap());
        }
    }
}

#[test]
fn transposed_dual_replays_every_coordinate_and_detects_one_bit_departures() {
    let certify = |rows: &[[f64; 4]], terms, limit| {
        let owner = PreparedSeparationCoordinator::new(panel(rows, terms)).unwrap();
        let generation = owner.view().prepared;
        let topology = DetectorTopology::new(generation.prepared()).unwrap();
        positive_rank::exact_dual(
            SeparationMethod::IteratedRectifier,
            generation,
            &topology,
            rows.iter().filter(|r| r[0] > 0.).count(),
            limit,
            SeparationConfig::default(),
        )
        .unwrap()
        .map(|proposal| proposal.certificate.unwrap().proof)
    };
    for terms in [Terms::OrdinaryAlias, Terms::AbsorbedSlope] {
        for scale in [1., 2.0_f64.powi(-21), 2.0_f64.powi(33)] {
            // Two copies of the negative point plus one positive point sum to
            // three copies of the origin. Splitting the slope coordinates on
            // either zero by one ULP gives a strict recession direction.
            let mut rows = [
                [0., -1., 1., -scale],
                [0., 2., 1., 2. * scale],
                [1., 0., 1., 0.],
            ];
            for _ in 0..2 {
                assert_eq!(
                    certify(&rows, terms, 256),
                    Some(Proof::ExactPositiveDual { boundary_rows: 2 })
                );
                assert!(certify(&rows, terms, 1).is_none());
                let mut shifted = rows;
                let row = shifted.iter_mut().find(|r| r[1] == 2.).unwrap();
                row[3] = f64::from_bits(row[3].to_bits() + 1);
                assert!(certify(&shifted, terms, 256).is_none());
                rows.reverse();
            }
        }
    }
    // Choosing the first zero as pivot and fixing the others at one would
    // require its multiplier to be negative. Pivoting on -1 instead gives
    // positive weights 1, 4, 1 for the three boundary rows.
    let unbalanced = [
        [0., 1., 1., 1.],
        [0., -1., 1., -1.],
        [0., 3., 1., 3.],
        [1., 0., 1., 0.],
    ];
    assert_eq!(
        certify(&unbalanced, Terms::OrdinaryAlias, 256),
        Some(Proof::ExactPositiveDual { boundary_rows: 3 })
    );
    // Six positive observations share the origin. Two boundary coordinates
    // have unequal multiplicities on both sides. A strictly positive dual
    // exists, but its feasible basis requires a non-leading pivot on both
    // axes. The full 14×14 echelon exceeds this memory budget.
    let mut split = vec![[1., 0., 1., 0.]; 6];
    for (x, z, copies) in [(1., 0., 2), (-1., 0., 1), (0., 1., 3), (0., -1., 2)] {
        split.extend(std::iter::repeat_n([0., x, 1., z], copies));
    }
    assert_eq!(
        certify(&split, Terms::OrdinaryAlias, 200),
        Some(Proof::ExactPositiveDual { boundary_rows: 8 })
    );
    // The alternating 2×2 cycle balances both FE axes and the finite slope.
    let mut crossed = [
        [1., 1., 1., 1.],
        [0., 3., 1., 2.],
        [0., 5., 2., 1.],
        [2., 7., 2., 2.],
    ];
    assert_eq!(
        certify(&crossed, Terms::TwoFe, 256),
        Some(Proof::ExactPositiveDual { boundary_rows: 2 })
    );
    crossed[1][1] = f64::from_bits(3f64.to_bits() + 1);
    assert!(certify(&crossed, Terms::TwoFe, 256).is_none());
}

#[test]
fn sparse_raw_dual_replays_joint_cells_and_global_leverage_margin() {
    let check = |groups: usize, shift: f64, opposite: f64, tol: f64, extreme: bool| {
        let mut rows = Vec::new();
        for j in 0..groups {
            let group = j as f64;
            // At the two interior x values the unperturbed local slope is -x.
            // Unequal opposite perturbations need unequal positive boundary
            // weights in their exact inverse ratio.
            let base = if extreme { 0. } else { group / 8. };
            rows.extend([
                [1., base - 1., group, 1. - base],
                [0., base, group, -base + shift],
                [2., base + 1., group, -1. - base],
                [0., base, group, -base + opposite],
            ]);
        }
        rows.rotate_left(1);
        let owner = PreparedSeparationCoordinator::new(panel(&rows, Terms::AbsorbedSlope)).unwrap();
        let model = owner.view().prepared;
        let topology = DetectorTopology::new(model.prepared()).unwrap();
        let config = SeparationConfig {
            certificate_tolerance: tol,
            ..SeparationConfig::default()
        };
        positive_rank::sparse_dual::supports(
            (
                SeparationMethod::Simplex,
                positive_rank::sparse_dual::Search::Full,
            ),
            model,
            &topology,
            2 * groups,
            20_000,
            config,
            &mut |_| Ok(()),
        )
        .unwrap()
    };
    assert!(check(64, -1., 2f64.powi(-12), 1e-7, false));
    assert!(check(64, -2f64.powi(-40), 2f64.powi(-40), 1e-7, false));
    // Each local relation remains exact, but the original-axis normalized
    // dual margin falls below tolerance at this leverage.
    assert!(!check(16, -1., 2f64.powi(-40), 1e-7, false));
    assert!(check(16, -1., 2f64.powi(-40), 1e-15, false));
    // Both zeros now depart on the same side. Their raw scores have one
    // sign in the direction x+slope, even at a one-bit displacement.
    assert!(!check(16, -1., -2f64.powi(-40), 1e-7, false));
    // Converting one column to a common integer scale exceeds i128.
    assert!(!check(1, -1., f64::from_bits(1), 1e-7, true));
}

#[test]
fn sparse_dual_sweeps_balance_asymmetric_boundary_multiplicities() {
    // Fixed-effect margins force the off-diagonal totals to agree. The
    // three repeated 01 rows require weights 1 each and the 10 row needs 3.
    // The finite regressor is additive across corners and must also balance.
    let mut rows = Vec::new();
    for block in 0..16 {
        for (y, a, b) in [
            (2., 0., 0.),
            (1., 1., 1.),
            (0., 0., 1.),
            (0., 0., 1.),
            (0., 0., 1.),
            (0., 1., 0.),
        ] {
            rows.push([
                y,
                0.5 + a + 0.5 * b,
                (2 * block) as f64 + a,
                (2 * block) as f64 + b,
            ]);
        }
    }
    for scale in [1., 2f64.powi(900), 2f64.powi(-900)] {
        let scaled: Vec<_> = rows
            .iter()
            .map(|&[y, x, g, h]| [y, x * scale, g, h])
            .collect();
        let coordinator = PreparedSeparationCoordinator::new(panel(&scaled, Terms::TwoFe)).unwrap();
        let model = coordinator.view().prepared;
        let topology = DetectorTopology::new(model.prepared()).unwrap();
        let check = |search| {
            positive_rank::sparse_dual::supports(
                (SeparationMethod::Simplex, search),
                model,
                &topology,
                32,
                4000,
                SeparationConfig::default(),
                &mut |_| Ok(()),
            )
            .unwrap()
        };
        assert!(!check(positive_rank::sparse_dual::Search::Direct));
        assert!(!check(positive_rank::sparse_dual::Search::Marginal));
        assert!(check(positive_rank::sparse_dual::Search::Full));
        if scale == 1. {
            // The compact 0/1 model cannot represent these raw values;
            // explicit simplex must retain its later full-dual search.
            let fit = simplex_only(panel(&scaled, Terms::TwoFe), 4000).unwrap();
            assert!(fit.separation.dropped_observations.is_empty());
            assert!(fit
                .separation
                .methods
                .iter()
                .any(|m| m.certificate.as_ref().is_some_and(|c| matches!(
                    c.proof,
                    Proof::ExactPositiveDual { boundary_rows: 64 }
                ))));
        }
    }
}

#[test]
fn ir_only_proves_symmetric_tiny_opposing_slope_movements_before_projection() {
    let gap = 2f64.powi(-40);
    let mut rows = Vec::new();
    for j in 0..32 {
        let g = j as f64;
        rows.extend([
            [1., -1., g, 1.],
            [2., 1., g, -1.],
            [0., 0., g, gap],
            [0., 0., g, -gap],
        ]);
    }
    let fit = detect_prepared_ppml_separation(
        panel(&rows, Terms::AbsorbedSlope),
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: true,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        },
        None,
        None,
    )
    .unwrap();
    assert!(fit.separation.dropped_observations.is_empty());
    let report = fit
        .separation
        .methods
        .iter()
        .find(|m| m.method == SeparationMethod::IteratedRectifier)
        .unwrap();
    assert_eq!(report.iterations, 0);
    assert_eq!(
        report.certificate.as_ref().unwrap().proof,
        Proof::ExactPositiveDual { boundary_rows: 64 }
    );
}

#[test]
fn ir_only_composes_local_duals_across_different_retained_faces() {
    let mut rows = Vec::new();
    let gap = 2f64.powi(-40);
    let mut boundary = 0;
    for j in 0..48 {
        let g = j as f64;
        rows.extend([[1., -1., g, 1.], [1., 1., g, -1.], [0., 0., g, 0.]]);
        boundary += 1;
        if j % 2 == 1 {
            rows.extend([[0., 0., g, -gap], [0., 0., g, gap]]);
            boundary += 2;
        }
    }
    rows.rotate_right(3);
    let fit = detect_prepared_ppml_separation(
        panel(&rows, Terms::AbsorbedSlope),
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: true,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        },
        None,
        None,
    )
    .unwrap();
    assert!(fit.separation.dropped_observations.is_empty());
    let report = fit
        .separation
        .methods
        .iter()
        .find(|m| m.method == SeparationMethod::IteratedRectifier)
        .unwrap();
    assert_eq!(report.iterations, 0);
    assert_eq!(
        report.certificate.as_ref().unwrap().proof,
        Proof::ExactPositiveDual {
            boundary_rows: boundary
        }
    );
}

#[test]
fn partition_masses_close_survivor_blocks_on_the_original_axes() {
    // The first two-by-two block retains only positives. The other fifteen
    // blocks have a complete cycle and a shared real-valued control. No
    // nonzero uniform positive weight can cancel the first block's margin.
    let mut rows = Vec::new();
    for block in 0..16 {
        for (a, b) in [(0., 0.), (0., 1.), (1., 0.), (1., 1.)] {
            if block == 0 && a != b {
                continue;
            }
            rows.push([
                f64::from(a == b),
                (block as f64 + 0.5) / 8.,
                (2 * block) as f64 + a,
                (2 * block) as f64 + b,
            ]);
        }
    }
    let owner = PreparedSeparationCoordinator::new(panel(&rows, Terms::TwoFe)).unwrap();
    let model = owner.view().prepared;
    let topo = DetectorTopology::new(model.prepared()).unwrap();
    assert!(positive_rank::sparse_dual::supports(
        (
            SeparationMethod::Simplex,
            positive_rank::sparse_dual::Search::Marginal
        ),
        model,
        &topo,
        32,
        4000,
        SeparationConfig::default(),
        &mut |_| Ok(())
    )
    .unwrap());
}

#[test]
fn finite_minor_replay_retains_one_ulp_positive_contrast_and_rejects_substitution() {
    let owner = |late: f64| {
        let (mut request, clusters, mut loaded, mut uses) = fixture();
        request.absorbed_terms.clear();
        let mut other = request.model.terms[0].clone();
        other.id = TermId::new(2).unwrap();
        other.order = TermOrder::new(2).unwrap();
        other.coefficient_slot = CoefficientSlot::new(2).unwrap();
        other.components[0].source = SourceId::new(2).unwrap();
        request.model.terms.push(other);
        uses[1] = NumericSourceUse::ordinary();
        loaded.outcome = column(&[1., 0., 2., 0., 1., 0.]);
        loaded.numeric_sources[0].1 = column(&[0., 1., 1., -2., 2., -1.]);
        loaded.numeric_sources[1].1 = column(&[0., 2., 1., 1., late, 1.]);
        loaded.numeric_sources[2].1 = column(&[10., 20., 30., 10., 20., 30.]);
        let numeric = prepare_numeric(&loaded, &uses);
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).unwrap()
    };
    let perturbed = f64::from_bits(2f64.to_bits() + 1);
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: true,
            mu: false,
        },
        drop_singletons: false,
        simplex_max_tableau_cells: 24,
        ..SeparationConfig::default()
    };
    let a = PreparedSeparationCoordinator::new(owner(perturbed)).unwrap();
    let model = a.view().prepared;
    let proposal = prepared_simplex_proposal(model, config, &mut |_| Ok(()), &mut None).unwrap();
    assert_eq!(
        proposal.certificate.as_ref().unwrap().proof,
        Proof::SimplexFullPositiveRank { columns: 3 }
    );
    let replay = |model| {
        replay_numerical_no_drop(
            model,
            &proposal,
            PpmlConfig::default(),
            config,
            PreparedOlsFitConfig::default(),
            false,
            &mut |_| Ok(()),
            None,
        )
    };
    assert_eq!(replay(model).unwrap(), proposal.certificate);
    assert!(positive_rank::finite::detect(model, 3, 11, &mut |_| Ok(()))
        .unwrap()
        .is_none());
    assert_eq!(
        positive_rank::finite::detect(model, 3, 24, &mut |_| Err(FeregError::new(
            ErrorCode::Interrupted,
            "Break"
        )))
        .unwrap_err()
        .code,
        ErrorCode::Interrupted
    );
    // Removing the last one-ulp departure makes x1-x2 vanish on every
    // positive row and strictly negative on all three zeros. A certificate
    // from the other model cannot be substituted, despite identical y and IDs.
    let b = PreparedSeparationCoordinator::new(owner(2.)).unwrap();
    assert!(
        positive_rank::finite::detect(b.view().prepared, 3, 24, &mut |_| Ok(()))
            .unwrap()
            .is_none()
    );
    assert!(replay(b.view().prepared).is_err());
    let full = detect_prepared_ppml_separation(
        owner(perturbed),
        PpmlConfig::default(),
        config,
        None,
        None,
    )
    .unwrap();
    assert!(full.separation.dropped_observations.is_empty());
    assert_eq!(
        full.separation.retained_observations,
        (0..6).collect::<Vec<_>>()
    );
    let separated = simplex_only(owner(2.), 4000).unwrap();
    assert_eq!(separated.separation.dropped_observations, [1, 3, 5]);
}

#[test]
fn finite_minor_checks_the_structural_constant_when_requested() {
    let owner = |intercept| {
        let (mut request, mut clusters, mut loaded, mut uses) = fixture();
        clusters.push(OlsClusterTermV1 {
            label: "g".into(),
            sources: vec![SourceId::new(2).unwrap()],
        });
        uses[1] = NumericSourceUse {
            ordinary: false,
            fixed_effect: false,
            cluster: true,
        };
        request.absorbed_terms.clear();
        request.include_intercept = intercept;
        loaded.outcome = column(&[1., 0., 1., 0., 1., 0.]);
        loaded.numeric_sources[0].1 = column(&[1., 0., 1., 0., 1., 0.]);
        let numeric = prepare_numeric(&loaded, &uses);
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).unwrap()
    };
    for intercept in [true, false] {
        let coordinator = PreparedSeparationCoordinator::new(owner(intercept)).unwrap();
        let outcome =
            positive_rank::finite::detect(coordinator.view().prepared, 3, 32, &mut |_| Ok(()))
                .unwrap();
        // Without an intercept, a column taking value one on positives has
        // full rank. Adding the constant permits x-1 to separate the zeros.
        assert_eq!(outcome.is_none(), intercept);
    }
}

#[test]
fn scarce_dual_workspace_can_only_expand_for_a_verified_full_fallback() {
    let check = |adaptive: bool, headroom: bool, opposite: f64| {
        let mut rows = Vec::new();
        for group in 0..64 {
            let g = group as f64;
            rows.extend([
                [1., -1., g, 1.],
                [0., 0., g, -1.],
                [2., 1., g, -1.],
                [0., 0., g, opposite],
            ]);
        }
        let mut owner = panel(&rows, Terms::AbsorbedSlope);
        let mut plan = retry::tests::plan(Some(2 << 30));
        if !headroom {
            plan.usable_bytes = Some(0);
        }
        owner.bind_separation_plan(plan, adaptive);
        let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
        let model = coordinator.view().prepared;
        let topology = DetectorTopology::new(model.prepared()).unwrap();
        let config = SeparationConfig {
            adaptive_sparse: adaptive,
            ..SeparationConfig::default()
        };
        positive_rank::sparse_dual::supports(
            (
                SeparationMethod::IteratedRectifier,
                positive_rank::sparse_dual::Search::Full,
            ),
            model,
            &topology,
            rows.len() / 2,
            20,
            config,
            &mut |_| Ok(()),
        )
        .unwrap()
    };
    let opposite = 2f64.powi(-12);
    assert!(!check(false, true, opposite));
    assert!(!check(true, false, opposite));
    assert!(check(true, true, opposite));
    assert!(!check(true, true, -opposite)); // both boundary directions have the same sign
}
