//! Current-weight categorical Schur geometry for three-FE finite systems.
//!
//! The original input and the existing certificate remain outside this owner.
//! Nothing here survives a change of weights or a rejected attempt.

use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Plan {
    pub(crate) order: [usize; 3],
    pub(crate) rows: usize,
    pub(crate) eliminated: usize,
    pub(crate) split: usize,
    pub(crate) reduced: usize,
    pub(crate) square: usize,
    pub(crate) entries: usize,
}

impl Plan {
    pub(crate) fn new(rows: usize, rhs: usize, levels: [usize; 3], threads: usize) -> Option<Self> {
        if rows == 0 || rhs == 0 || threads == 0 || levels.contains(&0) {
            return None;
        }
        let largest = (0..3).max_by_key(|&e| levels[e])?;
        let mut order = [largest, 0, 0];
        let mut next = 1;
        for effect in 0..3 {
            if effect != largest {
                order[next] = effect;
                next += 1;
            }
        }
        let eliminated = levels[largest];
        eliminated.checked_add(1)?;
        let split = levels[order[1]];
        let reduced = split.checked_add(levels[order[2]])?;
        let square = reduced.checked_mul(reduced)?;
        // A dense reduced matvec must cost no more multiply-adds than one
        // three-indicator forward/adjoint pair. This is a structural work
        // bound, independent of seeds, memory mode, RHS width or tolerances.
        if square as u128 > 6 * rows as u128 {
            return None;
        }
        let entries = rows.checked_mul(2)?;
        Some(Self {
            order,
            rows,
            eliminated,
            split,
            reduced,
            square,
            entries,
        })
    }
}

// The plan is checked before the first reservation. Actual capacities consume
// the same allowance, so allocator rounding cannot silently exceed admission.
pub(crate) struct AllocationBudget {
    remaining: u128,
    #[cfg(test)]
    fail_after: Option<usize>,
}

impl AllocationBudget {
    pub(crate) fn new(bytes: u128) -> Self {
        Self {
            remaining: bytes,
            #[cfg(test)]
            fail_after: None,
        }
    }

    pub(crate) fn reserve<T: Clone>(&mut self, count: usize, value: T) -> Result<Vec<T>> {
        #[cfg(test)]
        if let Some(left) = self.fail_after.as_mut() {
            if *left == 0 {
                return Err(allocation("injected three-FE Schur reservation failure"));
            }
            *left -= 1;
        }
        let bytes = (count as u128)
            .checked_mul(std::mem::size_of::<T>() as u128)
            .ok_or_else(|| allocation("three-FE Schur reservation overflows"))?;
        if bytes > self.remaining {
            return Err(allocation(
                "three-FE Schur reservation exceeds admitted workspace",
            ));
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| allocation("unable to reserve three-FE Schur workspace"))?;
        self.remaining = self
            .remaining
            .checked_sub(result.capacity() as u128 * std::mem::size_of::<T>() as u128)
            .ok_or_else(|| allocation("three-FE Schur capacity exceeds admitted workspace"))?;
        result.resize(count, value);
        Ok(result)
    }
}

pub(crate) struct Geometry {
    plan: Plan,
    mass: Vec<f64>,
    matrix: Vec<f64>,
    cross: Vec<(usize, f64)>,
    offsets: Vec<usize>,
    scales: Vec<f64>,
}

impl Plan {
    pub(crate) fn geometry_bytes(self) -> Option<u128> {
        let doubles = (self.eliminated as u128)
            .checked_add(2 * self.square as u128)?
            .checked_add(2 * self.reduced as u128)?;
        doubles
            .checked_mul(8)?
            .checked_add(
                ((self.eliminated + 1) as u128 + self.reduced as u128)
                    .checked_mul(std::mem::size_of::<usize>() as u128)?,
            )?
            .checked_add(
                (self.entries as u128).checked_mul(std::mem::size_of::<(usize, f64)>() as u128)?,
            )
    }
}

