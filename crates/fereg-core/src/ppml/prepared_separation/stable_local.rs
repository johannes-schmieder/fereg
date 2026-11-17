//! Full local FE span audit for a proposed no-separation dual. Gram rank
//! thresholds cannot erase a cone direction. Row-pivoted elimination exposes
//! each channel contrast in original-source coordinates; exact expansion sums
//! distinguish a zero dependence from a small, but nonzero, local direction.
use super::*;

pub(super) fn workspace_bound(generation: &PreparedPpmlModelGeneration) -> Result<(u128, u128)> {
    if generation
        .prepared()
        .as_ref()
        .effect_specs()
        .iter()
        .all(|s| s.slopes.is_empty())
    {
        return Ok((0, 0));
    }
    let topology = DetectorTopology::new(generation.prepared())?;
    let plan = audit_plan(generation, &topology)?;
    Ok((plan.required, plan.legacy))
}

pub(super) fn certify(
    generation: &PreparedPpmlModelGeneration,
    dual: &[f64],
    margin: f64,
    tolerance: f64,
    max_iterations: usize,
    admitted_total_bytes: u128,
) -> Result<bool> {
    if generation
        .prepared()
        .as_ref()
        .effect_specs()
        .iter()
        .all(|s| s.slopes.is_empty())
    {
        return Ok(true);
    }
    let topology = DetectorTopology::new(generation.prepared())?;
    let plan = audit_plan(generation, &topology)?;
    let admitted = plan.legacy.max(admitted_total_bytes);
    if plan.required > admitted {
        return Err(allocation("full FE span audit exceeds admitted workspace"));
    }
    if plan.equivalent {
        pooled_certified(
            generation,
            &topology,
            dual,
            margin * tolerance,
            &plan,
            admitted,
            max_iterations,
        )
    } else {
        complete_certified(
            generation,
            &topology,
            dual,
            margin * tolerance,
            &plan,
            admitted,
            max_iterations,
        )
    }
}

struct AuditPlan {
    n: usize,
    total: usize,
    dimensions: usize,
    intercept: bool,
    structural: bool,
    equivalent: bool,
    legacy: u128,
    required: u128,
}
fn audit_plan(
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
) -> Result<AuditPlan> {
    let categorical = generation.prepared().as_ref();
    let specs = categorical.effect_specs();
    let n = generation.outcome().len();
    let (mut total, mut maximum, mut members_max, mut dimensions) = (0_usize, 0, 0, 0_usize);
    let mut intercept = false;
    let mut slope_count = 0_usize;
    for (effect, spec) in specs.iter().enumerate() {
        let channels = spec.slopes.len() + usize::from(spec.intercept);
        total = total
            .checked_add(channels)
            .ok_or_else(|| allocation("dual channel count overflows"))?;
        slope_count = slope_count
            .checked_add(spec.slopes.len())
            .ok_or_else(|| allocation("dual slope count overflows"))?;
        maximum = maximum.max(channels);
        intercept |= spec.intercept;
        let source = topology.sources[topology.effect_to_source[effect]];
        dimensions = source
            .n_levels()
            .checked_mul(channels)
            .and_then(|v| dimensions.checked_add(v))
            .ok_or_else(|| allocation("dual FE dimension overflows"))?;
        for level in 0..source.n_levels() {
            crate::interrupt::checkpoint()?;
            members_max = members_max.max(source.checked_members(level)?.len());
        }
    }
    let structural = generation.fit_view().fit_structural_intercept() && !intercept;
    dimensions = dimensions
        .checked_add(usize::from(structural))
        .ok_or_else(|| allocation("dual FE dimension overflows"))?;
    let legacy = audit_envelope(n, specs.len(), total, maximum, members_max)?;
    // This N-word equivalence proof fits the already admitted seven-vector
    // linear envelope and is released before either coefficient audit bank.
    let source = topology.sources[topology.effect_to_source[0]];
    let mut representative =
        fallible_filled(n, 0_usize, "unable to reserve partition equivalence proof")?;
    if representative.capacity() as u128 * std::mem::size_of::<usize>() as u128 > legacy {
        return Err(allocation(
            "partition proof exceeds existing linear admission",
        ));
    }
    fill_representatives(source, &mut representative)?;
    let mut equivalent = !structural;
    for &other in topology.effect_to_source {
        equivalent &= same_partition(source, topology.sources[other], &representative)?;
    }
    let views = total as u128 * std::mem::size_of::<&[f64]>() as u128;
    let required = if equivalent {
        scratch_bytes(slope_count + usize::from(intercept), views)?
    } else {
        scratch_bytes(
            dimensions,
            views + dimensions as u128 * std::mem::size_of::<Column<'_>>() as u128,
        )?
    };
    Ok(AuditPlan {
        n,
        total,
        dimensions,
        intercept,
        structural,
        equivalent,
        legacy,
        required,
    })
}

