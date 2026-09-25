//! Current-weight products on immutable group-pair sparsity.
//!
//! Symbolic edges and observation mappings belong to the sample/topology. Only
//! numeric blocks are refilled when the owning weighted QR generation changes.
//! Reconstruction and independent acceptance never use cached residuals.
use super::{allocation, dot, filled, Group, ReusableCertificateGeometry, Worker};
use crate::error::Result;
use crate::fe::FeDesign;
use rayon::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Representation {
    Coupling,
    Schur,
}

struct Shape {
    eliminated: usize,
    p: usize,
    q: usize,
    groups: usize,
    local_rows: usize,
    diagonal: usize,
    dense: usize,
    pairs: usize,
    max_levels: usize,
    observation_work: u128,
}

impl Shape {
    fn new(design: &FeDesign, rhs: usize) -> Result<Option<Self>> {
        let effects = design.effects();
        if rhs == 0 || effects.len() < 2 {
            return Ok(None);
        }
        let eliminated = effects
            .iter()
            .enumerate()
            .max_by_key(|(_, e)| e.n_levels().saturating_mul(e.channels()))
            .unwrap()
            .0;
        let mut p = 0usize;
        let mut q = 0usize;
        let mut groups = 0usize;
        let mut diagonal = 0usize;
        let mut channels = 0usize;
        let mut max_levels = 0usize;
        for (i, e) in effects.iter().enumerate() {
            let coefficients = e
                .n_levels()
                .checked_mul(e.channels())
                .ok_or_else(allocation)?;
            groups = groups.checked_add(e.n_levels()).ok_or_else(allocation)?;
            channels = channels.checked_add(e.channels()).ok_or_else(allocation)?;
            max_levels = max_levels.max(e.n_levels());
            if i == eliminated {
                q = coefficients;
            } else {
                p = p.checked_add(coefficients).ok_or_else(allocation)?;
                diagonal = diagonal
                    .checked_add(
                        coefficients
                            .checked_mul(e.channels())
                            .ok_or_else(allocation)?,
                    )
                    .ok_or_else(allocation)?;
            }
        }
        if p == 0 {
            return Ok(None);
        }
        let observation_work = (design.nobs() as u128)
            .checked_mul(2 * channels as u128 + 1)
            .ok_or_else(allocation)?;
        let square = p as u128 * p as u128;
        // A dense product that cannot meet the application-work margin cannot
        // become preferable after adding its construction cost. Compact work
        // has no quadratic bank and is admitted independently of this choice.
        let dense = if square <= observation_work / 5 * 4 {
            usize::try_from(square).map_err(|_| allocation())?
        } else {
            0
        };
        let local_rows = design
            .nobs()
            .checked_mul(effects.len())
            .ok_or_else(allocation)?;
        let pairs = effects
            .len()
            .checked_mul(effects.len() - 1)
            .ok_or_else(allocation)?
            / 2;
        let shape = Self {
            eliminated,
            p,
            q,
            groups,
            local_rows,
            diagonal,
            dense,
            pairs,
            max_levels,
            observation_work,
        };
        Ok(Some(shape))
    }
}

struct PairBlocks {
    left: usize,
    right: usize,
    left_channels: usize,
    right_channels: usize,
    left_start: usize,
    right_start: usize,
    offsets: Vec<usize>,
    targets: Vec<usize>,
    observation_blocks: Vec<usize>,
    values: Vec<f64>,
}

