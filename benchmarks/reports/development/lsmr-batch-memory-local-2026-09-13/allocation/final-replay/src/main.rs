//! Isolated diagnostic; this allocator is not linked into fereg's safe core.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::time::Instant;
use within::{Effect, LsmrOptions, PreconditionerConfig, Solver};
use rayon::prelude::*;
use schwarz_precond::{mlsmr, MlsmrOptions, Operator, SolveError};
use std::sync::Barrier;

struct Tracker;
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static WATCH_SIZE: AtomicUsize = AtomicUsize::new(usize::MAX);
static WATCH_CURRENT: AtomicUsize = AtomicUsize::new(0);
static WATCH_PEAK: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: Tracker = Tracker;
fn add(bytes: usize) {
    let live = CURRENT.fetch_add(bytes, SeqCst) + bytes;
    PEAK.fetch_max(live, SeqCst);
}
fn watch_add(bytes: usize) {
    if bytes == WATCH_SIZE.load(SeqCst) {
        let count = WATCH_CURRENT.fetch_add(1, SeqCst) + 1;
        WATCH_PEAK.fetch_max(count, SeqCst);
    }
}
fn watch_remove(bytes: usize) {
    if bytes == WATCH_SIZE.load(SeqCst) { WATCH_CURRENT.fetch_sub(1, SeqCst); }
}
unsafe impl GlobalAlloc for Tracker {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() { add(layout.size()); watch_add(layout.size()); }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() { add(layout.size()); watch_add(layout.size()); }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        CURRENT.fetch_sub(layout.size(), SeqCst);
        watch_remove(layout.size());
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            watch_remove(layout.size());
            watch_add(new_size);
            if new_size >= layout.size() { add(new_size - layout.size()); }
            else { CURRENT.fetch_sub(layout.size() - new_size, SeqCst); }
        }
        new_ptr
    }
}
fn reset_peak() -> usize {
    let current = CURRENT.load(SeqCst);
    PEAK.store(current, SeqCst);
    current
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|v| v == "recurrence") {
        recurrence(&args[2..]);
        return;
    }
    assert!(args.len() >= 9, "N RHS THREADS LEVELS_LEFT LEVELS_RIGHT PRECOND WEIGHTED PERMUTED");
    let n: usize = args[1].parse().unwrap();
    let k: usize = args[2].parse().unwrap();
    let threads: usize = args[3].parse().unwrap();
    let left_levels: usize = args[4].parse().unwrap();
    let right_levels: usize = args[5].parse().unwrap();
    let (precond, requested) = match args[6].as_str() {
        "off" => (PreconditionerConfig::Off, fereg_core::fe::LsmrPreconditioner::Off),
        "diagonal" => (PreconditionerConfig::Diagonal, fereg_core::fe::LsmrPreconditioner::Diagonal),
        "schwarz" => (PreconditionerConfig::default(), fereg_core::fe::LsmrPreconditioner::Schwarz),
        _ => panic!("unknown preconditioner"),
    };
    let weighted = args[7] == "1";
    let permuted = args[8] == "1";
    let window: usize = args.get(9).map_or(k, |v| v.parse().unwrap());
    let chain = args.get(10).is_some_and(|v| v == "chain");
    assert!(left_levels <= n && right_levels <= n);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    let original_row = |i: usize| if permuted { n - 1 - i } else { i };
    let left: Vec<u32> = (0..n).map(|i| (original_row(i) * left_levels / n) as u32).collect();
    let right: Vec<u32> = (0..n).map(|row| {
        let i = original_row(row);
        if chain { return (i / 2 + i % 2) as u32; }
        ((i / 3 + if i % 3 == 2 { i / left_levels + 1 } else { 0 }) % right_levels) as u32
    }).collect();
    let original_weights = weighted.then(|| (0..n).map(|i| 0.5 + (i % 19) as f64 / 19.0).collect::<Vec<_>>());
    let original_rhs: Vec<Vec<f64>> = (0..k).map(|j| (0..n).map(|i| {
        ((i * 17 + j * 31) as f64 * 0.0031).sin() + (i % (j + 13)) as f64 * 0.01
    }).collect()).collect();
    let model = fereg_core::memory::estimate_fe_solver_memory(n, k, &[left_levels, right_levels], threads, Some(4)).unwrap();
    let bound = model.lsmr_bytes(requested);
    let options = LsmrOptions { tol: 1e-10, maxiter: 80, local_size: Some(4) };
    pool.install(|| {
        // The caller's FE codes, weight column and input values precede this
        // boundary. All adapter-owned transport and upstream allocations follow it.
        let start = reset_peak();
        let rhs = original_rhs.clone();
        let views: Vec<&[f64]> = rhs.iter().map(Vec::as_slice).collect();
        let effects = vec![Effect::new(&left, true, []).unwrap(), Effect::new(&right, true, []).unwrap()];
        let solver = Solver::new(effects, original_weights.clone(), precond).unwrap();
        let setup_peak = PEAK.load(SeqCst) - start;
        let retained = CURRENT.load(SeqCst) - start;
        WATCH_SIZE.store((left_levels + right_levels) * 4 * 8, SeqCst);
        let solve_start = reset_peak();
        let time = Instant::now();
        let result = solve_windowed(&solver, &views, &options, window);
        let seconds = time.elapsed().as_secs_f64();
        let solve_increment = PEAK.load(SeqCst) - solve_start;
        let complete_peak = setup_peak.max(retained + solve_increment);
        let max_iter = *result.iterations.iter().max().unwrap();
        let converged = result.converged.iter().all(|v| *v);
        let result_bytes = CURRENT.load(SeqCst) - solve_start;
        let history_lanes = WATCH_PEAK.load(SeqCst);
        WATCH_SIZE.store(usize::MAX, SeqCst);
        let within_bound = complete_peak as u128 <= bound;
        println!("{{\"n\":{n},\"rhs\":{k},\"threads\":{threads},\"left_levels\":{left_levels},\"right_levels\":{right_levels},\"preconditioner\":\"{}\",\"weighted\":{weighted},\"permuted\":{permuted},\"setup_peak\":{setup_peak},\"retained_with_transport\":{retained},\"solve_increment\":{solve_increment},\"complete_peak\":{complete_peak},\"result_bytes\":{result_bytes},\"history_lanes\":{history_lanes},\"iterations\":{max_iter},\"converged\":{converged},\"seconds\":{seconds},\"modeled_solver_bytes\":{bound},\"within_bound\":{within_bound}}}", args[6]);
        assert!(within_bound, "solver lifetime exceeded model: {complete_peak} > {bound}");
        if let Some(path) = std::env::var_os("FEREG_PROBE_OUTPUT") {
            use std::io::Write;
            let mut file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
            for value in result.x.iter().chain(&result.demeaned) {
                file.write_all(&value.to_le_bytes()).unwrap();
            }
        }
    });
}

