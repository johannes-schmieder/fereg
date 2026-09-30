use super::*;
use crate::{
    data::{DropReason, PreparedAdjustment, PreparedWeights, WeightKind},
    dof::{compute_absorbed_dof, compute_absorbed_dof_polled, DofPolicy},
    spec::ClusterTerm,
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, SourceEntry,
};
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture(n: usize) -> (ModelSpec, Prepared) {
    let id = |value| SourceId::new(value).unwrap();
    let mut model = ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=5)
                .map(|i| SourceEntry {
                    id: id(i),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (1..=3)
            .map(|i| AbsorbedTerm {
                id: AbsorbTermId::new(i).unwrap(),
                categorical_sources: vec![id(i)],
                slope_sources: if i == 3 { vec![id(5)] } else { vec![] },
                intercept: i != 3,
                label: format!("fe{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    for recipe in [vec![1], vec![4], vec![2, 4], vec![4, 2], vec![4]] {
        model.clusters.push(ClusterTerm {
            label: format!("c{}", model.clusters.len()),
            sources: recipe.into_iter().map(id).collect(),
        });
    }
    let sample = Prepared {
        topology: None,
        n_original: n,
        n_candidate: n,
        original_row_by_active: (0..n).collect(),
        outcome: vec![1.; n],
        regressors: vec![
            (0..n).map(|i| ((i * 31) % 2053) as f64).collect(),
            (0..n).map(|i| (i % 17) as f64).collect(),
            (0..n).map(|i| (i % 3) as f64).collect(),
            (0..n).map(|i| ((i * 13) % n) as f64).collect(),
            (0..n)
                .map(|i| if i % 3 == 0 { 0. } else { (i % 5) as f64 })
                .collect(),
        ],
        weights: PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values: (0..n).map(|i| (1 + i % 3) as f64).collect(),
        },
        adjustment: PreparedAdjustment::None,
        drop_reasons: vec![DropReason::Retained; n],
    };
    let sample = prepare_fixed_effects(&model, sample).unwrap();
    (model, sample)
}

fn every_checkpoint<T: std::fmt::Debug + PartialEq>(run: impl Fn(Poll<'_>) -> Result<T>) -> usize {
    let expected = run(None).unwrap();
    let count = AtomicUsize::new(0);
    let observe = || {
        count.fetch_add(1, Ordering::Relaxed);
        true
    };
    assert_eq!(run(Some(&observe)).unwrap(), expected);
    let total = count.load(Ordering::Relaxed);
    for stop in 1..=total {
        count.store(0, Ordering::Relaxed);
        let cancel = || count.fetch_add(1, Ordering::Relaxed) + 1 < stop;
        assert_eq!(run(Some(&cancel)).unwrap_err().code, ErrorCode::Interrupted);
        assert_eq!(count.load(Ordering::Relaxed), stop);
        assert_eq!(run(None).unwrap(), expected);
    }
    total
}

#[test]
fn cluster_coding_validation_and_dof_cancel_every_checkpoint_without_mutating_the_sample() {
    let (model, sample) = fixture(4099);
    let original = fixture(4099).1;
    let frame = ClusterFrame::new(&model, &sample).unwrap();
    let views = frame.views().unwrap();
    let codes: Vec<_> = views.iter().map(|view| view.codes).collect();
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let cluster_points = team.install(|| {
            every_checkpoint(|poll| {
                let frame = ClusterFrame::new_polled(&model, &sample, poll)?;
                let views = frame.views()?;
                assert!(std::ptr::eq(views[0].codes, codes[0]));
                assert!(std::ptr::eq(views[1].codes, views[4].codes));
                assert!(std::ptr::eq(views[2].codes, views[3].codes));
                Ok(views
                    .iter()
                    .map(|v| (v.codes.to_vec(), v.n_levels))
                    .collect::<Vec<_>>())
            })
        });
        assert!(cluster_points > 100);
        let mut dof_points = vec![];
        for policy in [
            DofPolicy::None,
            DofPolicy::FirstPair,
            DofPolicy::Pairwise,
            DofPolicy::Continuous,
        ] {
            for clusters in [&codes[..0], &codes[..1], &codes[..]] {
                let expected = compute_absorbed_dof(&sample, true, policy, clusters).unwrap();
                assert_eq!(
                    expected,
                    compute_absorbed_dof_polled(&sample, true, policy, clusters, Some(&|| true))
                        .unwrap()
                );
                let points = team.install(|| {
                    every_checkpoint(|poll| {
                        compute_absorbed_dof_polled(&sample, true, policy, clusters, poll)
                    })
                });
                assert!(points > 20);
                dof_points.push(points);
            }
        }
        let state = (cluster_points, dof_points);
        assert_eq!(*reference.get_or_insert(state.clone()), state);
        assert_eq!(sample, original);
    }
}

#[test]
fn metadata_errors_keep_their_identity_with_polling_enabled() {
    let (model, mut sample) = fixture(4099);
    sample.drop_reasons[4098] = DropReason::Singleton;
    let plain = sample.validate_alignment().unwrap_err();
    assert_eq!(
        plain,
        sample
            .validate_alignment_polled(Some(&|| true))
            .unwrap_err()
    );
    assert!(ClusterFrame::new_polled(&model, &sample, Some(&|| true)).is_err());
    sample.drop_reasons[4098] = DropReason::Retained;
    sample.regressors[4][4098] = f64::NAN;
    let topology = sample.topology.as_ref().unwrap();
    assert_eq!(
        topology.validate(&sample).unwrap_err(),
        topology
            .validate_polled(&sample, Some(&|| true))
            .unwrap_err()
    );
    sample.regressors[4][4098] = 0.;
    let mut invalid = vec![0u32; sample.n_active()];
    invalid[4098] = u32::MAX;
    assert_eq!(
        compute_absorbed_dof(&sample, true, DofPolicy::Pairwise, &[&invalid]).unwrap_err(),
        compute_absorbed_dof_polled(
            &sample,
            true,
            DofPolicy::Pairwise,
            &[&invalid],
            Some(&|| true)
        )
        .unwrap_err()
    );
}
