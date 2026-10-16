use crate::local::{Elimination, Forest};
use crate::preconditioner::{Full, Preconditioner, Shallow};
use crate::problem::{dot, norm, Problem};
use cmg::Laplacian;
use std::sync::Arc;
use std::time::Instant;

pub const ARMS: [&str; 8] = [
    "diagonal",
    "eliminate",
    "forest",
    "shallow",
    "full",
    "diagonal-full",
    "forest-full",
    "shallow-full",
];
const TOL: f64 = 1e-8;
const LIMIT: usize = 16000;

pub struct ResultRow {
    pub seconds: f64,
    pub setup: f64,
    pub graph_seconds: f64,
    pub apply_seconds: f64,
    pub iterations: usize,
    pub upgrades: usize,
    pub certificates: usize,
    pub residual: f64,
    pub digest: u64,
    pub graph_builds: usize,
    pub coarse_n: usize,
    pub solutions: Vec<Vec<f64>>,
    pub decisions: Vec<String>,
    pub tail_levels: usize,
}
struct Solver<'a> {
    p: &'a Problem,
    pre: Preconditioner,
    graph: Option<Arc<Laplacian>>,
    progressive: bool,
    row: ResultRow,
    iteration_work: f64,
    upgrade_work: f64,
    spent_work: f64,
}
impl<'a> Solver<'a> {
    fn new(p: &'a Problem, arm: &str) -> Self {
        assert!(ARMS.contains(&arm));
        let mut s = Self {
            p,
            pre: Preconditioner::Diagonal(p.diagonal.clone()),
            graph: None,
            progressive: arm.ends_with("-full"),
            row: ResultRow {
                seconds: 0.,
                setup: 0.,
                graph_seconds: 0.,
                apply_seconds: 0.,
                iterations: 0,
                upgrades: 0,
                certificates: 0,
                residual: 0.,
                digest: 14695981039346656037,
                graph_builds: 0,
                coarse_n: 0,
                solutions: vec![],
                decisions: vec![],
                tail_levels: 0,
            },
            iteration_work: (5 * p.n + 4 * p.firms.len()) as f64,
            upgrade_work: (32 * p.edge_bound() + 16 * p.n) as f64,
            spent_work: 0.,
        };
        if !arm.starts_with("diagonal") {
            let graph = s.ensure_graph();
            s.pre = match arm.split('-').next().unwrap() {
                "forest" => {
                    s.iteration_work += 5. * p.n as f64;
                    Preconditioner::Forest(Forest::new(&graph))
                }
                "eliminate" => {
                    let e = Elimination::new(&graph);
                    s.row.coarse_n = e.remaining();
                    s.iteration_work += 5. * p.n as f64;
                    Preconditioner::Elimination(e)
                }
                "shallow" => {
                    let shallow = Shallow::new(graph.clone());
                    s.row.coarse_n = shallow.coarse.vertex_count();
                    s.upgrade_work =
                        (32 * shallow.coarse.edge_count() + 16 * s.row.coarse_n) as f64;
                    s.iteration_work += (16 * graph.edge_count() + 12 * p.n) as f64;
                    Preconditioner::Shallow(Box::new(shallow))
                }
                "full" => Preconditioner::Full(Box::new(Full::new(&graph))),
                _ => unreachable!(),
            };
        }
        s
    }
    fn ensure_graph(&mut self) -> Arc<Laplacian> {
        if self.graph.is_none() {
            let t = Instant::now();
            self.graph = Some(Arc::new(self.p.graph()));
            self.row.graph_seconds += t.elapsed().as_secs_f64();
            self.row.graph_builds += 1;
        }
        self.graph.as_ref().unwrap().clone()
    }
    fn apply(&mut self, r: &[f64], z: &mut [f64]) -> Result<f64, String> {
        let t = Instant::now();
        self.pre.try_apply(r, z)?;
        self.p.project(z);
        self.row.apply_seconds += t.elapsed().as_secs_f64();
        let rz = dot(r, z);
        if !rz.is_finite() || rz <= 0. {
            return Err("nonpositive_preconditioner_energy".into());
        }
        Ok(rz)
    }
    fn upgrade(&mut self) {
        let t = Instant::now();
        if let Preconditioner::Shallow(s) = &mut self.pre {
            self.row.tail_levels = s.upgrade();
        } else {
            let graph = self.ensure_graph();
            let full = Full::new(&graph);
            self.row.tail_levels = full.levels();
            self.pre = Preconditioner::Full(Box::new(full));
        }
        self.row.setup += t.elapsed().as_secs_f64();
        self.row.upgrades += 1;
        self.progressive = false;
    }
    fn certificate(&mut self, b: &[f64], x: &[f64], r: &mut [f64]) -> f64 {
        self.p.apply(x, r);
        for (r, &b) in r.iter_mut().zip(b) {
            *r = b - *r;
        }
        self.row.certificates += 1;
        self.spent_work += (4 * self.p.firms.len() + 2 * self.p.n) as f64;
        norm(r)
    }
    fn solve(&mut self, b: &[f64], future: usize) -> Result<Vec<f64>, String> {
        let n = self.p.n;
        let bnorm = norm(b);
        let target = TOL * bnorm;
        let mut x = vec![0.; n];
        if bnorm == 0. {
            return Ok(x);
        }
        let mut r = b.to_vec();
        let mut z = vec![0.; n];
        let mut ap = vec![0.; n];
        let mut truth = vec![0.; n];
        let mut rz = self.apply(&r, &mut z)?;
        let mut direction = z.clone();
        let mut previous = bnorm;
        let mut previous_iteration = 0;
        let mut repairs = 0;
        for iteration in 1..=LIMIT {
            self.p.apply(&direction, &mut ap);
            let pap = dot(&direction, &ap);
            if !pap.is_finite() || pap <= 0. {
                return Err("nonpositive_operator_curvature".into());
            }
            let alpha = rz / pap;
            for i in 0..n {
                x[i] += alpha * direction[i];
                r[i] -= alpha * ap[i];
            }
            self.p.project(&mut r);
            self.row.iterations += 1;
            self.spent_work += self.iteration_work;
            let apparent = norm(&r) <= target;
            let checkpoint = self.progressive && iteration >= 16 && iteration.is_power_of_two();
            if apparent || checkpoint || iteration == LIMIT {
                let actual = self.certificate(b, &x, &mut truth);
                if !actual.is_finite() {
                    return Err("nonfinite_certificate".into());
                }
                if actual <= target {
                    self.p.project(&mut x);
                    // Centering changes floating point coordinates; certify the returned answer.
                    let final_residual = self.certificate(b, &x, &mut truth) / bnorm;
                    if !final_residual.is_finite() || final_residual > TOL {
                        return Err("final_certificate_failed".into());
                    }
                    self.row.residual = self.row.residual.max(final_residual);
                    return Ok(x);
                }
                let mut upgrade = false;
                if checkpoint {
                    let rate =
                        (actual / previous).powf(1. / (iteration - previous_iteration) as f64);
                    let remaining = if rate > 0. && rate < 1. {
                        (target / actual).ln() / rate.ln()
                    } else {
                        (LIMIT - iteration) as f64
                    };
                    let remaining = remaining.clamp(0., (LIMIT - iteration) as f64);
                    let projected = self.iteration_work
                        * (remaining + future as f64 * (iteration as f64 + remaining));
                    upgrade = projected >= 4. * self.upgrade_work
                        && self.spent_work >= 0.5 * self.upgrade_work;
                    self.row.decisions.push(format!(
                        "{{\"rhs\":{},\"iteration\":{},\"residual\":{:.17e},\"forecast_iterations\":{:.6},\"projected_score\":{:.3},\"setup_score\":{:.3},\"spent_score\":{:.3},\"upgrade\":{}}}",
                        self.row.solutions.len(), iteration, actual/bnorm, remaining,
                        projected, self.upgrade_work, self.spent_work, upgrade));
                    previous = actual;
                    previous_iteration = iteration;
                }
                if upgrade || apparent {
                    if upgrade {
                        self.upgrade();
                    } else {
                        repairs += 1;
                        if repairs > 2 {
                            return Err("residual_replacement_limit".into());
                        }
                    }
                    r.copy_from_slice(&truth);
                    self.p.project(&mut r);
                    rz = self.apply(&r, &mut z)?;
                    direction.copy_from_slice(&z);
                    continue;
                }
            }
            let next = self.apply(&r, &mut z)?;
            let beta = next / rz;
            for i in 0..n {
                direction[i] = z[i] + beta * direction[i];
            }
            rz = next;
        }
        Err("iteration_limit".into())
    }
}

