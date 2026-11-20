//! Repeated raw rows give identical primal inequalities within an outcome class.
//! A strict dual on representatives lifts by dividing each mass among its copies.
//! This can use different divisors on different rows, including positive rows.
use super::{
    fallible_collect, fallible_filled, projection::BoundaryCone, verify_indexed_farkas_certificate,
    BinaryDesign, CompactModel, CompactSearch, SeparationCertificate, MAX_WORK,
};
use crate::error::Result;
use crate::ppml_separation::SeparationConfig;
use std::hash::{DefaultHasher, Hash, Hasher};

type ReadRow<'a> = dyn FnMut(usize, &mut Vec<u32>) -> Result<bool> + 'a;

pub(super) struct RawSupport<'a> {
    columns: usize,
    /// Maximum stored terms and a separate bound for evaluating their recipes.
    terms: usize,
    visits: usize,
    read: &'a mut ReadRow<'a>,
}

impl<'a> RawSupport<'a> {
    pub(super) fn new(
        (columns, terms, visits): (usize, usize, usize),
        read: &'a mut ReadRow<'a>,
    ) -> Self {
        Self {
            columns,
            terms,
            visits,
            read,
        }
    }

    fn scratch(&self) -> Result<Vec<u32>> {
        let mut row = Vec::new();
        super::reserve(&mut row, self.terms, "unable to reserve binary row support")?;
        Ok(row)
    }

    fn row(&mut self, i: usize, dest: &mut Vec<u32>) -> Result<bool> {
        dest.clear();
        (self.read)(i, dest)
    }

    pub(super) fn pack(
        &mut self,
        n: usize,
        origins: &[usize],
        known_ones: Option<usize>,
        limit: usize,
    ) -> Result<Option<BinaryDesign>> {
        let fixed = n.saturating_add(1).saturating_add(self.terms.div_ceil(2));
        if u32::try_from(self.columns).is_err()
            || n as u128 > (1u128 << 53)
            || n.saturating_mul(self.visits.saturating_add(1))
                .saturating_mul(2)
                > MAX_WORK
            || fixed > limit
        {
            return Ok(None);
        }
        let origin = |i| origins.get(i).copied().unwrap_or(i);
        let mut row = self.scratch()?;
        let max_ones = (limit - fixed).saturating_mul(2);
        let ones = if let Some(known) = known_ones {
            known
        } else {
            let mut total = 0;
            for i in 0..n {
                crate::interrupt::row_checkpoint(i)?;
                if !self.row(origin(i), &mut row)? {
                    return Ok(None);
                }
                total += row.len();
                if total > max_ones {
                    return Ok(None);
                }
            }
            total
        };
        if ones > max_ones {
            return Ok(None);
        }
        let mut offsets = fallible_filled(n + 1, 0, "unable to reserve compact binary rows")?;
        let mut indices = Vec::new();
        super::reserve(
            &mut indices,
            ones,
            "unable to reserve compact binary indices",
        )?;
        for i in 0..n {
            crate::interrupt::row_checkpoint(i)?;
            if !self.row(origin(i), &mut row)? || row.len() > ones - indices.len() {
                return Ok(None);
            }
            indices.extend_from_slice(&row);
            offsets[i + 1] = indices.len();
        }
        Ok((indices.len() == ones).then_some(BinaryDesign {
            offsets,
            indices,
            columns: self.columns,
        }))
    }
}

pub(super) struct RowGroups {
    index: Vec<u32>,
    counts: Vec<usize>,
    boundary_rows: usize,
}

pub(crate) enum CompactInput {
    Design(BinaryDesign),
    Proof(CompactModel),
}

