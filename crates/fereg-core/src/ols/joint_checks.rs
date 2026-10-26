//! Fresh original-input checks after a joint PPML frame.
//!
//! Rows reduce over [`crate::parallel::fixed_tree_reduce`] leaves: each leaf
//! keeps row order, and the leaf tree depends only on the row count, so every
//! team width merges identical partials in the same order.
use super::*;

const LEAF_ROWS: usize = 8192;
/// Weighted squares in this range neither overflow nor lose a term that is
/// relevant beside the largest one; other leaves use scaled square roots.
const DIRECT_MIN: f64 = 1.0e-280;
const DIRECT_MAX: f64 = 1.0e280;

/// One compensated step. Knuth's branch-free TwoSum yields the same exact
/// rounding error as Neumaier's ordered form; finiteness is checked once per
/// leaf.
#[inline]
fn neumaier(value: f64, sum: &mut f64, correction: &mut f64) {
    let total = *sum + value;
    let partner = total - *sum;
    *correction += (*sum - (total - partner)) + (value - partner);
    *sum = total;
}

/// A sum of squares `scale^2 * square` (scale zero is the empty sum).
#[derive(Clone, Copy)]
struct Squares {
    scale: f64,
    square: f64,
}

impl Squares {
    const EMPTY: Self = Self {
        scale: 0.,
        square: 1.,
    };

    /// A leaf from its largest weighted square and their direct sum, or the
    /// scaled recurrence of `root(row)` over `rows` outside the direct range.
    fn leaf(
        maximum: f64,
        direct: f64,
        rows: std::ops::Range<usize>,
        root: impl Fn(usize) -> f64,
    ) -> Result<Self> {
        // Underflowed or overflowed squares, including all-zero ones, rescan.
        if (DIRECT_MIN..=DIRECT_MAX).contains(&maximum) && direct.is_finite() {
            return Ok(Self {
                scale: maximum.sqrt(),
                square: direct / maximum,
            });
        }
        let mut scaled = Self::EMPTY;
        for row in rows {
            update_scaled_square(root(row), &mut scaled.scale, &mut scaled.square)?;
        }
        Ok(scaled)
    }

    fn merge(self, other: Self) -> Self {
        if self.scale == 0. {
            return other;
        }
        if other.scale == 0. {
            return self;
        }
        let scale = self.scale.max(other.scale);
        let (left, right) = (self.scale / scale, other.scale / scale);
        Self {
            scale,
            square: self.square * left * left + other.square * right * right,
        }
    }

    fn norm(self) -> Result<f64> {
        stable_norm(self.scale, self.square)
    }
}

/// A Neumaier sum.
#[derive(Clone, Copy, Default)]
struct Sum {
    sum: f64,
    correction: f64,
}

impl Sum {
    fn add(&mut self, value: f64) {
        neumaier(value, &mut self.sum, &mut self.correction);
    }

    fn merge(mut self, other: Self) -> Self {
        self.correction += other.correction;
        neumaier(other.sum, &mut self.sum, &mut self.correction);
        self
    }

    fn total(self) -> Result<f64> {
        let total = self.sum + self.correction;
        if self.sum.is_finite() && self.correction.is_finite() && total.is_finite() {
            Ok(total)
        } else {
            Err(invalid_data(
                "weighted-linear compensated sum is not finite",
            ))
        }
    }
}

pub(super) struct JointResponseChecks {
    pub(super) reference_norm: f64,
    pub(super) weight_sum: f64,
    pub(super) response_mean: Option<f64>,
    pub(super) reconstruction: LinearReconstructionCertificate,
}

#[derive(Clone, Copy)]
struct ResponseMoments {
    response: Squares,
    residual: Squares,
    weight: Sum,
    mean: Sum,
}

impl ResponseMoments {
    fn merge(self, other: Self) -> Self {
        Self {
            response: self.response.merge(other.response),
            residual: self.residual.merge(other.residual),
            weight: self.weight.merge(other.weight),
            mean: self.mean.merge(other.mean),
        }
    }
}

