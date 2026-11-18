//! Solve x + a[g] + b[h] = 0 on the binding rows of each admissible pair of
//! intercepted effects. A spanning forest proposes potentials; an all-row
//! exact sign check independently validates any resulting recession direction.
use super::*;

pub(super) fn search(search: &mut Search<'_>, work: usize, limit: usize) -> Result<bool> {
    let model = search.model;
    let design = RawRows::new(model, true)?;
    let topology = DetectorTopology::new(model.prepared())?;
    let n = design.len();
    let mut remaining = work / n.saturating_mul(6).max(1);
    for left in 0..design.blocks.len() {
        for right in left + 1..design.blocks.len() {
            let effects = [left, right];
            let blocks = [&design.blocks[left], &design.blocks[right]];
            let vertices = blocks[0].levels.saturating_add(blocks[1].levels);
            if !blocks.iter().all(|b| b.intercept) || vertices.saturating_mul(4) > limit {
                continue;
            }
            let indexes = effects.map(|e| topology.sources[topology.effect_to_source[e]]);
            for column in 0..model.model().columns.len() {
                if remaining == 0 {
                    return Ok(false);
                }
                remaining -= 1;
                let Some(mut offsets) = potentials(search, column, blocks, indexes, vertices)?
                else {
                    continue;
                };
                for negative in [false, true] {
                    let witness = Witness::PairOffsets {
                        column,
                        effects,
                        offsets: std::mem::take(&mut offsets),
                        negative,
                    };
                    let count = strict_count(model, &witness, search.expected)?;
                    if count > search.count {
                        search.count = count;
                        search.best = Some(witness);
                        if count == search.expected.len() {
                            return Ok(true);
                        }
                        break; // Its reverse sign fails on these strict-negative rows.
                    } else if let Witness::PairOffsets {
                        offsets: values, ..
                    } = witness
                    {
                        offsets = values;
                    }
                }
            }
        }
    }
    Ok(false)
}

fn potentials(
    search: &Search<'_>,
    column: usize,
    blocks: [&Block<'_>; 2],
    indexes: [&LevelIndex; 2],
    vertices: usize,
) -> Result<Option<Vec<f64>>> {
    let model = search.model;
    let split = blocks[0].levels;
    let mut offsets = fallible_filled(vertices, f64::NAN, "unable to reserve exact FE potentials")?;
    let mut queue = Vec::new();
    queue
        .try_reserve_exact(vertices)
        .map_err(|_| allocation("unable to reserve FE potential queue"))?;
    for start in 0..vertices {
        if offsets[start].is_finite() {
            continue;
        }
        offsets[start] = 0.;
        queue.clear();
        queue.push(start);
        let mut head = 0;
        while let Some(&here) = queue.get(head) {
            head += 1;
            crate::interrupt::row_checkpoint(head)?;
            let side = usize::from(here >= split);
            let level = if side == 0 { here } else { here - split };
            for (j, &r) in indexes[side].checked_members(level)?.iter().enumerate() {
                crate::interrupt::row_checkpoint(j)?;
                if search
                    .expected
                    .binary_search(&model.original_rows()[r])
                    .is_ok()
                {
                    continue;
                }
                let other = blocks[1 - side].codes[r] as usize + if side == 0 { split } else { 0 };
                let x = model.fit_view().column_value(column, r)?;
                let value = -x - offsets[here];
                if !value.is_finite() {
                    return Ok(None);
                }
                if offsets[other].is_nan() {
                    offsets[other] = value;
                    queue.push(other);
                } else if offsets[other] != value {
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(offsets))
}

pub(super) fn visit(
    model: &PreparedPpmlModelGeneration,
    column: usize,
    effects: [usize; 2],
    offsets: &[f64],
    negative: bool,
    mut inspect: impl FnMut(usize, Ordering, f64) -> bool,
) -> Result<bool> {
    let design = RawRows::new(model, true)?;
    let a = &design.blocks[effects[0]];
    let b = &design.blocks[effects[1]];
    if !a.intercept || !b.intercept || offsets.len() != a.levels + b.levels {
        return Ok(false);
    }
    let sign = if negative { -1 } else { 1 };
    for r in 0..design.len() {
        crate::interrupt::row_checkpoint(r)?;
        let terms = [
            model.fit_view().column_value(column, r)?,
            offsets[a.codes[r] as usize],
            offsets[a.levels + b.codes[r] as usize],
        ];
        let Some((ord, value)) = exact_score(&terms, &[sign; 3]) else {
            return Ok(false);
        };
        if !inspect(r, ord, value) {
            return Ok(false);
        }
    }
    Ok(true)
}
