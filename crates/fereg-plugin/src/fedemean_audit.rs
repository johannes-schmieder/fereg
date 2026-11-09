//! Allocator audit of fedemean's reuse contract: once a projector has
//! projected one width, repeated projections of that width allocate nothing,
//! also when new weight values are written into the same weight buffer.

use fedemean::{
    CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Design, Effect, Outcome, Projector,
    Request, Settings, Weights,
};

/// SplitMix64.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn uniform(state: &mut u64) -> f64 {
    (next(state) >> 11) as f64 / (1_u64 << 53) as f64
}

const SPELL: usize = 5;
const FIRMS: usize = 12_000;
const YEARS: usize = 25;

/// Worker, firm and year codes of a mobility panel: every worker has
/// `SPELL` consecutive years and moves firms with probability 0.2 a year.
/// Enough firms that the reduced-space vector passes run in parallel.
fn panel(n: usize) -> [Vec<u32>; 3] {
    let mut state = 0x00a1_10c5;
    let (mut worker, mut firm, mut year) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    let mut current = 0;
    for row in 0..n {
        let (w, t) = (row / SPELL, row % SPELL);
        if t == 0 {
            // The first workers start at distinct firms: every firm is used.
            current = if w < FIRMS {
                w
            } else {
                (next(&mut state) % FIRMS as u64) as usize
            };
        } else if uniform(&mut state) < 0.2 {
            current = (next(&mut state) % FIRMS as u64) as usize;
        }
        worker.push(w as u32);
        firm.push(current as u32);
        year.push((w % (YEARS - SPELL + 1) + t) as u32);
    }
    [worker, firm, year]
}

#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn fedemean_repeated_projections_with_new_weights_allocate_nothing() {
    let _guard = crate::instrumentation::lock_action_observation();
    let n = 200_000;
    let codes = panel(n);
    let levels = [n.div_ceil(SPELL), FIRMS, YEARS];
    let offsets = [0, levels[0], levels[0] + levels[1]];
    let per_rhs: usize = levels.iter().sum();
    let mut state = 0x0bad_5eed;
    let base: Vec<f64> = (0..n).map(|_| 0.25 + uniform(&mut state)).collect();
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        drop(pool.broadcast(|_| ()));
        pool.install(|| {
            let effects = codes
                .iter()
                .zip(levels)
                .map(|(codes, levels)| Effect::new(codes, levels))
                .collect();
            let design = Design::new(n, effects).unwrap();
            let mut projector = Projector::new(design, Settings::default());
            let control = Control::default();
            // One batch with the residual Gram, then two batches (8 + 3).
            for (ncols, gram) in [(3, true), (11, false)] {
                let original: Vec<f64> =
                    (0..n * ncols).map(|_| uniform(&mut state) - 0.5).collect();
                let mut values = original.clone();
                let mut coefficients = vec![0.0; per_rhs * ncols];
                let mut weights = base.clone();
                let mut request = Request::new(1e-10, 1000);
                request.residual_gram = gram;
                for rep in 0..4 {
                    values.copy_from_slice(&original);
                    coefficients.fill(0.0);
                    if rep > 0 {
                        let shift = 0.05 * rep as f64;
                        for (i, (w, b)) in weights.iter_mut().zip(&base).enumerate() {
                            *w = b * (1.0 + shift * ((i % 3) as f64 - 1.0));
                        }
                    }
                    let (start, _) = crate::ffi::begin_allocation_observation();
                    let outcome = projector
                        .project(
                            Weights::Values(&weights),
                            ColumnsMut::new(&mut values, n, ncols).unwrap(),
                            Some(CoefficientsMut::new(
                                &mut coefficients,
                                CoefficientLayout::new(per_rhs, &offsets, &levels),
                            )),
                            &request,
                            &control,
                        )
                        .unwrap();
                    let usage = crate::ffi::allocation_usage();
                    let report = match outcome {
                        Outcome::Projected(report) => report,
                        other => panic!("{threads} workers, {ncols} columns: {other:?}"),
                    };
                    assert!(report.certificate.passed);
                    assert_eq!(projector.residual_gram().is_some(), gram);
                    eprintln!(
                        "fedemean audit: workers={threads} columns={ncols} rep={rep} \
                         lanes={} iterations={} events={} peak={} start={} end={}",
                        report.facts.lanes,
                        report.facts.iterations,
                        usage.events - start.events,
                        usage.peak_bytes,
                        start.current_bytes,
                        usage.current_bytes
                    );
                    if threads > 1 {
                        assert!(report.facts.lanes > 1);
                    }
                    if rep == 0 {
                        // The first call of each width builds its workspace.
                        continue;
                    }
                    assert_eq!(
                        usage.events, start.events,
                        "{threads} workers, {ncols} columns, repetition {rep} allocated"
                    );
                    assert_eq!(usage.peak_bytes, start.current_bytes);
                    assert_eq!(usage.current_bytes, start.current_bytes);
                }
            }
        });
    }
}