pub(super) fn check_joint_response(
    response: &[f64],
    residuals: &[f64],
    weights: &[f64],
    needs_mean: bool,
) -> Result<JointResponseChecks> {
    if response.len() != residuals.len() || response.len() != weights.len() {
        return Err(invalid_argument("joint response check dimensions differ"));
    }
    let leaf = |rows: std::ops::Range<usize>| -> Result<ResponseMoments> {
        let (mut weight, mut mean) = (Sum::default(), Sum::default());
        let (mut response_maximum, mut response_direct) = (0.0_f64, 0.0_f64);
        let (mut residual_maximum, mut residual_direct) = (0.0_f64, 0.0_f64);
        let mut valid = true;
        for ((&w, &value), &residual) in weights[rows.clone()]
            .iter()
            .zip(&response[rows.clone()])
            .zip(&residuals[rows.clone()])
        {
            valid &= w.is_finite() && w > 0.;
            weight.add(w);
            if needs_mean {
                mean.add(w * value);
            }
            let square = w * value * value;
            response_maximum = response_maximum.max(square);
            response_direct += square;
            let square = w * residual * residual;
            residual_maximum = residual_maximum.max(square);
            residual_direct += square;
        }
        if !valid {
            return Err(invalid_data(
                "weighted-linear norm weight is not finite and positive",
            ));
        }
        if !response_direct.is_finite() || !residual_direct.is_finite() {
            for row in rows.clone() {
                if !(response[row].is_finite() && residuals[row].is_finite()) {
                    return Err(invalid_data("weighted-linear norm value is not finite"));
                }
            }
        }
        Ok(ResponseMoments {
            response: Squares::leaf(response_maximum, response_direct, rows.clone(), |row| {
                weights[row].sqrt() * response[row]
            })?,
            residual: Squares::leaf(residual_maximum, residual_direct, rows, |row| {
                weights[row].sqrt() * residuals[row]
            })?,
            weight,
            mean,
        })
    };
    let (moments, reconstruction) = rayon::join(
        || {
            crate::parallel::fixed_tree_reduce(
                response.len(),
                LEAF_ROWS,
                &leaf,
                &ResponseMoments::merge,
            )
        },
        || {
            response
                .par_chunks(4096)
                .zip(residuals.par_chunks(4096))
                .map(|(response, residuals)| certify_frozen_combined_predictor(response, residuals))
                .try_reduce(LinearReconstructionCertificate::new, |left, right| {
                    Ok(LinearReconstructionCertificate {
                        relative_infinity_norm: left
                            .relative_infinity_norm
                            .max(right.relative_infinity_norm),
                        passed: left.passed && right.passed,
                    })
                })
        },
    );
    let moments = moments?;
    let reference = moments.response.norm()?.max(moments.residual.norm()?);
    let weight_sum = moments.weight.total()?;
    let mean = needs_mean
        .then(|| moments.mean.total().map(|total| total / weight_sum))
        .transpose()?;
    if weight_sum <= 0. || mean.is_some_and(|value| !value.is_finite()) {
        return Err(invalid_data("frozen weighted-linear mean is not finite"));
    }
    Ok(JointResponseChecks {
        reference_norm: reference,
        weight_sum,
        response_mean: mean,
        reconstruction: reconstruction?,
    })
}

/// One original column's joint score, weighted mean and optional norm.
#[derive(Clone, Copy)]
struct JointColumnCheck {
    squares: Squares,
    score: Sum,
    mean: Sum,
}

impl JointColumnCheck {
    const EMPTY: Self = Self {
        squares: Squares::EMPTY,
        score: Sum {
            sum: 0.,
            correction: 0.,
        },
        mean: Sum {
            sum: 0.,
            correction: 0.,
        },
    };

    fn merge(self, other: Self) -> Self {
        Self {
            squares: self.squares.merge(other.squares),
            score: self.score.merge(other.score),
            mean: self.mean.merge(other.mean),
        }
    }
}