fn pooled_certified(
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    dual: &[f64],
    bound: f64,
    plan: &AuditPlan,
    admitted: u128,
    iterations: usize,
) -> Result<bool> {
    let categorical = generation.prepared().as_ref();
    let mut slopes: Vec<&[f64]> = Vec::new();
    slopes
        .try_reserve_exact(plan.total)
        .map_err(|_| allocation("unable to reserve pooled slope views"))?;
    for spec in categorical.effect_specs() {
        for slope in &spec.slopes {
            let values = categorical.numeric_source_values(slope.source)?;
            if !slopes
                .iter()
                .any(|prior| std::ptr::eq::<[f64]>(*prior, values))
            {
                slopes.push(values);
            }
        }
    }
    let channels = slopes.len() + usize::from(plan.intercept);
    let overhead = slopes.capacity() as u128 * std::mem::size_of::<&[f64]>() as u128;
    let mut scratch = AuditScratch::new(channels, overhead, admitted)?;
    let source = topology.sources[topology.effect_to_source[0]];
    let mut work = 0_u128;
    for level in 0..source.n_levels() {
        crate::interrupt::checkpoint()?;
        work = work.saturating_add(audit_work(
            source.checked_members(level)?.len(),
            channels,
            channels,
        ));
    }
    if work > work_limit(plan.n, plan.total, iterations) {
        return Err(audit_refusal());
    }
    for level in 0..source.n_levels() {
        crate::interrupt::checkpoint()?;
        if !level_certified(
            source.checked_members(level)?,
            plan.intercept,
            &slopes,
            dual,
            bound,
            &mut scratch,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn complete_certified(
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    dual: &[f64],
    bound: f64,
    plan: &AuditPlan,
    admitted: u128,
    iterations: usize,
) -> Result<bool> {
    let categorical = generation.prepared().as_ref();
    let specs = categorical.effect_specs();
    admit_work(plan.n, plan.dimensions, plan.total, iterations)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(plan.dimensions)
        .map_err(|_| allocation("unable to reserve complete FE columns"))?;
    let mut known_sources: Vec<&[f64]> = Vec::new();
    known_sources
        .try_reserve_exact(plan.total)
        .map_err(|_| allocation("unable to reserve exact slope aliases"))?;
    let overhead = columns.capacity() as u128 * std::mem::size_of::<Column<'_>>() as u128
        + known_sources.capacity() as u128 * std::mem::size_of::<&[f64]>() as u128;
    let mut scratch = AuditScratch::new(plan.dimensions, overhead, admitted)?;
    let mut constant = plan.structural;
    if constant {
        columns.push(Column {
            members: None,
            values: None,
        });
    }
    // Sum of every partition's intercept columns equals the same global one.
    for (effect, spec) in specs.iter().enumerate().filter(|(_, s)| s.intercept) {
        let source = topology.sources[topology.effect_to_source[effect]];
        for level in 0..source.n_levels() - usize::from(constant) {
            columns.push(Column {
                members: Some(source.checked_members(level)?),
                values: None,
            });
        }
        constant |= spec.intercept;
    }
    for (effect, spec) in specs.iter().enumerate() {
        let source = topology.sources[topology.effect_to_source[effect]];
        for slope in &spec.slopes {
            crate::interrupt::checkpoint()?;
            let values = categorical.numeric_source_values(slope.source)?;
            let mut alias = false;
            for &prior in &known_sources {
                if exact_raw_alias(prior, values, constant, &mut scratch.expansion)? {
                    alias = true;
                    break;
                }
            }
            // A proven raw-source multiple has the same global sum across
            // partitions. Its final level is exactly the prior global sum
            // minus its other levels, independently of numerical elimination.
            for level in 0..source.n_levels() - usize::from(alias) {
                columns.push(Column {
                    members: Some(source.checked_members(level)?),
                    values: Some(values),
                });
            }
            if !alias {
                known_sources.push(values);
            }
        }
    }
    span_certified(
        0..plan.n,
        columns.len(),
        None,
        |c, r| columns[c].value(r),
        dual,
        bound,
        &mut scratch,
    )
}

fn exact_raw_alias(
    left: &[f64],
    right: &[f64],
    constant: bool,
    expansion: &mut Vec<f64>,
) -> Result<bool> {
    if left == right {
        return Ok(true);
    }
    let pivot = if constant {
        left.iter().position(|v| *v != left[0])
    } else {
        left.iter().position(|v| *v != 0.)
    };
    let Some(pivot) = pivot else {
        return Ok(constant && right.iter().all(|v| *v == right[0]));
    };
    let (multiple, offset) = if constant {
        let multiple = (right[pivot] - right[0]) / (left[pivot] - left[0]);
        (multiple, (-multiple).mul_add(left[0], right[0]))
    } else {
        (right[pivot] / left[pivot], 0.)
    };
    if !multiple.is_finite() || !offset.is_finite() {
        return Ok(false);
    }
    for row in 0..left.len() {
        crate::interrupt::row_checkpoint(row)?;
        if evaluate_values(
            &[offset, multiple, -1.],
            &|c, r| match c {
                0 => 1.,
                1 => left[r],
                _ => right[r],
            },
            row,
            expansion,
        ) != Some(0.)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

mod condition;
use condition::condition_certified;

struct Column<'a> {
    members: Option<&'a [usize]>,
    values: Option<&'a [f64]>,
}
impl Column<'_> {
    fn value(&self, row: usize) -> f64 {
        if self
            .members
            .is_some_and(|members| members.binary_search(&row).is_err())
        {
            0.
        } else {
            self.values.map_or(1., |values| values[row])
        }
    }
}

fn fill_representatives(source: &LevelIndex, representatives: &mut [usize]) -> Result<()> {
    for level in 0..source.n_levels() {
        let members = source.checked_members(level)?;
        for (position, &row) in members.iter().enumerate() {
            crate::interrupt::row_checkpoint(position)?;
            representatives[row] = members[0];
        }
    }
    Ok(())
}
fn same_partition(
    reference: &LevelIndex,
    other: &LevelIndex,
    representatives: &[usize],
) -> Result<bool> {
    if reference.n_levels() != other.n_levels() {
        return Ok(false);
    }
    for level in 0..reference.n_levels() {
        if reference.checked_members(level)?.is_empty() {
            return Ok(false);
        }
    }
    for level in 0..other.n_levels() {
        let members = other.checked_members(level)?;
        if members.is_empty() {
            return Ok(false);
        }
        for (position, &row) in members.iter().enumerate() {
            crate::interrupt::row_checkpoint(position)?;
            if representatives[row] != representatives[members[0]] {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn audit_envelope(
    n: usize,
    effects: usize,
    total: usize,
    maximum: usize,
    members: usize,
) -> Result<u128> {
    let cells = (n as u128)
        .checked_mul(total as u128 + effects as u128 + 7)
        .and_then(|v| {
            crate::fe::MapCapacity::new(maximum as u128, members as u128, 1)
                .ok()
                .and_then(|plan| plan.cells().ok())
                .and_then(|map| v.checked_add(map))
        })
        .ok_or_else(|| allocation("full FE audit envelope overflows"))?;
    cells
        .checked_mul(8)
        .ok_or_else(|| allocation("full FE audit byte envelope overflows"))
}
fn audit_refusal() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "no-separation full FE span certificate exceeds its admitted workspace or work budget",
    )
}
fn audit_work(n: usize, dimensions: usize, channels: usize) -> u128 {
    // Gram construction evaluates D² normalized basis pairs at every row;
    // each evaluation streams D coefficients and at most S active channels.
    let width = dimensions as u128 + (channels as u128).saturating_mul(channels as u128);
    (n as u128 + dimensions as u128)
        .saturating_mul(dimensions as u128)
        .saturating_mul(dimensions as u128)
        .saturating_mul(width)
}
fn work_limit(n: usize, channels: usize, iterations: usize) -> u128 {
    (iterations as u128)
        .saturating_mul(n as u128)
        .saturating_mul(channels.max(1) as u128)
}
fn admit_work(n: usize, dimensions: usize, channels: usize, iterations: usize) -> Result<()> {
    if audit_work(n, dimensions, channels) > work_limit(n, channels, iterations) {
        return Err(audit_refusal());
    }
    Ok(())
}

fn scratch_bytes(channels: usize, overhead: u128) -> Result<u128> {
    let c = channels as u128;
    c.checked_mul(c)
        .and_then(|v| v.checked_mul(4))
        .and_then(|v| v.checked_add(7 * c + 1))
        .and_then(|v| v.checked_mul(8))
        .and_then(|v| v.checked_add(overhead))
        .ok_or_else(|| allocation("full FE audit byte count overflows"))
}
struct AuditScratch {
    bank: Vec<f64>,
    gram: Vec<f64>,
    factor: Vec<f64>,
    inverse: Vec<f64>,
    scales: Vec<f64>,
    norms: Vec<f64>,
    moments: Vec<f64>,
    absolute_moments: Vec<f64>,
    pivots: Vec<usize>,
    expansion: Vec<f64>,
}
impl AuditScratch {
    fn new(channels: usize, overhead: u128, admitted: u128) -> Result<Self> {
        let cells = channels
            .checked_mul(channels)
            .ok_or_else(|| allocation("full FE audit bank overflows"))?;
        let expansion = channels
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| allocation("full FE audit expansion overflows"))?;
        let modeled = scratch_bytes(channels, overhead)?;
        if modeled > admitted {
            return Err(audit_refusal());
        }
        let bank = fallible_filled(cells, 0., "unable to reserve full FE dual geometry")?;
        let gram = fallible_filled(cells, 0., "unable to reserve dual condition Gram")?;
        let factor = fallible_filled(cells, 0., "unable to reserve dual condition factor")?;
        let inverse = fallible_filled(cells, 0., "unable to reserve dual condition inverse")?;
        let scales = fallible_filled(channels, 0., "unable to reserve dual scales")?;
        let norms = fallible_filled(channels, 0., "unable to reserve dual norms")?;
        let moments = fallible_filled(channels, 0., "unable to reserve dual moments")?;
        let absolute_moments =
            fallible_filled(channels, 0., "unable to reserve dual moment bounds")?;
        let pivots = fallible_filled(channels, 0_usize, "unable to reserve full FE dual pivots")?;
        let mut tails = Vec::new();
        tails
            .try_reserve_exact(expansion)
            .map_err(|_| allocation("unable to reserve full FE dual expansion"))?;
        let actual = overhead
            + 8 * ((bank.capacity()
                + gram.capacity()
                + factor.capacity()
                + inverse.capacity()
                + scales.capacity()
                + norms.capacity()
                + moments.capacity()
                + absolute_moments.capacity()
                + tails.capacity()) as u128)
            + std::mem::size_of::<usize>() as u128 * pivots.capacity() as u128;
        if actual > admitted {
            return Err(allocation("full FE audit capacities exceed admission"));
        }
        Ok(Self {
            bank,
            gram,
            factor,
            inverse,
            scales,
            norms,
            moments,
            absolute_moments,
            pivots,
            expansion: tails,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn level_certified(
    members: &[usize],
    intercept: bool,
    slopes: &[&[f64]],
    dual: &[f64],
    bound: f64,
    scratch: &mut AuditScratch,
) -> Result<bool> {
    let channels = slopes.len() + usize::from(intercept);
    span_certified(
        members.iter().copied(),
        channels,
        intercept.then_some(0),
        |channel, row| {
            if intercept && channel == 0 {
                1.
            } else {
                slopes[channel - usize::from(intercept)][row]
            }
        },
        dual,
        bound,
        scratch,
    )
}

#[allow(clippy::too_many_arguments)]
fn span_certified<I: Iterator<Item = usize> + Clone>(
    members: I,
    channels: usize,
    intercept: Option<usize>,
    value: impl Fn(usize, usize) -> f64,
    dual: &[f64],
    bound: f64,
    scratch: &mut AuditScratch,
) -> Result<bool> {
    let Some(first) = members.clone().next() else {
        return Ok(true);
    };
    let AuditScratch {
        bank,
        pivots,
        expansion,
        scales,
        norms,
        moments,
        absolute_moments,
        ..
    } = scratch;
    bank.fill(0.);
    let mut rank = 0;
    for channel in 0..channels {
        crate::interrupt::checkpoint()?;
        let (previous, rest) = bank.split_at_mut(rank * channels);
        let candidate = &mut rest[..channels];
        candidate.fill(0.);
        candidate[channel] = 1.;
        if let Some(constant) = intercept {
            if channel != constant {
                candidate[constant] = -value(channel, first);
            }
        }
        for (index, basis) in previous.chunks_exact(channels).enumerate() {
            crate::interrupt::checkpoint()?;
            let row = pivots[index];
            let Some(numerator) = evaluate_values(candidate, &value, row, expansion) else {
                return Ok(false);
            };
            let Some(denominator) = evaluate_values(basis, &value, row, expansion) else {
                return Ok(false);
            };
            if denominator == 0. {
                return Ok(false);
            }
            let multiplier = numerator / denominator;
            for (value, &basis) in candidate.iter_mut().zip(basis) {
                *value = (-multiplier).mul_add(basis, *value);
                if !value.is_finite() {
                    return Ok(false);
                }
            }
        }
        let (mut scale, mut pivot) = (0.0_f64, first);
        for (position, row) in members.clone().enumerate() {
            crate::interrupt::row_checkpoint(position)?;
            let Some(value) = evaluate_values(candidate, &value, row, expansion) else {
                return Ok(false);
            };
            if value.abs() > scale {
                scale = value.abs();
                pivot = row;
            }
        }
        // evaluate returns zero only for an exactly zero expansion. A merely
        // tiny direction must be tested at its own scale, never dropped.
        if scale == 0. {
            continue;
        }
        let (mut moment, mut correction, mut absolute, mut absolute_correction) = (0., 0., 0., 0.);
        let mut norm = ScaledSquares::ZERO;
        for (position, row) in members.clone().enumerate() {
            crate::interrupt::row_checkpoint(position)?;
            let Some(value) = evaluate_values(candidate, &value, row, expansion) else {
                return Ok(false);
            };
            let Some(normalized) = normalized_value(value, scale, 1.) else {
                return Ok(false);
            };
            neumaier_add(normalized * dual[row], &mut moment, &mut correction);
            neumaier_add(
                (normalized * dual[row]).abs(),
                &mut absolute,
                &mut absolute_correction,
            );
            norm.add(normalized.abs());
        }
        let residual = (moment + correction).abs() / norm.norm();
        if !residual.is_finite() || residual > bound {
            return Ok(false);
        }
        pivots[rank] = pivot;
        scales[rank] = scale;
        norms[rank] = norm.norm();
        moments[rank] = moment + correction;
        absolute_moments[rank] = absolute + absolute_correction;
        rank += 1;
    }
    condition_certified(members, channels, rank, &value, bound, scratch)
}

fn normalized_value(value: f64, scale: f64, norm: f64) -> Option<f64> {
    let scaled = value / scale;
    let normalized = scaled / norm;
    if !normalized.is_finite()
        || (value != 0.
            && (scaled.abs() < f64::MIN_POSITIVE || normalized.abs() < f64::MIN_POSITIVE))
    {
        None
    } else {
        Some(normalized)
    }
}

/// Products and sums are expanded without discarding their low parts. This is
/// a dependence proof over the actual stored doubles and coefficient vector,
/// not a tolerance test on a rounded Gram. Nonrepresentable arithmetic refuses
/// the no-drop certificate. The caller reserves 2*channels+1 entries once.
fn evaluate_values(
    coefficients: &[f64],
    values: &impl Fn(usize, usize) -> f64,
    row: usize,
    expansion: &mut Vec<f64>,
) -> Option<f64> {
    expansion.clear();
    for (channel, &coefficient) in coefficients.iter().enumerate() {
        if coefficient == 0. {
            continue;
        }
        let value = values(channel, row);
        if value == 0. {
            continue;
        }
        let high = coefficient * value;
        // The product's least nonzero dyadic bit must be representable.
        // This exact exponent test also admits tiny integer/power-of-two
        // products; a magnitude threshold alone misses carry/rounding edges.
        if !high.is_finite()
            || high == 0.
            || least_bit_exponent(coefficient) + least_bit_exponent(value) < -1074
        {
            return None;
        }
        let low = coefficient.mul_add(value, -high);
        grow(expansion, low)?;
        grow(expansion, high)?;
    }
    if expansion.is_empty() {
        return Some(0.);
    }
    let value = expansion.iter().copied().sum::<f64>();
    if value == 0. || !value.is_finite() {
        None
    } else {
        Some(value)
    }
}

fn least_bit_exponent(value: f64) -> i32 {
    let bits = value.to_bits() & 0x7fff_ffff_ffff_ffff;
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    let mantissa = if exponent == 0 {
        fraction
    } else {
        fraction | (1_u64 << 52)
    };
    let base = if exponent == 0 {
        -1074
    } else {
        exponent - 1075
    };
    base + mantissa.trailing_zeros() as i32
}

fn grow(expansion: &mut Vec<f64>, mut value: f64) -> Option<()> {
    if value == 0. {
        return Some(());
    }
    let mut used = 0;
    for index in 0..expansion.len() {
        let entry = expansion[index];
        let sum = value + entry;
        if !sum.is_finite() {
            return None;
        }
        let virtual_entry = sum - value;
        let error = (value - (sum - virtual_entry)) + (entry - virtual_entry);
        if error != 0. {
            expansion[used] = error;
            used += 1;
        }
        value = sum;
    }
    expansion.truncate(used);
    if value != 0. {
        expansion.push(value);
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn evaluate(
        coefficients: &[f64],
        intercept: bool,
        slopes: &[&[f64]],
        row: usize,
        expansion: &mut Vec<f64>,
    ) -> Option<f64> {
        evaluate_values(
            coefficients,
            &|channel, row| {
                if intercept && channel == 0 {
                    1.
                } else {
                    slopes[channel - usize::from(intercept)][row]
                }
            },
            row,
            expansion,
        )
    }
    #[test]
    fn exact_shared_slope_aliases_include_integer_power_two_and_affine_sources() {
        let left = [-1., 0., 1., 2.];
        let mut expansion = Vec::with_capacity(7);
        for (a, b) in [(1., 0.), (2., 0.), (3., 0.), (0.5, 8.)] {
            let mut right: Vec<_> = left.iter().map(|x| a * x + b).collect();
            assert!(exact_raw_alias(&left, &right, true, &mut expansion).unwrap());
            right[1] += 2.0_f64.powi(-48);
            assert!(!exact_raw_alias(&left, &right, true, &mut expansion).unwrap());
        }
    }

    #[test]
    fn full_span_norm_bound_refuses_ill_conditioned_basis_moment_false_certificate() {
        let dimension = 36;
        let n = dimension * 2;
        let slopes: Vec<Vec<f64>> = (1..dimension)
            .map(|j| {
                (0..n)
                    .map(|row| {
                        let i = row / 2;
                        if i < j {
                            0.
                        } else if i == j {
                            -1.
                        } else {
                            1.
                        }
                    })
                    .collect()
            })
            .collect();
        let views: Vec<_> = slopes.iter().map(Vec::as_slice).collect();
        let dual: Vec<_> = (0..n)
            .map(|row| {
                let i = row / 2;
                if i == 0 {
                    1.
                } else if i == dimension - 1 {
                    -2.0_f64.powi(-34)
                } else {
                    -2.0_f64.powi(-(i as i32))
                }
            })
            .collect();
        let members: Vec<_> = (0..n).collect();
        let mut scratch = AuditScratch::new(dimension, 0, u128::MAX).unwrap();
        assert!(!level_certified(&members, true, &views, &dual, 1e-10, &mut scratch).unwrap());
    }

    #[test]
    fn product_tail_underflow_cannot_prove_a_false_zero() {
        let a = f64::from_bits(0x2198000000000001);
        let b = f64::from_bits(0x21a8000000000001);
        let high = f64::from_bits(0x0352000000000002);
        let low = f64::from_bits(0x8008000000000000);
        let mut expansion = Vec::with_capacity(7);
        // The true residual is 2^-1075, below the binary64 subnormal range.
        assert_eq!(
            evaluate(&[-high, a, 1.], true, &[&[b], &[-low]], 0, &mut expansion),
            None
        );
    }

    #[test]
    fn complete_crossed_span_preserves_ordinary_duals_and_refuses_level_weighted_contrasts() {
        let n = 48;
        let g: Vec<_> = (0..n).map(|i| (i / 12) as u32).collect();
        let gi = LevelIndex::from_codes(&g, 4).unwrap();
        for contrast in [false, true] {
            let h: Vec<_> = (0..n)
                .map(|i| ((i / if contrast { 3 } else { 6 }) % 2) as u32)
                .collect();
            let hi = LevelIndex::from_codes(&h, 2).unwrap();
            let x1: Vec<_> = (0..n)
                .map(|i| (1 + (if contrast { i } else { i / 2 }) % 3) as f64 / 16.)
                .collect();
            let x2: Vec<_> = (0..n)
                .map(|i| {
                    if contrast {
                        (2.0_f64.powi(g[i] as i32 + 1) * x1[i] + 2.0_f64.powi(-48) * (i % 2) as f64)
                            / 2.0_f64.powi(h[i] as i32 + 1)
                    } else {
                        (1 + (i / 2) % 5) as f64 / 8.
                    }
                })
                .collect();
            let dual: Vec<_> = (0..n)
                .map(|i| {
                    if contrast {
                        [-0.5, 1., -0.5, 1., -2., 1.][i % 6]
                    } else if i % 2 == 0 {
                        -1.
                    } else {
                        1.
                    }
                })
                .collect();
            let mut columns = Vec::new();
            for level in 0..4 {
                columns.push(Column {
                    members: Some(gi.members(level)),
                    values: None,
                });
                columns.push(Column {
                    members: Some(gi.members(level)),
                    values: Some(&x1),
                });
            }
            for level in 0..2 {
                columns.push(Column {
                    members: Some(hi.members(level)),
                    values: Some(&x2),
                });
            }
            if contrast {
                for row in 0..n {
                    assert_eq!(
                        2.0_f64.powi(48)
                            * (2.0_f64.powi(g[row] as i32 + 1) * x1[row]
                                - 2.0_f64.powi(h[row] as i32 + 1) * x2[row]),
                        -((row % 2) as f64)
                    );
                }
                // Unresolved raw moments would accept this dual.
                for column in &columns {
                    let dot: f64 = (0..n).map(|row| column.value(row) * dual[row]).sum();
                    assert!(dot.abs() < 1e-10);
                }
            }
            let mut scratch = AuditScratch::new(columns.len(), 0, u128::MAX).unwrap();
            let certified = span_certified(
                0..n,
                columns.len(),
                None,
                |c, r| columns[c].value(r),
                &dual,
                1e-10,
                &mut scratch,
            )
            .unwrap();
            assert_eq!(certified, !contrast);
        }
    }

    #[test]
    fn partition_equivalence_ignores_labels_and_audit_admission_has_exact_boundaries() {
        let a = LevelIndex::from_codes(&[0, 0, 1, 1, 2, 2], 3).unwrap();
        let b = LevelIndex::from_codes(&[2, 2, 0, 0, 1, 1], 3).unwrap();
        let c = LevelIndex::from_codes(&[0, 1, 0, 1, 2, 2], 3).unwrap();
        let mut representatives = [0; 6];
        fill_representatives(&a, &mut representatives).unwrap();
        assert!(same_partition(&a, &b, &representatives).unwrap());
        assert!(!same_partition(&a, &c, &representatives).unwrap());
        let bytes = 8 * (4 * 9 + 7 + 4 * 3 + 3);
        assert!(AuditScratch::new(3, 0, bytes).is_ok());
        assert_eq!(
            AuditScratch::new(3, 0, bytes - 1).err().unwrap().code,
            ErrorCode::NonConvergence
        );
        assert!(admit_work(48, 10, 3, 1000).is_ok());
        assert_eq!(
            admit_work(48, 10, 3, 1).unwrap_err().code,
            ErrorCode::NonConvergence
        );
    }

    #[test]
    fn local_dual_checks_tiny_contrasts_but_accepts_exact_duplicate_channels() {
        let members: Vec<_> = (0..12).collect();
        let first: Vec<_> = (0..12).map(|i| (i % 3) as f64 - 1.).collect();
        let dual: Vec<_> = (0..12).map(|i| if i % 2 == 0 { -1. } else { 1. }).collect();
        for exponent in [0, 40, 48] {
            let second: Vec<_> = first
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    v + if exponent == 0 {
                        0.
                    } else {
                        2.0_f64.powi(-exponent) * (i % 2) as f64
                    }
                })
                .collect();
            let mut scratch = AuditScratch::new(3, 0, u128::MAX).unwrap();
            let certified = level_certified(
                &members,
                true,
                &[&first, &second],
                &dual,
                1e-10,
                &mut scratch,
            )
            .unwrap();
            assert_eq!(certified, exponent == 0, "exponent={exponent}");
        }
    }

    #[test]
    fn exact_expansions_retain_products_hidden_by_a_large_offset() {
        let values = [1e15, 1e15 + 1.];
        let mut expansion = Vec::with_capacity(5);
        assert_eq!(
            evaluate(&[-1e15, 1.], true, &[&values], 0, &mut expansion),
            Some(0.)
        );
        assert_eq!(
            evaluate(&[-1e15, 1.], true, &[&values], 1, &mut expansion),
            Some(1.)
        );
    }
}