/// A worker-firm(-year) mobility panel: `spell` years per worker, a move
/// with probability `mobility` each year, the first workers at distinct
/// firms.
fn mobility(n: usize, firms: usize, spell: usize, mobility: f64) -> [Vec<u32>; 3] {
    let mut state = 0x5c4a_7200 + firms as u64;
    let (mut worker, mut firm, mut year) = (Vec::new(), Vec::new(), Vec::new());
    let mut current = 0;
    for row in 0..n {
        let (w, t) = (row / spell, row % spell);
        if t == 0 {
            current = if w < firms {
                w
            } else {
                (next(&mut state) % firms as u64) as usize
            };
        } else if uniform(&mut state) < mobility {
            current = (next(&mut state) % firms as u64) as usize;
        }
        worker.push(w as u32);
        firm.push(current as u32);
        year.push((w % (YEARS - spell + 1) + t) as u32);
    }
    [worker, firm, year]
}

/// Bytes the projector allocates beside its reported workspace: the
/// report's per-column vectors.
const REPORT_SLACK: u64 = 64 << 10;

/// fedemean's additive Schwarz charges its build before it allocates, so
/// a cold projection that builds it stays within its reported workspace
/// (and `admission_bytes`), which holds the build's peak with its
/// transients: approximate, component, dense and envelope local solves,
/// the star and all pairs, a pair with slopes, one and four workers. At a
/// memory limit of exactly the reported bytes the projection runs; one
/// byte below it declines, and the declined projection's allocations stay
/// within that limit.
#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn fedemean_schwarz_build_stays_within_its_reported_workspace() {
    use fedemean::{
        Choice, Decline, Fallback, Method, Preconditioner, Reduction, SchwarzLocal, SchwarzSpec,
        Subdomains,
    };
    let _guard = crate::instrumentation::lock_action_observation();
    let auto = (Subdomains::Star, SchwarzLocal::Auto);
    let approximate = (Subdomains::Star, SchwarzLocal::Approximate);
    let exact = (Subdomains::Star, SchwarzLocal::Exact);
    let all = (Subdomains::AllPairs, SchwarzLocal::Auto);
    // (rows, firms, spell, mobility, effects, slope, specifications)
    let designs = [
        (200_000, 12_000, 5, 0.2, 2, false, vec![auto, approximate]),
        (200_000, 12_000, 5, 0.2, 3, false, vec![auto, all]),
        (200_000, 2_000, 5, 0.5, 2, false, vec![auto]),
        (200_000, 5_000, 8, 0.8, 2, false, vec![approximate]),
        (100_000, 300, 6, 0.6, 3, false, vec![auto, exact]),
        (100_000, 2_000, 5, 0.3, 2, true, vec![auto]),
    ];
    let mut state = 0x0bad_5eed;
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        drop(pool.broadcast(|_| ()));
        pool.install(|| {
            for (n, firms, spell, moves, k, slope, specs) in &designs {
                let codes = mobility(*n, *firms, *spell, *moves);
                let levels = [n.div_ceil(*spell), *firms, YEARS];
                let weights: Vec<f64> = (0..*n).map(|_| (2.0 * uniform(&mut state)).exp()).collect();
                let trend: Vec<f64> = codes[2].iter().map(|&t| f64::from(t)).collect();
                let channels: Vec<usize> = (0..*k).map(|e| 1 + usize::from(*slope && e == 1)).collect();
                let per: usize = (0..*k).map(|e| levels[e] * channels[e]).sum();
                let offsets: Vec<usize> = (0..*k)
                    .scan(0, |s, e| {
                        let o = *s;
                        *s += levels[e] * channels[e];
                        Some(o)
                    })
                    .collect();
                for &(subdomains, local) in specs {
                    let label = format!("{threads} workers, {n} rows, {firms} firms, {k} effects, slope {slope}, {subdomains:?} {local:?}");
                    let preconditioner = Preconditioner::Schwarz(SchwarzSpec { subdomains, local });
                    let run = |limit: Option<u128>| {
                        let effects = (0..*k)
                            .map(|e| {
                                let effect = Effect::new(&codes[e], levels[e]);
                                if *slope && e == 1 {
                                    effect.with_slopes(true, [trend.as_slice()]).unwrap()
                                } else {
                                    effect
                                }
                            })
                            .collect();
                        let design = Design::new(*n, effects).unwrap();
                        let choice = Choice {
                            reduction: Reduction::Full,
                            method: Method::Pcg,
                            preconditioner,
                            fallback: Fallback::Decline,
                        };
                        let mut projector = Projector::new(design, Settings { choice, lanes: Some(4) });
                        let ncols = 2;
                        let mut values: Vec<f64> = (0..n * ncols).map(|i| ((i % 97) as f64) - 48.0).collect();
                        let mut coefficients = vec![0.0; per * ncols];
                        let layout = CoefficientLayout::new(per, &offsets, &levels[..*k]).with_channels(&channels);
                        let mut request = Request::new(1e-8, 2000);
                        request.memory_limit = limit;
                        let (start, _) = crate::ffi::begin_allocation_observation();
                        let outcome = projector
                            .project(
                                Weights::Values(&weights),
                                ColumnsMut::new(&mut values, *n, ncols).unwrap(),
                                Some(CoefficientsMut::new(&mut coefficients, layout)),
                                &request,
                                &Control::default(),
                            )
                            .unwrap();
                        let peak = crate::ffi::allocation_usage().peak_bytes - start.current_bytes;
                        (outcome, peak, projector.admission_bytes(ncols))
                    };
                    let (outcome, peak, admission) = run(None);
                    let report = match outcome {
                        Outcome::Projected(report) => report,
                        other => panic!("{label}: {other:?}"),
                    };
                    let reported = report.workspace_bytes;
                    eprintln!("fedemean Schwarz audit: {label}: peak {peak} reported {reported} ({:.2})", peak as f64 / reported as f64);
                    assert!(u128::from(peak) <= reported + u128::from(REPORT_SLACK), "{label}: peak {peak} > reported {reported}");
                    assert_eq!(admission, Some(reported), "{label}");
                    let (outcome, peak, _) = run(Some(reported));
                    assert!(matches!(outcome, Outcome::Projected(_)), "{label}: {outcome:?}");
                    assert!(u128::from(peak) <= reported + u128::from(REPORT_SLACK), "{label}");
                    let below = reported - 1;
                    let (outcome, peak, _) = run(Some(below));
                    assert!(
                        matches!(outcome, Outcome::Declined { reason: Decline::MemoryLimit, .. }),
                        "{label}: {outcome:?}"
                    );
                    assert!(u128::from(peak) <= below + u128::from(REPORT_SLACK), "{label}: declined at peak {peak}");
                }
            }
        });
    }
}
