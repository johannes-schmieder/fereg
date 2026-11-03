//! Exceptional-column quotient coordinates and a graph forward-error witness.
//!
//! Negating the second categorical effect gives an incidence matrix A. For
//! g=A'Wr, route each subtree's sum of g along its parent edge. The flow f
//! satisfies A'f=g, so ||P_W r||_W <= ||W^(-1/2) f||. Rounded node and tree sums
//! are enclosed; root imbalances never replace the exact zero component sum.
//! This bound can be loose on weak graphs and may refuse a fit. W consists of
//! represented positive NormalizedWeights::value values, as elsewhere in the
//! numerical frame; this is not a certificate for exact rational division.
use super::*;
use crate::fe::BorrowedInterceptFeDesign;

const NONE: usize = usize::MAX;
const TINY: f64 = f64::from_bits(1);

pub(super) struct Deflated {
    pub(super) values: DenseMatrix,
    /// Actual additive FE coefficients in effect-major level order. For an
    /// empty explicit design this contains the global-intercept coefficient.
    pub(super) gauge: Vec<f64>,
    pub(super) roundoff_bound: f64,
    pub(super) exact_absorbed: bool,
}

pub(super) struct Reconstructed {
    pub(super) values: DenseMatrix,
    pub(super) roundoff_bound: f64,
}

/// Sufficient dependence proof using equality of original input values only.
/// An unsuccessful test says nothing about membership in the joint FE span.
pub(super) fn structurally_absorbed(
    design: &BorrowedInterceptFeDesign<'_>,
    mut value: impl FnMut(usize) -> Result<f64>,
) -> Result<Option<usize>> {
    dimensions(design)?;
    if design.effects().is_empty() {
        let first = finite(value(0)?)?;
        for row in 0..design.nobs() {
            checkpoint(row)?;
            if finite(value(row)?)? != first {
                return Ok(None);
            }
        }
        return Ok(Some(0));
    }
    for (effect_index, effect) in design.effects().iter().enumerate() {
        let mut constant = true;
        for level in 0..effect.n_levels() {
            checkpoint(level)?;
            let members = effect.level_index().members(level);
            if let Some(&first_row) = members.first() {
                let first = finite(value(first_row)?)?;
                for (position, &row) in members.iter().enumerate() {
                    checkpoint(position)?;
                    if finite(value(row)?)? != first {
                        constant = false;
                        break;
                    }
                }
                if !constant {
                    break;
                }
            }
        }
        if constant {
            return Ok(Some(effect_index));
        }
    }
    Ok(None)
}

pub(super) struct Forest<'d, 'a> {
    design: &'d BorrowedInterceptFeDesign<'a>,
    left: usize,
    parent: Vec<usize>,
    parent_row: Vec<usize>,
    order: Vec<usize>,
    potential: Vec<f64>,
    moments: Vec<Ball>,
}

impl<'d, 'a> Forest<'d, 'a> {
    /// Helper envelope: forest banks, an exported gauge, and two simultaneous
    /// single-column row banks. Charge the caller's retained projection and
    /// the FE solver workspace separately.
    pub(super) fn workspace_bytes(design: &BorrowedInterceptFeDesign<'_>) -> Result<u128> {
        let (left, nodes) = dimensions(design)?;
        let gauge = if design.effects().len() == 2 {
            nodes
        } else {
            left
        };
        Ok(
            nodes as u128 * (3 * std::mem::size_of::<usize>() + 24) as u128
                + gauge as u128 * 8
                + design.nobs() as u128 * 16,
        )
    }

    pub(super) fn new<R: FitReservation>(
        design: &'d BorrowedInterceptFeDesign<'a>,
        reservation: &mut R,
    ) -> Result<Self> {
        let (left, nodes) = dimensions(design)?;
        let mut result = Self {
            design,
            left,
            parent: reserved_filled(reservation, nodes, NONE, "rank repair forest parents")?,
            parent_row: reserved_filled(reservation, nodes, NONE, "rank repair forest rows")?,
            order: reserved_empty(reservation, nodes, "rank repair forest traversal")?,
            potential: reserved_filled(reservation, nodes, 0., "rank repair forest potentials")?,
            moments: reserved_filled(
                reservation,
                nodes,
                Ball::default(),
                "rank repair forest moments",
            )?,
        };
        // Ground one-effect graphs at the artificial right node. Their gauges
        // are consequently original level anchors without cancellation.
        if design.effects().len() < 2 {
            result.parent[left] = left;
            result.order.push(left);
            for row in 0..design.nobs() {
                checkpoint(row)?;
                let (node, _) = result.endpoints(row);
                result.discover(node, left, row);
            }
        } else {
            for root in 0..nodes {
                checkpoint(root)?;
                if result.parent[root] != NONE {
                    continue;
                }
                let mut cursor = result.order.len();
                result.parent[root] = root;
                result.order.push(root);
                while cursor < result.order.len() {
                    let node = result.order[cursor];
                    cursor += 1;
                    checkpoint(cursor)?;
                    let (effect, level) = if node < left {
                        (0, node)
                    } else {
                        (1, node - left)
                    };
                    for (position, &row) in design.effects()[effect]
                        .level_index()
                        .members(level)
                        .iter()
                        .enumerate()
                    {
                        checkpoint(position)?;
                        let (a, b) = result.endpoints(row);
                        result.discover(if node == a { b } else { a }, node, row);
                    }
                }
            }
        }
        Ok(result)
    }

