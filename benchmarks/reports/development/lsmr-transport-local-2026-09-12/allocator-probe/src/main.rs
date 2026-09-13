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
    assert_eq!(args.len(), 8, "N RHS THREADS LEVELS_LEFT LEVELS_RIGHT PRECOND WEIGHTED");
    let n: usize = args[1].parse().unwrap();
    let k: usize = args[2].parse().unwrap();
    let threads: usize = args[3].parse().unwrap();
    let left_levels: usize = args[4].parse().unwrap();
    let right_levels: usize = args[5].parse().unwrap();
    let precond = match args[6].as_str() {
        "off" => PreconditionerConfig::Off,
        "diagonal" => PreconditionerConfig::Diagonal,
        "schwarz" => PreconditionerConfig::default(),
        _ => panic!("unknown preconditioner"),
    };
    let weighted = args[7] == "1";
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    let left: Vec<u32> = (0..n).map(|i| (i / (n / left_levels)) as u32).collect();
    assert_eq!(*left.last().unwrap() as usize + 1, left_levels);
    let right: Vec<u32> = (0..n).map(|i| {
        // Repeat links within left groups and mix every third link across right levels.
        ((i / 3 + if i % 3 == 2 { i / left_levels + 1 } else { 0 }) % right_levels) as u32
    }).collect();
    let weights = weighted.then(|| (0..n).map(|i| 0.5 + (i % 19) as f64 / 19.0).collect::<Vec<_>>());
    let rhs: Vec<Vec<f64>> = (0..k).map(|j| (0..n).map(|i| {
        ((i * 17 + j * 31) as f64 * 0.0031).sin() + (i % (j + 13)) as f64 * 0.01
    }).collect()).collect();
    let views: Vec<&[f64]> = rhs.iter().map(Vec::as_slice).collect();
    let levels = [left_levels, right_levels];
    let model = fereg_core::memory::estimate_fe_solver_memory(n, k, &levels, threads, Some(4)).unwrap();
    let options = LsmrOptions { tol: 1e-10, maxiter: 80, local_size: Some(4) };
    pool.install(|| {
        let start = reset_peak();
        let effects = vec![Effect::new(&left, true, []).unwrap(), Effect::new(&right, true, []).unwrap()];
        let solver = Solver::new(effects, weights, precond).unwrap();
        let setup_peak = PEAK.load(SeqCst) - start;
        let retained = CURRENT.load(SeqCst) - start;
        // Start after solver construction: this size identifies each four-vector
        // history lane for these shapes. No pre-existing allocation is counted.
        WATCH_SIZE.store((left_levels + right_levels) * 4 * 8, SeqCst);
        let solve_start = reset_peak();
        let time = Instant::now();
        let result = solver.solve_batch(&views, &options).unwrap();
        let seconds = time.elapsed().as_secs_f64();
        let solve_peak = PEAK.load(SeqCst) - solve_start;
        let max_iter = *result.iterations.iter().max().unwrap();
        let converged = result.converged.iter().all(|v| *v);
        let result_bytes = CURRENT.load(SeqCst) - solve_start;
        let history_lanes = WATCH_PEAK.load(SeqCst);
        WATCH_SIZE.store(usize::MAX, SeqCst);
        eprintln!("history_lane_peak={history_lanes}; implied_recurrences={}", history_lanes / 2);
        println!("{{\"n\":{n},\"rhs\":{k},\"threads\":{threads},\"levels\":{},\"preconditioner\":\"{}\",\"weighted\":{weighted},\"setup_peak\":{setup_peak},\"retained\":{retained},\"solve_peak\":{solve_peak},\"result_bytes\":{result_bytes},\"iterations\":{max_iter},\"converged\":{converged},\"seconds\":{seconds},\"baseline_krylov\":{},\"baseline_off\":{},\"baseline_diagonal\":{},\"baseline_schwarz\":{}}}", left_levels + right_levels, args[6], model.krylov_bytes, model.lsmr_off_bytes, model.lsmr_diagonal_bytes, model.lsmr_schwarz_bytes);
    });
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
