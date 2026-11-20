//! Repeated raw rows give identical primal inequalities within an outcome class.
//! A strict dual on representatives lifts by dividing each mass among its copies.
//! This can use different divisors on different rows, including positive rows.
use super::{
    fallible_collect, fallible_filled, projection::BoundaryCone, BinaryDesign, CompactModel,
    CompactSearch, SeparationCertificate, MAX_WORK,
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

    /// Reading the recipes and visiting their sparse outputs takes one
    /// bounded pass even when the model has many physical observations.
    fn scan_work(&self, rows: usize, passes: usize) -> Option<usize> {
        self.visits
            .checked_add(self.terms)?
            .checked_add(1)?
            .checked_mul(rows)?
            .checked_mul(passes)
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
        if u32::try_from(self.columns).is_err() || n as u128 > (1u128 << 53) {
            return Ok(None);
        }
        if fixed > limit {
            return declined("pack", "memory");
        }
        if self
            .scan_work(n, 1 + usize::from(known_ones.is_none()))
            .is_none()
        {
            return declined("pack", "work_overflow");
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
                    return declined("pack", "memory");
                }
            }
            total
        };
        if ones > max_ones {
            return declined("pack", "memory");
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
    raw_pass: usize,
}

pub(crate) enum CompactInput {
    Design(BinaryDesign),
    Proof(CompactModel),
}

/// The u32 physical index can share the initial compact bank or, on a
/// concrete shortage, occupy separately admitted sparse scratch. Even after
/// that extension all representative algebra stays under its original limit.
struct IndexWorkspace<'a> {
    live: usize,
    ceiling: usize,
    admit: &'a mut dyn FnMut(usize) -> Option<usize>,
}

impl IndexWorkspace<'_> {
    fn require(&mut self, required: usize) -> bool {
        if required <= self.live {
            return true;
        }
        if required > self.ceiling {
            return false;
        }
        let Some(grant) = (self.admit)(required).filter(|&grant| grant >= required) else {
            return false;
        };
        self.live = grant.min(self.ceiling);
        true
    }
}