// Experimental adapter; not linked into the production core. Reserve the final
// result banks once, then cap the number of RHS frames upstream may enter.
fn solve_windowed(solver: &Solver<'_>, rhs: &[&[f64]], options: &LsmrOptions, width: usize) -> within::BatchSolveResult {
    assert!(width > 0);
    let mut chunks = rhs.chunks(width);
    let mut result = solver.solve_batch(chunks.next().unwrap(), options).unwrap();
    let remaining = rhs.len() - result.iterations.len();
    result.x.try_reserve_exact(result.n_dofs * remaining).unwrap();
    result.demeaned.try_reserve_exact(result.n_obs * remaining).unwrap();
    result.converged.try_reserve_exact(remaining).unwrap();
    result.iterations.try_reserve_exact(remaining).unwrap();
    result.residual.try_reserve_exact(remaining).unwrap();
    result.time_solve.try_reserve_exact(remaining).unwrap();
    for chunk in chunks {
        let mut next = solver.solve_batch(chunk, options).unwrap();
        assert_eq!(result.n_dofs, next.n_dofs);
        assert_eq!(result.n_obs, next.n_obs);
        assert_eq!(result.unidentified, next.unidentified);
        result.x.append(&mut next.x);
        result.demeaned.append(&mut next.demeaned);
        result.converged.append(&mut next.converged);
        result.iterations.append(&mut next.iterations);
        result.residual.append(&mut next.residual);
        result.time_solve.append(&mut next.time_solve);
        result.time_setup += next.time_setup;
        result.time_total += next.time_total;
    }
    result
}

// A separately implemented rectangular operator; no within FE setup or scatter buffers.
struct Rectangular<'a> {
    n: usize,
    p: usize,
    gate: &'a Barrier,
}
impl Operator for Rectangular<'_> {
    fn nrows(&self) -> usize { self.n }
    fn ncols(&self) -> usize { self.p }
    fn apply(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        // The first bidiagonal step follows allocation of all recurrence vectors.
        // One iteration and one worker per RHS make this a bounded rendezvous.
        self.gate.wait();
        for (i, yi) in y.iter_mut().enumerate() { *yi = x[i % self.p] * (1.0 + (i % 7) as f64); }
        Ok(())
    }
    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> {
        y.fill(0.0);
        for (i, xi) in x.iter().enumerate() { y[i % self.p] += xi * (1.0 + (i % 7) as f64); }
        Ok(())
    }
}
struct Identity(usize);
impl Operator for Identity {
    fn nrows(&self) -> usize { self.0 }
    fn ncols(&self) -> usize { self.0 }
    fn apply(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> { y.copy_from_slice(x); Ok(()) }
    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> Result<(), SolveError> { self.apply(x, y) }
}
fn recurrence(args: &[String]) {
    assert_eq!(args.len(), 4, "recurrence N COEFFICIENTS RHS HISTORY");
    let n: usize = args[0].parse().unwrap();
    let p: usize = args[1].parse().unwrap();
    let k: usize = args[2].parse().unwrap();
    let h: usize = args[3].parse().unwrap();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(k).build().unwrap();
    let gate = Barrier::new(k);
    let op = Rectangular { n, p, gate: &gate };
    let rhs: Vec<Vec<f64>> = (0..k).map(|j| (0..n).map(|i| ((i + j + 1) as f64 * 0.032).sin()).collect()).collect();
    pool.install(|| {
        let start = reset_peak();
        let result: Vec<_> = rhs.par_iter().map(|b| mlsmr(&op, b, &Identity(p), 1e-30, 1,
            MlsmrOptions { local_size: (h > 0).then_some(h), ..Default::default() }).unwrap()).collect();
        let peak = PEAK.load(SeqCst) - start;
        let expected = 8 * k * (2 * n + (6 + 2 * h.min(n).min(p)) * p);
        assert!(peak >= expected, "recurrence bank absent: {peak} < {expected}");
        assert!(peak <= expected + 8192, "unexpected diagnostic allocation: {peak} > {expected} + 8192");
        assert!(result.iter().all(|r| r.iterations == 1 && !r.converged));
        println!("{{\"kind\":\"recurrence\",\"n\":{n},\"coefficients\":{p},\"rhs\":{k},\"history\":{h},\"expected_vector_bytes\":{expected},\"observed_peak\":{peak},\"passed\":true}}");
    });
}