    fn discover(&mut self, node: usize, parent: usize, row: usize) {
        if self.parent[node] == NONE {
            self.parent[node] = parent;
            self.parent_row[node] = row;
            self.order.push(node);
        }
    }

    fn endpoints(&self, row: usize) -> (usize, usize) {
        let effects = self.design.effects();
        let left = effects
            .first()
            .map_or(0, |effect| effect.codes()[row] as usize);
        let right = if effects.len() == 2 {
            self.left + effects[1].codes()[row] as usize
        } else {
            self.left
        };
        (left, right)
    }

    pub(super) fn deflate<R: FitReservation>(
        &mut self,
        mut raw: Vec<f64>,
        weights: NormalizedWeights<'_>,
        reservation: &mut R,
    ) -> Result<Deflated> {
        validate_rows(self.design, &raw)?;
        self.potential.fill(0.);
        for (position, &node) in self.order.iter().enumerate() {
            checkpoint(position)?;
            let parent = self.parent[node];
            if parent == node {
                continue;
            }
            let value = raw[self.parent_row[node]];
            self.potential[node] = finite(if node < self.left {
                self.potential[parent] + value
            } else {
                self.potential[parent] - value
            })?;
        }
        let count = if self.design.effects().len() == 2 {
            self.parent.len()
        } else {
            self.left
        };
        let mut gauge = reserved_filled(reservation, count, 0., "rank repair exported gauge")?;
        for (node, value) in gauge.iter_mut().enumerate() {
            *value = if node < self.left {
                self.potential[node]
            } else {
                -self.potential[node]
            };
        }
        let (roundoff_bound, exact_absorbed) = self.subtract(&mut raw, &gauge, weights)?;
        Ok(Deflated {
            values: DenseMatrix::from_row_major(raw.len(), 1, raw)?,
            gauge,
            roundoff_bound,
            exact_absorbed,
        })
    }

    /// Rebuild a feasible residual from shifted input and returned FE
    /// coefficients. Projector residual rows deliberately are not an input.
    pub(super) fn reconstruct<R: FitReservation>(
        &self,
        shifted: &[f64],
        coefficients: &[f64],
        weights: NormalizedWeights<'_>,
        reservation: &mut R,
    ) -> Result<Reconstructed> {
        validate_rows(self.design, shifted)?;
        let mut values = reserved_empty(
            reservation,
            shifted.len(),
            "rank repair reconstructed residual",
        )?;
        values.extend_from_slice(shifted);
        let (roundoff_bound, _) = self.subtract(&mut values, coefficients, weights)?;
        Ok(Reconstructed {
            values: DenseMatrix::from_row_major(values.len(), 1, values)?,
            roundoff_bound,
        })
    }

    fn subtract(
        &self,
        values: &mut [f64],
        coefficients: &[f64],
        weights: NormalizedWeights<'_>,
    ) -> Result<(f64, bool)> {
        let count = if self.design.effects().len() == 2 {
            self.parent.len()
        } else {
            self.left
        };
        if coefficients.len() != count || coefficients.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument(
                "rank repair FE coefficient axes are inconsistent",
            ));
        }
        let mut errors = BoundNorm::default();
        let mut exact_absorbed = true;
        for (row, value) in values.iter_mut().enumerate() {
            checkpoint(row)?;
            let (left, right) = self.endpoints(row);
            let second = if self.design.effects().len() == 2 {
                coefficients[right]
            } else {
                0.
            };
            let residual = subtract_three(*value, coefficients[left], second)?;
            *value = residual.value;
            exact_absorbed &= residual.value == 0. && residual.radius == 0.;
            errors.add(mul_up(residual.radius, sqrt_up(weights.value(row)?)?)?)?;
        }
        Ok((errors.finish()?, exact_absorbed))
    }

    pub(super) fn flow_error(
        &mut self,
        residual: &[f64],
        weights: NormalizedWeights<'_>,
    ) -> Result<f64> {
        validate_rows(self.design, residual)?;
        self.moments.fill(Ball::default());
        for (row, &value) in residual.iter().enumerate() {
            checkpoint(row)?;
            let moment = Ball::product(weights.value(row)?, value)?;
            let (left, right) = self.endpoints(row);
            self.moments[left].add(moment)?;
            self.moments[right].add(Ball {
                value: -moment.value,
                ..moment
            })?;
        }
        let mut norm = BoundNorm::default();
        for (position, &node) in self.order.iter().rev().enumerate() {
            checkpoint(position)?;
            let parent = self.parent[node];
            if parent == node {
                continue;
            }
            let moment = self.moments[node];
            let magnitude = add_up(moment.value.abs(), moment.radius)?;
            let weight_root = sqrt_down(weights.value(self.parent_row[node])?)?;
            norm.add(div_up(magnitude, weight_root)?)?;
            self.moments[parent].add(moment)?;
        }
        norm.finish()
    }
}