impl PairBlocks {
    fn new(
        design: &FeDesign,
        left: usize,
        right: usize,
        left_start: usize,
        right_start: usize,
        marks: &mut [usize],
        touched: &mut [usize],
    ) -> Result<Self> {
        let l = &design.effects()[left];
        let r = &design.effects()[right];
        let mut offsets = filled(l.n_levels().checked_add(1).ok_or_else(allocation)?, 0usize)?;
        marks.fill(usize::MAX);
        for group in 0..l.n_levels() {
            let mut count = 0usize;
            for &row in l.level_index().members(group) {
                let target = r.codes()[row] as usize;
                if marks[target] != group {
                    marks[target] = group;
                    count += 1;
                }
            }
            offsets[group + 1] = offsets[group].checked_add(count).ok_or_else(allocation)?;
        }
        let edges = offsets[l.n_levels()];
        let mut targets = filled(edges, 0usize)?;
        let mut observation_blocks = filled(design.nobs(), 0usize)?;
        marks.fill(usize::MAX);
        let block_width = l
            .channels()
            .checked_mul(r.channels())
            .ok_or_else(allocation)?;
        for group in 0..l.n_levels() {
            let mut count = 0;
            for &row in l.level_index().members(group) {
                let target = r.codes()[row] as usize;
                if marks[target] != group {
                    marks[target] = group;
                    touched[count] = target;
                    count += 1;
                }
            }
            touched[..count].sort_unstable();
            for (at, &target) in touched[..count].iter().enumerate() {
                targets[offsets[group] + at] = target;
            }
            // The group members already have deterministic observation order.
            // Binary search is paid only here, never in a changing-weight fill.
            let neighbours = &targets[offsets[group]..offsets[group + 1]];
            for &row in l.level_index().members(group) {
                let target = r.codes()[row] as usize;
                let edge = offsets[group]
                    + neighbours
                        .binary_search(&target)
                        .expect("symbolic neighbour");
                observation_blocks[row] = edge.checked_mul(block_width).ok_or_else(allocation)?;
            }
        }
        Ok(Self {
            left,
            right,
            left_channels: l.channels(),
            right_channels: r.channels(),
            left_start,
            right_start,
            offsets,
            targets,
            observation_blocks,
            values: Vec::new(),
        })
    }
    fn numeric_len(&self) -> Result<usize> {
        self.targets
            .len()
            .checked_mul(self.left_channels)
            .and_then(|n| n.checked_mul(self.right_channels))
            .ok_or_else(allocation)
    }
    fn elements(&self) -> usize {
        self.offsets.capacity()
            + self.targets.capacity()
            + self.observation_blocks.capacity()
            + self.values.capacity()
    }
}

pub(super) struct PreparedCoupling {
    eliminated: usize,
    dimension: usize,
    starts: Vec<usize>,
    diagonal_starts: Vec<usize>,
    effect_groups: Vec<usize>,
    local_rows: Vec<usize>,
    pairs: Vec<PairBlocks>,
    coupling_pairs: usize,
    diagonal: Vec<f64>,
    schur: Vec<f64>,
    row_values: Vec<f64>,
    row_columns: Vec<usize>,
    representation: Representation,
    curvature_error_scale: f64,
    #[cfg(test)]
    pub(super) preparations: usize,
}

impl PreparedCoupling {
    pub(super) fn bytes(design: &FeDesign, rhs: usize) -> Result<Option<u128>> {
        let Some(s) = Shape::new(design, rhs)? else {
            return Ok(None);
        };
        let mut indices = 2 * s.groups as u128
            + design.effects().len() as u128
            + s.local_rows as u128
            + 2 * s.max_levels as u128;
        let mut numbers = s.diagonal as u128;
        if s.dense != 0 {
            indices += s.p as u128;
            numbers += s.dense as u128 + s.p as u128;
        }
        for left in 0..design.effects().len() {
            for right in left + 1..design.effects().len() {
                let (left, right) = if right == s.eliminated {
                    (right, left)
                } else {
                    (left, right)
                };
                let l = &design.effects()[left];
                let r = &design.effects()[right];
                let edges =
                    (design.nobs() as u128).min(l.n_levels() as u128 * r.n_levels() as u128);
                let block = l
                    .channels()
                    .checked_mul(r.channels())
                    .ok_or_else(allocation)?;
                let numeric = usize::try_from(edges)
                    .ok()
                    .and_then(|e| e.checked_mul(block))
                    .ok_or_else(allocation)?;
                indices += l.n_levels() as u128 + 1 + edges + design.nobs() as u128;
                numbers += numeric as u128;
            }
        }
        Ok(Some(
            std::mem::size_of::<Self>() as u128
                + s.pairs as u128 * std::mem::size_of::<PairBlocks>() as u128
                + indices * std::mem::size_of::<usize>() as u128
                + numbers * 8,
        ))
    }

