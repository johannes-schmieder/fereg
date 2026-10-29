//! Standalone allocator/science witness. No production allocation interface.
//! Run each dependency revision and worker count in a fresh process; no timings.
use rayon::prelude::*;
use schwarz_precond::{lsmr, mlsmr, MlsmrOptions, Operator, SolveError};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use within::{Design, Effect, LsmrOptions, PreconditionerConfig, Solver};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
struct Measured;
fn added(bytes: usize) {
    let live = LIVE.fetch_add(bytes, SeqCst) + bytes;
    PEAK.fetch_max(live, SeqCst);
}
// SAFETY: forwards each operation and its unchanged pointer/layout to System;
// the counters allocate nothing and are updated only for successful allocations.
unsafe impl GlobalAlloc for Measured {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            added(layout.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            added(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        LIVE.fetch_sub(layout.size(), SeqCst);
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, size) };
        if !q.is_null() {
            if size >= layout.size() {
                added(size - layout.size());
            } else {
                LIVE.fetch_sub(layout.size() - size, SeqCst);
            }
        }
        q
    }
}
#[global_allocator]
static ALLOCATOR: Measured = Measured;

fn measured<T>(f: impl FnOnce() -> T) -> (T, usize, isize) {
    let baseline = LIVE.load(SeqCst);
    PEAK.store(baseline, SeqCst);
    let value = f();
    let peak = PEAK.load(SeqCst).saturating_sub(baseline);
    let retained = LIVE.load(SeqCst) as isize - baseline as isize;
    (value, peak, retained)
}

/// Two copies of a diagonal system. Its exact least-squares optimum is known.
struct Rectangular {
    n: usize,
}
impl Rectangular {
    fn scale(j: usize) -> f64 {
        1.0 + (j % 17) as f64 / 16.0
    }
}
impl Operator for Rectangular {
    fn nrows(&self) -> usize {
        2 * self.n
    }
    fn ncols(&self) -> usize {
        self.n
    }
    fn apply(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        for (i, out) in y.iter_mut().enumerate() {
            let j = i % self.n;
            *out = Self::scale(j) * x[j];
        }
        Ok(())
    }
    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        for (j, out) in y.iter_mut().enumerate() {
            *out = Self::scale(j) * (x[j] + x[j + self.n]);
        }
        Ok(())
    }
}
struct Identity(usize);
impl Operator for Identity {
    fn nrows(&self) -> usize {
        self.0
    }
    fn ncols(&self) -> usize {
        self.0
    }
    fn apply(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        y.copy_from_slice(x);
        Ok(())
    }
    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        self.apply(x, y)
    }
}

fn recurrence(n: usize, history: Option<usize>, seeded: bool, modified: bool) {
    let op = Rectangular { n };
    let truth: Vec<_> = (0..n).map(|j| ((j % 13) as f64 - 6.0) / 13.0).collect();
    let seed: Vec<_> = truth.iter().map(|x| 0.75 * x).collect();
    let b: Vec<_> = (0..2 * n)
        .map(|i| Rectangular::scale(i % n) * truth[i % n] + if i < n { 0.125 } else { -0.125 })
        .collect();
    let (result, peak, retained) = measured(|| {
        if modified {
            mlsmr(
                &op,
                &b,
                &Identity(n),
                1e-12,
                256,
                MlsmrOptions {
                    warm_start: seeded.then_some(seed.as_slice()),
                    local_size: history,
                    ..Default::default()
                },
            )
            .expect("modified recurrence")
        } else {
            assert!(!seeded);
            lsmr(&op, &b, 1e-12, 256, history).expect("plain recurrence")
        }
    });
    let error = result
        .x
        .iter()
        .zip(&truth)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    assert!(
        result.converged && error < 1e-10,
        "independent diagonal optimum {error}"
    );
    let h = history.unwrap_or(0).min(n);
    let ordinary = 8usize
        .checked_mul(4 * n + n * if modified { 6 + 2 * h } else { 5 + h })
        .unwrap();
    // An audited restart can retain its previous coefficient vector.
    let bound = ordinary.checked_add(8 * n).unwrap();
    assert!(peak <= bound, "recurrence peak {peak} exceeds {bound}");
    println!("{{\"phase\":\"recurrence\",\"n\":{n},\"history\":{h},\"seeded\":{seeded},\"modified\":{modified},\"peak\":{peak},\"retained\":{retained},\"ordinary_payload\":{ordinary},\"restart_bound\":{bound},\"max_error\":{error:e},\"iterations\":{}}}", result.iterations);
}

fn additive() -> PreconditionerConfig {
    PreconditionerConfig::Additive {
        local_solver: Default::default(),
        #[cfg(not(feature = "repro-stack"))]
        reduction: within::ReductionStrategy::ParallelReduction,
    }
}