fn dimensions(design: &BorrowedInterceptFeDesign<'_>) -> Result<(usize, usize)> {
    if design.nobs() == 0 || design.effects().len() > 2 {
        return Err(invalid_argument(
            "rank repair requires one or two categorical partitions or a global intercept",
        ));
    }
    let left = design
        .effects()
        .first()
        .map_or(1, |effect| effect.n_levels());
    let right = design
        .effects()
        .get(1)
        .map_or(1, |effect| effect.n_levels());
    Ok((
        left,
        left.checked_add(right)
            .ok_or_else(|| allocation("rank repair forest dimensions overflow"))?,
    ))
}

fn validate_rows(design: &BorrowedInterceptFeDesign<'_>, values: &[f64]) -> Result<()> {
    dimensions(design)?;
    if values.len() != design.nobs() {
        return Err(invalid_argument(
            "rank repair rows are not finite and aligned",
        ));
    }
    for (row, value) in values.iter().enumerate() {
        checkpoint(row)?;
        if !value.is_finite() {
            return Err(invalid_argument(
                "rank repair rows are not finite and aligned",
            ));
        }
    }
    Ok(())
}

fn checkpoint(position: usize) -> Result<()> {
    if position % 4096 == 0 {
        crate::interrupt::checkpoint()?;
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct Ball {
    value: f64,
    radius: f64,
}

impl Ball {
    fn product(left: f64, right: f64) -> Result<Self> {
        let value = finite(left * right)?;
        if left == 0. || right == 0. || left == 1. || right == 1. {
            return Ok(Self { value, radius: 0. });
        }
        // A fused product residual is exact unless it underflows. One least
        // subnormal bounds that case, including a product rounded to zero.
        let tail = finite(left.mul_add(right, -value))?;
        Ok(Self {
            value,
            radius: add_up(tail.abs(), TINY)?,
        })
    }
    fn add(&mut self, other: Self) -> Result<()> {
        let (value, tail) = two_sum(self.value, other.value)?;
        self.radius = add_up(add_up(self.radius, other.radius)?, tail.abs())?;
        self.value = value;
        Ok(())
    }
}

fn subtract_three(raw: f64, left: f64, right: f64) -> Result<Ball> {
    let (first, e1) = two_sum(raw, -left)?;
    let (second, e2) = two_sum(first, -right)?;
    let (tail, e3) = two_sum(e1, e2)?;
    let (value, e4) = two_sum(second, tail)?;
    Ok(Ball {
        value,
        radius: add_up(e3.abs(), e4.abs())?,
    })
}

fn two_sum(a: f64, b: f64) -> Result<(f64, f64)> {
    let sum = finite(a + b)?;
    let part = finite(sum - a)?;
    let error = finite((a - (sum - part)) + (b - part))?;
    Ok((sum, error))
}

fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid_data("rank repair numerical accuracy is unresolved"))
    }
}

fn up(value: f64) -> Result<f64> {
    finite(if value == 0. {
        TINY
    } else {
        f64::from_bits(value.to_bits() + 1)
    })
}

pub(super) fn add_up(a: f64, b: f64) -> Result<f64> {
    if a == 0. {
        return Ok(b);
    }
    if b == 0. {
        return Ok(a);
    }
    up(finite(a + b)?)
}
fn mul_up(a: f64, b: f64) -> Result<f64> {
    if a == 0. || b == 0. {
        return Ok(0.);
    }
    up(finite(a * b)?)
}
fn div_up(a: f64, b: f64) -> Result<f64> {
    if a == 0. {
        return Ok(0.);
    }
    up(finite(a / b)?)
}
fn sqrt_up(value: f64) -> Result<f64> {
    if value == 0. {
        Ok(0.)
    } else {
        up(value.sqrt())
    }
}
fn sqrt_down(value: f64) -> Result<f64> {
    let root = finite(value.sqrt())?;
    let result = f64::from_bits(root.to_bits() - 1);
    if result > 0. {
        Ok(result)
    } else {
        Err(invalid_data(
            "rank repair positive weight cannot be resolved",
        ))
    }
}

#[derive(Default)]
struct BoundNorm {
    scale: f64,
    squares: f64,
}
impl BoundNorm {
    fn add(&mut self, value: f64) -> Result<()> {
        if value == 0. {
            return Ok(());
        }
        if self.scale < value {
            let ratio = div_up(self.scale, value)?;
            self.squares = add_up(1., mul_up(self.squares, mul_up(ratio, ratio)?)?)?;
            self.scale = value;
        } else {
            let ratio = div_up(value, self.scale)?;
            self.squares = add_up(self.squares, mul_up(ratio, ratio)?)?;
        }
        Ok(())
    }
    fn finish(self) -> Result<f64> {
        mul_up(self.scale, sqrt_up(self.squares)?)
    }
}

#[cfg(test)]
mod tests;