    pub(super) fn new(design: &FeDesign, groups: &[Group], rhs: usize) -> Result<Option<Self>> {
        let Some(s) = Shape::new(design, rhs)? else {
            return Ok(None);
        };
        let mut starts = filled(s.groups, 0usize)?;
        let mut diagonal_starts = filled(s.groups, 0usize)?;
        let mut effect_groups = filled(design.effects().len(), 0usize)?;
        let mut local_rows = filled(s.local_rows, 0usize)?;
        let mut position = 0;
        let mut diagonal = 0;
        for (i, g) in groups.iter().enumerate() {
            if g.level == 0 {
                effect_groups[g.effect] = i;
            }
            if g.effect != s.eliminated {
                starts[i] = position;
                diagonal_starts[i] = diagonal;
                diagonal += g.channels * g.channels;
                position += g.channels;
            }
            for (local, &row) in design.effects()[g.effect]
                .level_index()
                .members(g.level)
                .iter()
                .enumerate()
            {
                local_rows[g.effect * design.nobs() + row] = local;
            }
        }
        let mut pairs = Vec::new();
        pairs.try_reserve_exact(s.pairs).map_err(|_| allocation())?;
        if pairs.capacity() > s.pairs {
            return Err(allocation());
        }
        let mut marks = filled(s.max_levels, usize::MAX)?;
        let mut touched = filled(s.max_levels, 0usize)?;
        for right in 0..design.effects().len() {
            if right == s.eliminated {
                continue;
            }
            pairs.push(PairBlocks::new(
                design,
                s.eliminated,
                right,
                0,
                starts[effect_groups[right]],
                &mut marks,
                &mut touched,
            )?);
        }
        let coupling_pairs = pairs.len();
        for left in 0..design.effects().len() {
            if left == s.eliminated {
                continue;
            }
            for right in left + 1..design.effects().len() {
                if right == s.eliminated {
                    continue;
                }
                pairs.push(PairBlocks::new(
                    design,
                    left,
                    right,
                    starts[effect_groups[left]],
                    starts[effect_groups[right]],
                    &mut marks,
                    &mut touched,
                )?);
            }
        }
        let compact_work = pairs.iter().try_fold(s.diagonal as u128, |sum, pair| {
            pair.numeric_len().map(|n| sum + 2 * n as u128)
        })?;
        let mut schur_products = 0u128;
        for group in 0..design.effects()[s.eliminated].n_levels() {
            let width: usize = pairs[..coupling_pairs]
                .iter()
                .map(|pair| (pair.offsets[group + 1] - pair.offsets[group]) * pair.right_channels)
                .sum();
            schur_products += design.effects()[s.eliminated].channels() as u128
                * width as u128
                * (width as u128 + 1)
                / 2;
        }
        let schur_work = if s.dense == 0 {
            u128::MAX
        } else {
            s.dense as u128 + schur_products / rhs as u128
        };
        if compact_work.min(schur_work) > s.observation_work / 5 * 4 {
            return Ok(None);
        }
        let representation = if schur_work < compact_work {
            Representation::Schur
        } else {
            Representation::Coupling
        };
        for pair in &mut pairs {
            pair.values = filled(pair.numeric_len()?, 0.)?;
        }
        let dense = if representation == Representation::Schur {
            s.dense
        } else {
            0
        };
        let row = if dense == 0 { 0 } else { s.p };
        Ok(Some(Self {
            eliminated: s.eliminated,
            dimension: s.p,
            starts,
            diagonal_starts,
            effect_groups,
            local_rows,
            pairs,
            coupling_pairs,
            diagonal: filled(s.diagonal, 0.)?,
            schur: filled(dense, 0.)?,
            row_values: filled(row, 0.)?,
            row_columns: filled(row, 0)?,
            representation,
            curvature_error_scale: 64.
                * f64::EPSILON
                * (design.nobs() as f64 + s.q as f64 + s.p as f64)
                * design.effects().len() as f64,
            #[cfg(test)]
            preparations: 0,
        }))
    }

    pub(super) fn elements(&self) -> usize {
        self.starts.capacity()
            + self.diagonal_starts.capacity()
            + self.effect_groups.capacity()
            + self.local_rows.capacity()
            + self.diagonal.capacity()
            + self.schur.capacity()
            + self.row_values.capacity()
            + self.row_columns.capacity()
            + self.pairs.iter().map(PairBlocks::elements).sum::<usize>()
    }

