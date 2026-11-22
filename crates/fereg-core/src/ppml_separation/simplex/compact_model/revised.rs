//! Low-memory phase I for the valued cone when its row bank does not fit.
//!
//! The problem is the transposed phase I of `compact_tableau`: for boundary
//! rows `D_i` (projected on the positive rows' null space), seek `u >= 0`
//! with `sum_i u_i D_i = -sum_i D_i`, so that `1 + u` are positive balancing
//! weights; an infeasible phase I supplies `y` with `D_i y <= 0` and one
//! strict inequality. Equations are scaled by powers of two and signed as in
//! the tableau. Instead of a tableau over every distinct row, the revised
//! method keeps the `m x m` basis inverse and projects rows on demand: each
//! pricing pass streams the boundary rows once through `scores`, and only the
//! entering row is projected. Storage is `m^2` plus a few vectors of length
//! `m` and the raw width, independent of the boundary row count. The pivot
//! rules, the tolerance and the measured-progress stop are the tableau's.
//! Every result is a proposal that the caller certifies.
use super::super::compact_tableau::Proposal;
use super::fallible_filled;
use crate::error::Result;
use crate::linalg::neumaier_add;

/// Writes `D_i` into the output; false when the projection is refused.
pub(crate) type Project<'a> = &'a mut dyn FnMut(usize, &mut [f64]) -> Result<bool>;
/// Emits `(i, D_i . y)` for every boundary row in order.
pub(crate) type Scores<'a> = &'a mut dyn FnMut(&[f64], &mut dyn FnMut(usize, f64)) -> Result<bool>;

/// Projected boundary rows, produced on demand.
pub(crate) struct Rows<'a> {
    pub(crate) count: usize,
    pub(crate) dimension: usize,
    pub(crate) project: Project<'a>,
    pub(crate) scores: Scores<'a>,
}

/// Words the revised phase I holds besides the caller's projector.
pub(crate) fn workspace_words(boundary: usize, dimension: usize) -> Option<usize> {
    dimension
        .checked_mul(dimension)?
        .checked_add(dimension.checked_mul(8)?)?
        .checked_add(boundary)
}

struct State {
    m: usize,
    boundary: usize,
    /// Basis inverse, row-major.
    inverse: Vec<f64>,
    /// Basic variable per row: `< boundary` structural, else artificial.
    basic: Vec<usize>,
    values: Vec<f64>,
    scales: Vec<f64>,
}

impl State {
    /// `pi = c_B' B^-1` for unit cost on basic artificials.
    fn prices(&self, pi: &mut [f64]) {
        pi.fill(0.);
        for (row, &b) in self.inverse.chunks_exact(self.m).zip(&self.basic) {
            if b >= self.boundary {
                for (p, &v) in pi.iter_mut().zip(row) {
                    *p += v;
                }
            }
        }
    }

    fn objective(&self) -> f64 {
        self.values
            .iter()
            .zip(&self.basic)
            .filter(|&(_, &b)| b >= self.boundary)
            .map(|(&v, _)| v)
            .sum()
    }

    /// The tableau's ratio test on the entering column `alpha = B^-1 a`.
    fn leaving(&self, alpha: &[f64], tolerance: f64) -> Option<(usize, f64)> {
        let mut best = None;
        let (mut best_ratio, mut best_index) = (f64::INFINITY, usize::MAX);
        for (k, (&a, (&x, &b))) in alpha
            .iter()
            .zip(self.values.iter().zip(&self.basic))
            .enumerate()
        {
            if x < -tolerance {
                return None;
            }
            if a > tolerance {
                let ratio = x.max(0.) / a;
                if ratio.is_finite()
                    && (ratio < best_ratio || (ratio == best_ratio && b < best_index))
                {
                    best = Some(k);
                    best_ratio = ratio;
                    best_index = b;
                }
            }
        }
        best.map(|k| (k, best_ratio))
    }

