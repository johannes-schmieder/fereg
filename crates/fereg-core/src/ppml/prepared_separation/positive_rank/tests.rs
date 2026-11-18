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