fn within_case(
    rows: usize,
    levels: usize,
    threads: usize,
    weighted: bool,
    sloped: bool,
    rhs: usize,
    mode: &str,
) {
    // Dominant factor is already sorted; the second is deliberately unsorted.
    // With n/L below 12 it reaches Owner; above 12 it reaches Fold in #478.
    let f: Vec<u32> = (0..rows).map(|i| (i / (rows / levels)) as u32).collect();
    let g: Vec<u32> = (0..rows)
        .map(|i| ((i * 97 + i / levels) % levels) as u32)
        .collect();
    let z: Vec<f64> = (0..rows).map(|i| ((i % 31) as f64 - 15.0) / 16.0).collect();
    let w: Vec<f64> = (0..rows).map(|i| 1.0 + (i % 7) as f64 / 8.0).collect();
    let y: Vec<f64> = (0..rows)
        .map(|i| {
            (f[i] % 17) as f64 / 17.0 - (g[i] % 13) as f64 / 13.0
                + if sloped {
                    z[i] * (f[i] % 11) as f64 / 11.0
                } else {
                    0.0
                }
        })
        .collect();
    let effects = vec![
        Effect::new(&f, true, if sloped { vec![z.as_slice()] } else { vec![] }).unwrap(),
        Effect::new(&g, true, []).unwrap(),
    ];
    let (design, build_peak, design_retained) = measured(|| Design::new(effects).unwrap());
    let config = match mode {
        "off" => PreconditionerConfig::Off,
        "diagonal" => PreconditionerConfig::Diagonal,
        "additive" => additive(),
        _ => unreachable!(),
    };
    let (solver, prep_peak, prep_retained) =
        measured(|| Solver::new(&design, weighted.then_some(w.as_slice()), config).unwrap());
    let responses: Vec<&[f64]> = vec![y.as_slice(); rhs];
    let opts = LsmrOptions {
        tol: 1e-10,
        maxiter: 512,
        local_size: Some(4),
    };
    let (result, peak, retained) =
        measured(|| solver.solve_batch(&responses, Some(&opts)).unwrap());
    let max_residual = result.demeaned.iter().map(|x| x.abs()).fold(0.0, f64::max);
    assert!(
        result.converged.iter().all(|v| *v) && max_residual < 1e-7,
        "exact model span {max_residual}"
    );
    // This science check is independent of the recurrence's residual report:
    // y was explicitly constructed as f + g [+ f*z], so its projection is zero.
    let (repeat, repeat_peak, repeat_retained) =
        measured(|| solver.solve_batch(&responses, Some(&opts)).unwrap());
    let repeats_exactly = result
        .x
        .iter()
        .chain(&result.demeaned)
        .map(|x| x.to_bits())
        .eq(repeat.x.iter().chain(&repeat.demeaned).map(|x| x.to_bits()))
        && result.iterations == repeat.iterations;
    if cfg!(feature = "repro-stack") {
        assert!(repeats_exactly, "fixed-worker repeat failed");
    }
    println!("{{\"phase\":\"within\",\"rows\":{rows},\"levels\":{levels},\"threads\":{threads},\"weighted\":{weighted},\"sloped\":{sloped},\"rhs\":{rhs},\"mode\":\"{mode}\",\"design_peak\":{build_peak},\"design_retained\":{design_retained},\"prepare_peak\":{prep_peak},\"prepare_retained\":{prep_retained},\"solve_peak\":{peak},\"solve_retained\":{retained},\"repeat_peak\":{repeat_peak},\"repeat_retained\":{repeat_retained},\"max_abs_residual\":{max_residual:e},\"repeats_exactly\":{repeats_exactly}}}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let threads = args.get(1).map_or(1, |s| s.parse::<usize>().unwrap());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    let recurrence_only = args.iter().any(|arg| arg == "--recurrence-only");
    pool.install(|| {
        // Warm Rayon before opening a measurement window; no timing is recorded.
        (0..1024)
            .into_par_iter()
            .for_each(|_| std::hint::black_box(()));
        for n in [32, 4096] {
            for h in [None, Some(4), Some(if n == 32 { 100 } else { 8 })] {
                recurrence(n, h, false, false);
                recurrence(n, h, false, true);
                recurrence(n, h, true, true);
            }
        }
        if recurrence_only {
            return;
        }
        for mode in ["off", "diagonal", "additive"] {
            for (rows, levels, weighted, sloped, rhs) in [
                (8192, 1024, false, false, 1),
                (131072, 64, true, false, 4),
                (8192, 512, true, true, 1),
                (8192, 1024, false, false, 65),
            ] {
                within_case(rows, levels, threads, weighted, sloped, rhs, mode);
            }
        }
    });
}