impl BinaryDesign {
    pub(crate) fn prepare(
        outcome: &[f64],
        shape: (usize, usize, usize),
        limit: usize,
        config: SeparationConfig,
        pivots: &mut usize,
        read: &mut ReadRow<'_>,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Option<CompactInput>> {
        let mut raw = RawSupport::new(shape, read);
        let columns = raw.columns;
        let n = outcome.len();
        let positive = outcome.iter().filter(|&&y| y > 0.).count();
        let packed_admitted =
            super::compact_model_admitted(n, positive, columns, config.simplex_max_tableau_cells);
        // If both the unreduced panel and the positive solve fit, preserve the
        // ordinary compact route and avoid even constructing a raw-row index.
        let small = n.saturating_mul(columns) <= limit
            && super::dense_positive_admitted(n, positive, columns, limit);
        if !small {
            if let Some((index, distinct, ones)) = identify(outcome, &mut raw, limit, interrupt)? {
                if distinct < n {
                    if let Some(proof) = Self::repeated_model(
                        outcome,
                        &mut raw,
                        limit,
                        config,
                        pivots,
                        (index, distinct),
                        interrupt,
                    )? {
                        return Ok(Some(CompactInput::Proof(proof)));
                    }
                } else {
                    drop(index);
                }
                // Group identification already counted the ones on every
                // physical row. Reuse that scan on an all-distinct input.
                return if packed_admitted {
                    Ok(raw
                        .pack(n, &[], Some(ones), limit)?
                        .map(CompactInput::Design))
                } else {
                    Ok(None)
                };
            }
        }
        if !packed_admitted {
            return Ok(None);
        }
        Ok(raw.pack(n, &[], None, limit)?.map(CompactInput::Design))
    }

    fn repeated_model(
        outcome: &[f64],
        raw: &mut RawSupport,
        limit: usize,
        config: SeparationConfig,
        pivots: &mut usize,
        (index, distinct): (Vec<u32>, usize),
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Option<CompactModel>> {
        let index_cells = index.capacity().div_ceil(2);
        let Some(build_limit) = limit.checked_sub(index_cells + 3 * distinct) else {
            return Ok(None);
        };
        let mut origins = Vec::new();
        super::reserve(
            &mut origins,
            distinct,
            "unable to reserve distinct raw row IDs",
        )?;
        let mut counts =
            fallible_filled(distinct, 0usize, "unable to reserve raw row multiplicities")?;
        let mut boundary_rows = 0;
        for (i, (&id, &y)) in index.iter().zip(outcome).enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            let group = id as usize;
            if group == origins.len() {
                origins.push(i);
            }
            counts[group] += 1;
            boundary_rows += usize::from(y == 0.);
        }
        let signs = fallible_collect(
            origins.iter().map(|&i| f64::from(outcome[i] > 0.)),
            distinct,
            "unable to reserve distinct outcome classes",
        )?;
        let Some(design) = raw.pack(distinct, &origins, None, build_limit)? else {
            return Ok(None);
        };
        drop(origins);
        let mut groups = RowGroups {
            index,
            counts,
            boundary_rows,
        };
        let available = limit - index_cells - groups.counts.capacity() - signs.capacity();
        let proposal = CompactSearch::new(&signs, &design, config, Some(&groups), available)
            .run(pivots, interrupt)?;
        if matches!(proposal, Some(CompactModel::Terminal(_)))
            && !groups.verify(outcome, &signs, &design, raw, interrupt)?
        {
            return Ok(None);
        }
        if crate::diagnostics::solver_trace_enabled() {
            eprintln!("FEREG_SOLVER_TRACE|phase=compact_raw_groups|rows={}|distinct={distinct}|certified={}", outcome.len(), u8::from(proposal.is_some()));
        }
        Ok(proposal)
    }
}

