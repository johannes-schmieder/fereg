//! Numerical projections propose deleted rows. Only a recession direction of
//! the stored design, with exact equality on positive outcomes, authorizes them.
use super::exact_arithmetic::{exact_score, ExactNullspace};
use super::*;
use std::cmp::Ordering;

mod fe_pair;
mod pair;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Column {
    Finite(usize),
    Level { effect: usize, level: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Witness {
    PairOffsets {
        column: usize,
        effects: [usize; 2],
        offsets: Vec<f64>,
        negative: bool,
    },
    Pair {
        columns: [usize; 2],
        weights: [i64; 2],
    },
    Centered {
        column: Column,
        effect: Option<usize>,
        negative: bool,
    },
    Coefficients {
        values: Vec<i64>,
        units: Vec<f64>,
        absorbed: bool,
    },
    Saturated {
        absorbed: bool,
    },
}

struct Block<'a> {
    codes: &'a [u32],
    levels: usize,
    intercept: bool,
    slopes: Vec<&'a [f64]>,
}

pub(super) struct RawRows<'a> {
    model: &'a PreparedPpmlModelGeneration,
    blocks: Vec<Block<'a>>,
    width: usize,
}

impl<'a> RawRows<'a> {
    fn new(model: &'a PreparedPpmlModelGeneration, absorbed: bool) -> Result<Self> {
        let data = model.fit_view();
        let mut width = model.model().columns.len();
        let specs = if absorbed { data.effect_specs() } else { &[] };
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(specs.len())
            .map_err(|_| allocation("unable to reserve exact separation channels"))?;
        for (spec, &source) in specs.iter().zip(data.fe_topology().effect_to_topology()) {
            let topology = crate::fe::ValidatedBorrowedInterceptSource::from_finalized_source(
                &data.fe_topology,
                source,
            )
            .ok_or_else(|| {
                invalid_argument("exact separation source has no finalized membership")
            })?;
            let effect = topology.effect();
            let mut slopes = Vec::new();
            slopes
                .try_reserve_exact(spec.slopes.len())
                .map_err(|_| allocation("unable to reserve slope views"))?;
            for slope in &spec.slopes {
                slopes.push(data.numeric_source_values(slope.source)?);
            }
            width = width
                .checked_add(
                    effect
                        .n_levels()
                        .checked_mul(usize::from(spec.intercept) + slopes.len())
                        .ok_or_else(|| allocation("exact separation width overflow"))?,
                )
                .ok_or_else(|| allocation("exact separation width overflow"))?;
            blocks.push(Block {
                codes: effect.codes(),
                levels: effect.n_levels(),
                intercept: spec.intercept,
                slopes,
            });
        }
        Ok(Self {
            model,
            blocks,
            width,
        })
    }

    pub(super) fn len(&self) -> usize {
        self.model.outcome().len()
    }

    fn value(&self, row: usize, column: usize) -> Result<f64> {
        let finite = self.model.model().columns.len();
        if column < finite {
            return self.model.fit_view().column_value(column, row);
        }
        let mut index = column - finite;
        for block in &self.blocks {
            let channels = usize::from(block.intercept) + block.slopes.len();
            if index < channels * block.levels {
                if index / channels != block.codes[row] as usize {
                    return Ok(0.);
                }
                let channel = index % channels;
                return Ok(if block.intercept && channel == 0 {
                    1.
                } else {
                    block.slopes[channel - usize::from(block.intercept)][row]
                });
            }
            index -= channels * block.levels;
        }
        Err(invalid_argument(
            "exact separation coordinate outside design",
        ))
    }

    pub(super) fn write(&self, row: usize, values: &mut [f64]) -> Result<()> {
        let finite = self.model.model().columns.len();
        for (i, v) in values[..finite].iter_mut().enumerate() {
            *v = self.model.fit_view().column_value(i, row)?;
        }
        values[finite..].fill(0.);
        let mut start = finite;
        for block in &self.blocks {
            let channels = usize::from(block.intercept) + block.slopes.len();
            let mut position = start + channels * block.codes[row] as usize;
            if block.intercept {
                values[position] = 1.;
                position += 1;
            }
            for slope in &block.slopes {
                values[position] = slope[row];
                position += 1;
            }
            start += channels * block.levels;
        }
        Ok(())
    }
}

