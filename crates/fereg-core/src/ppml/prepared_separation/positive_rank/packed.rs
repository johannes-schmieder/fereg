//! A nonzero modular minor proves a lower bound on positive rank. Replaying
//! one independent integer alias per free coordinate on every original row
//! proves the matching upper bound on full rank, including dyadic regressors.
use super::super::exact_arithmetic::exact_score;
use super::*;
use std::cmp::Ordering;

const PRIME: u16 = 127;

fn byte_residue(x: f64) -> u8 {
    let bits = x.to_bits();
    let exponent = ((bits >> 52) & 2047) as i32;
    let mantissa = (bits & ((1u64 << 52) - 1)) | (u64::from(exponent != 0) << 52);
    // 2^7 = 1 (mod 127), including negative dyadic exponents.
    let shift = (exponent.max(1) - 1023 - 52).rem_euclid(7);
    let v = ((mantissa % u64::from(PRIME)) << shift) % u64::from(PRIME);
    if x.is_sign_negative() && v != 0 {
        PRIME as u8 - v as u8
    } else {
        v as u8
    }
}

#[derive(Clone, Copy)]
enum Axis<'a> {
    Finite(usize),
    Level(&'a [u32], u32),
}

impl Axis<'_> {
    fn at(self, model: &PreparedPpmlModelGeneration, r: usize) -> Result<f64> {
        match self {
            Self::Finite(c) => model.fit_view().column_value(c, r),
            Self::Level(codes, l) => Ok(f64::from(codes[r] == l)),
        }
    }
}

// Each normalized pivot row stores only its suffix from the pivot onward.
struct Triangle {
    width: usize,
    bank: Vec<u8>,
}

impl Triangle {
    fn start(&self, j: usize) -> usize {
        j * self.width - j * j.saturating_sub(1) / 2
    }
    fn pivot(&self, j: usize) -> bool {
        self.bank[self.start(j)] != 0
    }
    fn insert(&mut self, v: &mut [u8], work: &mut usize) -> bool {
        for j in 0..self.width {
            let x = u16::from(v[j]);
            if x == 0 {
                continue;
            }
            if !spend(work, self.width - j) {
                return false;
            }
            let start = self.start(j);
            let b = &mut self.bank[start..start + self.width - j];
            if b[0] == 0 {
                // Inverses cost at most 126 trial multiplications per new pivot.
                if !spend(work, PRIME as usize) {
                    return false;
                }
                let inv = (1..PRIME).find(|q| (q * x) % PRIME == 1).unwrap();
                for (dst, &val) in b.iter_mut().zip(&v[j..]) {
                    *dst = (inv * u16::from(val) % PRIME) as u8;
                }
                break;
            }
            for (dst, &val) in v[j..].iter_mut().zip(b.iter()) {
                *dst = ((u16::from(*dst) + PRIME * PRIME - x * u16::from(val)) % PRIME) as u8;
            }
        }
        true
    }
    fn relation(&self, free: usize, coeff: &mut [u8]) {
        coeff.fill(0);
        coeff[free] = 1;
        for j in (0..free).rev() {
            if !self.pivot(j) {
                continue;
            }
            let start = self.start(j);
            let sum = (1..=free - j).fold(0u16, |s, t| {
                (s + u16::from(self.bank[start + t]) * u16::from(coeff[j + t])) % PRIME
            });
            coeff[j] = ((PRIME - sum) % PRIME) as u8;
        }
    }
}

fn spend(work: &mut usize, cost: usize) -> bool {
    if cost > *work {
        return false;
    }
    *work -= cost;
    true
}

pub(super) struct AnchoredDesign<'model, 'codes> {
    pub(super) model: &'model PreparedPpmlModelGeneration,
    pub(super) levels: &'codes LevelIndex,
    pub(super) others: &'model [(usize, &'codes [u32])],
    pub(super) anchor_effect: usize,
    pub(super) width: usize,
}