pub fn run(p: &Problem, rhs: &[Vec<f64>], arm: &str) -> Result<ResultRow, String> {
    if arm == "auto-admitted" {
        return crate::router::run_config(
            p,
            rhs,
            crate::router::Config {
                memory: crate::guarded::DIAGNOSTIC_MEMORY,
                constructor: crate::guarded::Constructor::Admitted,
                ..crate::router::Config::default()
            },
            &mut crate::guarded::Control { cancel: |_| false },
        );
    }
    if arm == "auto-commit" {
        return crate::router::run(p, rhs);
    }
    if crate::protected::ARMS.contains(&arm) {
        return crate::protected::run(p, rhs, arm);
    }
    let total = Instant::now();
    let setup = Instant::now();
    let mut solver = Solver::new(
        p,
        if matches!(arm, "full-guarded" | "full-admitted") {
            "diagonal"
        } else {
            arm
        },
    );
    if matches!(arm, "full-guarded" | "full-admitted") {
        let base = crate::guarded::base(p, rhs).map_err(|e| e.to_string())?;
        let mut c = crate::guarded::Control { cancel: |_| false };
        let (limit, constructor) = if arm == "full-admitted" {
            (
                crate::guarded::DIAGNOSTIC_MEMORY,
                crate::guarded::Constructor::Admitted,
            )
        } else {
            (
                crate::guarded::MEMORY,
                crate::guarded::Constructor::Controlled,
            )
        };
        let built = if arm == "full-guarded" {
            crate::guarded::build(p, None, base, limit, &mut c)
        } else {
            crate::guarded::build_with(p, None, base, limit, constructor, &mut c)
        }
        .map_err(|e| e.to_string())?;
        solver.row.graph_builds = built.graph_builds;
        solver.row.graph_seconds = built.graph_seconds;
        solver
            .row
            .decisions
            .push(format!("{{\"memory_bound\":{}}}", built.peak));
        solver.pre = Preconditioner::Full(Box::new(built.full));
    }
    solver.row.setup += setup.elapsed().as_secs_f64();
    for (i, b) in rhs.iter().enumerate() {
        let x = solver.solve(b, rhs.len() - i - 1)?;
        for value in &x {
            solver.row.digest = (solver.row.digest ^ value.to_bits()).wrapping_mul(1099511628211);
        }
        solver.row.solutions.push(x);
    }
    let Solver {
        mut row,
        pre,
        graph,
        ..
    } = solver;
    drop(pre);
    drop(graph);
    row.seconds = total.elapsed().as_secs_f64();
    Ok(row)
}