/// Both banks belong to ordinary PPML linear algebra and are released before
/// the independent primal proof: seven observation vectors and seven finite
/// moment/factor squares. A requested simplex also reserves its own arena.
#[derive(Clone, Copy)]
pub(super) struct ProofBudget {
    pub(super) words: usize,
    pub(super) work_cells: usize,
}

impl ProofBudget {
    pub(super) fn new(
        model: &PreparedPpmlModelGeneration,
        method: SeparationMethod,
        config: SeparationConfig,
    ) -> Self {
        let k = model.model().columns.len();
        let projection = model
            .outcome()
            .len()
            .saturating_add(k.saturating_mul(k))
            .saturating_mul(7);
        let words = if config.methods.simplex {
            projection.max(config.simplex_max_tableau_cells)
        } else {
            projection
        };
        // IR and Mu also admit the shared linear projection. Credit its
        // work when the user reduces the separate simplex tableau ceiling.
        // This allowance is only for operations and allocates no extra words.
        let work_cells = if method == SeparationMethod::Simplex {
            config.simplex_max_tableau_cells
        } else {
            config
                .simplex_max_tableau_cells
                .max(projection.min(1_000_000))
        };
        Self { words, work_cells }
    }
}

fn visit(
    model: &PreparedPpmlModelGeneration,
    witness: &Witness,
    mut inspect: impl FnMut(usize, Ordering, f64) -> bool,
) -> Result<bool> {
    match witness {
        Witness::PairOffsets {
            column,
            effects,
            offsets,
            negative,
        } => fe_pair::visit(model, *column, *effects, offsets, *negative, inspect),
        Witness::Pair { columns, weights } => {
            let view = model.fit_view();
            for r in 0..model.outcome().len() {
                crate::interrupt::row_checkpoint(r)?;
                let values = [
                    view.column_value(columns[0], r)?,
                    view.column_value(columns[1], r)?,
                ];
                let Some((sign, value)) = exact_score(&values, weights) else {
                    return Ok(false);
                };
                if !inspect(r, sign, value) {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Witness::Centered {
            column,
            effect,
            negative,
        } => {
            let fit = model.fit_view();
            let group_codes = if let Column::Level { effect, .. } = column {
                let source = fit.fe_topology().effect_to_topology()[*effect];
                Some(
                    crate::fe::ValidatedBorrowedInterceptSource::from_finalized_source(
                        &fit.fe_topology,
                        source,
                    )
                    .ok_or_else(|| invalid_argument("unavailable intercept channel"))?
                    .effect()
                    .codes(),
                )
            } else {
                None
            };
            let at = |r: usize| -> Result<f64> {
                match column {
                    Column::Finite(c) => fit.column_value(*c, r),
                    Column::Level { level, .. } => Ok(f64::from(group_codes.unwrap()[r] == *level)),
                }
            };
            let mut group = |members: &[usize]| -> Result<bool> {
                let Some(&anchor) = members.iter().find(|&&r| model.outcome()[r] > 0.) else {
                    return Ok(false);
                };
                let origin = at(anchor)?;
                for (pos, &r) in members.iter().enumerate() {
                    crate::interrupt::row_checkpoint(pos)?;
                    let value = at(r)?;
                    let (a, b) = if *negative {
                        (origin, value)
                    } else {
                        (value, origin)
                    };
                    if !inspect(r, a.partial_cmp(&b).unwrap(), a - b) {
                        return Ok(false);
                    }
                }
                Ok(true)
            };
            if let Some(effect) = effect {
                let topology = DetectorTopology::new(model.prepared())?;
                let levels = topology.sources[topology.effect_to_source[*effect]];
                for level in 0..levels.n_levels() {
                    crate::interrupt::checkpoint()?;
                    if !group(levels.checked_members(level)?)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            } else {
                // No intercept coefficient is required for this uncentered submodel.
                for r in 0..model.outcome().len() {
                    crate::interrupt::row_checkpoint(r)?;
                    let x = at(r)?;
                    let value = if *negative { -x } else { x };
                    if !inspect(r, value.partial_cmp(&0.).unwrap(), value) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
        Witness::Coefficients {
            values: coefficients,
            units,
            absorbed,
        } => {
            let design = RawRows::new(model, *absorbed)?;
            if coefficients.len() != design.width {
                return Ok(false);
            }
            let mut row = fallible_filled(
                design.width,
                0.,
                "unable to reserve primal coefficient audit",
            )?;
            for r in 0..design.len() {
                crate::interrupt::row_checkpoint(r)?;
                design.write(r, &mut row)?;
                if !rescale(&mut row, units) {
                    return Ok(false);
                }
                let Some((sign, value)) = exact_score(&row, coefficients) else {
                    return Ok(false);
                };
                if !inspect(r, sign, value) {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Witness::Saturated { .. } => Ok(false), // Verified through the modular cell proof.
    }
}

fn strict_count(
    model: &PreparedPpmlModelGeneration,
    witness: &Witness,
    expected: &[usize],
) -> Result<usize> {
    let mut count = 0;
    if visit(model, witness, |r, sign, _| {
        if model.outcome()[r] > 0. {
            return sign == Ordering::Equal;
        }
        if sign == Ordering::Greater {
            return false;
        }
        if sign == Ordering::Less && expected.binary_search(&model.original_rows()[r]).is_ok() {
            count += 1;
        }
        true
    })? {
        Ok(count)
    } else {
        Ok(0)
    }
}

pub(super) fn verify(
    model: &PreparedPpmlModelGeneration,
    witness: &Witness,
    expected: &[usize],
    budget: ProofBudget,
    hint: &[f64],
) -> Result<bool> {
    if let Witness::Saturated { absorbed } = witness {
        return saturated(
            &RawRows::new(model, *absorbed)?,
            expected,
            budget.words,
            cell_work(budget),
            hint,
        );
    }
    Ok(strict_count(model, witness, expected)? == expected.len())
}

/// The integer cell proof meters each elimination/reconstruction stage. The
/// fixed cap bounds retries even if callers provide a larger simplex arena.
fn cell_work(budget: ProofBudget) -> usize {
    budget.work_cells.saturating_mul(64).min(128_000_000)
}

fn saturated(
    design: &RawRows<'_>,
    expected: &[usize],
    limit: usize,
    work: usize,
    hint: &[f64],
) -> Result<bool> {
    let k = design.width;
    if k == 0 || design.len().saturating_mul(k) > work {
        return Ok(false);
    }
    let selected = |r: usize| {
        expected
            .binary_search(&design.model.original_rows()[r])
            .is_ok()
    };
    if k.saturating_mul(k.saturating_add(8)) <= limit
        && k.min(design.len())
            .saturating_pow(3)
            .saturating_add(design.len().saturating_mul(k))
            <= work
        && super::primal_saturated::supports_target(
            design.len(),
            k,
            |r, row| design.write(r, row),
            selected,
            work,
        )?
    {
        return Ok(true);
    }
    super::primal_saturated::supports_dependent_target_with_hint(
        design.len(),
        k,
        (limit, work),
        |r, row| design.write(r, row),
        selected,
        |r, c| design.value(r, c),
        Some(hint),
    )
}

struct Search<'a> {
    model: &'a PreparedPpmlModelGeneration,
    expected: &'a [usize],
    best: Option<Witness>,
    count: usize,
}

impl Search<'_> {
    fn consider(&mut self, witness: Witness) -> Result<bool> {
        let count = strict_count(self.model, &witness, self.expected)?;
        if count > self.count {
            self.count = count;
            self.best = Some(witness);
        }
        Ok(count == self.expected.len())
    }
}

pub(super) fn find(
    model: &PreparedPpmlModelGeneration,
    expected: &[usize],
    budget: ProofBudget,
    hint: &[f64],
    allow_existence_proof: bool,
) -> Result<Option<(Witness, bool)>> {
    let mut search = Search {
        model,
        expected,
        best: None,
        count: 0,
    };
    let limit = budget.words;
    // Bound raw coordinate work independently of reusable allocation capacity.
    // Each of the two submodels and each proof stage receives a bounded pass.
    let work = budget.work_cells.saturating_mul(8);
    if centered_columns(&mut search, work)?
        || pair::search(&mut search, work)?
        || fe_pair::search(&mut search, work, limit)?
    {
        return Ok(search.best.map(|p| (p, true)));
    }
    for absorbed in [false, true] {
        let design = RawRows::new(model, absorbed)?;
        if allow_existence_proof && saturated(&design, expected, limit, cell_work(budget), hint)? {
            return Ok(Some((Witness::Saturated { absorbed }, true)));
        }
        let k = design.width;
        let dense_work = design
            .len()
            .saturating_mul(k)
            .saturating_mul(k.min(design.len()));
        if k > 0
            && k.saturating_mul(k.saturating_add(32)) <= limit
            && dense_work <= work
            && dense_columns(&mut search, &design, absorbed, hint)?
        {
            return Ok(search.best.map(|p| (p, true)));
        }
    }
    Ok(search.best.map(|p| (p, false)))
}

fn centered_columns(search: &mut Search<'_>, budget: usize) -> Result<bool> {
    let model = search.model;
    let finite = model.model().columns.len();
    let specs = model.fit_view().effect_specs();
    let levels = DetectorTopology::new(model.prepared())?;
    let groups = specs
        .iter()
        .enumerate()
        .filter(|(_, s)| s.intercept)
        .flat_map(|(e, _)| {
            let n = levels.sources[levels.effect_to_source[e]].n_levels();
            (0..n).map(move |l| Column::Level {
                effect: e,
                level: l as u32,
            })
        });
    let per_column = model
        .outcome()
        .len()
        .saturating_mul(specs.len() + 1)
        .saturating_mul(2)
        .max(1);
    for column in (0..finite)
        .map(Column::Finite)
        .chain(groups)
        .take(budget / per_column)
    {
        for effect in std::iter::once(None).chain(
            specs
                .iter()
                .enumerate()
                .filter_map(|(e, s)| s.intercept.then_some(Some(e))),
        ) {
            for negative in [false, true] {
                if search.consider(Witness::Centered {
                    column: column.clone(),
                    effect,
                    negative,
                })? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn dense_columns(
    search: &mut Search<'_>,
    design: &RawRows<'_>,
    absorbed: bool,
    hint: &[f64],
) -> Result<bool> {
    let model = search.model;
    let expected = search.expected;
    let k = design.width;
    let mut row = fallible_filled(k, 0., "unable to reserve primal equality row")?;
    let units = power_units(design, &mut row)?;
    let mut anchors = Vec::new();
    anchors
        .try_reserve_exact(k)
        .map_err(|_| allocation("unable to reserve primal interpolation anchors"))?;
    {
        let mut independent = ExactNullspace::new(k)?;
        for selected in [false, true] {
            for r in 0..design.len() {
                crate::interrupt::row_checkpoint(r)?;
                let wanted = expected.binary_search(&model.original_rows()[r]).is_ok();
                if wanted != selected {
                    continue;
                }
                design.write(r, &mut row)?;
                let rank = independent.rank();
                independent.insert(&row)?;
                if independent.rank() != rank {
                    anchors.push(r)
                }
                if independent.rank() == k {
                    break;
                }
            }
        }
    }
    // Positive and other binding rows determine the first anchors.
    // Fit the detector's relative magnitudes only on the remaining
    // independent rows. Every choice is verified on the entire sample.
    let maximum = infinity_norm(hint);
    for precision in [0., 8., 128., 2048.] {
        let target = |r: usize| {
            if expected.binary_search(&model.original_rows()[r]).is_err() {
                0.
            } else if precision == 0. || maximum == 0. {
                1.
            } else {
                (-hint[r] / maximum * precision).round()
            }
        };
        if let Some(proof) = interpolate(design, &units, absorbed, &anchors, target)? {
            if search.consider(proof)? {
                return Ok(true);
            }
        }
    }
    let mut space = ExactNullspace::new(k)?;
    for r in 0..design.len() {
        crate::interrupt::row_checkpoint(r)?;
        if model.outcome()[r] > 0. || expected.binary_search(&model.original_rows()[r]).is_err() {
            design.write(r, &mut row)?;
            if !rescale(&mut row, &units) {
                return Err(invalid_argument("primal equation lost its exact units"));
            }
            space.insert(&row)?;
            if space.rank() == k {
                break;
            }
        }
    }
    let mut free = fallible_filled(k, 0_i64, "unable to reserve primal free coordinates")?;
    for c in 0..k {
        if space.pivot(c) {
            continue;
        }
        free[c] = 1;
        if let Some(mut b) = space.reconstruct(&free)? {
            for reverse in [false, true] {
                if reverse {
                    if b.contains(&i64::MIN) {
                        break;
                    }
                    for v in &mut b {
                        *v = -*v;
                    }
                }
                if search.consider(Witness::Coefficients {
                    values: fallible_copy(&b, "unable to reserve exact primal coordinates")?,
                    units: fallible_copy(&units, "unable to reserve exact primal units")?,
                    absorbed,
                })? {
                    return Ok(true);
                }
            }
        }
        free[c] = 0;
    }
    Ok(false)
}

/// Rescale with exact powers of two. A failed round trip can only decline a
/// candidate. The exact sign check always refers to the original raw design.
fn rescale(row: &mut [f64], units: &[f64]) -> bool {
    if row.len() != units.len() {
        return false;
    }
    for (value, &unit) in row.iter_mut().zip(units) {
        let scaled = *value / unit;
        if !scaled.is_finite() || scaled * unit != *value {
            return false;
        }
        *value = scaled;
    }
    true
}

fn power_units(design: &RawRows<'_>, row: &mut [f64]) -> Result<Vec<f64>> {
    let mut units = fallible_filled(
        row.len(),
        0.0_f64,
        "unable to reserve primal coordinate scales",
    )?;
    for r in 0..design.len() {
        crate::interrupt::row_checkpoint(r)?;
        design.write(r, row)?;
        for (max, &v) in units.iter_mut().zip(row.iter()) {
            *max = max.max(v.abs());
        }
    }
    for unit in &mut units {
        *unit = if *unit == 0. {
            1.
        } else {
            f64::from_bits((unit.to_bits() & 0x7ff0000000000000).max(1u64 << 52))
        };
    }
    for r in 0..design.len() {
        crate::interrupt::row_checkpoint(r)?;
        design.write(r, row)?;
        for (unit, &v) in units.iter_mut().zip(row.iter()) {
            if v / *unit * *unit != v {
                *unit = 1.;
            }
        }
    }
    Ok(units)
}

/// Solve D b = -1 on proposed rows, zero elsewhere, using an augmented
/// modular kernel as a coefficient proposal. Re-evaluate every raw sign.
fn interpolate(
    design: &RawRows<'_>,
    units: &[f64],
    absorbed: bool,
    anchors: &[usize],
    target: impl Fn(usize) -> f64,
) -> Result<Option<Witness>> {
    let k = design.width;
    let mut space = ExactNullspace::new(k + 1)?;
    let mut row = fallible_filled(k + 1, 0., "unable to reserve interpolation equation")?;
    for &r in anchors {
        crate::interrupt::row_checkpoint(r)?;
        design.write(r, &mut row[..k])?;
        if !rescale(&mut row[..k], units) {
            return Ok(None);
        }
        row[k] = target(r);
        space.insert(&row)?;
        if space.pivot(k) {
            return Ok(None);
        }
    }
    let mut free = fallible_filled(k + 1, 0i64, "unable to reserve interpolation coordinates")?;
    free[k] = 1;
    let Some(mut values) = space.reconstruct(&free)? else {
        return Ok(None);
    };
    values.truncate(k);
    Ok(Some(Witness::Coefficients {
        values,
        units: fallible_copy(units, "unable to reserve interpolant units")?,
        absorbed,
    }))
}

pub(super) fn direction(
    model: &PreparedPpmlModelGeneration,
    witness: &Witness,
) -> Result<Option<Vec<f64>>> {
    let mut direction = fallible_filled(
        model.outcome().len(),
        0.,
        "unable to reserve exact primal direction",
    )?;
    if matches!(witness, Witness::Saturated { .. }) {
        return Ok(None); // An existential witness has no materialized scores.
    }
    let valid = visit(model, witness, |r, sign, value| {
        if !value.is_finite() || value.partial_cmp(&0.) != Some(sign) {
            return false;
        }
        direction[r] = value;
        true
    })?;
    Ok(valid.then_some(direction))
}

/// Every score is rounded once from a sum of integer multiples of 2^-1074.
/// A subnormal result is exact. At normal magnitudes the relative rounding
/// error is at most EPSILON. Thus a coordinatewise replay also provides an
/// upper bound for relative Euclidean distance to the full model span.
pub(super) fn rounded_span(
    model: &PreparedPpmlModelGeneration,
    proof: &Witness,
    direction: &[f64],
) -> Result<Option<f64>> {
    if matches!(proof, Witness::Saturated { .. }) {
        return Ok(None);
    }
    let matches = visit(model, proof, |r, _, score| score == direction[r])?;
    Ok(matches.then_some(f64::EPSILON))
}
