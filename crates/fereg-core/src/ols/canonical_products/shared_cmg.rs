//! A shared PPML CMG hierarchy as the preconditioner of the large components
//! of an implicit reduced Schur solve.
//!
//! The reduced operator on a component is `R^-1 S_ff R^-1`: the grounded
//! weighted Schur Laplacian `S` of the reduced effect, with its anchor level
//! removed and Jacobi roots `R`. For `B ~ S^+`, `S_ff^-1 = G' S^+ G` with
//! `G u = (u, -sum u)` placing the anchor, so `R G' B G R` preconditions it.
//! The hierarchy may come from an earlier weight frame. It is acceleration
//! only: the exact current operator, recurrence target and the unchanged
//! original-input certificate decide every solve.
use super::*;
use crate::fe::ReusableCategoricalMapAbsorber;
use std::sync::{Mutex, MutexGuard, TryLockError};

/// Smaller components finish in a few unpreconditioned steps.
const MIN_COMPONENT: usize = 64;
/// Steps a plain probe takes before a missing hierarchy is built, as a joint
/// frame's diagonal recurrence does.
const CHECKPOINT: usize = 64;

/// A canonical solve's admitted use of the PPML CMG owner.
pub(super) struct Plan {
    /// Scratch words admitted with the products' plan; zero until a missing
    /// hierarchy is built.
    words: usize,
    lazy: bool,
    /// `(block.start, anchor level)` of each preconditioned component.
    anchors: Vec<(usize, usize)>,
}

impl Plan {
    /// For an implicit cold solve of the owner's reduced effect with a large
    /// component, while a hierarchy is installed or may still be built.
    pub(super) fn new(
        owner: Option<&ReusableCategoricalMapAbsorber<'_>>,
        reduced_effect: usize,
        q: usize,
        d: usize,
        width: usize,
        components: &[ComponentFactor],
    ) -> Result<Option<Self>> {
        let Some((owner, (effect, levels, installed))) =
            owner.and_then(|owner| Some((owner, owner.ppml_cmg_reach()?)))
        else {
            return Ok(None);
        };
        if effect != reduced_effect
            || levels != q
            || !components.iter().any(|block| block.len >= MIN_COMPONENT)
        {
            return Ok(None);
        }
        let words = match owner.ppml_cmg_view().filter(|_| installed) {
            Some(view) => words(view.scratch_bytes()?, q, d, width)?,
            None => 0,
        };
        Ok(Some(Self {
            words,
            lazy: !installed,
            anchors: Vec::new(),
        }))
    }

    pub(super) fn words(&self) -> usize {
        self.words
    }

    /// Anchors of the preconditioned components, before the geometry banks end.
    pub(super) fn retain_anchors(
        &mut self,
        anchors: &[usize],
        components: &[ComponentFactor],
    ) -> Result<()> {
        for block in components.iter().filter(|block| block.len >= MIN_COMPONENT) {
            self.anchors.try_reserve(1).map_err(|_| allocation())?;
            self.anchors
                .push((block.start, anchors[block.matrix_offset]));
        }
        Ok(())
    }

    /// Without an installed hierarchy, run `probe` (the plain recurrence of
    /// one column on the largest component, at most [`CHECKPOINT`] steps) and
    /// build one on `weights` when it does not finish; a finished probe ends
    /// probing for the estimation. Scratch is then admitted only within
    /// `headroom` bytes of the products' allowance.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_after(
        &mut self,
        owner: &mut ReusableCategoricalMapAbsorber<'_>,
        weights: BorrowedMapWeights<'_>,
        probe: impl FnOnce(usize) -> bool,
        (q, d, width): (usize, usize, usize),
        headroom: u128,
        budget: &mut Budget,
    ) -> Result<()> {
        if !self.lazy {
            return Ok(());
        }
        if !probe(CHECKPOINT) {
            owner.note_quick_ppml_probe();
            return Ok(());
        }
        if !owner.build_ppml_cmg(weights, "ppml_canonical_cmg")? {
            return Ok(());
        }
        let Some(view) = owner.ppml_cmg_view() else {
            return Ok(());
        };
        let words = words(view.scratch_bytes()?, q, d, width)?;
        if words as u128 * 8 <= headroom {
            budget.extend(words as u128 * 8)?;
            self.words = words;
        }
        Ok(())
    }
}