    #[cfg(test)]
    pub(super) fn allocated_bytes(&self) -> u128 {
        let indices = self.starts.capacity()
            + self.diagonal_starts.capacity()
            + self.effect_groups.capacity()
            + self.local_rows.capacity()
            + self.row_columns.capacity()
            + self
                .pairs
                .iter()
                .map(|p| {
                    p.offsets.capacity() + p.targets.capacity() + p.observation_blocks.capacity()
                })
                .sum::<usize>();
        let numbers = self.diagonal.capacity()
            + self.schur.capacity()
            + self.row_values.capacity()
            + self
                .pairs
                .iter()
                .map(|p| p.values.capacity())
                .sum::<usize>();
        std::mem::size_of::<Self>() as u128
            + self.pairs.capacity() as u128 * std::mem::size_of::<PairBlocks>() as u128
            + indices as u128 * std::mem::size_of::<usize>() as u128
            + numbers as u128 * 8
    }

    pub(super) fn prepare(
        &mut self,
        design: &FeDesign,
        geometry: &ReusableCertificateGeometry,
        groups: &[Group],
    ) {
        #[cfg(test)]
        {
            self.preparations += 1;
        }
        let n = design.nobs();
        self.diagonal.fill(0.);
        for (index, g) in groups.iter().enumerate() {
            if g.effect == self.eliminated {
                continue;
            }
            let rows = design.effects()[g.effect]
                .level_index()
                .members(g.level)
                .len();
            let start = self.diagonal_starts[index];
            for a in 0..geometry.ranks[index] {
                let left = &geometry.basis[g.basis_start + a * rows..][..rows];
                for b in a..geometry.ranks[index] {
                    let right = &geometry.basis[g.basis_start + b * rows..][..rows];
                    let value = dot(left, right);
                    self.diagonal[start + a * g.channels + b] = value;
                    self.diagonal[start + b * g.channels + a] = value;
                }
            }
        }
        let effect_groups = &self.effect_groups;
        let local_rows = &self.local_rows;
        self.pairs.par_iter_mut().for_each(|pair| {
            pair.values.fill(0.);
            let left_effect = &design.effects()[pair.left];
            let right_effect = &design.effects()[pair.right];
            for level in 0..left_effect.n_levels() {
                let lg = effect_groups[pair.left] + level;
                let l = groups[lg];
                let members = left_effect.level_index().members(level);
                let ls = members.len();
                for (local, &row) in members.iter().enumerate() {
                    let rg = effect_groups[pair.right] + right_effect.codes()[row] as usize;
                    let r = groups[rg];
                    let rs = right_effect.level_index().members(r.level).len();
                    let lb = l.basis_start + local;
                    let rb = r.basis_start + local_rows[pair.right * n + row];
                    let target = pair.observation_blocks[row];
                    for a in 0..geometry.ranks[lg] {
                        let left = geometry.basis[lb + a * ls];
                        for b in 0..geometry.ranks[rg] {
                            let at = target + a * pair.right_channels + b;
                            pair.values[at] =
                                left.mul_add(geometry.basis[rb + b * rs], pair.values[at]);
                        }
                    }
                }
            }
        });
        if self.representation == Representation::Schur {
            self.form_schur(design, groups);
        }
    }

    fn form_schur(&mut self, design: &FeDesign, groups: &[Group]) {
        let p = self.dimension;
        self.schur.fill(0.);
        for (index, g) in groups.iter().enumerate() {
            if g.effect == self.eliminated {
                continue;
            }
            let start = self.starts[index];
            for a in 0..g.channels {
                for b in 0..g.channels {
                    self.schur[(start + a) * p + start + b] =
                        self.diagonal[self.diagonal_starts[index] + a * g.channels + b];
                }
            }
        }
        for pair in &self.pairs[self.coupling_pairs..] {
            let lc = pair.left_channels;
            let rc = pair.right_channels;
            for group in 0..pair.offsets.len() - 1 {
                let left = pair.left_start + group * lc;
                for edge in pair.offsets[group]..pair.offsets[group + 1] {
                    let right = pair.right_start + pair.targets[edge] * rc;
                    for a in 0..lc {
                        for b in 0..rc {
                            let v = pair.values[edge * lc * rc + a * rc + b];
                            self.schur[(left + a) * p + right + b] = v;
                            self.schur[(right + b) * p + left + a] = v;
                        }
                    }
                }
            }
        }
        let effect = &design.effects()[self.eliminated];
        for group in 0..effect.n_levels() {
            for basis in 0..effect.channels() {
                let mut count = 0;
                for pair in &self.pairs[..self.coupling_pairs] {
                    let lc = pair.left_channels;
                    let rc = pair.right_channels;
                    for edge in pair.offsets[group]..pair.offsets[group + 1] {
                        let right = pair.right_start + pair.targets[edge] * rc;
                        for b in 0..rc {
                            self.row_columns[count] = right + b;
                            self.row_values[count] = pair.values[edge * lc * rc + basis * rc + b];
                            count += 1;
                        }
                    }
                }
                for i in 0..count {
                    for j in i..count {
                        let at = self.row_columns[i] * p + self.row_columns[j];
                        self.schur[at] =
                            (-self.row_values[i]).mul_add(self.row_values[j], self.schur[at]);
                    }
                }
            }
        }
        for i in 0..p {
            for j in i + 1..p {
                self.schur[j * p + i] = self.schur[i * p + j];
            }
        }
    }

