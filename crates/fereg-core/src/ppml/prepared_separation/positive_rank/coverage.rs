//! Boundary rows that exactly match positive rows cannot separate, even if the
//! design contains structural aliases. Hashes only select comparison candidates.
use super::*;

enum Coordinate<'a> {
    Finite(usize),
    Group(&'a [u32]),
    Slope(&'a [f64]),
}

impl Coordinate<'_> {
    fn bits(&self, model: &PreparedPpmlModelGeneration, row: usize) -> Result<u64> {
        let value = match self {
            Self::Finite(i) => model.fit_view().column_value(*i, row)?,
            Self::Group(codes) => return Ok(u64::from(codes[row])),
            Self::Slope(values) => values[row],
        };
        if !value.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "nonfinite positive-support design coordinate",
            ));
        }
        Ok(if value == 0.0 { 0 } else { value.to_bits() })
    }
}

fn row_hash(
    model: &PreparedPpmlModelGeneration,
    keys: &[Coordinate<'_>],
    row: usize,
) -> Result<u64> {
    keys.iter().try_fold(0xcbf29ce484222325, |h: u64, key| {
        Ok((h ^ key.bits(model, row)?).wrapping_mul(0x100000001b3))
    })
}

/// On success, the optional unit-mass dual is +1 on each zero and subtracts
/// one from its matched positive. On an inconclusive scan discard the scratch.
/// The caller admits that optional vector separately. Remaining heap is bounded
/// by the supplied eight-byte cell budget, including hash collision chains.
pub(in super::super) fn positive_twins(
    model: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    boundary: usize,
    limit: usize,
    work: usize,
    mut dual: Option<&mut [f64]>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<bool> {
    let y = model.outcome();
    if boundary == 0 || boundary >= y.len() || boundary as u128 > (1u128 << 53) {
        return Ok(false);
    }
    if dual.as_ref().is_some_and(|d| d.len() != y.len()) {
        return Err(invalid_argument(
            "positive-twin dual and outcome differ in length",
        ));
    }
    let finite = model.model().columns.len();
    let Some(width) = topology
        .effect_specs
        .iter()
        .try_fold(finite, |w, e| w.checked_add(1)?.checked_add(e.slopes.len()))
    else {
        return Ok(false);
    };
    let cells = std::mem::size_of::<Coordinate<'_>>().div_ceil(8);
    if width
        .saturating_mul(cells)
        .saturating_add(boundary.saturating_mul(4))
        > limit
        || y.len().saturating_mul(width.max(1)) > work
    {
        return Ok(false);
    }
    interrupt(PpmlInterruptPoint::SeparationSimplex)?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(width)
        .map_err(|_| allocation("unable to reserve positive-twin coordinates"))?;
    keys.extend((0..finite).map(Coordinate::Finite));
    for (&source, effect) in topology.effect_to_source.iter().zip(topology.effect_specs) {
        keys.push(Coordinate::Group(codes(model, source)?));
        for slope in &effect.slopes {
            keys.push(Coordinate::Slope(
                model.fit_view().numeric_source_values(slope.source)?,
            ));
        }
    }
    let mut heads = fallible_filled(
        boundary,
        usize::MAX,
        "unable to reserve positive-twin bucket heads",
    )?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(boundary)
        .map_err(|_| allocation("unable to reserve positive-twin boundary rows"))?;
    for (row, &value) in y.iter().enumerate() {
        crate::interrupt::row_checkpoint(row)?;
        if value == 0. {
            if entries.len() == boundary {
                return Err(invalid_argument("positive-twin boundary count changed"));
            }
            let hash = row_hash(model, &keys, row)?;
            let bucket = (hash % boundary as u64) as usize;
            let next = std::mem::replace(&mut heads[bucket], entries.len());
            entries.push((hash, row, next));
        }
    }
    if entries.len() != boundary {
        return Err(invalid_argument("positive-twin boundary count changed"));
    }
    if let Some(d) = dual.as_deref_mut() {
        d.fill(0.);
    }
    let mut remaining = boundary;
    // Cap bucket traversals and full comparisons separately from the two scans.
    let mut checks = work;
    let mut traversed = 0usize;
    for (row, &value) in y.iter().enumerate() {
        crate::interrupt::row_checkpoint(row)?;
        if value <= 0. {
            continue;
        }
        let hash = row_hash(model, &keys, row)?;
        let bucket = (hash % boundary as u64) as usize;
        let mut pos = heads[bucket];
        let mut prev: Option<usize> = None;
        while pos != usize::MAX {
            crate::interrupt::row_checkpoint(traversed)?;
            traversed += 1;
            let (candidate_hash, zero, next) = entries[pos];
            let cost = if candidate_hash == hash {
                width.max(1)
            } else {
                1
            };
            let Some(left) = checks.checked_sub(cost) else {
                return Ok(false);
            };
            checks = left;
            let mut equal = candidate_hash == hash;
            if equal {
                for key in &keys {
                    if key.bits(model, row)? != key.bits(model, zero)? {
                        equal = false;
                        break;
                    }
                }
            }
            if equal {
                if let Some(p) = prev {
                    entries[p].2 = next;
                } else {
                    heads[bucket] = next;
                }
                if let Some(d) = dual.as_deref_mut() {
                    d[zero] = 1.;
                    d[row] -= 1.;
                }
                remaining -= 1;
                if remaining == 0 {
                    return Ok(true);
                }
            } else {
                prev = Some(pos);
            }
            pos = next;
        }
    }
    Ok(false)
}
