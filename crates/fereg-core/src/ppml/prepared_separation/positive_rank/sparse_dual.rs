//! Search for a strictly positive boundary multiplier in the kernel of the
//! raw design transpose. A partition is one block of disjoint columns, so
//! neither a sweep nor a certificate needs the expanded indicator matrix.
//! Floating projections only propose integers. Every accepted moment is exact.
mod cells;
use super::*;
use crate::ols::PreparedOlsColumnKind as Kind;
use exact_arithmetic::ExactSum;

#[derive(Clone, Copy)]
enum Values<'a> {
    One,
    Finite(usize),
    Slope(&'a [f64]),
}

#[derive(Clone, Copy)]
struct Family<'a> {
    codes: Option<&'a [u32]>,
    members: Option<&'a LevelIndex>,
    levels: usize,
    values: Values<'a>,
}

impl Family<'_> {
    fn level(self, r: usize) -> usize {
        self.codes.map_or(0, |codes| codes[r] as usize)
    }

    fn value(self, model: &PreparedPpmlModelGeneration, r: usize) -> Result<f64> {
        match self.values {
            Values::One => Ok(1.),
            Values::Finite(column) => model.fit_view().column_value(column, r),
            Values::Slope(x) => Ok(x[r]),
        }
    }

    /// The integer bound checked by the caller makes *all* intermediate
    /// indicator subtotals exact binary64 integers. Continuous moments use
    /// the full binary64 mantissa and exponent for every original row.
    fn balanced(
        self,
        model: &PreparedPpmlModelGeneration,
        weight: &impl Fn(usize) -> i64,
        sums: &mut [f64],
    ) -> Result<bool> {
        if matches!(self.values, Values::One) {
            let sums = &mut sums[..self.levels];
            sums.fill(0.);
            for r in 0..model.outcome().len() {
                crate::interrupt::row_checkpoint(r)?;
                sums[self.level(r)] += weight(r) as f64;
            }
            return Ok(sums.iter().all(|&s| s == 0.));
        }
        let check = |r: usize, sum: &mut ExactSum| -> Result<bool> {
            Ok(sum.add(self.value(model, r)?, weight(r)))
        };
        if let Some(groups) = self.members {
            for level in 0..self.levels {
                crate::interrupt::row_checkpoint(level)?;
                let mut sum = ExactSum::new();
                for (i, &r) in groups.checked_members(level)?.iter().enumerate() {
                    crate::interrupt::row_checkpoint(i)?;
                    if !check(r, &mut sum)? {
                        return Ok(false);
                    }
                }
                if !sum.zero() {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        let mut sum = ExactSum::new();
        for r in 0..model.outcome().len() {
            crate::interrupt::row_checkpoint(r)?;
            if !check(r, &mut sum)? {
                return Ok(false);
            }
        }
        Ok(sum.zero())
    }

    /// Orthogonal projection onto the moment equations of this block in
    /// observation space. The boundary lower bound is restored after a sweep.
    fn project(
        self,
        model: &PreparedPpmlModelGeneration,
        weights: &mut [f64],
        sums: &mut [f64],
        squares: &mut [f64],
    ) -> Result<bool> {
        // Projection is invariant to a nonzero common column scale.
        // Normalize before squaring to support tiny or very large raw units.
        let mut scale = 1.;
        if !matches!(self.values, Values::One) {
            scale = 0.0_f64;
            for r in 0..weights.len() {
                crate::interrupt::row_checkpoint(r)?;
                scale = scale.max(self.value(model, r)?.abs());
            }
            if scale == 0. {
                return Ok(true);
            }
        }
        let sums = &mut sums[..self.levels];
        let squares = &mut squares[..self.levels];
        sums.fill(0.);
        squares.fill(0.);
        for (r, &w) in weights.iter().enumerate() {
            crate::interrupt::row_checkpoint(r)?;
            let x = self.value(model, r)? / scale;
            let level = self.level(r);
            sums[level] += w * x;
            squares[level] += x * x;
        }
        for (s, &ss) in sums.iter_mut().zip(&*squares) {
            if !s.is_finite() || !ss.is_finite() {
                return Ok(false);
            }
            *s = if ss == 0. { 0. } else { *s / ss };
        }
        for (r, w) in weights.iter_mut().enumerate() {
            crate::interrupt::row_checkpoint(r)?;
            *w -= (self.value(model, r)? / scale) * sums[self.level(r)];
            if !w.is_finite() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

struct Design<'a> {
    families: Vec<Family<'a>>,
    levels: usize,
    pass: usize,
}

impl<'a> Design<'a> {
    fn new(
        model: &'a PreparedPpmlModelGeneration,
        topology: &DetectorTopology<'a>,
        limit: usize,
    ) -> Result<Option<(Self, usize)>> {
        let fit = model.fit_view();
        let columns = &model.model().columns;
        let scalar = columns
            .iter()
            .filter(|c| {
                matches!(
                    c.kind,
                    Kind::Continuous { .. } | Kind::CategoricalInteraction { .. }
                )
            })
            .count();
        let constant =
            fit.fit_structural_intercept() || columns.iter().any(|c| c.kind == Kind::Intercept);
        let capacity = topology.effect_specs.iter().fold(
            fit.categorical_sources()
                .len()
                .saturating_add(scalar)
                .saturating_add(usize::from(constant)),
            |sum, e| {
                sum.saturating_add(usize::from(e.intercept))
                    .saturating_add(e.slopes.len())
            },
        );
        let metadata = capacity.saturating_mul(std::mem::size_of::<Family>().div_ceil(8));
        if capacity == 0 || metadata >= limit {
            return Ok(None);
        }
        let mut families = Vec::new();
        families
            .try_reserve_exact(capacity)
            .map_err(|_| allocation("unable to reserve sparse dual column families"))?;
        // Checking all levels, including the reference and any excluded
        // levels, is a sufficient condition for every emitted indicator.
        for axis in fit.categorical_sources() {
            families.push(Family {
                codes: Some(axis.codes(fit.fe_topology())?),
                levels: axis.n_levels(fit.fe_topology())?,
                members: None,
                values: Values::One,
            });
        }
        for (spec, &source) in topology.effect_specs.iter().zip(topology.effect_to_source) {
            let members = topology.sources[source];
            let family = Family {
                codes: Some(codes(model, source)?),
                levels: members.n_levels(),
                members: Some(members),
                values: Values::One,
            };
            if spec.intercept {
                families.push(family);
            }
            for slope in &spec.slopes {
                families.push(Family {
                    values: Values::Slope(fit.numeric_source_values(slope.source)?),
                    ..family
                });
            }
        }
        if constant {
            families.push(Family {
                codes: None,
                levels: 1,
                members: None,
                values: Values::One,
            });
        }
        let mut cost = families.len();
        for (column, c) in columns.iter().enumerate() {
            let units = match &c.kind {
                Kind::Continuous { .. } => 1,
                Kind::CategoricalInteraction { components } => components.len().max(1),
                _ => continue,
            };
            cost = cost.saturating_add(units);
            families.push(Family {
                codes: None,
                levels: 1,
                members: None,
                values: Values::Finite(column),
            });
        }
        let levels = families.iter().map(|f| f.levels).max().unwrap_or(0);
        // Extra linear visits cover lower bounds, weight initialization/clipping,
        // norm checks and filling or traversing the per-level scratch.
        let pass = model
            .outcome()
            .len()
            .saturating_mul(cost.saturating_mul(2).saturating_add(4))
            .saturating_add(columns.len())
            .saturating_add(capacity);
        Ok(Some((
            Self {
                families,
                levels,
                pass,
            },
            limit - metadata,
        )))
    }

    fn certify(
        &self,
        model: &PreparedPpmlModelGeneration,
        weight: impl Fn(usize) -> i64,
        sums: &mut [f64],
        tolerance: f64,
    ) -> Result<bool> {
        let n = model.outcome().len();
        let max_abs = (1u64 << 53) / n as u64;
        let mut margin = i64::MAX;
        let mut norm2 = 0.;
        for (r, &y) in model.outcome().iter().enumerate() {
            crate::interrupt::row_checkpoint(r)?;
            let w = weight(r);
            if w.unsigned_abs() > max_abs {
                return Ok(false);
            }
            norm2 += (w as f64).powi(2);
            if y == 0. {
                margin = margin.min(w);
            }
        }
        if margin as f64 <= tolerance * norm2.sqrt() {
            return Ok(false);
        }
        for family in &self.families {
            if !family.balanced(model, &weight, sums)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Within each level of a single categorical axis, use the smallest integer
/// weights balancing total positive-outcome and boundary mass. A level with
/// only positive outcomes receives zero weight. All other axes and numerical
/// channels still require the independent exact replay.
fn partition_balance(
    model: &PreparedPpmlModelGeneration,
    design: &Design<'_>,
    axis: Family<'_>,
    sums: &mut [f64],
    tolerance: f64,
) -> Result<bool> {
    let y = model.outcome();
    let mut masses = fallible_filled(
        axis.levels,
        [0i64; 2],
        "unable to reserve sparse partition masses",
    )?;
    for (r, &value) in y.iter().enumerate() {
        crate::interrupt::row_checkpoint(r)?;
        masses[axis.level(r)][usize::from(value > 0.)] += 1;
    }
    for (g, w) in masses.iter_mut().enumerate() {
        crate::interrupt::row_checkpoint(g)?;
        let [boundary, positive] = *w;
        if boundary == 0 {
            *w = [0, 0];
            continue;
        }
        if positive == 0 {
            return Ok(false);
        }
        let (mut a, mut b) = (positive, boundary);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        *w = [positive / a, -(boundary / a)];
    }
    design.certify(
        model,
        |r| masses[axis.level(r)][usize::from(y[r] > 0.)],
        sums,
        tolerance,
    )
}

#[derive(Clone, Copy)]
pub(in crate::ppml::prepared_separation) enum Search {
    Marginal,
    Direct,
    Full,
}

impl Search {
    fn try_uniform(self, passes: usize, partition_fits: bool) -> bool {
        // A uniform positive dual on a complete indicator axis implies the
        // same positive/boundary ratio in every occupied level. Its primitive
        // weights then equal the partition weights on every row. A four-pass
        // full search can check both possibilities with that single candidate,
        // leaving two projection passes and one independent exact replay.
        !matches!(self, Self::Full) || !partition_fits || passes > 4
    }
}

pub(in crate::ppml::prepared_separation) fn supports(
    search: Search,
    model: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    limit: usize,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<bool> {
    let y = model.outcome();
    let boundary = y.len() - positive;
    if positive == 0 || boundary == 0 {
        return Ok(false);
    }
    let Some((design, limit)) = Design::new(model, topology, limit)? else {
        return Ok(false);
    };
    // Charge the bounded multiword stack accumulator as well as the heap
    // banks against a small configured detector workspace.
    let numeric = design
        .families
        .iter()
        .any(|f| !matches!(f.values, Values::One));
    let exact = if numeric {
        std::mem::size_of::<ExactSum>().div_ceil(8)
    } else {
        0
    };
    let Some(limit) = limit.checked_sub(exact) else {
        return Ok(false);
    };
    let attempts = config
        .simplex_max_iterations
        .saturating_mul(10)
        .saturating_add(2);
    let mut work = primal::ProofBudget::sparse_work(design.pass, attempts);
    if design.levels > limit || work < design.pass {
        return Ok(false);
    }
    interrupt(PpmlInterruptPoint::SeparationSimplex)?;
    let mut sums = fallible_filled(design.levels, 0., "unable to reserve sparse dual moments")?;
    let axis = if !matches!(search, Search::Direct)
        && topology
            .effect_specs
            .iter()
            .all(|spec| spec.slopes.is_empty())
    {
        design
            .families
            .iter()
            .filter(|f| f.codes.is_some())
            .max_by_key(|f| f.levels)
            .filter(|f| f.levels.saturating_mul(2) <= limit - design.levels)
            .copied()
    } else {
        None
    };
    if search.try_uniform(work / design.pass, axis.is_some()) {
        let mut p = positive;
        let mut b = boundary;
        while b != 0 {
            (p, b) = (b, p % b);
        }
        let initial = |r: usize| {
            if y[r] == 0. {
                (positive / p) as i64
            } else {
                -((boundary / p) as i64)
            }
        };
        if design.certify(model, initial, &mut sums, config.certificate_tolerance)? {
            trace(model, "uniform");
            return Ok(true);
        }
        work -= design.pass;
    }
    if let Some(axis) = axis.filter(|_| work >= design.pass) {
        if partition_balance(
            model,
            &design,
            axis,
            &mut sums,
            config.certificate_tolerance,
        )? {
            trace(model, "partition");
            return Ok(true);
        }
        work -= design.pass;
    }
    if matches!(search, Search::Marginal) {
        return Ok(false);
    }
    if numeric {
        // Preserve room for a whole projection and its replay. The absolute
        // exploration cap applies to hashing probes and nonlinear local elimination.
        let budget = work
            .saturating_sub(3usize.saturating_mul(design.pass))
            .min(primal::ProofBudget::MAX_SPARSE_WORK);
        let budget = match search {
            // The pre-rectifier probe spends at most linear work on local
            // systems; larger cells remain available to the full fallback.
            Search::Direct => budget.min(design.pass.saturating_mul(8)),
            Search::Full | Search::Marginal => budget,
        };
        let mut local = budget;
        if cells::prove(
            model,
            &design,
            limit - design.levels,
            &mut local,
            config.certificate_tolerance,
        )? {
            trace(model, "categorical_cells");
            return Ok(true);
        }
        work -= budget - local;
    }
    if matches!(search, Search::Direct) {
        return Ok(false);
    }
    projected_support(model, &design, &mut sums, limit, work, config, interrupt)
}

/// Explore the residual cone by alternating projections. Only the exact
/// original-coordinate replay of a proposed integer multiplier can close it.
fn projected_support(
    model: &PreparedPpmlModelGeneration,
    design: &Design<'_>,
    sums: &mut [f64],
    limit: usize,
    work: usize,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<bool> {
    let y = model.outcome();
    let mut passes = work / design.pass.max(1);
    if design.levels.saturating_mul(2).saturating_add(y.len()) > limit || passes < 3 {
        return Ok(false);
    }
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=sparse_dual_projection_start|generation={}|rows={}|passes={passes}",
            model.generation(), y.len(),
        ));
    }
    let mut squares = fallible_filled(
        design.levels,
        0.,
        "unable to reserve sparse dual squared norms",
    )?;
    let mut weights = fallible_collect(
        (0..y.len()).map(|r| if y[r] == 0. { 1. } else { -1. }),
        y.len(),
        "unable to reserve sparse dual observation weights",
    )?;
    for _ in 0..config.simplex_max_iterations {
        if passes < 3 {
            break;
        }
        interrupt(PpmlInterruptPoint::SeparationSimplex)?;
        passes -= 2;
        for family in &design.families {
            if !family.project(model, &mut weights, sums, &mut squares)? {
                return Ok(false);
            }
        }
        let mut margin = f64::INFINITY;
        for (r, &w) in weights.iter().enumerate() {
            crate::interrupt::row_checkpoint(r)?;
            if y[r] == 0. {
                margin = margin.min(w);
            }
        }
        if margin > 0. {
            // Scale invariance permits clearing small common denominators.
            // Exact replay, not the rounding or proximity, is decisive.
            for denominator in [1., 2., 3., 4., 6., 8., 12., 24.] {
                if passes == 0 {
                    break;
                }
                passes -= 1;
                let scale = denominator / margin;
                let rounded = |r: usize| (weights[r] * scale).round() as i64;
                if design.certify(model, rounded, sums, config.certificate_tolerance)? {
                    trace(model, "projected");
                    return Ok(true);
                }
            }
        }
        for (r, w) in weights.iter_mut().enumerate() {
            crate::interrupt::row_checkpoint(r)?;
            if y[r] == 0. {
                *w = w.max(1.);
            }
        }
    }
    Ok(false)
}

fn trace(model: &PreparedPpmlModelGeneration, proposal: &str) {
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=sparse_positive_dual|generation={}|rows={}|proposal={proposal}",
            model.generation(), model.outcome().len(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_full_scans_keep_a_projection_and_exact_replay_with_or_without_an_axis() {
        let pass = primal::ProofBudget::MAX_SPARSE_WORK / 4 + 1;
        let scans = primal::ProofBudget::sparse_work(pass, 100) / pass;
        assert_eq!(scans, 4);
        for partition in [false, true] {
            let uniform = Search::Full.try_uniform(scans, partition);
            assert_eq!(scans - usize::from(uniform) - usize::from(partition), 3);
        }
        assert!(Search::Marginal.try_uniform(scans, true));
        assert!(Search::Full.try_uniform(5, true));
    }
}