    fn pivot(&mut self, leaving: usize, alpha: &[f64], entering: usize) -> bool {
        let m = self.m;
        let diagonal = alpha[leaving];
        let (before, tail) = self.inverse.split_at_mut(leaving * m);
        let (pivot, after) = tail.split_at_mut(m);
        for v in pivot.iter_mut() {
            *v /= diagonal;
        }
        self.values[leaving] /= diagonal;
        let x = self.values[leaving];
        for (k, row) in before
            .chunks_exact_mut(m)
            .chain(after.chunks_exact_mut(m))
            .enumerate()
        {
            let k = if k < leaving { k } else { k + 1 };
            let factor = alpha[k];
            if factor != 0. {
                for (v, &p) in row.iter_mut().zip(pivot.iter()) {
                    *v -= factor * p;
                }
                self.values[k] -= factor * x;
            }
        }
        self.basic[leaving] = entering;
        self.inverse.iter().all(|v| v.is_finite()) && self.values.iter().all(|v| v.is_finite())
    }
}

/// Power-of-two equation scales and the signed right side, from one pass.
fn initialize(
    rows: &mut Rows<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<State>> {
    let (m, boundary) = (rows.dimension, rows.count);
    let mut maxima = fallible_filled(m, 0f64, "unable to reserve revised simplex scales")?;
    let mut sums = fallible_filled(m, 0f64, "unable to reserve revised simplex sums")?;
    let mut corrections = fallible_filled(m, 0f64, "unable to reserve revised simplex sums")?;
    let mut row = fallible_filled(m, 0f64, "unable to reserve revised simplex row")?;
    for i in 0..boundary {
        if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        if !(rows.project)(i, &mut row)? || row.iter().any(|v| !v.is_finite()) {
            return Ok(None);
        }
        for (k, &v) in row.iter().enumerate() {
            maxima[k] = maxima[k].max(v.abs());
        }
    }
    let mut scales = fallible_filled(m, 1f64, "unable to reserve revised simplex scales")?;
    for (scale, &maximum) in scales.iter_mut().zip(&maxima) {
        let bits = maximum.to_bits();
        *scale = if bits == 0 {
            1.
        } else if bits < (1 << 52) {
            f64::from_bits(1 << (63 - bits.leading_zeros()))
        } else {
            f64::from_bits(bits & 0x7ff0_0000_0000_0000)
        };
    }
    for i in 0..boundary {
        if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        if !(rows.project)(i, &mut row)? {
            return Ok(None);
        }
        for k in 0..m {
            neumaier_add(row[k] / scales[k], &mut sums[k], &mut corrections[k]);
        }
    }
    let mut values = fallible_filled(m, 0f64, "unable to reserve revised simplex values")?;
    for k in 0..m {
        let rhs = -(sums[k] + corrections[k]);
        if rhs < 0. {
            scales[k] = -scales[k];
        }
        values[k] = rhs.abs();
    }
    let mut inverse = fallible_filled(m * m, 0f64, "unable to reserve revised simplex inverse")?;
    for k in 0..m {
        inverse[k * m + k] = 1.;
    }
    let mut basic = fallible_filled(m, 0usize, "unable to reserve revised simplex basis")?;
    for (k, b) in basic.iter_mut().enumerate() {
        *b = boundary + k;
    }
    Ok(Some(State {
        m,
        boundary,
        inverse,
        basic,
        values,
        scales,
    }))
}

/// Positive weights `1 + u` from a feasible basis, checked axis by axis as
/// the tableau does.
fn positive_weights(
    rows: &mut Rows<'_>,
    state: &State,
    tolerance: f64,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Vec<f64>>> {
    let (m, boundary) = (state.m, state.boundary);
    let mut weights = fallible_filled(boundary, 1., "unable to reserve revised simplex weights")?;
    for (&x, &b) in state.values.iter().zip(&state.basic) {
        if x < -tolerance {
            return Ok(None);
        }
        if b < boundary {
            weights[b] += x.max(0.);
        }
    }
    let largest = weights.iter().copied().fold(1., f64::max);
    let mut sums = fallible_filled(3 * m, 0f64, "unable to reserve revised simplex check")?;
    let mut row = fallible_filled(m, 0f64, "unable to reserve revised simplex row")?;
    for (i, &w) in weights.iter().enumerate() {
        if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        if !(rows.project)(i, &mut row)? {
            return Ok(None);
        }
        for (k, &v) in row.iter().enumerate() {
            let term = (v / state.scales[k]) * (w / largest);
            let (sum, rest) = sums.split_at_mut(m);
            let (correction, magnitude) = rest.split_at_mut(m);
            neumaier_add(term, &mut sum[k], &mut correction[k]);
            magnitude[k] += term.abs();
        }
    }
    for k in 0..m {
        if (sums[k] + sums[m + k]).abs() > tolerance * sums[2 * m + k] {
            return Ok(None);
        }
    }
    Ok(Some(weights))
}

/// The Farkas direction `y` with `D_i y <= 0` from the final prices,
/// normalized and checked on every row as the tableau does.
fn direction(
    rows: &mut Rows<'_>,
    state: &State,
    pi: &[f64],
    tolerance: f64,
) -> Result<Option<Vec<f64>>> {
    let lambda = pi.iter().fold(0f64, |a, &v| a.max(v.abs()));
    if lambda == 0. {
        return Ok(None);
    }
    let smallest = pi
        .iter()
        .zip(&state.scales)
        .filter_map(|(&v, &s)| (v != 0.).then_some(s.abs()))
        .fold(f64::INFINITY, f64::min);
    let mut coordinates =
        fallible_filled(state.m, 0., "unable to reserve revised simplex direction")?;
    for ((c, &v), &s) in coordinates.iter_mut().zip(pi).zip(&state.scales) {
        if v != 0. {
            *c = (v / lambda) * (smallest / s);
        }
    }
    let (mut negative, mut positive, mut finite) = (0f64, 0f64, true);
    if !(rows.scores)(&coordinates, &mut |_, score| {
        finite &= score.is_finite();
        negative = negative.max(-score);
        positive = positive.max(score);
    })? {
        return Ok(None);
    }
    Ok((finite && negative > 0. && positive <= tolerance * negative).then_some(coordinates))
}

/// Revised phase I; `(tolerance, max_iterations, per_dimension)` as in the
/// tableau. Returns `None` when it cannot propose.
pub(crate) fn propose(
    rows: &mut Rows<'_>,
    (tolerance, max_iterations, per_dimension): (f64, usize, usize),
    pivots: &mut usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Proposal>> {
    let (m, boundary) = (rows.dimension, rows.count);
    if m == 0 || boundary == 0 || !(0.0..1.0).contains(&tolerance) || tolerance == 0. {
        return Ok(None);
    }
    let Some(mut state) = initialize(rows, interrupt)? else {
        return Ok(None);
    };
    let mut pivot_limit = max_iterations.saturating_sub(*pivots);
    if per_dimension > 0 {
        pivot_limit = pivot_limit.min(per_dimension.saturating_mul(m));
    }
    let mut pi = fallible_filled(m, 0f64, "unable to reserve revised simplex prices")?;
    let mut y = fallible_filled(m, 0f64, "unable to reserve revised simplex prices")?;
    let mut alpha = fallible_filled(m, 0f64, "unable to reserve revised simplex column")?;
    let mut column = fallible_filled(m, 0f64, "unable to reserve revised simplex column")?;
    let mut is_basic = fallible_filled(boundary, false, "unable to reserve revised simplex marks")?;
    let (mut degenerate, mut bland) = (0usize, false);
    for iteration in 0..=pivot_limit {
        interrupt()?;
        state.prices(&mut pi);
        if pi.iter().any(|v| !v.is_finite()) {
            return Ok(None);
        }
        if state.objective().abs() <= tolerance {
            if let Some(weights) = positive_weights(rows, &state, tolerance, interrupt)? {
                return Ok(Some(Proposal::PositiveWeights {
                    weights,
                    iterations: *pivots,
                }));
            }
        }
        for ((dest, &p), &s) in y.iter_mut().zip(&pi).zip(&state.scales) {
            *dest = p / s;
        }
        // Largest reduced cost over nonbasic rows; Bland takes the first.
        let mut entering: Option<usize> = None;
        let mut largest = tolerance;
        let mut finite = true;
        let priced = (rows.scores)(&y, &mut |i, score| {
            finite &= score.is_finite();
            if !is_basic[i] && score > largest && !(bland && entering.is_some()) {
                entering = Some(i);
                largest = score;
            }
        })?;
        if !priced || !finite {
            return Ok(None);
        }
        let Some(q) = entering else {
            return Ok(direction(rows, &state, &pi, tolerance)?.map(|coordinates| {
                Proposal::Direction {
                    coordinates,
                    iterations: *pivots,
                }
            }));
        };
        if !(rows.project)(q, &mut column)? {
            return Ok(None);
        }
        for (c, &s) in column.iter_mut().zip(&state.scales) {
            *c /= s;
        }
        for (a, row) in alpha.iter_mut().zip(state.inverse.chunks_exact(m)) {
            *a = row.iter().zip(&column).map(|(&b, &c)| b * c).sum();
        }
        let Some((leaving, step)) = state.leaving(&alpha, tolerance) else {
            return Ok(None);
        };
        if iteration == pivot_limit {
            super::trace_refusal("revised_pivot_limit", pivot_limit, m);
            return Ok(None);
        }
        *pivots += 1;
        let out = state.basic[leaving];
        if !state.pivot(leaving, &alpha, q) {
            return Ok(None);
        }
        if out < boundary {
            is_basic[out] = false;
        }
        is_basic[q] = true;
        if step <= tolerance {
            degenerate += 1;
            bland |= degenerate >= m;
            if per_dimension > 0 && degenerate >= 2 * m {
                super::trace_refusal("revised_stalled", degenerate, m);
                return Ok(None);
            }
        } else {
            degenerate = 0;
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(cone: &[Vec<f64>]) -> Option<Proposal> {
        let m = cone[0].len();
        let mut project = |i: usize, out: &mut [f64]| -> Result<bool> {
            out.copy_from_slice(&cone[i]);
            Ok(true)
        };
        let mut scores = |y: &[f64], emit: &mut dyn FnMut(usize, f64)| -> Result<bool> {
            for (i, row) in cone.iter().enumerate() {
                emit(i, row.iter().zip(y).map(|(&a, &b)| a * b).sum());
            }
            Ok(true)
        };
        let mut rows = Rows {
            count: cone.len(),
            dimension: m,
            project: &mut project,
            scores: &mut scores,
        };
        propose(&mut rows, (1e-10, 64, 0), &mut 0, &mut || Ok(())).unwrap()
    }

    #[test]
    fn matches_the_tableau_on_balanced_and_separated_cones() {
        for scale in [2f64.powi(-100), 1., 2f64.powi(100)] {
            let separated = vec![vec![1., -scale], vec![-1., scale], vec![2., -scale]];
            let Some(Proposal::Direction { coordinates, .. }) = run(&separated) else {
                panic!("expected a Farkas direction");
            };
            let scores: Vec<f64> = separated
                .iter()
                .map(|r| r[0] * coordinates[0] + r[1] * coordinates[1])
                .collect();
            assert!(scores[2] < 0.);
            assert!(scores[..2].iter().all(|&v| v.abs() <= -1e-12 * scores[2]));
        }
        let balanced = vec![vec![1., 0.], vec![-1., 1.], vec![0., -1.], vec![2., -2.]];
        let Some(Proposal::PositiveWeights { weights, .. }) = run(&balanced) else {
            panic!("expected positive weights");
        };
        assert!(weights.iter().all(|&w| w >= 1.));
        for k in 0..2 {
            let sum: f64 = balanced.iter().zip(&weights).map(|(r, &w)| r[k] * w).sum();
            assert!(sum.abs() < 1e-9, "axis {k}: {sum}");
        }
    }
}