impl BinaryDesign {
    pub(crate) fn prepare(
        outcome: &[f64],
        shape: (usize, usize, usize),
        (limit, admit): (usize, &mut dyn FnMut(usize) -> Option<usize>),
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
            // Only the physical u32 index may extend the original compact
            // allowance. Hashing, representatives and all tableau/algebra
            // scratch still share that original allowance.
            let held = n.div_ceil(2);
            let mut space = IndexWorkspace {
                live: limit,
                ceiling: limit.saturating_add(held),
                admit,
            };
            let mut collision_work = MAX_WORK;
            // An extension may restart the scan on a larger hash table.
            // All attempts share four linear row passes plus one fixed
            // allowance for unsuccessful collision probes.
            let mut rows_left = n.saturating_mul(4);
            let groups = loop {
                let mut shortfall = None;
                let groups = identify(
                    outcome,
                    &mut raw,
                    space.live,
                    (&mut collision_work, &mut rows_left),
                    &mut shortfall,
                    interrupt,
                )?;
                if !shortfall.is_some_and(|need| space.require(need)) {
                    break groups;
                }
            };
            if let Some(IdentifiedRows {
                index,
                distinct,
                ones,
                unique_ones,
            }) = groups
            {
                if distinct < n {
                    if let Some(proof) = Self::repeated_model(
                        outcome,
                        &mut raw,
                        &mut space,
                        config,
                        pivots,
                        (index, distinct, unique_ones),
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
        space: &mut IndexWorkspace<'_>,
        config: SeparationConfig,
        pivots: &mut usize,
        (index, distinct, unique_ones): (Vec<u32>, usize, usize),
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Option<CompactModel>> {
        let held = index.capacity().div_ceil(2);
        let bookkeeping = held.saturating_add(3usize.saturating_mul(distinct));
        let packed = distinct
            .saturating_add(1)
            .saturating_add(raw.terms.div_ceil(2))
            .saturating_add(unique_ones.div_ceil(2));
        if !space.require(bookkeeping.saturating_add(packed)) {
            return declined("representatives", "memory");
        }
        let build_limit = space.live - bookkeeping;
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
        let Some(design) = raw.pack(distinct, &origins, Some(unique_ones), build_limit)? else {
            return Ok(None);
        };
        drop(origins);
        let mut groups = RowGroups {
            index,
            counts,
            boundary_rows,
            // Identification admitted two passes over these same physical rows.
            raw_pass: raw
                .scan_work(outcome.len(), 1)
                .expect("admitted raw row span"),
        };
        let fixed = held + groups.counts.capacity() + signs.capacity();
        let available = space.live - fixed;
        let mut search = CompactSearch::new(&signs, &design, config, Some(&groups), available);
        let proposal = search.run(pivots, interrupt, &mut |required| {
            let total = fixed
                .saturating_add(design.cells())
                .saturating_add(required);
            space
                .require(total)
                .then(|| space.live - fixed - design.cells())
        })?;
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

fn declined<T>(stage: &'static str, reason: &'static str) -> Result<Option<T>> {
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!("FEREG_SOLVER_TRACE|phase=compact_raw_refusal|stage={stage}|reason={reason}");
    }
    Ok(None)
}

#[derive(Debug)]
struct IdentifiedRows {
    index: Vec<u32>,
    distinct: usize,
    ones: usize,
    unique_ones: usize,
}

/// Hashes only locate a candidate. Re-read its sparse coordinates and compare
/// them exactly before merging. Row IDs point to the first occurrence, so
/// collision order cannot change the order passed to the solver.
fn identify(
    outcome: &[f64],
    raw: &mut RawSupport,
    limit: usize,
    (collision_work, rows_left): (&mut usize, &mut usize),
    index_shortfall: &mut Option<usize>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<IdentifiedRows>> {
    let n = outcome.len();
    let width = raw.columns;
    if width == 0 || n < 2 || u32::try_from(n).is_err() || u32::try_from(width).is_err() {
        return Ok(None);
    }
    let fixed = n
        .div_ceil(2)
        .saturating_add(raw.terms.div_ceil(2).saturating_mul(2));
    let slots = limit.saturating_sub(fixed).min(n.saturating_mul(2));
    if width.saturating_mul(width) > limit {
        return declined("identify", "memory");
    }
    // Each physical row may be read and hashed once, and compared with one
    // exact representative. Only unsuccessful probes spend exploratory work.
    if raw.scan_work(n, 2).is_none() {
        return declined("identify", "work_overflow");
    }
    if slots == 0 {
        *index_shortfall = fixed.checked_add(1);
        return declined("identify", "memory");
    }
    let comparison = raw.visits + raw.terms + 1;
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
    let mut unique_ones = 0;
    for (i, &y) in outcome.iter().enumerate() {
        let Some(left) = rows_left.checked_sub(1) else {
            return declined("identify", "scan_work");
        };
        *rows_left = left;
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
            let (tag, first_row) = table[slot];
            let origin = first_row as usize;
            if origin == u32::MAX as usize {
                table[slot] = (fingerprint, i as u32);
                index[i] = distinct as u32;
                distinct += 1;
                unique_ones += row.len();
                break;
            }
            let mut collision = 1;
            if tag == fingerprint && (outcome[origin] > 0.) == (y > 0.) {
                if !raw.row(origin, &mut prior)? {
                    return Ok(None);
                }
                if row == prior {
                    index[i] = index[origin];
                    break;
                }
                collision += comparison;
            }
            // A false full-key comparison uses the row's linear credit until
            // it is charged here. Never read another candidate without paying
            // for the miss; even a refused search stays within those bounds.
            let Some(left) = collision_work.checked_sub(collision) else {
                return declined("identify", "collision_work");
            };
            *collision_work = left;
            crate::interrupt::checkpoint()?;
            slot = (slot + 1) % slots;
            if slot == first {
                *index_shortfall = fixed
                    .checked_add(slots)
                    .and_then(|used| used.checked_add(1));
                return declined("identify", "memory");
            }
        }
    }
    Ok(Some(IdentifiedRows {
        index,
        distinct,
        ones,
        unique_ones,
    }))
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
        let nullity = cone.rows.first().map_or(0, Vec::len);
        if nullity == 0
            || boundary.len() != weights.len()
            || !cone.index.is_empty() && cone.index.len() != boundary.len()
        {
            return Ok(None);
        }
        let cells = self.counts.len().div_ceil(2).saturating_add(nullity);
        if cells > available {
            return declined("physical_farkas", "memory");
        }
        // Physical order is part of this numerical replay. Allow one scan of
        // raw recipe and term data; a growing projected dimension beyond that
        // linear floor still has its own fixed work limit.
        if self
            .boundary_rows
            .checked_mul(nullity)
            .is_none_or(|work| work > MAX_WORK.max(self.raw_pass))
        {
            return declined("physical_farkas", "projected_work");
        }
        let mut positions = fallible_filled(
            self.counts.len(),
            u32::MAX,
            "unable to reserve distinct boundary map",
        )?;
        for (i, &g) in boundary.iter().enumerate() {
            positions[g] = i as u32;
        }
        let mut moments =
            fallible_filled(nullity, 0_f64, "unable to reserve physical Farkas moments")?;
        let mut minimum = f64::INFINITY;
        let mut maximum = 0_f64;
        let mut count = 0;
        for (i, &group) in self.index.iter().enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            let g = group as usize;
            let position = positions[g];
            if position != u32::MAX {
                let pos = position as usize;
                let weight = weights[pos] / self.counts[g] as f64;
                if !weight.is_finite() || weight <= 0. {
                    return Ok(None);
                }
                minimum = minimum.min(weight);
                maximum = maximum.max(weight);
                count += 1;
                let projected = cone.index.get(pos).map_or(pos, |&id| id as usize);
                let Some(row) = cone.rows.get(projected).filter(|r| r.len() == nullity) else {
                    return Ok(None);
                };
                for (value, &coordinate) in moments.iter_mut().zip(row) {
                    *value = coordinate.mul_add(-weight, *value);
                }
            }
        }
        if count != self.boundary_rows
            || maximum <= f64::MIN_POSITIVE
            || moments.iter().any(|x| !x.is_finite())
        {
            return Ok(None);
        }
        let residual = moments.iter().map(|x| x.abs()).fold(0., f64::max);
        // Every physical inequality has weight at least its observed minimum.
        // The additions for each moment follow exactly the physical row order.
        Ok(super::super::farkas_residual_certificate(
            count, nullity, maximum, -minimum, residual, 0., tolerance,
        ))
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
                raw_pass: 0,
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
            (&mut { MAX_WORK }, &mut { usize::MAX }),
            &mut None,
            &mut || Ok(()),
        )
        .unwrap();
        assert!(result.is_none());
        let mut read = |_: usize, _: &mut Vec<u32>| panic!("recipe work exceeds the bound");
        let mut expensive = RawSupport::new((2, 1, usize::MAX / 2), &mut read);
        assert!(identify(
            &[1., 0.],
            &mut expensive,
            64,
            (&mut { MAX_WORK }, &mut { usize::MAX }),
            &mut None,
            &mut || Ok(())
        )
        .unwrap()
        .is_none());
        assert!(expensive.pack(2, &[], None, 64).unwrap().is_none());
        let error = identify(
            &[1., 0.],
            &mut RawSupport::new((2, 2, 2), &mut |_, _| panic!("should poll first")),
            64,
            (&mut { MAX_WORK }, &mut { usize::MAX }),
            &mut None,
            &mut || Err(crate::interrupt::interrupted()),
        )
        .unwrap_err();
        assert_eq!(error.code, crate::error::ErrorCode::Interrupted);
    }

    #[test]
    fn physical_reads_and_confirmed_matches_are_linear_even_without_collision_credit() {
        let y = [0.; 4];
        let mut calls = 0;
        let mut read = |_: usize, row: &mut Vec<u32>| {
            calls += 1;
            row.push(0);
            Ok(true)
        };
        let mut raw = RawSupport::new((2, 1, MAX_WORK), &mut read);
        let IdentifiedRows {
            index,
            distinct,
            ones,
            unique_ones,
        } = identify(
            &y,
            &mut raw,
            64,
            (&mut 0, &mut { usize::MAX }),
            &mut None,
            &mut || Ok(()),
        )
        .unwrap()
        .unwrap();
        assert_eq!((distinct, ones, unique_ones), (1, 4, 1));
        let design = raw.pack(1, &[0], None, 64).unwrap().unwrap();
        let mut groups = RowGroups {
            index,
            counts: vec![4],
            boundary_rows: 4,
            raw_pass: raw.scan_work(4, 1).unwrap(),
        };
        assert!(groups
            .verify(&y, &[0.], &design, &mut raw, &mut || Ok(()))
            .unwrap());
        assert_eq!(calls, 4 + 3 + 2 + 4);
    }

    #[test]
    fn each_unmatched_hash_slot_spends_the_fixed_exploratory_allowance() {
        // Two distinct supports sharing the first slot of a four-slot table.
        let mut slots: [Option<Vec<u32>>; 4] = Default::default();
        let (a, b) = (0..16)
            .find_map(|bits| {
                let row: Vec<u32> = (0..4).filter(|j| bits & (1 << j) != 0).collect();
                let mut hash = DefaultHasher::new();
                true.hash(&mut hash);
                row.hash(&mut hash);
                let s = (hash.finish() % 4) as usize;
                slots[s].replace(row.clone()).map(|first| (first, row))
            })
            .unwrap();
        for (work, admitted) in [(0, false), (1, true)] {
            let mut read = |i: usize, row: &mut Vec<u32>| {
                row.extend_from_slice(if i == 0 { &a } else { &b });
                Ok(true)
            };
            let mut raw = RawSupport::new((4, 4, 4), &mut read);
            assert_eq!(
                identify(
                    &[1.; 2],
                    &mut raw,
                    64,
                    (&mut { work }, &mut { usize::MAX }),
                    &mut None,
                    &mut || Ok(())
                )
                .unwrap()
                .is_some(),
                admitted
            );
        }
    }
    #[test]
    fn numerical_margin_is_checked_after_splitting_unequal_groups() {
        let cone = BoundaryCone {
            rows: vec![vec![1.], vec![-1.]],
            live_cells: 0,
            groups: vec![],
            index: vec![],
        };
        assert!(super::super::verify_indexed_farkas_certificate(
            &cone.rows,
            &[],
            &[-1., -1., -1.],
            0.1
        )
        .unwrap()
        .is_some());
        let mut index = vec![1; 21];
        index[0] = 0;
        let groups = RowGroups {
            index,
            counts: vec![1, 20],
            boundary_rows: 21,
            raw_pass: 0,
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
    #[test]
    fn streamed_original_farkas_preserves_expanded_numerical_certificate() {
        let cone = BoundaryCone {
            rows: vec![vec![-0.3, -0.03], vec![1., 0.1]],
            groups: Vec::new(),
            index: vec![1, 0],
            live_cells: 0,
        };
        let index: Vec<u32> = (0..15).map(|i| u32::from(i % 4 == 0)).collect();
        let groups = RowGroups {
            index: index.clone(),
            counts: vec![11, 4],
            boundary_rows: 15,
            raw_pass: 0,
        };
        let weights = [3., 10.];
        let mut expanded: Vec<f64> = index
            .iter()
            .map(|&i| -weights[i as usize] / groups.counts[i as usize] as f64)
            .collect();
        expanded.push(-3. / 11.);
        let projected: Vec<_> = index.iter().map(|&i| 1 - i).collect();
        let expected = super::super::verify_indexed_farkas_certificate(
            &cone.rows, &projected, &expanded, 1e-8,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            groups.farkas(&cone, &[0, 1], &weights, 1e-8, 3).unwrap(),
            Some(expected)
        );
    }
}
