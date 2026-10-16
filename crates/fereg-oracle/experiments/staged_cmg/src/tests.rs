use crate::local::{Elimination, Forest};
use crate::preconditioner::{Full, Preconditioner, Shallow};
use crate::problem::{dot, generate, norm, Problem, Rng};
use crate::solve::{run, ARMS};
use cmg::{Components, Laplacian};
use std::sync::Arc;

fn dense_problem(p: &Problem) -> Vec<Vec<f64>> {
    let mut a = vec![vec![0.; p.n]; p.n];
    for (w, e) in p.offsets.windows(2).enumerate() {
        for i in e[0]..e[1] {
            a[p.firms[i]][p.firms[i]] += p.weights[i];
            for j in e[0]..e[1] {
                a[p.firms[i]][p.firms[j]] -= p.weights[i] * p.weights[j] / p.totals[w];
            }
        }
    }
    a
}
fn dense_solve(a: &[Vec<f64>], labels: &[usize], b: &[f64]) -> Vec<f64> {
    let mut seen = vec![false; labels.iter().max().map_or(0, |x| x + 1)];
    let mut kept = vec![];
    for (i, &c) in labels.iter().enumerate() {
        if seen[c] {
            kept.push(i);
        } else {
            seen[c] = true;
        }
    }
    let n = kept.len();
    let mut l = vec![vec![0.; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
            l[i][j] = if i == j {
                (a[kept[i]][kept[j]] - s).sqrt()
            } else {
                (a[kept[i]][kept[j]] - s) / l[j][j]
            };
            assert!(l[i][j].is_finite());
        }
    }
    let mut z = vec![0.; n];
    for i in 0..n {
        z[i] = (b[kept[i]] - (0..i).map(|j| l[i][j] * z[j]).sum::<f64>()) / l[i][i];
    }
    for i in (0..n).rev() {
        z[i] = (z[i] - (i + 1..n).map(|j| l[j][i] * z[j]).sum::<f64>()) / l[i][i];
    }
    let mut x = vec![0.; b.len()];
    for (&i, &v) in kept.iter().zip(&z) {
        x[i] = v;
    }
    let mut sizes = vec![0; seen.len()];
    for &c in labels {
        sizes[c] += 1;
    }
    crate::problem::project(labels, &sizes, &mut x);
    x
}
fn near(a: &[f64], b: &[f64], tol: f64) {
    let err: Vec<_> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    assert!(
        norm(&err) <= tol * norm(b).max(1.),
        "relative error {}",
        norm(&err) / norm(b).max(1.)
    );
}
fn vector(n: usize, seed: u64) -> Vec<f64> {
    let mut rng = Rng(seed);
    (0..n).map(|_| 2. * rng.unit() - 1.).collect()
}

#[test]
fn incidence_clique_and_independent_energy_agree() {
    // Repeated cells, singleton worker, disconnected components and isolated firm.
    let p = Problem::new(
        7,
        vec![
            vec![(0, 2.), (0, 1.), (1, 5.), (2, 0.01)],
            vec![(1, 2.), (2, 7.)],
            vec![(3, 3.), (4, 2.)],
            vec![(5, 100.)],
        ],
    );
    let graph = p.graph();
    let mut x = vector(p.n, 17);
    p.project(&mut x);
    let mut ax = vec![0.; p.n];
    p.apply(&x, &mut ax);
    near(&ax, &graph.matvec(&x).unwrap(), 2e-14);
    let dense = dense_problem(&p);
    near(
        &ax,
        &dense.iter().map(|row| dot(row, &x)).collect::<Vec<_>>(),
        2e-14,
    );
    let mut energy = 0.;
    for (w, e) in p.offsets.windows(2).enumerate() {
        let mean = (e[0]..e[1])
            .map(|j| p.weights[j] * x[p.firms[j]])
            .sum::<f64>()
            / p.totals[w];
        energy += (e[0]..e[1])
            .map(|j| p.weights[j] * (x[p.firms[j]] - mean).powi(2))
            .sum::<f64>();
    }
    assert!((dot(&x, &ax) - energy).abs() <= 2e-14 * energy);
    assert_eq!(p.diagonal[5], 0.);
    assert_eq!(p.diagonal[6], 0.);
    for (i, &d) in p.diagonal.iter().enumerate() {
        assert!((d - dense[i][i]).abs() < 2e-14);
    }
}

