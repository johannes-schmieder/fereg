//! An exactly balanced integer dual proves full-span orthogonality even
//! when numerical QR cannot identify all finite-column aliases.
use super::*;

/// Certify the proposed multipliers directly on every original model column.
/// A failed exact operation declines this proof and leaves the ordinary
/// projector available. No rank or conditioning assumption enters the check.
pub(super) fn certify(
    generation: &PreparedPpmlModelGeneration,
    dual: &[f64],
    tolerance: f64,
) -> Result<bool> {
    if dual.len() != generation.outcome().len()
        || !dual
            .iter()
            .all(|value| value.fract() == 0. && value.abs() <= (1_u64 << 53) as f64)
    {
        return Ok(false);
    }
    let margin = generation
        .outcome()
        .iter()
        .zip(dual)
        .filter_map(|(&y, &value)| (y == 0.).then_some(value))
        .fold(f64::INFINITY, f64::min);
    if !margin.is_finite() || margin <= tolerance * stable_norm(dual) {
        return Ok(false);
    }
    let fit = generation.fit_view();
    if fit.fit_structural_intercept() && !zero_moment(0..dual.len(), dual, |_| Ok(1.))? {
        return Ok(false);
    }
    for column in 0..fit.model().columns.len() {
        crate::interrupt::checkpoint()?;
        if !zero_moment(0..dual.len(), dual, |row| fit.column_value(column, row))? {
            return Ok(false);
        }
    }
    let categorical = generation.prepared().as_ref();
    let topology = DetectorTopology::new(generation.prepared())?;
    for (effect, spec) in categorical.effect_specs().iter().enumerate() {
        let source = topology.sources[topology.effect_to_source[effect]];
        for level in 0..source.n_levels() {
            crate::interrupt::checkpoint()?;
            let members = source.checked_members(level)?;
            if spec.intercept && !zero_moment(members.iter().copied(), dual, |_| Ok(1.))? {
                return Ok(false);
            }
            for slope in &spec.slopes {
                let values = categorical.numeric_source_values(slope.source)?;
                if !zero_moment(members.iter().copied(), dual, |row| Ok(values[row]))? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// Exact products with integer multipliers are multiples of 2^-1074, so a
/// nonzero multiplication error cannot underflow to zero in the fused check.
/// The magnitude-ordered FastTwoSum correction detects every rounded sum.
fn zero_moment(
    rows: impl Iterator<Item = usize>,
    dual: &[f64],
    value: impl Fn(usize) -> Result<f64>,
) -> Result<bool> {
    let mut sum = 0.0_f64;
    for (position, row) in rows.enumerate() {
        crate::interrupt::row_checkpoint(position)?;
        let integer = dual[row];
        if integer == 0. {
            continue;
        }
        let value = value(row)?;
        let term = value * integer;
        let next = sum + term;
        if !next.is_finite() || value.mul_add(integer, -term) != 0. {
            return Ok(false);
        }
        let (large, small) = if sum.abs() >= term.abs() {
            (sum, term)
        } else {
            (term, sum)
        };
        if next - large != small {
            return Ok(false);
        }
        sum = next;
    }
    Ok(sum == 0.)
}

/// The exact rational counterpart of `certify`: weights of any size on the
/// listed rows (zero elsewhere), every zero-outcome row strictly positive
/// and every original model column's moment exactly zero. Like `certify`,
/// no rank or conditioning assumption enters.
pub(super) fn certify_exact(
    generation: &PreparedPpmlModelGeneration,
    dual: &crate::ppml_separation::ExactDual,
) -> Result<bool> {
    let outcome = generation.outcome();
    let listed = dual.listed();
    if dual.scale.signum() <= 0 || listed >= u32::MAX as usize {
        return Ok(false);
    }
    let mut index = fallible_filled(
        outcome.len(),
        u32::MAX,
        "unable to reserve exact dual index",
    )?;
    for (slot, row) in dual.rows().enumerate() {
        if row >= outcome.len() || index[row] != u32::MAX {
            return Ok(false);
        }
        index[row] = slot as u32;
    }
    for (row, &y) in outcome.iter().enumerate() {
        let positive = index[row] != u32::MAX && dual.positive(index[row] as usize);
        if y == 0. && !positive {
            return Ok(false);
        }
    }
    let fit = generation.fit_view();
    let all = || dual.rows();
    // One term list, sized once by the listed rows, serves every moment.
    let mut terms: Vec<(usize, i64, i32)> = Vec::new();
    terms
        .try_reserve_exact(listed)
        .map_err(|_| crate::error::allocation("unable to reserve exact moment terms"))?;
    let terms = std::cell::RefCell::new(terms);
    let moment = |rows: &mut dyn Iterator<Item = usize>, value: &dyn Fn(usize) -> Result<f64>| {
        exact_zero_moment(rows, (dual, &index), &mut terms.borrow_mut(), value)
    };
    if fit.fit_structural_intercept() && !moment(&mut all(), &|_| Ok(1.))? {
        return Ok(false);
    }
    for column in 0..fit.model().columns.len() {
        crate::interrupt::checkpoint()?;
        if !moment(&mut all(), &|row| fit.column_value(column, row))? {
            return Ok(false);
        }
    }
    let categorical = generation.prepared().as_ref();
    let topology = DetectorTopology::new(generation.prepared())?;
    for (effect, spec) in categorical.effect_specs().iter().enumerate() {
        let source = topology.sources[topology.effect_to_source[effect]];
        for level in 0..source.n_levels() {
            crate::interrupt::checkpoint()?;
            let members = source.checked_members(level)?;
            let listed = || {
                members
                    .iter()
                    .copied()
                    .filter(|&row| index[row] != u32::MAX)
            };
            if spec.intercept && !moment(&mut listed(), &|_| Ok(1.))? {
                return Ok(false);
            }
            for slope in &spec.slopes {
                let values = categorical.numeric_source_values(slope.source)?;
                if !moment(&mut listed(), &|row| Ok(values[row]))? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// The exact sum of `weight(row) * value(row)` is zero. Each binary64 value
/// is `m 2^e`; the integer-weight and explicit-weight parts are each aligned
/// at their smallest exponent and added exactly.
fn exact_zero_moment(
    rows: &mut dyn Iterator<Item = usize>,
    (dual, index): (&crate::ppml_separation::ExactDual, &[u32]),
    terms: &mut Vec<(usize, i64, i32)>,
    value: &dyn Fn(usize) -> Result<f64>,
) -> Result<bool> {
    use crate::ppml_separation::Big;
    let (integers, split) = (dual.integers.len(), dual.integers.len() + dual.scaled.len());
    terms.clear();
    for (position, row) in rows.enumerate() {
        crate::interrupt::row_checkpoint(position)?;
        let v = value(row)?;
        if !v.is_finite() {
            return Ok(false);
        }
        if v != 0. {
            if terms.len() == terms.capacity() {
                // Every listed row appears at most once per moment.
                return Err(crate::error::allocation(
                    "exact moment terms exceed listed rows",
                ));
            }
            let (m, e) = dyadic_parts(v);
            terms.push((index[row] as usize, m, e));
        }
    }
    let low = |explicit: bool| {
        terms
            .iter()
            .filter(|&&(slot, _, _)| (slot >= split) == explicit)
            .map(|&(_, _, e)| e)
            .min()
    };
    let (low_integer, low_explicit) = (low(false), low(true));
    let (mut integer, mut explicit) = (Big::zero(), Big::zero());
    for &(slot, m, e) in terms.iter() {
        if slot < integers {
            let k = i128::from(dual.integers[slot].1) * i128::from(m);
            let shift = (e - low_integer.unwrap_or(e)) as u32;
            integer = integer.add(&Big::from_i128(k)?.shl(shift)?)?;
        } else if slot < split {
            let shift = (e - low_integer.unwrap_or(e)) as u32;
            let term = dual.scaled[slot - integers].1.mul_i64(m)?.shl(shift)?;
            integer = integer.add(&term)?;
        } else {
            let shift = (e - low_explicit.unwrap_or(e)) as u32;
            explicit = explicit.add(&dual.basis[slot - split].1.mul_i64(m)?.shl(shift)?)?;
        }
    }
    let integer = dual.scale.mul(&integer)?;
    let total = match (low_integer, low_explicit) {
        (Some(a), Some(b)) => {
            let low = a.min(b);
            integer
                .shl((a - low) as u32)?
                .add(&explicit.shl((b - low) as u32)?)?
        }
        (Some(_), None) => integer,
        (None, Some(_)) => explicit,
        (None, None) => return Ok(true),
    };
    Ok(total.is_zero())
}

/// `v = m 2^e` with odd `m`, for finite nonzero `v`.
fn dyadic_parts(v: f64) -> (i64, i32) {
    let bits = v.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    let (mut m, mut e) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), exponent - 1075)
    };
    let shift = m.trailing_zeros();
    m >>= shift;
    e += shift as i32;
    let m = m as i64;
    (if bits >> 63 == 1 { -m } else { m }, e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_integer_multipliers_require_exact_channel_moments() {
        let cancels = |values: &[f64], dual: &[f64]| {
            zero_moment(0..values.len(), dual, |row| Ok(values[row])).unwrap()
        };
        let unit = f64::from_bits(1);
        assert!(cancels(&[unit, 2. * unit], &[2., -1.]));
        assert!(cancels(&[3., 5., 2.], &[1., -1., 1.]));
        // A rounded dot product is zero in each case, but the exact results
        // are nonzero. Neither roundoff in a sum nor a product proves balance.
        let large = (1_u64 << 53) as f64;
        assert!(!cancels(&[large, 1., large], &[1., 1., -1.]));
        assert!(!cancels(&[large - 1., 3. * (large - 1.)], &[3., -1.]));
        assert!(!cancels(&[1., 1. + f64::EPSILON], &[1., -1.]));
    }
}
