//! Resumable copy of the non-progressive recurrence; fixed primitive-work tariffs.
use crate::preconditioner::Preconditioner;
use crate::problem::{dot, norm, Problem};
use std::time::Instant;

pub const LIMIT: usize = 16000;
pub const TOL: f64 = 1e-8;
#[derive(Clone, Copy)]
pub struct Tariff {
    pub n: u64,
    pub matvec: u64,
    pub project: u64,
    pub apply: u64,
}
impl Tariff {
    pub fn new(p: &Problem, apply: u64) -> Self {
        Self {
            n: p.n as u64,
            matvec: 16 * p.firms.len() as u64 + 8 * p.totals.len() as u64 + 2 * p.n as u64,
            project: 8 * p.n as u64 + 4 * p.sizes.len() as u64,
            apply,
        }
    }
    pub fn init_bound(self) -> u64 {
        self.matvec * 2 + self.project * 4 + self.apply + 80 * self.n + 512
    }
    pub fn step_bound(self) -> u64 {
        self.matvec * 3 + self.project * 5 + self.apply + 100 * self.n + 512
    }
}
pub struct PcgState {
    pub x: Vec<f64>,
    r: Vec<f64>,
    z: Vec<f64>,
    d: Vec<f64>,
    ap: Vec<f64>,
    truth: Vec<f64>,
    rz: f64,
    bnorm: f64,
    target: f64,
    repairs: usize,
    recursive_norm: f64,
    pub iterations: usize,
    pub certificates: usize,
    pub residual: f64,
    pub done: bool,
    pub work: u64,
    pub trace: u64,
    pub apply_seconds: f64,
    tariff: Tariff,
}
impl PcgState {
    pub fn new(
        p: &Problem,
        b: &[f64],
        pre: &mut Preconditioner,
        tariff: Tariff,
        start: Option<&[f64]>,
    ) -> Result<Self, String> {
        let bnorm = norm(b);
        let mut s = Self {
            x: vec![0.; p.n],
            r: b.to_vec(),
            z: vec![0.; p.n],
            d: vec![0.; p.n],
            ap: vec![0.; p.n],
            truth: vec![0.; p.n],
            rz: 0.,
            bnorm,
            target: 0.,
            repairs: 0,
            recursive_norm: bnorm,
            iterations: 0,
            certificates: 0,
            residual: 0.,
            done: false,
            work: 16 * tariff.n,
            trace: 14695981039346656037,
            apply_seconds: 0.,
            tariff,
        };
        s.target = TOL * s.bnorm;
        if s.bnorm == 0. {
            s.done = true;
            return Ok(s);
        }
        if let Some(x) = start {
            s.x.copy_from_slice(x);
            s.work += 2 * tariff.n;
            let actual = s.certificate(p, b);
            if !actual.is_finite() {
                return Err("nonfinite_certificate".into());
            }
            if actual <= s.target {
                s.finish(p, b)?;
                return Ok(s);
            }
            s.r.copy_from_slice(&s.truth);
            p.project(&mut s.r);
            s.work += 2 * tariff.n + tariff.project;
        }
        s.rz = s.apply(p, pre)?;
        s.d.copy_from_slice(&s.z);
        s.work += 2 * tariff.n;
        Ok(s)
    }
    fn apply(&mut self, p: &Problem, pre: &mut Preconditioner) -> Result<f64, String> {
        let t = Instant::now();
        pre.try_apply(&self.r, &mut self.z)?;
        p.project(&mut self.z);
        self.apply_seconds += t.elapsed().as_secs_f64();
        self.work += self.tariff.apply + self.tariff.project + 2 * self.tariff.n;
        let rz = dot(&self.r, &self.z);
        if !rz.is_finite() || rz <= 0. {
            return Err("nonpositive_preconditioner_energy".into());
        }
        Ok(rz)
    }
    fn certificate(&mut self, p: &Problem, b: &[f64]) -> f64 {
        p.apply(&self.x, &mut self.truth);
        for (r, &b) in self.truth.iter_mut().zip(b) {
            *r = b - *r;
        }
        self.certificates += 1;
        self.work += self.tariff.matvec + 4 * self.tariff.n;
        norm(&self.truth)
    }
    fn finish(&mut self, p: &Problem, b: &[f64]) -> Result<(), String> {
        p.project(&mut self.x);
        self.work += self.tariff.project;
        self.residual = self.certificate(p, b) / self.bnorm;
        if !self.residual.is_finite() || self.residual > TOL {
            return Err("final_certificate_failed".into());
        }
        self.done = true;
        Ok(())
    }
    /// Extra selector checkpoint: no residual replacement or recurrence mutation.
    pub fn probe(&mut self, p: &Problem, b: &[f64]) -> Result<f64, String> {
        let actual = self.certificate(p, b);
        if !actual.is_finite() {
            return Err("nonfinite_certificate".into());
        }
        if actual <= self.target {
            self.finish(p, b)?;
        }
        Ok(actual)
    }
    /// Commit to a fixed preconditioner without renewing iteration or repair caps.
    pub fn restart(
        &mut self,
        p: &Problem,
        b: &[f64],
        pre: &mut Preconditioner,
    ) -> Result<(), String> {
        self.probe(p, b)?;
        if self.done {
            return Ok(());
        }
        self.r.copy_from_slice(&self.truth);
        p.project(&mut self.r);
        self.work += 2 * self.tariff.n + self.tariff.project;
        self.rz = self.apply(p, pre)?;
        self.d.copy_from_slice(&self.z);
        self.work += 2 * self.tariff.n;
        Ok(())
    }
    pub fn failure_context(&mut self, p: &Problem, b: &[f64], reason: String) -> String {
        // Diagnostics only after a failed recurrence; accepted arithmetic and
        // certification cadence are unchanged. Reuse already admitted scratch.
        p.apply(&self.x, &mut self.ap);
        for (value, rhs) in self.ap.iter_mut().zip(b) {
            *value = rhs - *value;
        }
        format!("{reason}_iteration_{}_repairs_{}_recursive_{:.17e}_true_{:.17e}_bnorm_{:.17e}_target_{:.17e}",
            self.iterations,self.repairs,norm(&self.r),norm(&self.ap),self.bnorm,self.target)
    }
    /// Routing telemetry only; the norm is already computed by the recurrence.
    pub fn progress(&self) -> (f64, usize) {
        (self.recursive_norm, self.repairs)
    }
    pub fn advance(
        &mut self,
        p: &Problem,
        b: &[f64],
        pre: &mut Preconditioner,
        cap_boundary: bool,
    ) -> Result<(), String> {
        assert!(!self.done);
        p.apply(&self.d, &mut self.ap);
        self.work += self.tariff.matvec + 2 * self.tariff.n;
        let pap = dot(&self.d, &self.ap);
        if !pap.is_finite() || pap <= 0. {
            return Err("nonpositive_operator_curvature".into());
        }
        let alpha = self.rz / pap;
        for i in 0..p.n {
            self.x[i] += alpha * self.d[i];
            self.r[i] -= alpha * self.ap[i];
        }
        p.project(&mut self.r);
        self.work += 10 * self.tariff.n + self.tariff.project + 32;
        self.iterations += 1;
        self.trace = (self.trace ^ self.r[0].to_bits()).wrapping_mul(1099511628211);
        self.recursive_norm = norm(&self.r);
        let apparent = self.recursive_norm <= self.target;
        if apparent || cap_boundary || self.iterations == LIMIT {
            let actual = self.certificate(p, b);
            if !actual.is_finite() {
                return Err("nonfinite_certificate".into());
            }
            if actual <= self.target {
                return self.finish(p, b);
            }
            if apparent {
                self.repairs += 1;
                if self.repairs > 2 {
                    return Err("residual_replacement_limit".into());
                }
                self.r.copy_from_slice(&self.truth);
                p.project(&mut self.r);
                self.work += 2 * self.tariff.n + self.tariff.project;
                self.rz = self.apply(p, pre)?;
                self.d.copy_from_slice(&self.z);
                self.work += 2 * self.tariff.n;
                return Ok(());
            }
        }
        let next = self.apply(p, pre)?;
        let beta = next / self.rz;
        for i in 0..p.n {
            self.d[i] = self.z[i] + beta * self.d[i];
        }
        self.work += 4 * self.tariff.n;
        self.rz = next;
        Ok(())
    }
}

