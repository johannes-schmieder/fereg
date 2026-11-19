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