#[test]
fn forest_is_linear_symmetric_bounded_and_handles_singular_trees() {
    let graph =
        Laplacian::from_edges(5, [(0, 1, 2.), (1, 2, 3.), (2, 3, 0.1), (0, 3, 1.)]).unwrap();
    let comp = Components::from_laplacian(&graph);
    let n = 5;
    let mut b = vector(n, 42);
    comp.center_in_place(&mut b).unwrap();
    let exact = dense_solve(&graph.to_dense(), comp.labels(), &b);
    let lower: f64 = b
        .iter()
        .zip(graph.diagonal())
        .filter(|(_, d)| **d > 0.)
        .map(|(b, d)| 0.5 * b * b / d)
        .sum();
    for keep in [
        vec![false; 4],
        vec![true, false, false, false],
        vec![true, false, true, true],
    ] {
        let forest = Forest::with_edges(&graph, &keep);
        let mut x = vec![0.; n];
        forest.apply(&b, &mut x);
        comp.center_in_place(&mut x).unwrap();
        let energy = dot(&b, &x);
        assert!(energy >= lower * (1. - 1e-12) && energy <= dot(&b, &exact) * (1. + 1e-12));
    }
    let tree = Laplacian::from_edges(5, [(0, 1, 2.), (1, 2, 0.3), (2, 3, 1.)]).unwrap();
    let tree_comp = Components::from_laplacian(&tree);
    let mut b = vector(n, 12);
    tree_comp.center_in_place(&mut b).unwrap();
    let mut x = vec![0.; n];
    Forest::with_edges(&tree, &[true; 3]).apply(&b, &mut x);
    tree_comp.center_in_place(&mut x).unwrap();
    near(
        &x,
        &dense_solve(&tree.to_dense(), tree_comp.labels(), &b),
        2e-13,
    );
}

#[test]
fn elimination_with_exact_core_recovers_dense_inverse() {
    for case in ["fringe", "grid", "clusters"] {
        let p = generate(case, 64);
        let graph = p.graph();
        let mut elim = Elimination::new(&graph);
        for b in p.rhs(3) {
            let mut x = vec![0.; p.n];
            elim.apply_with(&b, &mut x, |g, r, z| {
                let c = Components::from_laplacian(g);
                z.copy_from_slice(&dense_solve(&g.to_dense(), c.labels(), r));
            });
            p.project(&mut x);
            near(&x, &dense_solve(&dense_problem(&p), &p.labels, &b), 2e-8);
        }
    }
}

#[test]
fn every_preconditioner_is_fixed_linear_symmetric_positive() {
    let p = generate("clusters", 1024);
    let graph = Arc::new(p.graph());
    let mut shallow = Shallow::new(graph.clone());
    shallow.upgrade();
    let mut pres = vec![
        Preconditioner::Diagonal(p.diagonal.clone()),
        Preconditioner::Forest(Forest::new(&graph)),
        Preconditioner::Elimination(Elimination::new(&graph)),
        Preconditioner::Shallow(Box::new(Shallow::new(graph.clone()))),
        Preconditioner::Shallow(Box::new(shallow)),
        Preconditioner::Full(Box::new(Full::new(&graph))),
    ];
    let mut a = vector(p.n, 41);
    let mut b = vector(p.n, 59);
    p.project(&mut a);
    p.project(&mut b);
    let combined: Vec<_> = a.iter().zip(&b).map(|(a, b)| 2. * a - 3. * b).collect();
    for pre in &mut pres {
        let mut x = vec![0.; p.n];
        let mut y = x.clone();
        let mut z = x.clone();
        let mut again = x.clone();
        pre.apply(&a, &mut x);
        p.project(&mut x);
        pre.apply(&b, &mut y);
        p.project(&mut y);
        pre.apply(&combined, &mut z);
        p.project(&mut z);
        pre.apply(&a, &mut again);
        p.project(&mut again);
        assert_eq!(x, again);
        near(
            &z,
            &x.iter()
                .zip(&y)
                .map(|(x, y)| 2. * x - 3. * y)
                .collect::<Vec<_>>(),
            2e-10,
        );
        let scale = dot(&a, &x).sqrt() * dot(&b, &y).sqrt();
        assert!(dot(&a, &x) > 0. && dot(&b, &y) > 0.);
        assert!((dot(&a, &y) - dot(&b, &x)).abs() < 2e-10 * scale);
    }
}

