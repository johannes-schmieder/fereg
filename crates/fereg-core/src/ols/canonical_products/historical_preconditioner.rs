//! Retained fixed inverse actions for canonical strict checkpoints.
//!
//! Every attempt rebuilds the exact current operator and uses the unchanged
//! native certificate. The per-component, original-RHS ledger survives a
//! numerical refusal, so a cold retry receives only its remaining work budget.
use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Clone)]
pub(crate) struct Cache {
    pub(super) weights: Vec<f64>,
    pub(super) roots: Vec<f64>,
    pub(super) free: Vec<usize>,
    pub(super) parent: Vec<usize>,
    pub(super) anchors: Vec<usize>,
    pub(super) a: Vec<u32>,
    pub(super) b: Vec<u32>,
    pub(super) generated: Vec<usize>,
    pub(super) components: Vec<ComponentFactor>,
    pub(super) factor: Vec<f64>,
    pub(super) iterative: bool,
    pub(super) left: usize,
    pub(crate) build_weight_generation: u64,
}
impl Cache {
    pub(crate) fn bytes(&self) -> u128 {
        let f = self.weights.capacity() + self.roots.capacity() + self.factor.capacity();
        let i = self.free.capacity()
            + self.parent.capacity()
            + self.anchors.capacity()
            + self.generated.capacity();
        (f * 8
            + i * size_of::<usize>()
            + (self.a.capacity() + self.b.capacity()) * 4
            + self.components.capacity() * size_of::<ComponentFactor>()
            + size_of::<Self>()
            + 32) as u128
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate(
        &self,
        weights: &[f64],
        roots: &[f64],
        free: &[usize],
        parent: &[usize],
        anchors: &[usize],
        a: &[u32],
        b: &[u32],
        generated: &[usize],
        left: usize,
    ) -> Result<f64> {
        if self.left != left
            || self.a != a
            || self.b != b
            || self.free != free
            || self.parent != parent
            || self.anchors != anchors
            || self.generated != generated
            || self.roots.len() != roots.len()
            || self.weights.len() != weights.len()
        {
            return Err(declined());
        }
        let mut rho = 1_f64;
        for (&now, &old) in weights.iter().zip(&self.weights) {
            if !now.is_finite() || now <= 0. || !old.is_finite() || old <= 0. {
                return Err(declined());
            }
            let ratio = now / old;
            if !ratio.is_finite() || ratio <= 0. || !(1. / ratio).is_finite() {
                return Err(declined());
            }
            rho = rho.max(ratio).max(1. / ratio);
        }
        for (h, &f) in free.iter().enumerate() {
            if f != usize::MAX {
                if !roots[h].is_finite()
                    || roots[h] <= 0.
                    || !self.roots[h].is_finite()
                    || self.roots[h] <= 0.
                {
                    return Err(declined());
                }
                let t = roots[h] / self.roots[h];
                if !t.is_finite() || t <= 0. {
                    return Err(declined());
                }
            }
        }
        Ok(rho)
    }
    fn apply(
        &self,
        block: &ComponentFactor,
        roots: &[f64],
        input: &[f64],
        output: &mut [f64],
        temp: &mut [f64],
    ) -> Result<()> {
        let old = self
            .components
            .iter()
            .find(|c| c.start == block.start && c.len == block.len)
            .ok_or_else(declined)?;
        let n = block.len;
        let matrix = self
            .factor
            .get(old.offset..old.offset + n * n)
            .ok_or_else(declined)?;
        for (h, &f) in self.free.iter().enumerate() {
            if f != usize::MAX && f >= block.start && f < block.start + n {
                temp[f - block.start] = roots[h] / self.roots[h];
            }
        }
        for i in 0..n {
            output[i] = input[i] * temp[i];
        }
        inverse_action(matrix, self.iterative, output)?;
        for i in 0..n {
            output[i] *= temp[i];
        }
        Ok(())
    }
}

// Keep the triangular traversal order identical to the retained factor.
#[allow(clippy::needless_range_loop)]
fn inverse_action(matrix: &[f64], iterative: bool, values: &mut [f64]) -> Result<()> {
    let n = values.len();
    if matrix.len() != n * n {
        return Err(declined());
    }
    for i in 0..n {
        let diagonal = matrix[i * n + i];
        if !diagonal.is_finite() || diagonal <= 0. {
            return Err(declined());
        }
        for k in 0..i {
            values[i] -= matrix[i * n + k] * values[k];
        }
        values[i] /= diagonal;
    }
    if iterative {
        for i in 0..n {
            values[i] *= matrix[i * n + i];
        }
    }
    for i in (0..n).rev() {
        for k in i + 1..n {
            values[i] -= matrix[k * n + i] * values[k];
        }
        values[i] /= matrix[i * n + i];
    }
    if values.iter().any(|x| !x.is_finite()) {
        return Err(declined());
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct RhsReport {
    rhs: usize,
    component: usize,
    iterations: usize,
    operator: usize,
    preconditioner: usize,
    recurrence_relative: f64,
    true_relative: f64,
    accepted_recurrence: bool,
}

fn pcg(
    values: &mut [f64],
    scratch: &mut [f64],
    cap: usize,
    tolerance: f64,
    apply: impl Fn(&[f64], &mut [f64]),
    precondition: impl Fn(&[f64], &mut [f64], &mut [f64]) -> Result<()>,
    report: &mut RhsReport,
) -> Result<usize> {
    let n = values.len();
    if scratch.len() < 6 * n {
        return Err(declined());
    }
    let scale = values.iter().map(|x| x.abs()).fold(0_f64, f64::max);
    if !scale.is_finite() {
        return Err(declined());
    }
    if scale == 0. {
        report.accepted_recurrence = true;
        report.true_relative = 0.;
        return Ok(0);
    }
    let (r, rest) = scratch.split_at_mut(n);
    let (p, rest) = rest.split_at_mut(n);
    let (ap, rest) = rest.split_at_mut(n);
    let (z, rest) = rest.split_at_mut(n);
    let (original, temp) = rest.split_at_mut(n);
    let temp = &mut temp[..n];
    for i in 0..n {
        original[i] = values[i] / scale;
        r[i] = original[i];
    }
    values.fill(0.);
    let reference = original.iter().map(|v| v * v).sum::<f64>().sqrt();
    let target = tolerance * reference;
    report.recurrence_relative = 1.;
    let mut result = Err(declined());
    if cap > 0 {
        report.preconditioner += 1;
        let initial = match precondition(r, z, temp) {
            Err(error) if error.code != ErrorCode::NonConvergence => return Err(error),
            other => other,
        };
        let mut energy: f64 = r.iter().zip(z.iter()).map(|(r, z)| r * z).sum();
        if initial.is_err() || !energy.is_finite() || energy <= 0. {
            report.operator += 1;
            apply(values, ap);
            report.true_relative = original
                .iter()
                .zip(ap.iter())
                .map(|(b, a)| (b - a).powi(2))
                .sum::<f64>()
                .sqrt()
                / reference;
            return Err(declined());
        }
        p.copy_from_slice(z);
        for iteration in 1..=cap {
            report.iterations = iteration;
            report.operator += 1;
            apply(p, ap);
            let curvature: f64 = p.iter().zip(ap.iter()).map(|(p, a)| p * a).sum();
            if !curvature.is_finite() || curvature <= 0. {
                break;
            }
            let alpha = energy / curvature;
            let hardware = crate::kernels::installed();
            let square =
                crate::kernels::dispatch!(hardware, joint_step_energy(alpha, p, ap, values, r));
            report.recurrence_relative = square.sqrt() / reference;
            if !square.is_finite() {
                break;
            }
            if square.sqrt() <= target {
                report.accepted_recurrence = true;
                result = Ok(iteration);
                break;
            }
            if iteration == cap {
                break;
            }
            report.preconditioner += 1;
            match precondition(r, z, temp) {
                Err(error) if error.code != ErrorCode::NonConvergence => return Err(error),
                Err(_) => break,
                Ok(()) => {}
            }
            let next: f64 = r.iter().zip(z.iter()).map(|(r, z)| r * z).sum();
            if !next.is_finite() || next <= 0. {
                break;
            }
            let beta = next / energy;
            crate::kernels::dispatch!(hardware, affine_direction(beta, z, p));
            energy = next;
        }
    }
    // Diagnostic exact-current operator check is charged, even on exhaustion.
    report.operator += 1;
    apply(values, ap);
    report.true_relative = original
        .iter()
        .zip(ap.iter())
        .map(|(b, a)| (b - a).powi(2))
        .sum::<f64>()
        .sqrt()
        / reference;
    for value in values {
        *value *= scale;
        if !value.is_finite() {
            return Err(declined());
        }
    }
    // Native independent original-input FE certificate remains the final gate.
    result
}

pub(crate) fn trace(message: std::fmt::Arguments<'_>) {
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=historical_preconditioner_{message}"
        ));
    }
}