#[cfg(test)]
mod preservation_tests {
    use super::*;
    #[test]
    fn probe_preserves_recurrence_and_commit_preserves_caps() {
        let p = crate::problem::generate("grid", 64);
        let b = p.rhs(1).remove(0);
        let mut pre = Preconditioner::Diagonal(p.diagonal.clone());
        let tariff = Tariff::new(&p, 8 * p.n as u64);
        let mut state = PcgState::new(&p, &b, &mut pre, tariff, None).unwrap();
        for _ in 0..16 {
            state.advance(&p, &b, &mut pre, false).unwrap();
        }
        let prior = (
            state.x.clone(),
            state.r.clone(),
            state.z.clone(),
            state.d.clone(),
            state.ap.clone(),
            state.rz,
        );
        state.probe(&p, &b).unwrap();
        assert_eq!(
            (
                state.x.clone(),
                state.r.clone(),
                state.z.clone(),
                state.d.clone(),
                state.ap.clone(),
                state.rz
            ),
            prior
        );
        state.repairs = 1;
        let before = (state.iterations, state.repairs, state.bnorm, state.target);
        let mut full = Preconditioner::Full(Box::new(crate::preconditioner::Full::new(&p.graph())));
        state.restart(&p, &b, &mut full).unwrap();
        assert_eq!(
            (state.iterations, state.repairs, state.bnorm, state.target),
            before
        );
        while !state.done && state.iterations < LIMIT {
            state.advance(&p, &b, &mut full, false).unwrap();
        }
        assert!(state.done);
        assert!(state.repairs <= 2);
    }
    #[test]
    fn parked_construction_preserves_every_cheap_recurrence_vector() {
        use crate::builder::BuildTask;
        let p = crate::problem::generate("grid", 64);
        let b = p.rhs(1).remove(0);
        let mut pre_a = Preconditioner::Diagonal(p.diagonal.clone());
        let mut pre_b = Preconditioner::Diagonal(p.diagonal.clone());
        let tariff = Tariff::new(&p, 8 * p.n as u64);
        let mut a = PcgState::new(&p, &b, &mut pre_a, tariff, None).unwrap();
        let mut bstate = PcgState::new(&p, &b, &mut pre_b, tariff, None).unwrap();
        std::thread::scope(|scope| {
            let mut task = BuildTask::start(scope, &p, None, 0, 1 << 30, (1, true)).unwrap();
            while !a.done {
                a.advance(&p, &b, &mut pre_a, false).unwrap();
                if !task.terminal {
                    task.grant(task.report.snapshot.issued + 257).unwrap();
                }
                bstate.advance(&p, &b, &mut pre_b, false).unwrap();
                assert_eq!(
                    (&a.x, &a.r, &a.z, &a.d, &a.ap, &a.truth),
                    (
                        &bstate.x,
                        &bstate.r,
                        &bstate.z,
                        &bstate.d,
                        &bstate.ap,
                        &bstate.truth
                    )
                );
                assert_eq!(
                    (a.rz, a.target, a.repairs, a.certificates, a.work),
                    (
                        bstate.rz,
                        bstate.target,
                        bstate.repairs,
                        bstate.certificates,
                        bstate.work
                    )
                );
            }
            task.abandon().unwrap();
        });
    }
}
