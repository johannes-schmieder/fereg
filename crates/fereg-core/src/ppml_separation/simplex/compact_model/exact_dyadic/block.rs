//! The exact dual by level blocks, for designs with one high-cardinality
//! absorbed slope effect.
//!
//! The general exact dual solves one square system over every pivot
//! coordinate, so its determinant and Cramer numerators grow with the rank
//! and the Chinese remaindering costs about rank x primes^2 limb operations.
//! Here the coordinates of each level of the block effect (its intercept and
//! slopes) are solved by their own small system: the boundary rows outside
//! a level's basis get weights that are multiples of that level's
//! determinant, `d_i = det_l * kappa_i` with `kappa_i ~ C w_i / det_l`, so the
//! level's basis weights `adj(B_l') sum kappa_i y_i` are integers of the same
//! size as `C w_i` and the level moments vanish exactly. The remaining
//! (border) coordinates — finite columns and the other effects — are then
//! balanced by a small border system over adjustments of a few rows, each
//! carried through its level's basis. Numbers stay at a few hundred bits per
//! level plus the border determinant; nothing grows with the level count.
//! The caller's exact check accepts or refuses the result.
use super::super::{fallible_filled, reserve};
use super::modular::{crt_solve, primes, Budget, Elimination, Factor, Field, System};
use super::{dyadic, log_norm, read_terms, residue, Big, ExactDual, Reader, Term};
use crate::error::Result;

/// The block effect's coordinates: level `l`, channel `c` is raw coordinate
/// `base + channels * l + c`; `codes[row]` is the row's level.
#[derive(Clone, Copy)]
pub(crate) struct Blocks<'a> {
    pub(crate) base: usize,
    pub(crate) channels: usize,
    pub(crate) levels: usize,
    pub(crate) codes: &'a [u32],
}

impl Blocks<'_> {
    fn channel(&self, column: usize) -> Option<u32> {
        let end = self.base + self.channels * self.levels;
        (column >= self.base && column < end).then(|| ((column - self.base) % self.channels) as u32)
    }
}

/// One candidate row: positive rows (weight 0) before boundary rows.
#[derive(Clone, Copy)]
struct Candidate {
    row: usize,
    weight: f64,
    boundary: bool,
}

/// A level's basis, pivot channels, square system and determinant.
struct Level {
    /// Candidate indices of the basis rows, in insertion order.
    basis: Vec<usize>,
    /// Pivot channel of each basis row: system column `k` is `pivots[k]`.
    pivots: Vec<u32>,
    system: System,
    det: Big,
    bits: f64,
}

type Outcome<T> = std::result::Result<T, &'static str>;
/// Solved levels with their candidate ranges.
type LevelList = [(std::ops::Range<usize>, Level)];
/// A selected border row, its level and its adjugate solve.
type Adjust = (usize, u32, Vec<Big>);

/// Scaled integer entries `(index, m, k)` = m 2^k of a row's terms: level
/// channels (`index` = channel) and border coordinates (`index` = border slot).
struct Split {
    level: Vec<(u32, i64, u32)>,
    border: Vec<(u32, i64, u32)>,
}

struct Layout<'a> {
    read: Reader<'a>,
    width: usize,
    blocks: Blocks<'a>,
    low: Vec<i32>,
    border_of: Vec<u32>,
    borders: usize,
}

impl Layout<'_> {
    fn split(&self, row: usize) -> Result<Option<Split>> {
        let Some(terms) = read_terms(self.read, row, self.width)? else {
            return Ok(None);
        };
        let mut split = Split {
            level: Vec::new(),
            border: Vec::new(),
        };
        reserve(
            &mut split.level,
            self.blocks.channels,
            "unable to reserve block terms",
        )?;
        reserve(
            &mut split.border,
            terms.len(),
            "unable to reserve block terms",
        )?;
        let level = self.blocks.codes[row] as usize;
        for Term {
            column,
            mantissa,
            exponent,
        } in terms
        {
            let c = column as usize;
            let k = (exponent - self.low[c]) as u32;
            match self.blocks.channel(c) {
                Some(channel) => {
                    if (c - self.blocks.base) / self.blocks.channels != level {
                        return Ok(None);
                    }
                    split.level.push((channel, mantissa, k));
                }
                None => split.border.push((self.border_of[c], mantissa, k)),
            }
        }
        Ok(Some(split))
    }
}