impl AnchoredDesign<'_, '_> {
    pub(super) fn spans(
        &self,
        finite: impl Iterator<Item = usize>,
        ray: Option<&mut Vec<i64>>,
        (limit, mut work): (usize, usize),
        interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    ) -> Result<bool> {
        let Self {
            levels,
            others,
            width,
            ..
        } = *self;
        let Some(square) = width.checked_mul(width.saturating_add(1)) else {
            return Ok(false);
        };
        let bytes = square / 2;
        // Axis descriptors, two modular bytes, sparse indices and 2w raw values
        // and integer coefficients. All allocations below reserve exact capacities.
        let per_axis = std::mem::size_of::<Axis>() + 2 + std::mem::size_of::<usize>() + 4 * 8;
        let raw_width = width
            .saturating_add(levels.n_levels())
            .saturating_add(others.len())
            .saturating_add(1);
        let extra = if ray.is_some() {
            raw_width
                .saturating_add(levels.n_levels())
                .saturating_mul(8)
        } else {
            0
        };
        if width == 0
            || bytes
                .saturating_add(extra)
                .saturating_add(width.saturating_mul(per_axis))
                .div_ceil(8)
                > limit
        {
            return Ok(false);
        }
        let mut axes = Vec::new();
        axes.try_reserve_exact(width)
            .map_err(|_| allocation("unable to reserve anchored rank coordinates"))?;
        axes.extend(finite.map(Axis::Finite));
        for (block, &(start, codes)) in others.iter().enumerate() {
            let end = others.get(block + 1).map_or(width, |&(s, _)| s);
            axes.extend((1..=end - start).map(|level| Axis::Level(codes, level as u32)));
        }
        let mut tri = Triangle {
            width,
            bank: fallible_filled(bytes, 0, "unable to reserve packed rank pivots")?,
        };
        let mut row = fallible_filled(width, 0u8, "unable to reserve packed rank row")?;
        if !self.build_basis(&axes, &mut tri, &mut row, &mut work, interrupt)? {
            return Ok(false);
        }
        self.replay_aliases(&axes, &tri, &mut row, ray, work, interrupt)
    }