#[derive(Default)]
struct Work {
    iterations: AtomicUsize,
    operators: AtomicUsize,
    preconditioners: AtomicUsize,
}

/// One canonical attempt, including retained geometry and any budgeted retry.
/// The envelope is the existing Fast-products reservation; no ordinary solver
/// allowance is increased to retain or consume historical state.
pub(crate) struct Call {
    pub(super) cache: Option<Cache>,
    pending: Option<Cache>,
    pub(super) rho: f64,
    producer: u64,
    pub(super) retry: bool,
    pub(super) envelope: u128,
    identity_reserve: u128,
    width: usize,
    coordinates: Vec<(usize, usize)>,
    work: Vec<Work>,
}
impl Call {
    pub(crate) fn new(cache: Option<Cache>, envelope: u128, identity_reserve: u128) -> Self {
        let producer = cache.as_ref().map_or(0, |c| c.build_weight_generation);
        Self {
            cache,
            rho: 1.,
            producer,
            pending: None,
            retry: false,
            envelope,
            identity_reserve,
            width: 0,
            coordinates: Vec::new(),
            work: Vec::new(),
        }
    }
    pub(crate) fn reuse(&self) -> bool {
        self.cache.is_some()
    }
    pub(super) fn ledger_bytes(&self) -> u128 {
        (self.coordinates.capacity() * size_of::<(usize, usize)>()
            + self.work.capacity() * size_of::<Work>()) as u128
    }
    pub(super) fn maximum_consumed(&self) -> usize {
        self.work
            .iter()
            .map(|w| w.iterations.load(Ordering::Relaxed))
            .max()
            .unwrap_or(0)
    }
    pub(crate) fn total_consumed(&self) -> usize {
        self.work
            .iter()
            .map(|w| w.iterations.load(Ordering::Relaxed))
            .sum()
    }
    pub(super) fn charged_bytes(&self) -> u128 {
        self.live_bytes()
            + self
                .pending
                .as_ref()
                .map_or(0, |c| c.bytes() + self.identity_reserve)
    }
    pub(super) fn live_bytes(&self) -> u128 {
        self.ledger_bytes()
            + self
                .cache
                .as_ref()
                .map_or(0, |c| c.bytes() + self.identity_reserve)
    }
    pub(super) fn initialize_ledger(&mut self, width: usize) -> Result<()> {
        let cache = self.cache.as_ref().expect("cache for ledger");
        let count = cache
            .components
            .len()
            .checked_mul(width)
            .ok_or_else(declined)?;
        let bytes = count as u128 * size_of::<Work>() as u128
            + cache.components.len() as u128 * size_of::<(usize, usize)>() as u128;
        if self
            .live_bytes()
            .checked_add(bytes)
            .is_none_or(|v| v > self.envelope)
        {
            return Err(declined());
        }
        self.coordinates
            .try_reserve_exact(cache.components.len())
            .map_err(|_| declined())?;
        self.work.try_reserve_exact(count).map_err(|_| declined())?;
        self.coordinates
            .extend(cache.components.iter().map(|c| (c.start, c.len)));
        self.work.resize_with(count, Work::default);
        self.width = width;
        Ok(())
    }
    pub(super) fn prepare_ledger(
        &mut self,
        components: &[ComponentFactor],
        width: usize,
    ) -> Result<()> {
        if self.work.is_empty() {
            let count = components.len().checked_mul(width).ok_or_else(allocation)?;
            let bytes = (count as u128) * size_of::<Work>() as u128
                + (components.len() as u128) * size_of::<(usize, usize)>() as u128;
            if self
                .live_bytes()
                .checked_add(bytes)
                .is_none_or(|b| b > self.envelope)
            {
                return Err(declined());
            }
            self.coordinates
                .try_reserve_exact(components.len())
                .map_err(|_| declined())?;
            self.work.try_reserve_exact(count).map_err(|_| declined())?;
            self.coordinates
                .extend(components.iter().map(|c| (c.start, c.len)));
            self.work.resize_with(count, Work::default);
            self.width = width;
        } else if self.width != width
            || !self
                .coordinates
                .iter()
                .copied()
                .eq(components.iter().map(|c| (c.start, c.len)))
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "historical preconditioner retry changed original RHS/component identities",
            ));
        }
        Ok(())
    }
    fn record(&self, j: usize, block: &ComponentFactor) -> &Work {
        let component = self
            .coordinates
            .iter()
            .position(|&(start, len)| start == block.start && len == block.len)
            .expect("historical preconditioner checked component");
        &self.work[j * self.coordinates.len() + component]
    }
    pub(super) fn remaining(&self, j: usize, block: &ComponentFactor, maximum: usize) -> usize {
        let limit = maximum.min(block.len);
        if self.work.is_empty() {
            limit
        } else {
            limit.saturating_sub(self.record(j, block).iterations.load(Ordering::Relaxed))
        }
    }
    pub(super) fn spent(&self) -> bool {
        self.work.iter().any(|w| {
            w.operators.load(Ordering::Relaxed) > 0 || w.preconditioners.load(Ordering::Relaxed) > 0
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn solve(
        &self,
        j: usize,
        block: &ComponentFactor,
        roots: &[f64],
        values: &mut [f64],
        scratch: &mut [f64],
        maximum: usize,
        tolerance: f64,
        apply: impl Fn(&[f64], &mut [f64]),
    ) -> Result<usize> {
        let cache = self
            .cache
            .as_ref()
            .expect("historical preconditioner consumer cache");
        let mut report = RhsReport {
            rhs: j,
            component: block.start,
            ..RhsReport::default()
        };
        let cap = maximum.min(block.len) / 2;
        let result = if cap == 0 && values.iter().any(|&v| v != 0.) {
            Err(declined())
        } else {
            pcg(
                values,
                scratch,
                cap,
                tolerance,
                apply,
                |r, z, tmp| cache.apply(block, roots, r, z, tmp),
                &mut report,
            )
        };
        let work = self.record(j, block);
        work.iterations
            .fetch_add(report.iterations, Ordering::Relaxed);
        work.operators.fetch_add(report.operator, Ordering::Relaxed);
        work.preconditioners
            .fetch_add(report.preconditioner, Ordering::Relaxed);
        trace(format_args!("rhs|rhs={}|component={}|iterations={}|operators={}|preconditioners={}|recurrence={:.17e}|true_residual={:.17e}|passed={}",report.rhs,report.component,report.iterations,report.operator,report.preconditioner,report.recurrence_relative,report.true_relative,result.is_ok()));
        result
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn cold_solve(
        &self,
        j: usize,
        block: &ComponentFactor,
        values: &mut [f64],
        scratch: &mut [f64],
        maximum: usize,
        tolerance: f64,
        apply: impl Fn(&[f64], &mut [f64]),
    ) -> Result<usize> {
        let maximum = self.remaining(j, block, maximum);
        let work = self.record(j, block);
        solve_reduced_cg_with_product(values, scratch, maximum, tolerance, |input, output| {
            work.iterations.fetch_add(1, Ordering::Relaxed);
            work.operators.fetch_add(1, Ordering::Relaxed);
            apply(input, output);
        })
    }
    // Drop old numerical storage before cold admission. Once any work was
    // spent, its original RHS/component ledger remains authoritative.
    pub(super) fn retire_for_retry(&mut self) {
        self.retry = self.spent();
        self.cache = None;
        self.pending = None;
        if !self.retry {
            self.work = Vec::new();
            self.coordinates = Vec::new();
            self.width = 0;
        }
        trace(format_args!(
            "cold_retry|spent={}|ledger_bytes={}",
            self.retry,
            self.ledger_bytes()
        ));
    }
    pub(crate) fn terminal(&self, checkpoint: bool, generation: u64, workspace: u128) {
        if !crate::diagnostics::solver_trace_enabled() {
            return;
        }
        trace(format_args!("result|kind={}|checkpoint={checkpoint}|producer={}|consumer={generation}|rho={:.17e}|cache_bytes={}|ledger_bytes={}|workspace={workspace}|envelope={}|iterations={}|operators={}|preconditioners={}|certified=true",if self.reuse(){"reused"}else if self.retry{"cold_retry"}else{"cold"},self.producer,self.rho,self.cache.as_ref().or(self.pending.as_ref()).map_or(0,Cache::bytes),self.ledger_bytes(),self.envelope,self.total_consumed(),self.work.iter().map(|w|w.operators.load(Ordering::Relaxed)).sum::<usize>(),self.work.iter().map(|w|w.preconditioners.load(Ordering::Relaxed)).sum::<usize>()));
    }
    pub(crate) fn failed(&self, checkpoint: bool, generation: u64, reason: &str) {
        if !crate::diagnostics::solver_trace_enabled() {
            return;
        }
        trace(format_args!("result|kind=declined|checkpoint={checkpoint}|producer={}|consumer={generation}|rho={:.17e}|cache_bytes={}|ledger_bytes={}|envelope={}|iterations={}|operators={}|preconditioners={}|certified=false|reason={reason}",self.producer,self.rho,self.cache.as_ref().or(self.pending.as_ref()).map_or(0,Cache::bytes),self.ledger_bytes(),self.envelope,self.total_consumed(),self.work.iter().map(|w|w.operators.load(Ordering::Relaxed)).sum::<usize>(),self.work.iter().map(|w|w.preconditioners.load(Ordering::Relaxed)).sum::<usize>()));
    }
    pub(crate) fn take_retained(&mut self) -> Option<Cache> {
        self.pending.take().or_else(|| self.cache.take())
    }
    pub(super) fn clear_pending(&mut self) {
        self.pending = None;
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn retain(
        &mut self,
        phase_bytes: u128,
        weights: &[f64],
        roots: &[f64],
        free: &[usize],
        parent: &[usize],
        anchors: &[usize],
        a: &[u32],
        b: &[u32],
        generated: &[usize],
        components: &[ComponentFactor],
        factor: &[f64],
        iterative: bool,
        implicit: bool,
        left: usize,
    ) {
        if self.envelope == 0 || self.cache.is_some() || implicit {
            return;
        }
        if PRECONDITIONER_FAULT.get() == 2 {
            PRECONDITIONER_FAULT.set(0);
            trace(format_args!(
                "retention_miss|reason=injected_optional_allocation"
            ));
            return;
        }
        let bytes = (weights.len() + roots.len() + factor.len()) as u128 * 8
            + (free.len() + parent.len() + anchors.len() + generated.len()) as u128
                * size_of::<usize>() as u128
            + (a.len() + b.len()) as u128 * 4
            + components.len() as u128 * size_of::<ComponentFactor>() as u128
            + size_of::<Cache>() as u128
            + 32;
        if phase_bytes
            .checked_add(bytes)
            .and_then(|v| v.checked_add(self.identity_reserve + self.ledger_bytes()))
            .is_none_or(|v| v > self.envelope)
        {
            trace(format_args!("retention_miss|reason=overlap"));
            return;
        }
        fn copy<T: Clone>(source: &[T]) -> Option<Vec<T>> {
            let mut out = Vec::new();
            out.try_reserve_exact(source.len()).ok()?;
            out.extend_from_slice(source);
            Some(out)
        }
        let capture = || {
            Some(Cache {
                weights: copy(weights)?,
                roots: copy(roots)?,
                free: copy(free)?,
                parent: copy(parent)?,
                anchors: copy(anchors)?,
                a: copy(a)?,
                b: copy(b)?,
                generated: copy(generated)?,
                components: copy(components)?,
                factor: copy(factor)?,
                iterative,
                left,
                build_weight_generation: 0,
            })
        };
        if let Some(cache) = capture() {
            let total = phase_bytes + cache.bytes() + self.identity_reserve + self.ledger_bytes();
            if total <= self.envelope {
                trace(format_args!(
                    "producer_pending|cache_bytes={}|peak={total}|envelope={}",
                    cache.bytes(),
                    self.envelope
                ));
                self.pending = Some(cache);
            }
        }
    }
}

// Numerical refusals finish all original RHS/component attempts. Fatal errors
// still abort the transaction immediately; they can never initiate a retry.
pub(super) fn finish_numerical<T>(
    result: Result<T>,
    refusal: &std::sync::atomic::AtomicBool,
) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.code == ErrorCode::NonConvergence => {
            refusal.store(true, Ordering::Relaxed);
            Ok(None)
        }
        Err(error) => Err(error),
    }
}
pub(super) fn consumer_setup_check() -> Result<()> {
    if PRECONDITIONER_FAULT.get() == 6 {
        PRECONDITIONER_FAULT.set(0);
        Err(declined())
    } else {
        Ok(())
    }
}
crate::fault::fault_plan! {static PRECONDITIONER_FAULT:u8=0;}
pub(super) fn late_check() -> Result<()> {
    match PRECONDITIONER_FAULT.replace(0) {
        1 => Err(declined()),
        3 => Err(FeregError::new(
            ErrorCode::Allocation,
            "injected historical preconditioner mandatory allocation failure",
        )),
        4 => Err(FeregError::new(
            ErrorCode::Interrupted,
            "injected historical preconditioner Break",
        )),
        5 => Err(FeregError::new(
            ErrorCode::Internal,
            "injected historical preconditioner internal failure",
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dot(a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b).map(|(a, b)| a * b).sum()
    }
    #[test]
    fn historical_preconditioner_fixed_inverse_actions_are_linear_symmetric_positive() {
        let matrix = [4., 1., 1., 3.];
        let factor = [2., 0., 0.5, 2.75_f64.sqrt()];
        for (m, sgs) in [(&matrix[..], true), (&factor[..], false)] {
            let a = [0.7, -1.2];
            let b = [-2., 0.3];
            let mut ba = a;
            let mut bb = b;
            let mut sum = [a[0] + b[0], a[1] + b[1]];
            inverse_action(m, sgs, &mut ba).unwrap();
            inverse_action(m, sgs, &mut bb).unwrap();
            inverse_action(m, sgs, &mut sum).unwrap();
            for i in 0..2 {
                assert!((sum[i] - ba[i] - bb[i]).abs() < 1e-14);
            }
            assert!((dot(&a, &bb) - dot(&b, &ba)).abs() < 1e-14);
            assert!(dot(&a, &ba) > 0.);
            if !sgs {
                assert!((ba[0] - (3. * a[0] - a[1]) / 11.).abs() < 1e-14);
                assert!((ba[1] - (4. * a[1] - a[0]) / 11.).abs() < 1e-14);
            } else {
                // Independent explicit SGS inverse: [[13/48,-1/12],[-1/12,1/3]].
                assert!((ba[0] - (13. * a[0] / 48. - a[1] / 12.)).abs() < 1e-14);
                assert!((ba[1] - (-a[0] / 12. + a[1] / 3.)).abs() < 1e-14);
            }
        }
    }
    fn small_cache() -> Cache {
        Cache {
            build_weight_generation: 0,
            weights: vec![1., 1., 1.],
            roots: vec![1., 2., 1.],
            free: vec![0, 1, usize::MAX],
            parent: vec![0, 0, 0],
            anchors: vec![2, usize::MAX, usize::MAX],
            a: vec![0, 0, 1],
            b: vec![0, 1, 2],
            generated: vec![0],
            components: vec![ComponentFactor {
                len: 2,
                ..ComponentFactor::default()
            }],
            factor: vec![2., 0., 0.5, 2.75_f64.sqrt()],
            iterative: false,
            left: 0,
        }
    }
    #[test]
    fn historical_preconditioner_transport_identity_checks_and_unbounded_positive_drift() {
        let cache = small_cache();
        let roots = [2., 1., 1.];
        cache
            .validate(
                &[20., 0.05, 1.],
                &roots,
                &cache.free,
                &cache.parent,
                &cache.anchors,
                &cache.a,
                &cache.b,
                &cache.generated,
                0,
            )
            .unwrap();
        let mut out = [0.; 2];
        let input = [0.7, -1.2];
        cache
            .apply(&cache.components[0], &roots, &input, &mut out, &mut [0.; 2])
            .unwrap();
        let transported = [2. * input[0], 0.5 * input[1]];
        assert!((out[0] - 2. * (3. * transported[0] - transported[1]) / 11.).abs() < 1e-14);
        assert!((out[1] - 0.5 * (4. * transported[1] - transported[0]) / 11.).abs() < 1e-14);
        assert!(cache
            .validate(
                &cache.weights,
                &roots,
                &[1, 0, usize::MAX],
                &cache.parent,
                &cache.anchors,
                &cache.a,
                &cache.b,
                &cache.generated,
                0
            )
            .is_err());
    }
    #[test]
    fn historical_preconditioner_rejects_nonpositive_weights_roots_and_all_coordinate_changes() {
        let cache = small_cache();
        for mode in 0..8 {
            let mut other = cache.clone();
            match mode {
                0 => other.weights[0] = -1.,
                1 => other.roots[0] = 0.,
                2 => other.anchors[0] = 1,
                3 => other.parent[1] = 1,
                4 => other.a[0] = 1,
                5 => other.b[0] = 1,
                6 => other.generated[0] = 1,
                _ => other.free.swap(0, 1),
            }
            assert!(cache
                .validate(
                    &other.weights,
                    &other.roots,
                    &other.free,
                    &other.parent,
                    &other.anchors,
                    &other.a,
                    &other.b,
                    &other.generated,
                    other.left
                )
                .is_err());
        }
    }
    #[test]
    fn historical_preconditioner_pcg_zero_tiny_rhs_exhaustion_and_breakdown() {
        for scale in [0., 1e-200, 1.] {
            let mut rhs = [scale, 2. * scale];
            let mut report = RhsReport::default();
            let result = pcg(
                &mut rhs,
                &mut [0.; 12],
                1,
                1e-14,
                |x, y| y.copy_from_slice(x),
                |r, z, _| {
                    z.copy_from_slice(r);
                    Ok(())
                },
                &mut report,
            );
            assert!(result.is_ok());
            assert_eq!(rhs, [scale, 2. * scale]);
            assert_eq!(report.operator, if scale == 0. { 0 } else { 2 });
            assert_eq!(report.true_relative, 0.);
        }
        let mut report = RhsReport::default();
        assert!(pcg(
            &mut [1., 2.],
            &mut [0.; 12],
            1,
            1e-14,
            |x, y| {
                y[0] = x[0];
                y[1] = 4. * x[1]
            },
            |r, z, _| {
                z.copy_from_slice(r);
                Ok(())
            },
            &mut report
        )
        .is_err());
        assert_eq!(report.iterations, 1);
        assert_eq!(report.operator, 2);
        let mut report = RhsReport::default();
        assert!(pcg(
            &mut [1., 2.],
            &mut [0.; 12],
            2,
            1e-14,
            |x, y| {
                for i in 0..2 {
                    y[i] = -x[i]
                }
            },
            |r, z, _| {
                z.copy_from_slice(r);
                Ok(())
            },
            &mut report
        )
        .is_err());
        assert_eq!(report.operator, 2);
        assert!(pcg(
            &mut [1., 2.],
            &mut [0.; 12],
            2,
            1e-14,
            |x, y| y.copy_from_slice(x),
            |r, z, _| {
                for i in 0..2 {
                    z[i] = -r[i]
                }
                Ok(())
            },
            &mut RhsReport::default()
        )
        .is_err());
    }
    #[test]
    fn historical_preconditioner_ledger_preserves_distinct_rhs_caps_after_retirement() {
        let mut call = Call::new(Some(small_cache()), 100_000, 0);
        call.initialize_ledger(3).unwrap();
        let block = call.cache.as_ref().unwrap().components[0];
        call.record(0, &block)
            .iterations
            .store(1, Ordering::Relaxed);
        call.record(0, &block).operators.store(2, Ordering::Relaxed);
        call.record(1, &block)
            .iterations
            .store(2, Ordering::Relaxed);
        call.retire_for_retry();
        assert_eq!(call.remaining(0, &block, 100), 1);
        assert_eq!(call.remaining(1, &block, 100), 0);
        assert_eq!(call.remaining(2, &block, 100), 2);
        assert_eq!(call.total_consumed(), 3);
        assert!(call.retry && call.cache.is_none());
        let mut out = [1., 1.];
        assert!(call
            .cold_solve(1, &block, &mut out, &mut [0.; 6], 100, 1e-14, |x, y| y
                .copy_from_slice(x))
            .is_err());
        assert_eq!(call.total_consumed(), 3);
    }
    #[test]
    fn historical_preconditioner_canonical_retention_retry_and_fatal_errors() {
        rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| {
                let n = 4096;
                let g: Vec<_> = (0..n).map(|i| (i % 8) as f64).collect();
                let h: Vec<_> = (0..n).map(|i| (i / 8 % 8) as f64).collect();
                let x: Vec<_> = (0..n)
                    .map(|i| ((i * 17 % 101) as f64 / 19.).sin())
                    .collect();
                let y: Vec<_> = (0..n)
                    .map(|i| 0.7 * x[i] + g[i] * 0.2 - h[i] * 0.3 + ((i * 11 % 97) as f64).cos())
                    .collect();
                let p = super::super::tests::prepared(&y, &[x], &g, &h);
                let w = vec![1.; n];
                let cfg = MapConfig {
                    tolerance: 1e-12,
                    ..MapConfig::default()
                };
                let allowance = 16 * n as u128 * 8;
                let mut old = Call::new(None, allowance, 512);
                let native = try_build_with_preconditioner(
                    &p,
                    &[0],
                    &y,
                    BorrowedMapWeights::Provided(&w),
                    1.,
                    cfg,
                    allowance,
                    0,
                    &mut Vec::new(),
                    &mut old,
                    None,
                )
                .unwrap()
                .unwrap();
                assert!(native.report.certified());
                let cache = old
                    .take_retained()
                    .expect("fixture must retain direct producer");
                assert!(!cache.iterative);
                let cold = try_build(
                    &p,
                    &[0],
                    &y,
                    BorrowedMapWeights::Provided(&w),
                    1.,
                    cfg,
                    allowance,
                    0,
                    &mut Vec::new(),
                )
                .unwrap()
                .unwrap();
                assert_eq!(native.response, cold.response);
                let mut probe = Call::new(Some(cache.clone()), allowance, 512);
                let current = try_build_with_preconditioner(
                    &p,
                    &[0],
                    &y,
                    BorrowedMapWeights::Provided(&w),
                    1.,
                    cfg,
                    allowance,
                    0,
                    &mut Vec::new(),
                    &mut probe,
                    None,
                )
                .unwrap()
                .unwrap();
                let exact = current.minimum_workspace_bytes + probe.live_bytes();
                assert!(exact < allowance);
                for envelope in [exact - 1, exact] {
                    let mut call = Call::new(Some(cache.clone()), envelope, 512);
                    let result = try_build_with_preconditioner(
                        &p,
                        &[0],
                        &y,
                        BorrowedMapWeights::Provided(&w),
                        1.,
                        cfg,
                        envelope,
                        0,
                        &mut Vec::new(),
                        &mut call,
                        None,
                    )
                    .unwrap()
                    .unwrap();
                    assert!(result.report.certified());
                    assert!(
                        result.report.solver_workspace_peak.bytes().unwrap() as u128 <= envelope
                    );
                    if envelope == exact {
                        assert!(call.total_consumed() > 0);
                    } else {
                        assert_eq!(call.total_consumed(), 0);
                        let reference = try_build(
                            &p,
                            &[0],
                            &y,
                            BorrowedMapWeights::Provided(&w),
                            1.,
                            cfg,
                            envelope,
                            0,
                            &mut Vec::new(),
                        )
                        .unwrap()
                        .unwrap();
                        assert_eq!(result.response, reference.response);
                    }
                }
                let design = p.fit_borrowed_intercept_map_design().unwrap().unwrap();
                let topology = Plan::new(&design, 2, 0, 1, 0)
                    .unwrap()
                    .topology_storage_bytes()
                    .unwrap();
                let obsolete_words = ((allowance - topology - cache.bytes() / 2) / 8) as usize;
                let mut obsolete = vec![0.; obsolete_words];
                let mut reference_obsolete = vec![0.; obsolete_words];
                let mut call = Call::new(Some(cache.clone()), allowance, 512);
                let result = try_build_with_preconditioner(
                    &p,
                    &[0],
                    &y,
                    BorrowedMapWeights::Provided(&w),
                    1.,
                    cfg,
                    allowance,
                    0,
                    &mut obsolete,
                    &mut call,
                    None,
                )
                .unwrap()
                .unwrap();
                let reference = try_build(
                    &p,
                    &[0],
                    &y,
                    BorrowedMapWeights::Provided(&w),
                    1.,
                    cfg,
                    allowance,
                    0,
                    &mut reference_obsolete,
                )
                .unwrap()
                .unwrap();
                assert_eq!(
                    call.total_consumed(),
                    0,
                    "coexisting obsolete storage declines before speculative work"
                );
                assert_eq!(result.response, reference.response);

                for fault in [0, 1, 2, 3, 4, 5, 6] {
                    let mut call = Call::new(
                        if fault == 2 {
                            None
                        } else {
                            Some(cache.clone())
                        },
                        allowance,
                        512,
                    );
                    PRECONDITIONER_FAULT.set(fault);
                    let result = try_build_with_preconditioner(
                        &p,
                        &[0],
                        &y,
                        BorrowedMapWeights::Provided(&w),
                        1.,
                        cfg,
                        allowance,
                        0,
                        &mut Vec::new(),
                        &mut call,
                        None,
                    );
                    match fault {
                        3..=5 => assert_eq!(
                            result.err().unwrap().code,
                            match fault {
                                3 => ErrorCode::Allocation,
                                4 => ErrorCode::Interrupted,
                                _ => ErrorCode::Internal,
                            }
                        ),
                        _ => {
                            let result = result.unwrap().expect("native certificate");
                            assert!(result.report.certified());
                            assert!(result
                                .response
                                .iter()
                                .zip(&cold.response)
                                .all(|(a, b)| (a - b).abs() < 1e-10));
                            if fault != 2 && fault != 6 {
                                assert!(call.total_consumed() > 0);
                            }
                            if fault == 1 {
                                assert!(call.retry);
                                assert_eq!(call.work.len(), 2);
                                assert!(call
                                    .work
                                    .iter()
                                    .all(|w| w.iterations.load(Ordering::Relaxed) > 0));
                            }
                            if fault == 6 {
                                assert_eq!(call.total_consumed(), 0);
                                assert_eq!(result.response, cold.response);
                            }
                            if fault == 2 {
                                assert!(call.take_retained().is_none());
                                assert_eq!(result.response, cold.response);
                            }
                        }
                    }
                    PRECONDITIONER_FAULT.set(0);
                }
                let mut bad = cache.clone();
                bad.anchors[0] = usize::MAX;
                let mut call = Call::new(Some(bad), allowance, 512);
                let result = try_build_with_preconditioner(
                    &p,
                    &[0],
                    &y,
                    BorrowedMapWeights::Provided(&w),
                    1.,
                    cfg,
                    allowance,
                    0,
                    &mut Vec::new(),
                    &mut call,
                    None,
                )
                .unwrap()
                .unwrap();
                assert_eq!(result.response, cold.response);
                assert_eq!(call.total_consumed(), 0);
            });
    }
    #[test]
    fn historical_preconditioner_parallel_numerical_refusal_finishes_every_rhs_without_budget_reset(
    ) {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        team.install(|| {
            let block = ComponentFactor {
                len: 4,
                ..ComponentFactor::default()
            };
            let diagonal = [1_f64, 2., 3., 4.];
            // A=D+u*u'+v*v'; the first b=D*x is orthogonal to both updates
            // in preconditioned coordinates, but cold CG needs all four steps.
            let apply = |x: &[f64], y: &mut [f64]| {
                for i in 0..4 {
                    y[i] = diagonal[i] * x[i];
                }
                let u = x[0] + x[1];
                let v = x[1] + x[2];
                y[0] += u;
                y[1] += u + v;
                y[2] += v;
            };
            let mut factor = vec![0.; 16];
            for i in 0..4 {
                factor[i * 4 + i] = diagonal[i].sqrt();
            }
            let cache = Cache {
                weights: vec![1.; 4],
                roots: vec![1.; 4],
                free: vec![0, 1, 2, 3],
                parent: vec![0; 4],
                anchors: vec![3],
                a: vec![0; 4],
                b: vec![0, 1, 2, 3],
                generated: vec![0],
                components: vec![block],
                factor,
                iterative: false,
                left: 0,
                build_weight_generation: 1,
            };
            for _ in 0..12 {
                let mut call = Call::new(Some(cache.clone()), 100_000, 0);
                call.initialize_ledger(2).unwrap();
                let refusal = std::sync::atomic::AtomicBool::new(false);
                let mut rhs = [[1., -2., 3., 4.], [1., 1., 1., 1.]];
                rhs.par_iter_mut()
                    .enumerate()
                    .try_for_each(|(j, b)| -> Result<()> {
                        finish_numerical(
                            call.solve(j, &block, &[1.; 4], b, &mut [0.; 24], 4, 1e-14, apply),
                            &refusal,
                        )?;
                        Ok(())
                    })
                    .unwrap();
                assert!(refusal.load(Ordering::Relaxed));
                assert_eq!(call.record(0, &block).iterations.load(Ordering::Relaxed), 1);
                assert_eq!(call.record(1, &block).iterations.load(Ordering::Relaxed), 2);
                assert_eq!(call.record(0, &block).operators.load(Ordering::Relaxed), 2);
                assert_eq!(call.record(1, &block).operators.load(Ordering::Relaxed), 3);
                call.retire_for_retry();
                assert_eq!(call.remaining(0, &block, 4), 3);
                assert_eq!(call.remaining(1, &block, 4), 2);
                assert!(call
                    .cold_solve(
                        0,
                        &block,
                        &mut [1., -2., 3., 4.],
                        &mut [0.; 12],
                        4,
                        1e-14,
                        apply
                    )
                    .is_err());
                assert_eq!(call.record(0, &block).iterations.load(Ordering::Relaxed), 4);
                assert!(solve_reduced_cg_with_product(
                    &mut [1., -2., 3., 4.],
                    &mut [0.; 12],
                    4,
                    1e-14,
                    apply
                )
                .is_ok());
            }
        });
    }
    #[test]
    fn historical_preconditioner_retention_exact_envelope_includes_identity_ledger_and_old_projection(
    ) {
        let cache = small_cache();
        let identity = 512;
        let native_peak = 8000;
        let obsolete = 2048;
        for with_ledger in [false, true] {
            let mut measure = Call::new(None, 100_000, identity);
            if with_ledger {
                measure.prepare_ledger(&cache.components, 2).unwrap();
            }
            let required =
                native_peak + obsolete + cache.bytes() + identity + measure.ledger_bytes();
            for envelope in [required - 1, required] {
                let mut call = Call::new(None, envelope, identity);
                if with_ledger {
                    call.prepare_ledger(&cache.components, 2).unwrap();
                }
                call.retain(
                    native_peak + obsolete,
                    &cache.weights,
                    &cache.roots,
                    &cache.free,
                    &cache.parent,
                    &cache.anchors,
                    &cache.a,
                    &cache.b,
                    &cache.generated,
                    &cache.components,
                    &cache.factor,
                    cache.iterative,
                    false,
                    cache.left,
                );
                assert_eq!(call.pending.is_some(), envelope == required);
                if call.pending.is_some() {
                    assert_eq!(native_peak + obsolete + call.charged_bytes(), envelope);
                }
            }
        }
    }
}