impl Geometry {
    pub(crate) fn prepare(
        design: &BorrowedInterceptFeDesign<'_>,
        w: &[f64],
        plan: Plan,
        budget: &mut AllocationBudget,
    ) -> Result<Option<Self>> {
        let [a, b, c] = plan.order.map(|e| &design.effects()[e]);
        let q = plan.reduced;
        let mut mass = budget.reserve(plan.eliminated, 0.)?;
        let mut matrix = budget.reserve(plan.square, 0.)?;
        let mut cross = budget.reserve(plan.entries, (0_usize, 0.))?;
        let mut offsets = budget.reserve(plan.eliminated + 1, 0_usize)?;
        let mut cooccurrence = budget.reserve(plan.square, 0.)?;
        let mut group_weights = budget.reserve(q, 0.)?;
        let mut touched = budget.reserve(q, 0_usize)?;
        let mut used = 0;
        for g in 0..plan.eliminated {
            let mut count = 0;
            for &row in a.level_index().members(g) {
                mass[g] += w[row];
                let left = b.codes()[row] as usize;
                let right = plan.split + c.codes()[row] as usize;
                cooccurrence[left * q + right] += w[row];
                for index in [left, right] {
                    if group_weights[index] == 0. {
                        touched[count] = index;
                        count += 1;
                    }
                    group_weights[index] += w[row];
                }
            }
            if mass[g] <= 0. || !mass[g].is_finite() {
                return Ok(None);
            }
            touched[..count].sort_unstable();
            let left_count = touched[..count].partition_point(|&j| j < plan.split);
            let constant = |j| {
                if j < plan.split {
                    left_count == 1
                } else {
                    count - left_count == 1
                }
            };
            for (i, &left) in touched[..count].iter().enumerate() {
                let lv = group_weights[left];
                cross[used] = (left, lv);
                used += 1;
                for &right in &touched[i..count] {
                    // Group-constant indicators have exactly zero covariance.
                    // Enforce that topological identity before floating subtraction.
                    let covariance = if constant(left) || constant(right) {
                        0.
                    } else if left == right {
                        lv * ((mass[g] - lv) / mass[g])
                    } else {
                        cooccurrence[left * q + right] - lv * (group_weights[right] / mass[g])
                    };
                    matrix[left * q + right] += covariance;
                    cooccurrence[left * q + right] = 0.;
                }
            }
            for &j in &touched[..count] {
                group_weights[j] = 0.;
            }
            offsets[g + 1] = used;
        }
        drop(cooccurrence);
        drop(group_weights);
        drop(touched);
        let mut scales = budget.reserve(q, 0.)?;
        for j in 0..q {
            let diagonal = matrix[j * q + j];
            if diagonal < 0. || !diagonal.is_finite() {
                return Ok(None);
            }
            scales[j] = if diagonal == 0. { 1. } else { diagonal.sqrt() };
        }
        for i in 0..q {
            for j in i..q {
                let v = matrix[i * q + j] / scales[i] / scales[j];
                if !v.is_finite() {
                    return Ok(None);
                }
                matrix[i * q + j] = v;
                matrix[j * q + i] = v;
            }
        }
        Ok(Some(Self {
            plan,
            mass,
            matrix,
            cross,
            offsets,
            scales,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn solve_column(
        &self,
        design: &BorrowedInterceptFeDesign<'_>,
        w: &[f64],
        value: impl Fn(usize) -> Result<f64>,
        norm: f64,
        cap: usize,
        tolerance: f64,
        coefficients: &mut [f64],
        work: &mut [f64],
    ) -> Result<usize> {
        let plan = self.plan;
        let [a, b, c] = plan.order.map(|e| &design.effects()[e]);
        let q = plan.reduced;
        let Self {
            mass,
            matrix,
            cross,
            offsets,
            scales,
            ..
        } = self;
        let (arhs, solution) = coefficients.split_at_mut(plan.eliminated);
        let (residual, rest) = work.split_at_mut(q);
        let (direction, rest) = rest.split_at_mut(q);
        let (ax, rhs) = rest.split_at_mut(q);
        arhs.fill(0.);
        rhs.fill(0.);
        solution.fill(0.);
        for row in 0..plan.rows {
            let wy = w[row] * if norm == 0. { 0. } else { value(row)? / norm };
            arhs[a.codes()[row] as usize] += wy;
            rhs[b.codes()[row] as usize] += wy;
            rhs[plan.split + c.codes()[row] as usize] += wy;
        }
        for g in 0..plan.eliminated {
            for &(j, v) in &cross[offsets[g]..offsets[g + 1]] {
                rhs[j] -= v * (arhs[g] / mass[g]);
            }
        }
        for j in 0..q {
            rhs[j] /= scales[j];
        }
        residual.copy_from_slice(rhs);
        direction.copy_from_slice(rhs);
        let mut rr = residual.iter().map(|v| v * v).sum::<f64>();
        let mut iterations = 0;
        while iterations < cap {
            if residual.iter().map(|v| v.abs()).fold(0., f64::max) <= tolerance * 0.05 {
                break;
            }
            for (row, target) in matrix.chunks_exact(q).zip(ax.iter_mut()) {
                let mut sums = [0.; 8];
                for (j, (&v, &x)) in row.iter().zip(direction.iter()).enumerate() {
                    sums[j % 8] = v.mul_add(x, sums[j % 8]);
                }
                *target = sums.iter().sum();
            }
            let curvature = direction
                .iter()
                .zip(ax.iter())
                .map(|(x, y)| x * y)
                .sum::<f64>();
            if curvature <= 0. || !curvature.is_finite() || !rr.is_finite() {
                break;
            }
            let alpha = rr / curvature;
            for j in 0..q {
                solution[j] = alpha.mul_add(direction[j], solution[j]);
                residual[j] = (-alpha).mul_add(ax[j], residual[j]);
            }
            let next = residual.iter().map(|v| v * v).sum::<f64>();
            let beta = next / rr;
            for j in 0..q {
                direction[j] = beta.mul_add(direction[j], residual[j]);
            }
            rr = next;
            iterations += 1;
        }
        for j in 0..q {
            solution[j] /= scales[j];
        }
        for g in 0..plan.eliminated {
            for &(j, v) in &cross[offsets[g]..offsets[g + 1]] {
                arhs[g] -= v * solution[j];
            }
            arhs[g] /= mass[g];
        }
        Ok(iterations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

    fn codes(nested: usize) -> [Vec<u32>; 3] {
        let mut result = [Vec::new(), Vec::new(), Vec::new()];
        for a in 0..5 {
            for b in 0..4 {
                for c in 0..3 {
                    for _ in 0..2 {
                        result[0].push(a);
                        result[1].push(if nested == 2 { a % 4 } else { b });
                        result[2].push(if nested == 1 { b % 3 } else { c });
                    }
                }
            }
        }
        result
    }

    #[test]
    fn centered_three_fe_projection_matches_constructed_weighted_oracle() {
        for nested in 0..3 {
            let codes = codes(nested);
            let levels = [5, 4, 3];
            let indices: [_; 3] =
                std::array::from_fn(|e| LevelIndex::from_codes(&codes[e], levels[e]).unwrap());
            let sources: [_; 3] = std::array::from_fn(|e| {
                ValidatedBorrowedInterceptSource::new(&codes[e], levels[e], &indices[e]).unwrap()
            });
            let n = codes[0].len();
            for threads in [1, 4] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let design = BorrowedInterceptFeDesign::new(
                            n,
                            sources.iter().map(|s| s.effect()).collect(),
                        )
                        .unwrap();
                        let mut owner =
                            ReusableCategoricalMapAbsorber::new(design, 5, threads).unwrap();
                        owner.enable_checkpoint_lsmr(true);
                        for (frame, scale) in [1., 1e-100, 1e100].into_iter().enumerate() {
                            let ratio = 2. + frame as f64;
                            let weights: Vec<_> = (0..n)
                                .map(|r| {
                                    scale
                                        * (1. + (r / 2 % 7) as f64)
                                        * if r % 2 == 0 { 1. } else { ratio }
                                })
                                .collect();
                            // Contrasts cancel within each identical explicit FE
                            // row pair. Their weighted projection is known without
                            // another iterative algorithm or the Schur equations.
                            let expected: Vec<_> = (0..n)
                                .map(|r| if r % 2 == 0 { 0.5 } else { -0.5 / ratio })
                                .collect();
                            let columns: Vec<Vec<_>> = (0..5)
                                .map(|k| {
                                    (0..n)
                                        .map(|r| {
                                            if k == 4 {
                                                0.
                                            } else {
                                                (k + 1) as f64 * expected[r]
                                                    + 0.3 * codes[0][r] as f64
                                                    - 0.7 * codes[1][r] as f64
                                                    + 0.2 * codes[2][r] as f64
                                            }
                                        })
                                        .collect()
                                })
                                .collect();
                            let raw = DenseMatrix::from_columns(&columns).unwrap();
                            let config = MapConfig {
                                tolerance: 1e-12,
                                ..MapConfig::default()
                            };
                            let fit = owner
                                .absorb(BorrowedMapWeights::Provided(&weights), raw.clone(), config)
                                .unwrap();
                            assert_eq!(fit.report().used_solver, FeSolverKind::Lsmr);
                            assert_eq!(fit.report().checkpoint_retries, 0);
                            assert!(fit.report().certified());
                            // Keep the legacy matrix indices and numerical traversal order explicit.
                            #[allow(clippy::needless_range_loop)]
                            for r in 0..n {
                                for k in 0..5 {
                                    let oracle = if k == 4 {
                                        0.
                                    } else {
                                        (k + 1) as f64 * expected[r]
                                    };
                                    assert!((fit.residualized()[(r, k)] - oracle).abs() < 1e-10);
                                }
                            }
                            for _ in 0..2 {
                                owner.inject_checkpoint_lsmr_certificate_failure();
                                let fallback = owner
                                    .absorb(
                                        BorrowedMapWeights::Provided(&weights),
                                        raw.clone(),
                                        config,
                                    )
                                    .unwrap();
                                assert!(fallback.report().certified());
                                assert!(fallback.report().fallback_used);
                                assert!(fallback.report().warnings.iter().any(
                                    |w| w.contains("untouched authoritative checkpoint inputs")
                                ));
                                for (&actual, &reference) in fallback
                                    .residualized()
                                    .as_slice()
                                    .iter()
                                    .zip(fit.residualized().as_slice())
                                {
                                    assert!((actual - reference).abs() < 1e-10);
                                }
                            }
                            for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
                                owner.inject_checkpoint_lsmr_fatal_failure(code);
                                assert_eq!(
                                    owner
                                        .absorb(
                                            BorrowedMapWeights::Provided(&weights),
                                            raw.clone(),
                                            config
                                        )
                                        .unwrap_err()
                                        .code,
                                    code
                                );
                            }
                            assert_eq!(raw, DenseMatrix::from_columns(&columns).unwrap());
                        }
                    });
            }
        }
    }

    #[test]
    fn three_fe_schur_admission_and_reservations_are_bounded() {
        assert!(Plan::new(100, 1, [100, 100, 100], 1).is_none());
        assert!(Plan::new(usize::MAX, usize::MAX, [1, 2, 3], 4).is_none());
        assert!(Plan::new(100, 1, [usize::MAX, 1, 2], 4).is_none());
        let codes = codes(0);
        let indices: [_; 3] =
            std::array::from_fn(|e| LevelIndex::from_codes(&codes[e], [5, 4, 3][e]).unwrap());
        let sources: [_; 3] = std::array::from_fn(|e| {
            ValidatedBorrowedInterceptSource::new(&codes[e], [5, 4, 3][e], &indices[e]).unwrap()
        });
        let n = codes[0].len();
        let design =
            BorrowedInterceptFeDesign::new(n, sources.iter().map(|s| s.effect()).collect())
                .unwrap();
        let weights = vec![1.; n];
        let plan = Plan::new(n, 2, [5, 4, 3], 4).unwrap();
        let bytes = plan.geometry_bytes().unwrap();
        for failed in 0..8 {
            let mut budget = AllocationBudget::new(bytes);
            budget.fail_after = Some(failed);
            assert_eq!(
                Geometry::prepare(&design, &weights, plan, &mut budget)
                    .err()
                    .unwrap()
                    .code,
                ErrorCode::Allocation
            );
        }
        let mut budget = AllocationBudget::new(bytes);
        assert!(Geometry::prepare(&design, &weights, plan, &mut budget)
            .unwrap()
            .is_some());
        let mut short = AllocationBudget::new(bytes - 1);
        assert_eq!(
            Geometry::prepare(&design, &weights, plan, &mut short)
                .err()
                .unwrap()
                .code,
            ErrorCode::Allocation
        );
        let extreme: Vec<_> = (0..n)
            .map(|r| if r == 0 { f64::MIN_POSITIVE } else { f64::MAX })
            .collect();
        assert_eq!(
            validate_weights(BorrowedMapWeights::Provided(&extreme), n)
                .unwrap_err()
                .code,
            ErrorCode::InvalidData
        );
    }
}