fn big_term(m: i128, k: u32) -> Result<Big> {
    Big::from_i128(m)?.shl(k)
}

/// `value` as a binary64 (top 64 bits), for weight scaling only.
fn approximate(value: &Big) -> f64 {
    let limbs = value.limbs();
    let bits = value.bits();
    if bits == 0 {
        return 0.;
    }
    let drop = bits.saturating_sub(64);
    let (word, offset) = ((drop / 64) as usize, (drop % 64) as u32);
    let mut top = limbs[word] >> offset;
    if offset != 0 && word + 1 < limbs.len() {
        top |= limbs[word + 1] << (64 - offset);
    }
    (top as f64) * 2f64.powi(drop.min(2000) as i32)
}

/// Column exponents, the border numbering and the candidate order.
fn layout<'a>(
    (read, width, blocks): (Reader<'a>, usize, Blocks<'a>),
    candidates: &[Candidate],
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Outcome<Layout<'a>>> {
    let mut low = fallible_filled(width, i32::MAX, "unable to reserve block scales")?;
    for (n, c) in candidates.iter().enumerate() {
        if n % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        let Some(terms) = read_terms(read, c.row, width)? else {
            return Ok(Err("block_entry"));
        };
        if !budget.charge(terms.len().saturating_mul(2).saturating_add(1)) {
            return Ok(Err("block_work"));
        }
        for t in terms {
            low[t.column as usize] = low[t.column as usize].min(t.exponent);
        }
    }
    let mut border_of = fallible_filled(width, u32::MAX, "unable to reserve block border")?;
    let mut borders = 0u32;
    for (c, slot) in border_of.iter_mut().enumerate() {
        if blocks.channel(c).is_none() && low[c] != i32::MAX {
            *slot = borders;
            borders += 1;
        }
    }
    Ok(Ok(Layout {
        read,
        width,
        blocks,
        low,
        border_of,
        borders: borders as usize,
    }))
}

