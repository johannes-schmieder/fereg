use super::*;

fn fixture(rows: usize, weights: TestWeights<'_>) -> PreparedOlsData {
    let x = (0..rows)
        .map(|r| (r * 37 % 257) as f64 / 64.0 - 2.0)
        .collect::<Vec<_>>();
    let z = (0..rows)
        .map(|r| (r * 19 % 113) as f64 / 32.0 - 1.0)
        .collect::<Vec<_>>();
    let groups = (0..rows).map(|r| (r % 17) as f64).collect::<Vec<_>>();
    let y = (0..rows)
        .map(|r| groups[r] + 1.25 * x[r] - 0.75 * z[r] + (r * 7 % 31) as f64 / 128.0)
        .collect::<Vec<_>>();
    prepared(
        &y,
        &[x.clone(), z, x.clone(), x],
        &[false, false, false, true],
        Some(&groups),
        true,
        weights,
    )
}

fn owned_config() -> PreparedOlsFitConfig {
    PreparedOlsFitConfig {
        ownership: PreparedOlsOwnership {
            retain_projected_scores: true,
            ..PreparedOlsOwnership::default()
        },
        retained_score_capacity_limit: Some(u128::MAX),
        ..PreparedOlsFitConfig::default()
    }
}

fn run<R: FitReservation + Send>(
    data: &PreparedOlsData,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    retain: bool,
    cache_limit: usize,
) -> Result<CertifiedPreparedOlsFit> {
    ThreadTeam::new(config.execution.threads).install(|| {
        fit_with_controls_internal_mode(
            data,
            config,
            reservation,
            FitPerturbations::default(),
            retain,
            cache_limit,
        )
    })?
}

fn untimed(mut fit: CertifiedPreparedOlsFit) -> CertifiedPreparedOlsFit {
    fit.absorption_report.certification_seconds = 0.0;
    fit.residual_absorption_report.certification_seconds = 0.0;
    without_solve_timings(fit)
}