#[test]
fn all_arms_agree_with_dense_and_repeat_including_upgrades() {
    let mut upgrades = 0;
    for case in ["expander", "fringe", "clusters", "grid", "hubs"] {
        let p = generate(case, 64);
        let rhs = p.rhs(2);
        let a = dense_problem(&p);
        for arm in ARMS.into_iter().chain(crate::protected::ARMS) {
            let first = run(&p, &rhs, arm).unwrap_or_else(|e| panic!("{case}/{arm}: {e}"));
            let second = run(&p, &rhs, arm).unwrap();
            assert_eq!(first.digest, second.digest);
            assert_eq!(first.iterations, second.iterations);
            assert_eq!(first.upgrades, second.upgrades);
            assert!(
                first.graph_builds
                    <= if crate::protected::ARMS.contains(&arm) {
                        rhs.len()
                    } else {
                        1
                    }
            );
            if arm == "diagonal" {
                assert_eq!(first.graph_builds, 0);
            }
            for (b, x) in rhs.iter().zip(&first.solutions) {
                near(x, &dense_solve(&a, &p.labels, b), 2e-6);
            }
            upgrades += first.upgrades;
        }
    }
    assert!(upgrades > 0, "must exercise restart and tail upgrade");
}

#[test]
fn each_progressive_arm_restarts_and_reuses_a_recursive_tail() {
    let p = generate("grid", 4096);
    let rhs = p.rhs(2);
    for arm in ["diagonal-full", "forest-full", "shallow-full"] {
        let r = run(&p, &rhs, arm).unwrap();
        assert_eq!(r.upgrades, 1, "{arm}");
        assert!(r.tail_levels > 1, "{arm}: tail was not recursive");
        assert_eq!(r.graph_builds, 1);
        assert_eq!(r.solutions.len(), 2);
        assert!(
            r.decisions
                .iter()
                .any(|d| d.contains("\"rhs\":0") && d.contains("\"upgrade\":true")),
            "{arm}"
        );
        assert!(
            r.decisions.iter().all(|d| d.contains("\"rhs\":0")),
            "tail must persist into RHS 2"
        );
    }
}

#[test]
fn disconnected_solves_and_isolates_agree_with_dense() {
    let p = Problem::new(
        7,
        vec![
            vec![(0, 2.), (1, 3.)],
            vec![(1, 1.), (2, 5.)],
            vec![(3, 2.), (4, 4.)],
            vec![(5, 2.)],
        ],
    );
    let rhs = p.rhs(2);
    let dense = dense_problem(&p);
    for arm in ARMS.into_iter().chain(crate::protected::ARMS) {
        let r = run(&p, &rhs, arm).unwrap();
        for (b, x) in rhs.iter().zip(r.solutions) {
            near(&x, &dense_solve(&dense, &p.labels, b), 2e-12);
        }
    }
}