/// A level's basis (positive rows first) modulo a prime, its system and its
/// exact determinant.
fn level_basis(
    layout: &Layout<'_>,
    candidates: &[Candidate],
    range: std::ops::Range<usize>,
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Outcome<Option<Level>>> {
    let field = primes().next().expect("a 62-bit prime exists");
    let q = layout.blocks.channels;
    let mut elimination = Elimination::new(q, field)?;
    let (mut basis, mut pivots, mut rows) = (Vec::new(), Vec::new(), Vec::new());
    let mut residues: Vec<(u32, u64)> = Vec::new();
    reserve(&mut residues, q, "unable to reserve block row")?;
    for index in range {
        if elimination.rank() == q {
            break;
        }
        let Some(split) = layout.split(candidates[index].row)? else {
            return Ok(Err("block_entry"));
        };
        residues.clear();
        residues.extend(
            split
                .level
                .iter()
                .map(|&(c, m, k)| (c, residue(field, m, k))),
        );
        if !budget.charge(q.saturating_mul(q).saturating_add(split.level.len() * 12))
            || !elimination.reduce(&residues, None, &mut budget.work)?
        {
            return Ok(Err("block_work"));
        }
        let Some(channel) = elimination.remainder().map(|(c, _)| c).min() else {
            continue;
        };
        elimination.install(channel)?;
        reserve(&mut basis, 1, "unable to reserve block basis")?;
        reserve(&mut pivots, 1, "unable to reserve block basis")?;
        reserve(&mut rows, 1, "unable to reserve block basis")?;
        basis.push(index);
        pivots.push(channel);
        rows.push(split.level);
    }
    if basis.is_empty() {
        return Ok(Ok(None));
    }
    let mut position = [u32::MAX; 64];
    if q > position.len() {
        return Ok(Err("block_channels"));
    }
    for (k, &c) in pivots.iter().enumerate() {
        position[c as usize] = k as u32;
    }
    let mut top = 0u32;
    let mut system_rows = Vec::new();
    reserve(
        &mut system_rows,
        rows.len(),
        "unable to reserve block system",
    )?;
    for terms in rows {
        let mut row = Vec::new();
        reserve(&mut row, terms.len(), "unable to reserve block system")?;
        for (c, m, k) in terms {
            if position[c as usize] != u32::MAX {
                top = top.max(k);
                row.push((position[c as usize], m, k));
            }
        }
        system_rows.push(row);
    }
    let system = System::dyadic(system_rows, top);
    let bits: f64 = system
        .rows
        .iter()
        .map(|row| log_norm(row.iter().map(|&(_, m, k)| (m, k))).max(0.))
        .sum();
    let zeros = fallible_filled(basis.len(), Big::zero(), "unable to reserve block rhs")?;
    let (det, _) = match crt_solve(&system, &zeros, true, bits + 4., budget, interrupt)? {
        Ok(solved) => solved,
        Err(reason) => return Ok(Err(reason)),
    };
    Ok(Ok(Some(Level {
        basis,
        pivots,
        system,
        det,
        bits,
    })))
}

/// `adj(B_l') v` for a right side on the level's pivot channels, exactly.
fn adjugate(
    level: &Level,
    rhs: &[Big],
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Outcome<Vec<Big>>> {
    let rhs_bits = rhs.iter().map(Big::bits).max().unwrap_or(0) as f64;
    let bits = level.bits + rhs_bits + 0.5 * (rhs.len() as f64).log2() + 4.;
    match crt_solve(&level.system, rhs, true, bits, budget, interrupt)? {
        Ok((det, numerators)) if det == level.det => Ok(Ok(numerators)),
        Ok(_) => Ok(Err("block_determinant")),
        Err(reason) => Ok(Err(reason)),
    }
}

/// A level row's channel entries on the pivot channels, as exact integers.
fn on_pivots(level: &Level, terms: &[(u32, i64, u32)], kappa: (i64, i32)) -> Result<Vec<Big>> {
    let mut out = fallible_filled(
        level.pivots.len(),
        Big::zero(),
        "unable to reserve block row",
    )?;
    for &(c, m, k) in terms {
        if let Some(p) = level.pivots.iter().position(|&pc| pc == c) {
            let product = i128::from(m) * i128::from(kappa.0);
            out[p] = out[p].add(&big_term(product, k + kappa.1 as u32)?)?;
        }
    }
    Ok(out)
}

/// The exact block dual, or the reason it was not formed.
pub(crate) fn block_dual(
    (read, width, blocks): (Reader<'_>, usize, Blocks<'_>),
    (boundary, weights, pivots): (&[usize], &[f64], &[usize]),
    stage_work: usize,
    admit: &mut dyn FnMut(usize) -> bool,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Outcome<ExactDual>> {
    if boundary.is_empty() || boundary.len() != weights.len() || blocks.channels == 0 {
        return Ok(Err("block_shape"));
    }
    let mut budget = Budget {
        work: stage_work,
        held: 0,
        admit,
    };
    let n = boundary.len() + pivots.len();
    if !budget.hold(n.saturating_mul(16)) {
        return Ok(Err("exact_storage"));
    }
    let mut candidates = Vec::new();
    reserve(&mut candidates, n, "unable to reserve block candidates")?;
    candidates.extend(pivots.iter().map(|&row| Candidate {
        row,
        weight: 0.,
        boundary: false,
    }));
    for (&row, &weight) in boundary.iter().zip(weights) {
        if !(weight.is_finite() && weight > 0.) {
            return Ok(Err("block_weight_range"));
        }
        candidates.push(Candidate {
            row,
            weight,
            boundary: true,
        });
    }
    if candidates.iter().any(|c| c.row >= blocks.codes.len()) {
        return Ok(Err("block_shape"));
    }
    candidates.sort_by(|a, b| {
        let key = |c: &Candidate| (blocks.codes[c.row], c.boundary);
        key(a)
            .cmp(&key(b))
            .then(b.weight.total_cmp(&a.weight))
            .then(a.row.cmp(&b.row))
    });
    let layout = match layout((read, width, blocks), &candidates, &mut budget, interrupt)? {
        Ok(layout) => layout,
        Err(reason) => return Ok(Err(reason)),
    };
    let solved = match levels(&layout, &candidates, &mut budget, interrupt)? {
        Ok(solved) => solved,
        Err(reason) => return Ok(Err(reason)),
    };
    let built = border(&layout, &candidates, solved, &mut budget, interrupt)?;
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=exact_block_dual|levels={}|borders={}|rows={n}|outcome={}|work_left={}",
            blocks.levels,
            layout.borders,
            built.as_ref().err().copied().unwrap_or("formed"),
            budget.work
        );
    }
    Ok(built)
}

/// Weights after the level solves: per candidate (0 for positive rows
/// outside every basis), each solved level with its candidate range, and
/// each candidate's solved level (`u32::MAX`: none).
struct Solved {
    weights: Vec<Big>,
    levels: Vec<(std::ops::Range<usize>, Level)>,
    level_of: Vec<u32>,
}

fn ranges(blocks: &Blocks<'_>, candidates: &[Candidate]) -> Result<Vec<std::ops::Range<usize>>> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < candidates.len() {
        let level = blocks.codes[candidates[start].row];
        let end = start
            + candidates[start..]
                .iter()
                .take_while(|c| blocks.codes[c.row] == level)
                .count();
        reserve(&mut out, 1, "unable to reserve block levels")?;
        out.push(start..end);
        start = end;
    }
    Ok(out)
}