/// Original-column scores, means and norms after a joint solve, in the
/// frozen score, RHS and solve banks. Contiguous columns of one task share
/// each leaf's row scan; norms come from the solve's own scan when supplied.
pub(super) fn joint_column_checks<C: crate::fe::slope_coefficients::joint::JointFiniteDesign>(
    originals: &C,
    weights: &[f64],
    residuals: &[f64],
    input_norms: Option<&[f64]>,
    banks: &mut FrozenLinearWorkspace,
) -> Result<()> {
    use crate::fe::slope_coefficients::joint::{column_group_width, MAX_COLUMN_GROUP};
    type Group = [JointColumnCheck; MAX_COLUMN_GROUP];
    let width = column_group_width(banks.scores.len());
    let n = weights.len();
    banks
        .scores
        .par_chunks_mut(width)
        .zip(banks.rhs.par_chunks_mut(width))
        .zip(banks.solve.par_chunks_mut(width))
        .enumerate()
        .try_for_each(|(group, ((scores, means), norms))| -> Result<()> {
            let first = group * width;
            let columns = scores.len();
            let leaf = |rows: std::ops::Range<usize>| -> Result<Group> {
                let mut checks: Group = [JointColumnCheck::EMPTY; MAX_COLUMN_GROUP];
                for (offset, check) in checks.iter_mut().enumerate().take(columns) {
                    let column = first + offset;
                    let norms = input_norms.is_none();
                    match originals
                        .column_slice(column)
                        .filter(|values| values.len() == n)
                    {
                        Some(values) => {
                            // Register-held accumulators; a field update
                            // through `check` would serialize on memory.
                            let (mut maximum, mut direct) = (0.0_f64, 0.0_f64);
                            let (mut score, mut mean) = (Sum::default(), Sum::default());
                            for ((&weight, &value), &residual) in weights[rows.clone()]
                                .iter()
                                .zip(&values[rows.clone()])
                                .zip(&residuals[rows.clone()])
                            {
                                let weighted = weight * value;
                                score.add(weighted * residual);
                                mean.add(weighted);
                                if norms {
                                    let square = weighted * value;
                                    maximum = maximum.max(square);
                                    direct += square;
                                }
                            }
                            check.score = score;
                            check.mean = mean;
                            if norms {
                                if !direct.is_finite()
                                    && values[rows.clone()].iter().any(|v| !v.is_finite())
                                {
                                    return Err(invalid_data(
                                        "weighted-linear norm value is not finite",
                                    ));
                                }
                                check.squares =
                                    Squares::leaf(maximum, direct, rows.clone(), |row| {
                                        weights[row].sqrt() * values[row]
                                    })?;
                            }
                        }
                        None => {
                            let mut squares = Squares::EMPTY;
                            originals.visit_column_range(column, rows.clone(), |row, value| {
                                if norms {
                                    update_scaled_square(
                                        weights[row].sqrt() * value,
                                        &mut squares.scale,
                                        &mut squares.square,
                                    )?;
                                }
                                check.score.add(weights[row] * value * residuals[row]);
                                check.mean.add(weights[row] * value);
                                Ok(())
                            })?;
                            check.squares = squares;
                        }
                    }
                }
                Ok(checks)
            };
            let merge = |mut left: Group, right: Group| {
                for (left, right) in left.iter_mut().zip(right) {
                    *left = left.merge(right);
                }
                left
            };
            let checks = crate::parallel::fixed_tree_reduce(n, LEAF_ROWS, &leaf, &merge)?;
            for (offset, check) in checks.iter().take(columns).enumerate() {
                norms[offset] = match input_norms {
                    Some(norms) => norms[first + offset],
                    None => check.squares.norm()?,
                };
                scores[offset] = check.score.total()?.abs();
                means[offset] = check.mean.total()?;
            }
            Ok(())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Columns(Vec<Vec<f64>>, bool);

    impl crate::fe::slope_coefficients::joint::JointFiniteDesign for Columns {
        fn nrows(&self) -> usize {
            self.0[0].len()
        }
        fn ncols(&self) -> usize {
            self.0.len()
        }
        fn column_slice(&self, column: usize) -> Option<&[f64]> {
            self.1.then(|| self.0[column].as_slice())
        }
        fn visit_column_range(
            &self,
            column: usize,
            range: std::ops::Range<usize>,
            mut visit: impl FnMut(usize, f64) -> Result<()>,
        ) -> Result<()> {
            for row in range {
                visit(row, self.0[column][row])?;
            }
            Ok(())
        }
        fn visit_rows(
            &self,
            _: &mut [f64],
            _: impl FnMut(usize, &[f64]) -> Result<()>,
        ) -> Result<()> {
            unreachable!("column checks scan columns")
        }
    }

    #[test]
    fn column_checks_match_sequential_compensated_passes_for_every_team() {
        let n = 30_011;
        let k = 7;
        let weights: Vec<f64> = (0..n).map(|i| 0.25 + (i % 9) as f64).collect();
        let residuals: Vec<f64> = (0..n).map(|i| ((i * 31) % 97) as f64 - 48.).collect();
        for scale in [1e-160, 1., 1e160] {
            let columns: Vec<Vec<f64>> = (0..k)
                .map(|j| {
                    (0..n)
                        .map(|i| (((i * (j + 3)) % 23) as f64 - 11.) * scale)
                        .collect()
                })
                .collect();
            let mut reference: Option<Vec<[u64; 3]>> = None;
            for (threads, sliced) in [(1, true), (3, true), (8, true), (4, false)] {
                let design = Columns(columns.clone(), sliced);
                let mut banks = FrozenLinearWorkspace::default();
                banks.scores.resize(k, 0.);
                banks.rhs.resize(k, 0.);
                banks.solve.resize(k, 0.);
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        joint_column_checks(&design, &weights, &residuals, None, &mut banks)
                    })
                    .unwrap();
                for (j, values) in columns.iter().enumerate() {
                    let (mut score, mut correction) = (0., 0.);
                    let (mut mean, mut mean_correction) = (0., 0.);
                    let (mut norm_scale, mut square) = (0., 1.);
                    for row in 0..n {
                        compensated_add(
                            weights[row] * values[row] * residuals[row],
                            &mut score,
                            &mut correction,
                        )
                        .unwrap();
                        compensated_add(
                            weights[row] * values[row],
                            &mut mean,
                            &mut mean_correction,
                        )
                        .unwrap();
                        update_scaled_square(
                            weights[row].sqrt() * values[row],
                            &mut norm_scale,
                            &mut square,
                        )
                        .unwrap();
                    }
                    let norm = stable_norm(norm_scale, square).unwrap();
                    let close = |actual: f64, expected: f64, magnitude: f64| {
                        (actual - expected).abs() <= 1e-14 * magnitude
                    };
                    let magnitude = 11. * scale * weights.iter().sum::<f64>();
                    assert!(close(
                        banks.scores[j],
                        (score + correction).abs(),
                        magnitude * 48.
                    ));
                    assert!(close(banks.rhs[j], mean + mean_correction, magnitude));
                    assert!((banks.solve[j] - norm).abs() <= 1e-12 * norm);
                }
                let bits: Vec<[u64; 3]> = (0..k)
                    .map(|j| {
                        [
                            banks.scores[j].to_bits(),
                            banks.rhs[j].to_bits(),
                            banks.solve[j].to_bits(),
                        ]
                    })
                    .collect();
                if sliced {
                    match &reference {
                        None => reference = Some(bits),
                        Some(expected) => assert_eq!(*expected, bits),
                    }
                }
            }
        }
        let design = Columns(vec![vec![1., f64::NAN, 1.]], true);
        let mut banks = FrozenLinearWorkspace::default();
        banks.scores.resize(1, 0.);
        banks.rhs.resize(1, 0.);
        banks.solve.resize(1, 0.);
        assert!(joint_column_checks(&design, &[1.; 3], &[1.; 3], None, &mut banks).is_err());
        let supplied = [2.5];
        joint_column_checks(
            &Columns(vec![vec![1., 2., 3.]], true),
            &[1.; 3],
            &[1.; 3],
            Some(&supplied),
            &mut banks,
        )
        .unwrap();
        assert_eq!(banks.solve[0], 2.5);
        assert_eq!(banks.scores[0], 6.);
    }
}