/// Hashes only locate a candidate. Re-read its sparse coordinates and compare
/// them exactly before merging. Row IDs point to the first occurrence, so
/// collision order cannot change the order passed to the solver.
fn identify(
    outcome: &[f64],
    raw: &mut RawSupport,
    limit: usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<(Vec<u32>, usize, usize)>> {
    let n = outcome.len();
    let width = raw.columns;
    if width == 0
        || n < 2
        || u32::try_from(n).is_err()
        || u32::try_from(width).is_err()
        || width.saturating_mul(width) > limit
    {
        return Ok(None);
    }
    let fixed = n.div_ceil(2).saturating_add(2 * raw.terms.div_ceil(2));
    let slots = limit.saturating_sub(fixed).min(n.saturating_mul(2));
    let Some(mut work) = MAX_WORK.checked_sub(
        n.saturating_mul(raw.visits.saturating_add(1))
            .saturating_mul(2),
    ) else {
        return Ok(None);
    };
    if slots == 0 {
        return Ok(None);
    }
    let mut table = fallible_filled(
        slots,
        (0u32, u32::MAX),
        "unable to reserve raw row hash slots",
    )?;
    let mut index = fallible_filled(n, u32::MAX, "unable to reserve raw row index")?;
    let mut row = raw.scratch()?;
    let mut prior = raw.scratch()?;
    let mut distinct = 0;
    let mut ones = 0;
    for (i, &y) in outcome.iter().enumerate() {
        if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        if !raw.row(i, &mut row)? {
            return Ok(None);
        }
        let mut hasher = DefaultHasher::new();
        (y > 0.).hash(&mut hasher);
        row.hash(&mut hasher);
        let hash = hasher.finish();
        ones += row.len();
        let first = (hash % slots as u64) as usize;
        let fingerprint = (hash >> 32) as u32;
        let mut slot = first;
        loop {
            let Some(left) = work.checked_sub(1) else {
                return Ok(None);
            };
            work = left;
            let (tag, first_row) = table[slot];
            let origin = first_row as usize;
            if origin == u32::MAX as usize {
                table[slot] = (fingerprint, i as u32);
                index[i] = distinct as u32;
                distinct += 1;
                break;
            }
            if tag == fingerprint && (outcome[origin] > 0.) == (y > 0.) {
                let Some(left) = work.checked_sub(raw.visits.saturating_mul(2).saturating_add(1))
                else {
                    return Ok(None);
                };
                work = left;
                if !raw.row(origin, &mut prior)? {
                    return Ok(None);
                }
                if row == prior {
                    index[i] = index[origin];
                    break;
                }
            }
            crate::interrupt::checkpoint()?;
            slot = (slot + 1) % slots;
            if slot == first {
                return Ok(None);
            }
        }
    }
    Ok(Some((index, distinct, ones)))
}

impl RowGroups {
    /// Check every original row against the *packed design used in the exact
    /// proof*, independently of the hash and comparisons made at collection.
    /// Splitting a representative's exact mass into count equal, positive
    /// rational parts preserves its contribution to every original moment.
    fn verify(
        &mut self,
        outcome: &[f64],
        signs: &[f64],
        design: &BinaryDesign,
        raw: &mut RawSupport,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<bool> {
        let mut row = raw.scratch()?;
        for (i, (&y, &group)) in outcome.iter().zip(&self.index).enumerate() {
            if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
                interrupt()?;
            }
            let g = group as usize;
            if (y > 0.) != (signs[g] > 0.) || self.counts[g] == 0 {
                return Ok(false);
            }
            self.counts[g] -= 1;
            if !raw.row(i, &mut row)? || row != design.row(g) {
                return Ok(false);
            }
        }
        Ok(self.counts.iter().all(|&count| count == 0))
    }

    /// Replay the numerical Farkas vector on the original physical axis, with
    /// its own normalization, before the independent exact original-design proof.
    pub(super) fn farkas(
        &self,
        cone: &BoundaryCone,
        boundary: &[usize],
        weights: &[f64],
        tolerance: f64,
        available: usize,
    ) -> Result<Option<SeparationCertificate>> {
        let cells =
            self.counts.len().div_ceil(2) + self.boundary_rows.div_ceil(2) + self.boundary_rows + 1;
        if cells > available || self.boundary_rows.saturating_mul(cone.rows[0].len()) > MAX_WORK {
            return Ok(None);
        }
        let mut positions = fallible_filled(
            self.counts.len(),
            u32::MAX,
            "unable to reserve distinct boundary map",
        )?;
        for (i, &g) in boundary.iter().enumerate() {
            positions[g] = i as u32;
        }
        let mut index = Vec::new();
        let mut dual = Vec::new();
        super::reserve(
            &mut index,
            self.boundary_rows,
            "unable to reserve raw Farkas row map",
        )?;
        super::reserve(
            &mut dual,
            self.boundary_rows + 1,
            "unable to reserve raw Farkas weights",
        )?;
        let mut minimum = f64::INFINITY;
        for (i, &group) in self.index.iter().enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            let g = group as usize;
            let position = positions[g];
            if position != u32::MAX {
                let weight = weights[position as usize] / self.counts[g] as f64;
                minimum = minimum.min(weight);
                index.push(
                    cone.index
                        .get(position as usize)
                        .copied()
                        .unwrap_or(position),
                );
                dual.push(-weight);
            }
        }
        dual.push(-minimum);
        verify_indexed_farkas_certificate(&cone.rows, &index, &dual, tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_replay_checks_raw_coordinates_signs_and_every_multiplicity() {
        let design = BinaryDesign::from_rows(2, 2, 32, |_, row| {
            row.copy_from_slice(&[1., 0.]);
            Ok(())
        })
        .unwrap()
        .unwrap();
        let signs = [1., 0.];
        let outcome = [2., 0., 0., 0.];
        for (counts, changed_row, changed_class, expected) in [
            ([1, 3], None, false, true),
            ([1, 3], Some(3), false, false),
            ([1, 3], None, true, false),
            ([2, 2], None, false, false),
        ] {
            let mut groups = RowGroups {
                index: vec![0, 1, 1, 1],
                counts: counts.to_vec(),
                boundary_rows: 3,
            };
            let mut y = outcome;
            if changed_class {
                y[2] = 1.;
            }
            let ok = groups
                .verify(
                    &y,
                    &signs,
                    &design,
                    &mut RawSupport::new((2, 2, 2), &mut |i, row| {
                        row.push(0);
                        if changed_row == Some(i) {
                            row.push(1);
                        }
                        Ok(true)
                    }),
                    &mut || Ok(()),
                )
                .unwrap();
            assert_eq!(ok, expected);
        }
    }

    #[test]
    fn refusal_and_interrupt_precede_expensive_materialization() {
        let result = identify(
            &[1., 0.],
            &mut RawSupport::new((200, 2, 2), &mut |_, _| panic!("width square does not fit")),
            39_999,
            &mut || Ok(()),
        )
        .unwrap();
        assert!(result.is_none());
        let mut read = |_: usize, _: &mut Vec<u32>| panic!("recipe work exceeds the bound");
        let mut expensive = RawSupport::new((2, 1, MAX_WORK / 2), &mut read);
        assert!(identify(&[1., 0.], &mut expensive, 64, &mut || Ok(()))
            .unwrap()
            .is_none());
        assert!(expensive.pack(2, &[], None, 64).unwrap().is_none());
        let error = identify(
            &[1., 0.],
            &mut RawSupport::new((2, 2, 2), &mut |_, _| panic!("should poll first")),
            64,
            &mut || Err(crate::interrupt::interrupted()),
        )
        .unwrap_err();
        assert_eq!(error.code, crate::error::ErrorCode::Interrupted);
    }
    #[test]
    fn numerical_margin_is_checked_after_splitting_unequal_groups() {
        let cone = BoundaryCone {
            rows: vec![vec![1.], vec![-1.]],
            live_cells: 0,
            groups: vec![],
            index: vec![],
        };
        assert!(
            verify_indexed_farkas_certificate(&cone.rows, &[], &[-1., -1., -1.], 0.1)
                .unwrap()
                .is_some()
        );
        let mut index = vec![1; 21];
        index[0] = 0;
        let groups = RowGroups {
            index,
            counts: vec![1, 20],
            boundary_rows: 21,
        };
        assert!(groups
            .farkas(&cone, &[0, 1], &[1., 1.], 0.1, 100)
            .unwrap()
            .is_none());
        let cert = groups
            .farkas(&cone, &[0, 1], &[1., 1.], 0.01, 100)
            .unwrap()
            .unwrap();
        assert_eq!(cert.negative_margin_relative, 0.05);
    }
}