/// Every level's basis and exact weights.
fn levels(
    layout: &Layout<'_>,
    candidates: &[Candidate],
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Outcome<Solved>> {
    let spans = ranges(&layout.blocks, candidates)?;
    let mut levels: Vec<(std::ops::Range<usize>, Level)> = Vec::new();
    reserve(&mut levels, spans.len(), "unable to reserve block levels")?;
    let mut level_of = fallible_filled(candidates.len(), u32::MAX, "unable to reserve block map")?;
    let mut in_basis = fallible_filled(candidates.len(), false, "unable to reserve block marks")?;
    for range in spans {
        interrupt()?;
        let solved = match level_basis(layout, candidates, range.clone(), budget, interrupt)? {
            Ok(solved) => solved,
            Err(reason) => return Ok(Err(reason)),
        };
        let Some(solved) = solved else {
            continue;
        };
        for &b in &solved.basis {
            in_basis[b] = true;
        }
        for i in range.clone() {
            level_of[i] = levels.len() as u32;
        }
        levels.push((range, solved));
    }
    // One scale C = 2^s keeps every kappa_i = C w_i / det_l at least 2^20.
    let mut s = f64::NEG_INFINITY;
    for (range, level) in &levels {
        let det = approximate(&level.det).log2();
        for i in range.clone() {
            if candidates[i].boundary && !in_basis[i] {
                s = s.max(20. + det - candidates[i].weight.log2());
            }
        }
    }
    let s = if s.is_finite() { s.ceil() as i32 } else { 0 };
    if !(-900..=900).contains(&s) {
        return Ok(Err("block_weight_range"));
    }
    let mut weights = Vec::new();
    reserve(
        &mut weights,
        candidates.len(),
        "unable to reserve block weights",
    )?;
    weights.extend((0..candidates.len()).map(|_| Big::zero()));
    for (range, level) in &levels {
        let det = approximate(&level.det);
        let mut rhs = fallible_filled(
            level.pivots.len(),
            Big::zero(),
            "unable to reserve block rhs",
        )?;
        for i in range.clone() {
            let c = candidates[i];
            if !c.boundary || in_basis[i] {
                continue;
            }
            let kappa = (c.weight * 2f64.powi(s) / det).round();
            if !(kappa >= 1. && kappa.is_finite()) {
                return Ok(Err("block_weight_range"));
            }
            let parts = dyadic(kappa);
            let Some(split) = layout.split(c.row)? else {
                return Ok(Err("block_entry"));
            };
            if !budget.charge(split.level.len().saturating_mul(8).saturating_add(8)) {
                return Ok(Err("block_work"));
            }
            for (slot, term) in rhs.iter_mut().zip(on_pivots(level, &split.level, parts)?) {
                *slot = slot.sub(&term)?;
            }
            weights[i] = level
                .det
                .mul(&big_term(i128::from(parts.0), parts.1 as u32)?)?;
        }
        let basis_weights = match adjugate(level, &rhs, budget, interrupt)? {
            Ok(numerators) => numerators,
            Err(reason) => return Ok(Err(reason)),
        };
        for (&b, w) in level.basis.iter().zip(basis_weights) {
            weights[b] = w;
        }
    }
    Ok(Ok(Solved {
        weights,
        levels,
        level_of,
    }))
}

/// Balance the border coordinates by adjusting a few rows through their
/// levels' bases, then assemble the dual.
fn border(
    layout: &Layout<'_>,
    candidates: &[Candidate],
    solved: Solved,
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Outcome<ExactDual>> {
    let nb = layout.borders;
    let Solved {
        weights,
        levels,
        level_of,
    } = solved;
    // Exact border residual of the level-balanced weights.
    let mut residual = fallible_filled(nb, Big::zero(), "unable to reserve block residual")?;
    for (i, c) in candidates.iter().enumerate() {
        if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        if weights[i].is_zero() {
            continue;
        }
        let Some(split) = layout.split(c.row)? else {
            return Ok(Err("block_entry"));
        };
        let cost = split
            .border
            .len()
            .saturating_mul(weights[i].limbs().len() + 4);
        if !budget.charge(cost) {
            return Ok(Err("block_work"));
        }
        for &(b, m, k) in &split.border {
            residual[b as usize] = residual[b as usize].add(&weights[i].mul_i64(m)?.shl(k)?)?;
        }
    }
    let selected = match select_border(layout, candidates, (&levels, &level_of), budget, interrupt)?
    {
        Ok(selected) => selected,
        Err(reason) => return Ok(Err(reason)),
    };
    solve_border(
        layout,
        candidates,
        (weights, &levels, &level_of),
        (selected, residual),
        budget,
        interrupt,
    )
}

/// Rows whose adjustments make the border system nonsingular, chosen
/// modulo a prime on their Schur vectors (positive rows first), with the
/// pivot border slot of each.
fn select_border(
    layout: &Layout<'_>,
    candidates: &[Candidate],
    (levels, level_of): (&LevelList, &[u32]),
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Outcome<Vec<(usize, u32)>>> {
    let nb = layout.borders;
    let field = primes().next().expect("a 62-bit prime exists");
    let mut elimination = Elimination::new(nb.max(1), field)?;
    let mut in_basis = fallible_filled(candidates.len(), false, "unable to reserve block marks")?;
    for (_, level) in levels {
        for &b in &level.basis {
            in_basis[b] = true;
        }
    }
    let mut order: Vec<usize> = Vec::new();
    reserve(
        &mut order,
        candidates.len(),
        "unable to reserve block order",
    )?;
    order.extend((0..candidates.len()).filter(|&i| !in_basis[i] && level_of[i] != u32::MAX));
    order.sort_by(|&a, &b| {
        let (ca, cb) = (candidates[a], candidates[b]);
        ca.boundary
            .cmp(&cb.boundary)
            .then(cb.weight.total_cmp(&ca.weight))
            .then(a.cmp(&b))
    });
    let mut selected = Vec::new();
    let mut vector = fallible_filled(nb, 0u64, "unable to reserve block schur")?;
    let mut sparse: Vec<(u32, u64)> = Vec::new();
    reserve(&mut sparse, nb, "unable to reserve block schur")?;
    for (n, &i) in order.iter().enumerate() {
        if elimination.rank() == nb {
            break;
        }
        if n % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        let level = &levels[level_of[i] as usize].1;
        let Some(split) = layout.split(candidates[i].row)? else {
            return Ok(Err("block_entry"));
        };
        let q = level.pivots.len();
        if !budget.charge((q * q * q + nb + split.border.len() * 4).saturating_mul(4)) {
            return Ok(Err("block_work"));
        }
        if !schur_vector(layout, level, (&split, candidates), field, &mut vector)? {
            return Ok(Err("block_schur"));
        }
        sparse.clear();
        sparse.extend(
            vector
                .iter()
                .enumerate()
                .filter(|(_, &v)| v != 0)
                .map(|(b, &v)| (b as u32, v)),
        );
        if !elimination.reduce(&sparse, None, &mut budget.work)? {
            return Ok(Err("block_work"));
        }
        let Some(pivot) = elimination.remainder().map(|(b, _)| b).min() else {
            continue;
        };
        elimination.install(pivot)?;
        reserve(&mut selected, 1, "unable to reserve block border rows")?;
        selected.push((i, pivot));
    }
    Ok(Ok(selected))
}

/// `det_l x_g^B - X_B^B adj(B_l') y_g` modulo the field, dense over the
/// border slots. False when the level factor is singular modulo the prime.
fn schur_vector(
    layout: &Layout<'_>,
    level: &Level,
    (split, candidates): (&Split, &[Candidate]),
    field: Field,
    out: &mut [u64],
) -> Result<bool> {
    out.fill(0);
    let mut powers = fallible_filled(
        level.system.top as usize + 1,
        0u64,
        "unable to reserve block powers",
    )?;
    powers[0] = 1;
    for k in 1..powers.len() {
        powers[k] = field.add(powers[k - 1], powers[k - 1]);
    }
    let residues: Vec<Vec<(u32, u64)>> = level
        .system
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|&(c, m, k)| (c, field.mul(field.signed(m), powers[k as usize])))
                .collect()
        })
        .collect();
    let mut work = usize::MAX;
    let Some(factor) = Factor::new(&residues, field, &mut work)? else {
        return Ok(false);
    };
    let det = field.big(&level.det);
    let mut t = fallible_filled(level.pivots.len(), 0u64, "unable to reserve block solve")?;
    for &(c, m, k) in &split.level {
        if let Some(p) = level.pivots.iter().position(|&pc| pc == c) {
            t[p] = field.add(t[p], residue(field, m, k));
        }
    }
    factor.solve_transposed(&mut t);
    for &(b, m, k) in &split.border {
        out[b as usize] = field.add(out[b as usize], field.mul(det, residue(field, m, k)));
    }
    for (&basis, &u) in level.basis.iter().zip(&t) {
        let coefficient = field.mul(det, u);
        if coefficient == 0 {
            continue;
        }
        let Some(row) = layout.split(candidates[basis].row)? else {
            return Ok(false);
        };
        for (b, m, k) in row.border {
            let delta = field.mul(coefficient, residue(field, m, k));
            out[b as usize] = field.sub(out[b as usize], delta);
        }
    }
    Ok(true)
}