/// Per-worker scratch, the level map and the anchors of one solve.
fn words(scratch_bytes: u128, q: usize, d: usize, width: usize) -> Result<usize> {
    let slots = rayon::current_num_threads().clamp(1, width.max(1)) as u128;
    let words = scratch_bytes
        .checked_add(2 * q as u128 * 8)
        .and_then(|bytes| bytes.checked_mul(slots))
        .and_then(|bytes| bytes.checked_add((d as u128 + 2 * q as u128) * 8))
        .map(|bytes| bytes.div_ceil(8))
        .ok_or_else(declined)?;
    usize::try_from(words).map_err(|_| declined())
}

pub(super) struct SharedCmg<'v, 'h> {
    view: &'v PpmlCmgView<'h>,
    /// Level of each free reduced coordinate.
    levels: Vec<usize>,
    anchors: Vec<(usize, usize)>,
    slots: Vec<Mutex<Slot>>,
    charged: u128,
}

struct Slot {
    scratch: PpmlCmgScratch,
    input: Vec<f64>,
    output: Vec<f64>,
}

impl<'v, 'h> SharedCmg<'v, 'h> {
    /// Charge the plan's admitted words and build the per-worker slots, or
    /// `None` when the plan admitted none.
    pub(super) fn new(
        view: &'v PpmlCmgView<'h>,
        plan: Plan,
        free: &[usize],
        d: usize,
        width: usize,
        budget: &mut Budget,
    ) -> Result<Option<Self>> {
        if plan.words == 0 {
            return Ok(None);
        }
        let charged = plan.words as u128 * 8;
        budget.charge(charged)?;
        let mut levels = fallible(d, 0_usize)?;
        for (level, &coordinate) in free.iter().enumerate() {
            if coordinate != usize::MAX {
                levels[coordinate] = level;
            }
        }
        let count = rayon::current_num_threads().clamp(1, width.max(1));
        let mut slots = Vec::new();
        slots.try_reserve_exact(count).map_err(|_| allocation())?;
        for _ in 0..count {
            slots.push(Mutex::new(Slot {
                scratch: view.scratch()?,
                input: fallible(free.len(), 0.)?,
                output: fallible(free.len(), 0.)?,
            }));
        }
        Ok(Some(Self {
            view,
            levels,
            anchors: plan.anchors,
            slots,
            charged,
        }))
    }

    pub(super) fn release(self, budget: &mut Budget) -> Result<()> {
        let charged = self.charged;
        drop(self);
        budget.credit(charged)
    }

    fn anchor(&self, block: &ComponentFactor) -> Option<usize> {
        self.anchors
            .iter()
            .find(|(start, _)| *start == block.start)
            .map(|(_, anchor)| *anchor)
    }