    fn build_basis(
        &self,
        axes: &[Axis<'_>],
        tri: &mut Triangle,
        row: &mut [u8],
        work: &mut usize,
        interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    ) -> Result<bool> {
        let Self {
            model,
            levels,
            width,
            ..
        } = *self;
        let mut anchor = fallible_filled(width, 0u8, "unable to reserve packed rank anchor")?;
        let write = |r, dest: &mut [u8]| -> Result<()> {
            for (dst, axis) in dest.iter_mut().zip(axes) {
                let v = axis.at(model, r)?;
                if !v.is_finite() {
                    return Err(invalid_argument("nonfinite anchored rank coordinate"));
                }
                *dst = byte_residue(v);
            }
            Ok(())
        };
        for g in 0..levels.n_levels() {
            interrupt(PpmlInterruptPoint::SeparationSimplex)?;
            let members = levels.checked_members(g)?;
            let mut anchored = false;
            for (pos, &r) in members.iter().enumerate() {
                crate::interrupt::row_checkpoint(pos)?;
                if model.outcome()[r] <= 0. {
                    continue;
                }
                if !spend(work, width) {
                    return Ok(false);
                }
                if !anchored {
                    write(r, &mut anchor)?;
                    anchored = true;
                    continue;
                }
                write(r, row)?;
                for (v, &a) in row.iter_mut().zip(&anchor) {
                    *v = ((u16::from(*v) + PRIME - u16::from(a)) % PRIME) as u8;
                }
                if !tri.insert(row, work) {
                    return Ok(false);
                }
            }
            if !anchored {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn replay_aliases(
        &self,
        axes: &[Axis<'_>],
        tri: &Triangle,
        row: &mut [u8],
        mut ray: Option<&mut Vec<i64>>,
        mut work: usize,
        interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    ) -> Result<bool> {
        let Self {
            model,
            levels,
            width,
            ..
        } = *self;
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(width)
            .map_err(|_| allocation("unable to reserve anchored alias indices"))?;
        let mut coefficients =
            fallible_filled(2 * width, 0i64, "unable to reserve anchored alias weights")?;
        let mut values = fallible_filled(
            2 * width,
            0.0,
            "unable to reserve anchored alias coordinates",
        )?;
        let mut all_aliased = true;
        let mut offsets = fallible_filled(
            if ray.is_some() { levels.n_levels() } else { 0 },
            0i64,
            "unable to reserve candidate anchor offsets",
        )?;
        'relation: for free in 0..width {
            if tri.pivot(free) {
                continue;
            }
            interrupt(PpmlInterruptPoint::SeparationSimplex)?;
            if !spend(&mut work, (free + 1).saturating_pow(2)) {
                return Ok(false);
            }
            tri.relation(free, row);
            indices.clear();
            indices.extend((0..width).filter(|&c| row[c] != 0));
            let s = indices.len();
            for (i, &c) in indices.iter().enumerate() {
                let v = i64::from(row[c]);
                coefficients[i] = if v > i64::from(PRIME / 2) {
                    v - i64::from(PRIME)
                } else {
                    v
                };
                coefficients[s + i] = -coefficients[i];
            }
            // Concatenate the original raw coordinates and negated anchor
            // coefficients. Subtracting floating coordinates first can round away
            // a nonzero contrast, especially on extreme scales.
            let mut orientation = None;
            let mut integral = true;
            for g in 0..levels.n_levels() {
                let members = levels.checked_members(g)?;
                if !spend(
                    &mut work,
                    members.len().saturating_add(1).saturating_add(2 * s),
                ) {
                    return Ok(false);
                }
                let mut first = None;
                for (position, &r) in members.iter().enumerate() {
                    crate::interrupt::row_checkpoint(position)?;
                    if model.outcome()[r] > 0. {
                        first = Some(r);
                        break;
                    }
                }
                let Some(first) = first else { return Ok(false) };
                for (i, &c) in indices.iter().enumerate() {
                    values[s + i] = axes[c].at(model, first)?;
                }
                if let Some(anchor_offset) = offsets.get_mut(g) {
                    let Some((_, offset)) = exact_score(&values[s..2 * s], &coefficients[s..2 * s])
                    else {
                        return Ok(false);
                    };
                    integral &= offset.fract() == 0. && offset.abs() <= (1u64 << 53) as f64;
                    *anchor_offset = offset as i64;
                }
                for (pos, &r) in members.iter().enumerate() {
                    crate::interrupt::row_checkpoint(pos)?;
                    if r == first {
                        continue;
                    }
                    if !spend(&mut work, 2 * s) {
                        return Ok(false);
                    }
                    for (i, &c) in indices.iter().enumerate() {
                        values[i] = axes[c].at(model, r)?;
                    }
                    let Some((sign, _)) = exact_score(&values[..2 * s], &coefficients[..2 * s])
                    else {
                        return Ok(false);
                    };
                    if sign == Ordering::Equal {
                        continue;
                    }
                    all_aliased = false;
                    if ray.is_none() {
                        return Ok(false);
                    }
                    if model.outcome()[r] > 0. || orientation.is_some_and(|seen| seen != sign) {
                        continue 'relation;
                    }
                    orientation = Some(sign);
                }
            }
            if let (Some(sign), Some(output)) = (orientation, ray.as_deref_mut()) {
                if !integral {
                    continue;
                }
                self.materialize_ray(
                    axes,
                    &indices,
                    &coefficients,
                    &offsets,
                    sign,
                    (output, &mut work),
                )?;
                return Ok(false);
            }
        }
        Ok(all_aliased)
    }

    fn materialize_ray(
        &self,
        axes: &[Axis<'_>],
        indices: &[usize],
        coefficients: &[i64],
        offsets: &[i64],
        sign: Ordering,
        (output, work): (&mut Vec<i64>, &mut usize),
    ) -> Result<()> {
        let Self {
            model,
            levels,
            others,
            anchor_effect,
            width,
        } = *self;
        // Materialize a raw integer proposal; the independent primal replay
        // checks the anchor coefficients and every observation again.
        let finite_count = model.model().columns.len();
        let raw_actual = finite_count
            + levels.n_levels()
            + others
                .iter()
                .enumerate()
                .map(|(b, &(start, _))| 1 + others.get(b + 1).map_or(width, |o| o.0) - start)
                .sum::<usize>();
        // The generic witness is replayed on the full design, not
        // its compressed axes. Reserve work for each row-wide check.
        let replay_work = model
            .outcome()
            .len()
            .saturating_mul(raw_actual)
            .saturating_mul(5);
        if !spend(work, replay_work) {
            return Ok(());
        }
        *output = fallible_filled(
            raw_actual,
            0i64,
            "unable to reserve anchored primal coefficients",
        )?;
        let orientation = if sign == Ordering::Greater { -1 } else { 1 };
        for (i, &c) in indices.iter().enumerate() {
            if let Axis::Finite(index) = axes[c] {
                output[index] = orientation * coefficients[i];
            }
        }
        let mut start = finite_count;
        let mut block = 0;
        for e in 0..=others.len() {
            if e == anchor_effect {
                for (v, &a) in output[start..].iter_mut().zip(offsets) {
                    *v = orientation * a;
                }
                start += offsets.len();
            } else {
                let lo = others[block].0;
                let hi = others.get(block + 1).map_or(width, |o| o.0);
                for (i, &c) in indices.iter().enumerate() {
                    if (lo..hi).contains(&c) {
                        output[start + 1 + c - lo] = orientation * coefficients[i];
                    }
                }
                start += 1 + hi - lo;
                block += 1;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_field_preserves_subnormal_and_extreme_dyadics() {
        assert_eq!(byte_residue(1.), 1);
        assert_eq!(
            (u16::from(byte_residue(f64::from_bits(1))) * (1 << (1074 % 7))) % PRIME,
            1
        );
        assert_eq!(
            (u16::from(byte_residue(f64::MAX)) + u16::from(byte_residue(-f64::MAX))) % PRIME,
            0
        );
    }
}
