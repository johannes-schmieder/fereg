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
        for arm in ARMS {
            let first = run(&p, &rhs, arm).unwrap_or_else(|e| panic!("{case}/{arm}: {e}"));
            let second = run(&p, &rhs, arm).unwrap();
            assert_eq!(first.digest, second.digest);
            assert_eq!(first.iterations, second.iterations);
            assert_eq!(first.upgrades, second.upgrades);
            assert!(first.graph_builds <= 1);
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
    for arm in ARMS {
        let r = run(&p, &rhs, arm).unwrap();
        for (b, x) in rhs.iter().zip(r.solutions) {
            near(&x, &dense_solve(&dense, &p.labels, b), 2e-12);
        }
    }
}