    // At most one column task per worker holds a slot.
    fn slot(&self) -> MutexGuard<'_, Slot> {
        loop {
            for slot in &self.slots {
                match slot.try_lock() {
                    Ok(guard) => return guard,
                    Err(TryLockError::Poisoned(poisoned)) => return poisoned.into_inner(),
                    Err(TryLockError::WouldBlock) => {}
                }
            }
            std::thread::yield_now();
        }
    }

    /// The preconditioned solve of one large component, or `None` for a
    /// component this hierarchy does not precondition.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn solve(
        &self,
        block: &ComponentFactor,
        roots: &[f64],
        values: &mut [f64],
        scratch: &mut [f64],
        max_iterations: usize,
        tolerance: f64,
        apply: impl Fn(&[f64], &mut [f64]),
    ) -> Result<Option<usize>> {
        let Some(anchor) = self.anchor(block) else {
            return Ok(None);
        };
        let levels = &self.levels[block.start..block.start + block.len];
        let mut slot = self.slot();
        let Slot {
            scratch: cmg,
            input,
            output,
        } = &mut *slot;
        let precondition = |residual: &[f64], result: &mut [f64]| -> Result<()> {
            let mut sum = 0.;
            for (&level, &value) in levels.iter().zip(residual) {
                let scaled = roots[level] * value;
                input[level] = scaled;
                sum += scaled;
            }
            input[anchor] = -sum;
            let applied = self.view.apply(input, output, cmg);
            for &level in levels {
                input[level] = 0.;
            }
            input[anchor] = 0.;
            applied?;
            let base = output[anchor];
            for (&level, value) in levels.iter().zip(result.iter_mut()) {
                *value = roots[level] * (output[level] - base);
            }
            Ok(())
        };
        solve_reduced_pcg(
            values,
            scratch,
            max_iterations,
            tolerance,
            apply,
            precondition,
        )
        .map(Some)
    }
}

fn fallible<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let mut result = Vec::new();
    result.try_reserve_exact(n).map_err(|_| allocation())?;
    result.resize(n, value);
    Ok(result)
}

/// Preconditioned counterpart of `solve_reduced_cg_with_product`, with the
/// same normalization and unpreconditioned recurrence target.
fn solve_reduced_pcg(
    values: &mut [f64],
    scratch: &mut [f64],
    max_iterations: usize,
    tolerance: f64,
    apply: impl Fn(&[f64], &mut [f64]),
    mut precondition: impl FnMut(&[f64], &mut [f64]) -> Result<()>,
) -> Result<usize> {
    let n = values.len();
    if scratch.len() < 4 * n {
        return Err(declined());
    }
    let scale = values.iter().map(|x| x.abs()).fold(0_f64, f64::max);
    if !scale.is_finite() {
        return Err(declined());
    }
    if scale == 0. {
        return Ok(0);
    }
    let (residual, rest) = scratch.split_at_mut(n);
    let (direction, rest) = rest.split_at_mut(n);
    let (product, rest) = rest.split_at_mut(n);
    let preconditioned = &mut rest[..n];
    for (residual, value) in residual.iter_mut().zip(values.iter()) {
        *residual = value / scale;
    }
    values.fill(0.);
    let target = tolerance * residual.iter().map(|x| x * x).sum::<f64>().sqrt();
    let hardware = crate::kernels::installed();
    precondition(residual, preconditioned)?;
    direction.copy_from_slice(preconditioned);
    let mut energy: f64 = residual
        .iter()
        .zip(preconditioned.iter())
        .map(|(r, z)| r * z)
        .sum();
    for iteration in 1..=max_iterations {
        if !energy.is_finite() || energy <= 0. {
            return Err(declined());
        }
        apply(direction, product);
        let curvature: f64 = direction
            .iter()
            .zip(product.iter())
            .map(|(x, y)| x * y)
            .sum();
        if !curvature.is_finite() || curvature <= 0. {
            return Err(declined());
        }
        let alpha = energy / curvature;
        let square = crate::kernels::dispatch!(
            hardware,
            joint_step_energy(alpha, direction, product, values, residual)
        );
        if !square.is_finite() {
            return Err(declined());
        }
        if square.sqrt() <= target {
            for value in values {
                *value *= scale;
                if !value.is_finite() {
                    return Err(declined());
                }
            }
            return Ok(iteration);
        }
        precondition(residual, preconditioned)?;
        let next: f64 = residual
            .iter()
            .zip(preconditioned.iter())
            .map(|(r, z)| r * z)
            .sum();
        crate::kernels::dispatch!(
            hardware,
            affine_direction(next / energy, preconditioned, direction)
        );
        energy = next;
    }
    Err(declined())
}
