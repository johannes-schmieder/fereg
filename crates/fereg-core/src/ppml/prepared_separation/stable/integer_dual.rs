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