fn exact_fit_and_scores(mut packed: CertifiedPreparedOlsFit, mut owned: CertifiedPreparedOlsFit) {
    assert!(packed.rank_certificate.passed && owned.rank_certificate.passed);
    assert!(packed.score_certificate.passed && owned.score_certificate.passed);
    let packed_cache = packed
        .fast_score_cache
        .take()
        .unwrap()
        .into_kept(&packed.kept_generated_columns, &mut SystemFitReservation)
        .unwrap();
    let owned_cache = owned.fast_score_cache.take().unwrap();
    let PreparedOlsFastScoreCache::Packed {
        generated_columns: a,
        within: x,
        means: am,
    } = packed_cache
    else {
        panic!("packed control")
    };
    let PreparedOlsFastScoreCache::Selected {
        generated_columns: b,
        within: y,
        means: bm,
    } = owned_cache
    else {
        panic!("admitted selected ownership")
    };
    assert_eq!(a, b);
    assert_eq!(
        am.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        bm.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
    for row in 0..x.nrows() {
        for (column, generated) in a.iter().copied().enumerate() {
            assert_eq!(
                x[(row, column)].to_bits(),
                y[(row, generated + 1)].to_bits()
            );
        }
    }
    // Every certificate, residual, coefficient, finite decision and FE report
    // (including solver signatures) remains exact; only clocks and cache layout differ.
    assert_eq!(untimed(packed), untimed(owned));
}

#[test]
fn ownership_preserves_packed_bits_and_explicit_solver_reports() {
    let data = fixture(32_771, TestWeights::Unit);
    for workers in [1, 4] {
        for solver in [crate::fe::FeSolverKind::Auto, crate::fe::FeSolverKind::Lsmr] {
            let mut config = owned_config();
            config.execution.threads = ThreadConfig::from_requested(workers, true).unwrap();
            config.absorb.solver = solver;
            config.absorb.lsmr.preconditioner = crate::fe::LsmrPreconditioner::Diagonal;
            let mut baseline = config;
            baseline.ownership = PreparedOlsOwnership::default();
            let mut old_allocations = InjectedReservation::default();
            let old = run(&data, baseline, &mut old_allocations, true, usize::MAX).unwrap();
            let mut new_allocations = InjectedReservation::default();
            let new = run(&data, config, &mut new_allocations, true, usize::MAX).unwrap();
            let is_copy = |record: &(usize, &'static str)| {
                record.1 == "unable to reserve prepared OLS fast score cache values"
            };
            assert!(old_allocations.records.iter().any(is_copy));
            assert!(!new_allocations.records.iter().any(is_copy));
            exact_fit_and_scores(old, new);
        }
    }
}

#[test]
fn ownership_admission_uses_full_capacity_before_omission() {
    let data = fixture(103, TestWeights::Unit);
    let mut owned = run(
        &data,
        owned_config(),
        &mut SystemFitReservation,
        true,
        usize::MAX,
    )
    .unwrap();
    let bytes_with_cache = owned.owned_buffer_bytes();
    let cache = owned.fast_score_cache.take().unwrap();
    let PreparedOlsFastScoreCache::Selected {
        generated_columns,
        within,
        means,
    } = cache
    else {
        panic!("selected cache")
    };
    let full_bytes = within.storage_capacity() as u128 * 8;
    let metadata_bound =
        data.model.columns.len() as u128 * (std::mem::size_of::<usize>() as u128 + 8);
    let admission = full_bytes + metadata_bound;
    assert!(generated_columns.len() < data.model.columns.len());
    assert_eq!(
        bytes_with_cache - owned.owned_buffer_bytes(),
        full_bytes
            + generated_columns.capacity() as u128 * std::mem::size_of::<usize>() as u128
            + means.capacity() as u128 * 8
    );
    for (ceiling, selected) in [
        (None, false),
        (Some(admission - 1), false),
        (Some(admission), true),
    ] {
        let config = PreparedOlsFitConfig {
            retained_score_capacity_limit: ceiling,
            ..owned_config()
        };
        let fit = run(&data, config, &mut SystemFitReservation, true, usize::MAX).unwrap();
        assert_eq!(
            matches!(
                fit.fast_score_cache,
                Some(PreparedOlsFastScoreCache::Selected { .. })
            ),
            selected
        );
    }
}

#[test]
fn ownership_keeps_weighted_large_cache_and_no_score_paths_unchanged() {
    let weights = (0..103).map(|r| (r % 5 + 1) as f64).collect::<Vec<_>>();
    for weight in [
        TestWeights::Analytic(&weights),
        TestWeights::Probability(&weights),
        TestWeights::Frequency(&weights),
    ] {
        let data = fixture(103, weight);
        let old = run(
            &data,
            PreparedOlsFitConfig::default(),
            &mut SystemFitReservation,
            true,
            usize::MAX,
        )
        .unwrap();
        let new = run(
            &data,
            owned_config(),
            &mut SystemFitReservation,
            true,
            usize::MAX,
        )
        .unwrap();
        assert!(matches!(
            new.fast_score_cache,
            Some(PreparedOlsFastScoreCache::Packed { .. })
        ));
        assert_eq!(untimed(old), untimed(new));
    }
    let data = fixture(103, TestWeights::Unit);
    for (retain, limit) in [(true, 0), (false, usize::MAX)] {
        let old = run(
            &data,
            PreparedOlsFitConfig::default(),
            &mut SystemFitReservation,
            retain,
            limit,
        )
        .unwrap();
        let new = run(
            &data,
            owned_config(),
            &mut SystemFitReservation,
            retain,
            limit,
        )
        .unwrap();
        assert_eq!(untimed(old), untimed(new));
    }
}

#[test]
fn ownership_handles_empty_accepted_basis_without_losing_full_charge() {
    let groups = (0..103).map(|r| (r % 7) as f64).collect::<Vec<_>>();
    let y = (0..103)
        .map(|r| groups[r] + (r % 5) as f64 / 8.0)
        .collect::<Vec<_>>();
    for x in [Vec::new(), vec![vec![1.0; 103]]] {
        let data = prepared(
            &y,
            &x,
            &vec![false; x.len()],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        let old = run(
            &data,
            PreparedOlsFitConfig::default(),
            &mut SystemFitReservation,
            true,
            usize::MAX,
        )
        .unwrap();
        let new = run(
            &data,
            owned_config(),
            &mut SystemFitReservation,
            true,
            usize::MAX,
        )
        .unwrap();
        assert!(new.kept_generated_columns.is_empty());
        let Some(PreparedOlsFastScoreCache::Selected { within, .. }) = &new.fast_score_cache else {
            panic!("empty selected basis retains projected owner")
        };
        assert_eq!(within.ncols(), data.model.columns.len() + 1);
        exact_fit_and_scores(old, new);
    }
}

#[test]
fn ownership_allocation_failure_is_fatal_and_actual_capacity_is_checked() {
    let data = fixture(103, TestWeights::Unit);
    let mut reservations = InjectedReservation::default();
    run(&data, owned_config(), &mut reservations, true, usize::MAX).unwrap();
    let mut checked = 0;
    for (index, (_, label)) in reservations.records.iter().enumerate() {
        if matches!(
            *label,
            "unable to reserve prepared OLS strided score identities"
                | "unable to reserve prepared OLS fast score cache means"
                | "unable to reserve prepared OLS fast score cache partials"
        ) {
            let mut failed = InjectedReservation {
                fail_at: Some(index + 1),
                ..InjectedReservation::default()
            };
            assert!(run(&data, owned_config(), &mut failed, true, usize::MAX).is_err());
            checked += 1;
        }
    }
    assert!(checked >= 2);
    struct ExcessMetadata;
    impl FitReservation for ExcessMetadata {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            label: &'static str,
        ) -> Result<()> {
            let excess = if label == "unable to reserve prepared OLS strided score identities" {
                100
            } else {
                0
            };
            target
                .try_reserve_exact(elements + excess)
                .map_err(|_| allocation("test reserve"))
        }
    }
    let ceiling = data.sample.n_active() as u128 * (data.model.columns.len() as u128 + 1) * 8
        + data.model.columns.len() as u128 * (std::mem::size_of::<usize>() as u128 + 8);
    let config = PreparedOlsFitConfig {
        retained_score_capacity_limit: Some(ceiling),
        ..owned_config()
    };
    let error = run(&data, config, &mut ExcessMetadata, true, usize::MAX).unwrap_err();
    assert_eq!(
        error,
        allocation("owned projected score capacity exceeds admission")
    );
}