    pub(super) fn apply(&self, input: &[f64], output: &mut [f64], work: &mut [f64]) -> Option<f64> {
        let p = self.dimension;
        let x = input;
        let y = output;
        let c = work;
        if self.representation == Representation::Schur {
            let whole = p / 4 * 4;
            for start in (0..whole).step_by(4) {
                let mut sums = [0.; 4];
                for (j, &v) in x.iter().enumerate() {
                    sums[0] = self.schur[start * p + j].mul_add(v, sums[0]);
                    sums[1] = self.schur[(start + 1) * p + j].mul_add(v, sums[1]);
                    sums[2] = self.schur[(start + 2) * p + j].mul_add(v, sums[2]);
                    sums[3] = self.schur[(start + 3) * p + j].mul_add(v, sums[3]);
                }
                y[start..start + 4].copy_from_slice(&sums);
            }
            for i in whole..p {
                y[i] = dot(&self.schur[i * p..(i + 1) * p], x);
            }
        } else {
            self.compact_base(x, y);
            self.subtract_eliminated(x, y, c, 0..self.pairs[0].offsets.len() - 1);
        }
        let curvature = dot(x, y);
        if !curvature.is_finite() || curvature <= self.curvature_error_scale * dot(x, x) {
            return None;
        }
        Some(curvature)
    }
    #[inline(always)]
    fn compact_base(&self, x: &[f64], y: &mut [f64]) {
        // Local blocks include the measured QR Gram, including rank-zero
        // coordinates; no exact-identity or full-local-rank assumption.
        let mut coordinate = 0;
        let mut value = 0;
        for effect in 0..self.effect_groups.len() {
            if effect == self.eliminated {
                continue;
            }
            let pair = self.pairs[..self.coupling_pairs]
                .iter()
                .find(|pair| pair.right == effect)
                .expect("remaining partition");
            let width = pair.right_channels;
            let levels = if effect + 1 < self.effect_groups.len() {
                self.effect_groups[effect + 1] - self.effect_groups[effect]
            } else {
                self.starts.len() - self.effect_groups[effect]
            };
            for _ in 0..levels {
                for a in 0..width {
                    y[coordinate + a] = dot(
                        &self.diagonal[value + a * width..value + (a + 1) * width],
                        &x[coordinate..coordinate + width],
                    );
                }
                coordinate += width;
                value += width * width;
            }
        }
        for pair in &self.pairs[self.coupling_pairs..] {
            let lc = pair.left_channels;
            let rc = pair.right_channels;
            for group in 0..pair.offsets.len() - 1 {
                let left = pair.left_start + group * lc;
                for edge in pair.offsets[group]..pair.offsets[group + 1] {
                    let right = pair.right_start + pair.targets[edge] * rc;
                    for a in 0..lc {
                        for b in 0..rc {
                            let v = pair.values[edge * lc * rc + a * rc + b];
                            y[left + a] = v.mul_add(x[right + b], y[left + a]);
                            y[right + b] = v.mul_add(x[left + a], y[right + b]);
                        }
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn subtract_eliminated(
        &self,
        x: &[f64],
        y: &mut [f64],
        c: &mut [f64],
        groups: std::ops::Range<usize>,
    ) {
        // Finish C and C' for one eliminated group while its block values
        // are hot. Each invocation writes only its own output and scratch.
        let lc = self.pairs[0].left_channels;
        for group in groups {
            let local = &mut c[..lc];
            local.fill(0.);
            for pair in &self.pairs[..self.coupling_pairs] {
                let rc = pair.right_channels;
                for edge in pair.offsets[group]..pair.offsets[group + 1] {
                    let right = pair.right_start + pair.targets[edge] * rc;
                    for a in 0..lc {
                        for b in 0..rc {
                            local[a] = pair.values[edge * lc * rc + a * rc + b]
                                .mul_add(x[right + b], local[a]);
                        }
                    }
                }
            }
            for pair in &self.pairs[..self.coupling_pairs] {
                let rc = pair.right_channels;
                for edge in pair.offsets[group]..pair.offsets[group + 1] {
                    let right = pair.right_start + pair.targets[edge] * rc;
                    for a in 0..lc {
                        for b in 0..rc {
                            y[right + b] = (-pair.values[edge * lc * rc + a * rc + b])
                                .mul_add(local[a], y[right + b]);
                        }
                    }
                }
            }
        }
    }

    /// Ordinary joint solving has one RHS; the other column workers are idle.
    /// Each owns a partial product. Merge in worker order before the unchanged
    /// curvature guard; only original-row reconstruction can accept a solve.
    pub(super) fn apply_joint(
        &self,
        input: &[f64],
        output: &mut [f64],
        scratch: &mut [f64],
        helpers: &mut [Worker],
    ) -> Option<f64> {
        let groups = self.pairs[0].offsets.len() - 1;
        let width = (helpers.len() + 1).min(groups);
        if self.representation == Representation::Schur || width < 2 {
            return self.apply(input, output, scratch);
        }
        let helpers = &mut helpers[..width - 1];
        let chunk = groups / width;
        let extra = groups % width;
        rayon::join(
            || {
                self.compact_base(input, output);
                self.subtract_eliminated(input, output, scratch, 0..chunk + usize::from(extra > 0));
            },
            || helpers.par_iter_mut().enumerate().for_each(|(index, worker)| {
                worker.product.fill(0.);
                let first = chunk * (index + 1) + extra.min(index + 1);
                let last = chunk * (index + 2) + extra.min(index + 2);
                self.subtract_eliminated(
                    input, &mut worker.product, &mut worker.eliminated_scratch, first..last,
                );
            }),
        );
        for worker in helpers {
            for (value, partial) in output.iter_mut().zip(&worker.product) {
                *value += partial;
            }
        }
        let curvature = dot(input, output);
        if !curvature.is_finite() || curvature <= self.curvature_error_scale * dot(input, input) {
            return None;
        }
        Some(curvature)
    }

    #[cfg(test)]
    pub(super) fn force_compact_for_test(&mut self) {
        self.representation = Representation::Coupling;
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn symbolic_edges_map_observations_to_sorted_group_pair_blocks() {
        let left = vec![0, 0, 0, 1, 1, 2, 2, 2];
        let right = vec![1, 0, 1, 1, 0, 1, 1, 0];
        let design = FeDesign::new(
            8,
            vec![
                EffectTerm::new(left, 3, true, vec![vec![0.; 8]]).unwrap(),
                EffectTerm::new(right, 2, true, vec![vec![0.; 8], vec![0.; 8]]).unwrap(),
            ],
        )
        .unwrap();
        let mut marks = vec![usize::MAX; 3];
        let mut touched = vec![0; 3];
        let pair = PairBlocks::new(&design, 0, 1, 0, 0, &mut marks, &mut touched).unwrap();
        assert_eq!(pair.offsets, vec![0, 2, 4, 6]);
        assert_eq!(pair.targets, vec![0, 1, 0, 1, 0, 1]);
        assert_eq!(pair.observation_blocks, vec![6, 0, 6, 18, 12, 30, 30, 24]);
        assert_eq!(pair.numeric_len().unwrap(), 36);
        // Symbolic preparation carries no numeric-weight authority.
        assert!(pair.values.is_empty());
        for row in 0..8 {
            let edge = pair.observation_blocks[row] / 6;
            let group = design.effects()[0].codes()[row] as usize;
            assert!(edge >= pair.offsets[group] && edge < pair.offsets[group + 1]);
            assert_eq!(
                pair.targets[edge],
                design.effects()[1].codes()[row] as usize
            );
        }
    }
}
