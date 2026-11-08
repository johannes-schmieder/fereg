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