#[test]
fn protected_disabled_and_refused_are_exact_cheap_replays() {
    use crate::protected::{run_config, Config};
    for n in [64, 1024] {
        for case in ["expander", "fringe", "clusters", "grid", "hubs"] {
            let p = generate(case, n);
            let rhs = p.rhs(2);
            for starter in ["diagonal", "forest"] {
                let baseline = run(&p, &rhs, starter).unwrap();
                for config in [
                    Config {
                        denominator: 0,
                        memory: 1 << 30,
                        iterations: 16000,
                    },
                    Config {
                        denominator: 1,
                        memory: 12 * 1024 * 1024,
                        iterations: 16000,
                    },
                ] {
                    for suffix in ["q1", "p1", "c1"] {
                        let row =
                            run_config(&p, &rhs, &format!("{starter}-{suffix}"), config).unwrap();
                        assert_eq!(
                            (row.digest, row.iterations, row.certificates),
                            (baseline.digest, baseline.iterations, baseline.certificates),
                            "{case}/{n}/{starter}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn controlled_full_matches_existing_full_exactly() {
    for n in [64, 1024] {
        for case in ["expander", "fringe", "clusters", "grid", "hubs"] {
            let p = generate(case, n);
            let rhs = p.rhs(2);
            let a = run(&p, &rhs, "full").unwrap();
            for arm in ["full-controlled", "full-chunked"] {
                let b = run(&p, &rhs, arm).unwrap();
                assert_eq!(
                    (a.digest, a.iterations, a.certificates),
                    (b.digest, b.iterations, b.certificates),
                    "{case}/{n}"
                );
            }
        }
    }
}
#[test]
fn parked_builder_quanta_and_abandon_are_deterministic() {
    use crate::builder::{BuildTask, CAP};
    let p = generate("clusters", 64);
    let b = p.rhs(1).remove(0);
    let mut expected = None;
    for quantum in [257, 4096, 65536, CAP] {
        std::thread::scope(|scope| {
            let mut task = BuildTask::start(scope, &p, None, 0, 1 << 30, (7, false)).unwrap();
            while !task.terminal {
                task.grant((task.report.snapshot.issued + quantum).min(CAP))
                    .unwrap();
            }
            let mut ready = task.ready.take().unwrap();
            let mut x = vec![0.; p.n];
            ready.pre.apply(&b, &mut x);
            let signature = (
                task.report.trace,
                task.report.snapshot.charged,
                ready.cycle,
                x,
            );
            if let Some(first) = &expected {
                assert_eq!(first, &signature);
            } else {
                expected = Some(signature);
            }
        });
    }
    let mut expected = None;
    for _ in 0..2 {
        std::thread::scope(|scope| {
            let mut task = BuildTask::start(scope, &p, None, 0, 1 << 30, (8, false)).unwrap();
            task.grant(10000).unwrap();
            assert!(!task.terminal);
            task.abandon().unwrap();
            assert!(task.terminal && task.ready.is_none());
            assert!(task.report.snapshot.charged <= 10000);
            let signature = (task.report.trace, task.report.snapshot.charged);
            if let Some(first) = expected {
                assert_eq!(first, signature);
            } else {
                expected = Some(signature);
            }
        });
    }
}
#[test]
fn shared_iteration_exhaustion_remains_failure() {
    let p = generate("grid", 1024);
    for arm in ["diagonal-q1", "forest-q1"] {
        let result = crate::protected::run_config(
            &p,
            &p.rhs(1),
            arm,
            crate::protected::Config {
                denominator: 1,
                memory: 1 << 30,
                iterations: 20,
            },
        );
        assert!(result.err().unwrap().starts_with("iteration_limit"));
    }
}
#[test]
fn protected_hard_case_installs_and_reuses_full() {
    let p = generate("grid", 4096);
    {
        let arm = "diagonal-q1";
        let a = run(&p, &p.rhs(2), arm).unwrap();
        let b = run(&p, &p.rhs(2), arm).unwrap();
        assert_eq!((a.digest, a.iterations), (b.digest, b.iterations));
        assert_eq!(a.upgrades, 1, "{arm}");
        assert!(a.tail_levels > 1);
        assert!(
            a.decisions.iter().any(|d| d.contains("challenger\"")),
            "{arm}"
        );
    }
}

#[test]
fn shared_cap_counts_actual_challenger_work_and_rejects_relaxation() {
    let p = generate("grid", 4096);
    let rhs = p.rhs(1);
    let complete = run(&p, &rhs, "diagonal-q1").unwrap();
    let install: usize = complete.decisions[0]
        .split("\"install_iteration\":")
        .nth(1)
        .unwrap()
        .trim_end_matches('}')
        .parse()
        .unwrap();
    let config = crate::protected::Config {
        denominator: 1,
        memory: 1 << 30,
        iterations: install + 64,
    };
    let error = crate::protected::run_config(&p, &rhs, "diagonal-q1", config)
        .err()
        .unwrap();
    assert!(error.starts_with("iteration_limit_incumbent_"), "{error}");
    let heavy: usize = error.rsplit('_').next().unwrap().parse().unwrap();
    assert!(heavy > 0, "{error}");
    let invalid = crate::protected::Config {
        iterations: 16001,
        ..config
    };
    assert_eq!(
        crate::protected::run_config(&p, &rhs, "diagonal-q1", invalid)
            .err()
            .as_deref(),
        Some("invalid_iteration_cap")
    );
}

fn decision_integer(row: &str, key: &str) -> u64 {
    row.split(&format!("\"{key}\":"))
        .nth(1)
        .unwrap()
        .split([',', '}'])
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn persistent_frontier_survives_zero_rhs_and_refunds_only_at_batch_end() {
    let p = generate("grid", 1024);
    let b = p.rhs(1).remove(0);
    let rhs = vec![b.clone(), vec![0.; p.n], b.clone()];
    let cheap = run(&p, &rhs, "diagonal").unwrap();
    let a = run(&p, &rhs, "diagonal-p14").unwrap();
    assert_eq!(a.solutions, cheap.solutions);
    assert_eq!(
        (a.iterations, a.certificates, a.residual),
        (cheap.iterations, cheap.certificates, cheap.residual)
    );
    assert_eq!(a.upgrades, 0);
    assert!(a.decisions[0].contains("\"build_status\":\"parked\""));
    assert!(a.decisions[1].contains("\"resumed\":true"));
    assert_eq!(
        decision_integer(&a.decisions[0], "issued_end"),
        decision_integer(&a.decisions[1], "issued_start")
    );
    assert_eq!(
        decision_integer(&a.decisions[0], "charged_end"),
        decision_integer(&a.decisions[1], "charged_end")
    );
    assert_eq!(
        decision_integer(&a.decisions[1], "issued_start"),
        decision_integer(&a.decisions[1], "issued_end")
    );
    assert_eq!(
        decision_integer(&a.decisions[1], "charged_end"),
        decision_integer(&a.decisions[2], "charged_start")
    );
    assert!(a.decisions[2].contains("\"build_status\":\"abandoned\""));
    assert_eq!(decision_integer(&a.decisions[2], "reserved_build_work"), 0);
    assert!(a
        .decisions
        .iter()
        .all(|d| decision_integer(d, "task_id") == 1));
}

#[test]
fn persistent_work_can_install_after_an_earlier_rhs_completes() {
    let p = generate("grid", 4096);
    let rhs = p.rhs(3);
    let a = run(&p, &rhs, "diagonal-p14").unwrap();
    let b = run(&p, &rhs, "diagonal-p14").unwrap();
    assert_eq!(a.solutions, b.solutions);
    assert_eq!((a.digest, a.iterations), (b.digest, b.iterations));
    assert_eq!(a.upgrades, 1);
    assert_eq!(a.graph_builds, 1);
    assert!(a.decisions[0].contains("\"build_status\":\"parked\""));
    assert!(a.decisions[2].contains("\"build_status\":\"complete\""));
    assert!(a.decisions[2].contains("\"resumed\":true"));
    assert!(a
        .decisions
        .iter()
        .all(|d| decision_integer(d, "task_id") == 1));
    let first = run(&p, &rhs[..1], "diagonal").unwrap();
    assert_eq!(a.solutions[0], first.solutions[0]);
}

#[test]
fn chunked_builder_is_partition_independent_and_matches_full() {
    use crate::builder::{BuildTask, CAP};
    let p = generate("clusters", 64);
    let b = p.rhs(1).remove(0);
    let mut full = Full::new(&p.graph());
    let mut expected = vec![0.; p.n];
    full.apply(&b, &mut expected);
    let mut signature = None;
    for quantum in [257, 4096, 65536, CAP] {
        std::thread::scope(|scope| {
            let mut task = BuildTask::start(scope, &p, None, 0, 1 << 30, (7, true)).unwrap();
            while !task.terminal {
                task.grant((task.report.snapshot.issued + quantum).min(CAP))
                    .unwrap();
            }
            let mut ready = task.ready.take().unwrap();
            let mut x = vec![0.; p.n];
            ready.pre.apply(&b, &mut x);
            assert_eq!(
                x.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            );
            let observed = (task.report.trace, task.report.snapshot.charged, ready.cycle);
            if let Some(prior) = signature {
                assert_eq!(observed, prior);
            } else {
                signature = Some(observed);
            }
        });
    }
}

#[test]
fn persistent_failure_after_a_park_joins_and_later_batches_work() {
    let p = generate("expander", 256);
    let b = p.rhs(1).remove(0);
    let result = run(&p, &[b.clone(), vec![f64::NAN; p.n]], "diagonal-p14");
    assert!(result.is_err());
    let a = run(&p, &[b.clone(), b], "diagonal-p14").unwrap();
    assert!(a.decisions[0].contains("\"build_status\":\"parked\""));
    assert!(a.decisions[1].contains("\"resumed\":true"));
}
