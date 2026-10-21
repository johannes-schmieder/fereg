//! Setup attribution smoke harness (bench/cmg_setup_attribution.py smoke).
//!
//! Builds two deterministic duplicate-heavy graphs through the same CMG calls
//! as fereg's automatic Schur route (`Laplacian::build`, then
//! `CmgPreconditioner::build_with` with the route's component options, a
//! cancellation query and an unbounded budget) on teams of 1 and 4 workers.
//! Prints one result digest per call; with the `attribution` feature it also
//! prints the attribution trace line and a minimal profile line whose
//! hierarchy time encloses `build_with`. Not timing evidence.

use cmg::{
    ApplyContext, BuildContext, BuildOptions, CmgOptions, CmgPreconditioner, ComponentBuildOptions,
    Laplacian, ParallelExecutor, ParallelOptions,
};
use std::time::Instant;

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// 114,375 input edges over 45,000 distinct pairs (duplicates with distinct
/// weights): above the hosted initial threshold, while every contraction
/// stays below the parallel contraction floor. `random` pairs on 12,000
/// vertices stop on fill; the `banded` circulant on 9,000 vertices (each
/// vertex joined to its next five) coarsens to a direct terminal factor.
fn edges(graph: &str) -> (usize, Vec<(usize, usize, f64)>) {
    let n = if graph == "random" { 12_000 } else { 9_000 };
    let edges = (0..114_375_u64)
        .map(|i| {
            let weight = 1.0 + (mix(i) % 7) as f64 / 4.0;
            if graph == "random" {
                let pair = mix(i % 45_000);
                let u = (pair % n as u64) as usize;
                let v = (u + 1 + (pair >> 32) as usize % (n - 1)) % n;
                (u, v, weight)
            } else {
                let pair = (mix(i) % 45_000) as usize;
                let u = pair / 5;
                (u, (u + pair % 5 + 1) % n, weight)
            }
        })
        .collect();
    (n, edges)
}

fn fnv(bits: impl Iterator<Item = u64>) -> u64 {
    bits.fold(0xcbf2_9ce4_8422_2325, |hash, value| {
        (hash ^ value).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn main() {
    let never = || false;
    let options = BuildOptions {
        hierarchy: CmgOptions::default(),
        components: ComponentBuildOptions {
            prune_coarse_isolates: true,
            preserve_unpruned_stopping: true,
            factor_terminal_components: true,
        },
    };
    for (graph_name, workers) in [("random", 1), ("random", 4), ("banded", 1), ("banded", 4)] {
        let (n, edges) = edges(graph_name);
        let executor = ParallelExecutor::new(ParallelOptions {
            threads: workers,
            ..ParallelOptions::default()
        })
        .expect("executor");
        let team = executor.team();
        let context = BuildContext {
            team: Some(team),
            cancel: Some(&never),
            budget_bytes: Some(usize::MAX),
        };
        println!("SMOKE_CALL_BEGIN|graph={graph_name}|workers={workers}");
        #[cfg(feature = "attribution")]
        cmg::setup_attribution::reset();
        let graph = Laplacian::build(n, &edges, context).expect("graph");
        let started = Instant::now();
        let preconditioner =
            CmgPreconditioner::build_with(&graph, options, context).expect("build");
        let seconds = started.elapsed().as_secs_f64();
        let x: Vec<f64> = (0..n).map(|v| ((v % 97) as f64).sin()).collect();
        let rhs = graph.matvec(&x).expect("rhs");
        let mut output = vec![0.0; n];
        let mut workspace = preconditioner.try_workspace().expect("workspace");
        preconditioner
            .apply_compatible_into_with(
                &rhs,
                &mut output,
                &mut workspace,
                ApplyContext {
                    plan: None,
                    team: Some(team),
                    cancel: None,
                },
            )
            .expect("apply");
        let report = preconditioner.hierarchy().report();
        println!(
            "SMOKE_DIGEST|graph={graph_name}|workers={workers}|graph_edges={}|vertices={:?}|nonzeros={:?}|repeats={:?}|terminal={:?}|factor_nonzeros={:?}|apply_fnv={:016x}",
            graph.edge_count(),
            report.vertex_counts(),
            report.matrix_nonzeros(),
            preconditioner.repeat_counts(),
            report.terminal_reason(),
            preconditioner.terminal_factor().map(|factor| factor.factor_nonzeros()),
            fnv(output.iter().map(|value| value.to_bits())),
        );
        #[cfg(feature = "attribution")]
        {
            println!(
                "FEREG_SOLVER_TRACE|phase=auto_schur_setup_profile|outcome=built|completed_phases=6|hierarchy_seconds={seconds:.9}"
            );
            println!(
                "FEREG_SOLVER_TRACE|phase=auto_schur_setup_attribution|outcome=built|{}",
                cmg::setup_attribution::trace_fields()
            );
        }
        #[cfg(not(feature = "attribution"))]
        let _ = seconds;
        println!("SMOKE_CALL_END");
    }
}