/// The exact border system on the selected rows, and the assembled dual.
fn solve_border(
    layout: &Layout<'_>,
    candidates: &[Candidate],
    (weights, levels, level_of): (Vec<Big>, &LevelList, &[u32]),
    (selected, residual): (Vec<(usize, u32)>, Vec<Big>),
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Outcome<ExactDual>> {
    let g = selected.len();
    let mut position = fallible_filled(layout.borders, u32::MAX, "unable to reserve block pivots")?;
    for (k, &(_, b)) in selected.iter().enumerate() {
        position[b as usize] = k as u32;
    }
    // Exact Schur rows and each row's adjugate solve.
    let mut rows = Vec::new();
    let mut adjusts = Vec::new();
    reserve(&mut rows, g, "unable to reserve block border system")?;
    reserve(&mut adjusts, g, "unable to reserve block border system")?;
    for &(i, _) in &selected {
        let level = &levels[level_of[i] as usize].1;
        let Some(split) = layout.split(candidates[i].row)? else {
            return Ok(Err("block_entry"));
        };
        let y = on_pivots(level, &split.level, (1, 0))?;
        let v = match adjugate(level, &y, budget, interrupt)? {
            Ok(v) => v,
            Err(reason) => return Ok(Err(reason)),
        };
        let mut dense = fallible_filled(g, Big::zero(), "unable to reserve block border row")?;
        for &(b, m, k) in &split.border {
            if let Some(&p) = position.get(b as usize).filter(|&&p| p != u32::MAX) {
                dense[p as usize] = dense[p as usize].add(&level.det.mul_i64(m)?.shl(k)?)?;
            }
        }
        for (&basis, vk) in level.basis.iter().zip(&v) {
            if vk.is_zero() {
                continue;
            }
            let Some(row) = layout.split(candidates[basis].row)? else {
                return Ok(Err("block_entry"));
            };
            for (b, m, k) in row.border {
                if let Some(&p) = position.get(b as usize).filter(|&&p| p != u32::MAX) {
                    dense[p as usize] = dense[p as usize].sub(&vk.mul_i64(m)?.shl(k)?)?;
                }
            }
        }
        rows.push(
            dense
                .into_iter()
                .enumerate()
                .filter(|(_, v)| !v.is_zero())
                .map(|(p, v)| (p as u32, v))
                .collect::<Vec<_>>(),
        );
        adjusts.push((i, level_of[i], v));
    }
    let mut rhs = fallible_filled(g, Big::zero(), "unable to reserve block border rhs")?;
    for &(_, b) in &selected {
        let p = position[b as usize] as usize;
        rhs[p] = residual[b as usize].negated()?;
    }
    let (det, numerators) = if g == 0 {
        (Big::from_i128(1)?, Vec::new())
    } else {
        let row_bits: f64 = rows
            .iter()
            .map(|row| {
                let top = row.iter().map(|(_, v)| v.bits()).max().unwrap_or(0) as f64;
                top + 0.5 * (row.len().max(1) as f64).log2()
            })
            .sum();
        let rhs_bits = rhs.iter().map(Big::bits).max().unwrap_or(0) as f64;
        let bits = row_bits + rhs_bits + 0.5 * (g as f64).log2() + 4.;
        let system = System {
            rows: Vec::new(),
            top: 0,
            wide: rows,
        };
        match crt_solve(&system, &rhs, true, bits, budget, interrupt)? {
            Ok(solved) => solved,
            Err(reason) => return Ok(Err(reason)),
        }
    };
    assemble(candidates, (weights, levels), (det, numerators, adjusts))
}

/// The dual: `det_B` times the level-balanced weights, plus each selected
/// row's adjustment carried through its level's basis.
fn assemble(
    candidates: &[Candidate],
    (weights, levels): (Vec<Big>, &LevelList),
    (det, numerators, adjusts): (Big, Vec<Big>, Vec<Adjust>),
) -> Result<Outcome<ExactDual>> {
    let n = candidates.len();
    let mut explicit: Vec<Option<Big>> = Vec::new();
    reserve(&mut explicit, n, "unable to reserve block dual")?;
    explicit.extend((0..n).map(|_| None));
    for ((i, level, v), num) in adjusts.iter().zip(&numerators) {
        let level = &levels[*level as usize].1;
        let current = explicit[*i]
            .take()
            .map_or_else(|| det.mul(&weights[*i]), Ok)?;
        explicit[*i] = Some(current.add(&level.det.mul(num)?)?);
        for (&basis, vk) in level.basis.iter().zip(v) {
            let current = explicit[basis]
                .take()
                .map_or_else(|| det.mul(&weights[basis]), Ok)?;
            explicit[basis] = Some(current.sub(&vk.mul(num)?)?);
        }
    }
    let (mut scaled, mut basis) = (Vec::new(), Vec::new());
    for (i, (c, w)) in candidates.iter().zip(weights).enumerate() {
        match explicit[i].take() {
            Some(value) => {
                if c.boundary && value.signum() <= 0 {
                    return Ok(Err("block_boundary_weight"));
                }
                if !value.is_zero() {
                    reserve(&mut basis, 1, "unable to reserve block dual")?;
                    basis.push((c.row, value));
                }
            }
            None => {
                if c.boundary && w.signum() <= 0 {
                    return Ok(Err("block_boundary_weight"));
                }
                if !w.is_zero() {
                    reserve(&mut scaled, 1, "unable to reserve block dual")?;
                    scaled.push((c.row, w));
                }
            }
        }
    }
    Ok(Ok(ExactDual {
        scale: det,
        integers: Vec::new(),
        scaled,
        basis,
    }))
}
