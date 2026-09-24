//! Current-call categorical Schur geometry for three-FE PPML checkpoints.
//!
//! The original input and the existing certificate remain outside this owner.
//! Nothing here survives a change of weights or a rejected attempt.

use super::*;

#[derive(Clone, Copy, Debug)]
struct Plan {
    order: [usize; 3],
    rows: usize,
    rhs: usize,
    eliminated: usize,
    split: usize,
    reduced: usize,
    square: usize,
    entries: usize,
    outputs: usize,
    columns_per_worker: usize,
    worker_width: usize,
    workspace: usize,
    bytes: u128,
}

impl Plan {
    fn admit(self, allowance: u128) -> Option<AllocationBudget> {
        (self.bytes <= allowance).then(|| AllocationBudget::new(allowance))
    }

    fn new(rows: usize, rhs: usize, levels: [usize; 3], threads: usize) -> Option<Self> {
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
        let outputs = rows.checked_mul(rhs)?;
        let workers = threads.min(rhs);
        let columns_per_worker = rhs.div_ceil(workers);
        let worker_width = eliminated.checked_add(reduced.checked_mul(5)?)?;
        let workspace = worker_width.checked_mul(workers)?;
        // Sum every allocation, including sequential construction scratch and
        // both conversion banks. This deliberately takes no lifetime credit.
        let doubles = [
            rows, eliminated, square, square, reduced, reduced, workspace, outputs, outputs,
        ]
        .into_iter()
        .try_fold(0_u128, |sum, v| sum.checked_add(v as u128))?;
        let indices = (eliminated as u128)
            .checked_add(1)?
            .checked_add(reduced as u128)?;
        let bytes = doubles
            .checked_mul(8)?
            .checked_add(indices.checked_mul(std::mem::size_of::<usize>() as u128)?)?
            .checked_add(
                (entries as u128).checked_mul(std::mem::size_of::<(usize, f64)>() as u128)?,
            )?
            .checked_add((rhs as u128).checked_mul(std::mem::size_of::<usize>() as u128)?)?
            .checked_add(16 * std::mem::size_of::<Vec<f64>>() as u128)?;
        Some(Self {
            order,
            rows,
            rhs,
            eliminated,
            split,
            reduced,
            square,
            entries,
            outputs,
            columns_per_worker,
            worker_width,
            workspace,
            bytes,
        })
    }
}

// The plan is checked before the first reservation. Actual capacities consume
// the same allowance, so allocator rounding cannot silently exceed admission.
struct AllocationBudget {
    remaining: u128,
    #[cfg(test)]
    fail_after: Option<usize>,
}

impl AllocationBudget {
    fn new(bytes: u128) -> Self {
        Self {
            remaining: bytes,
            #[cfg(test)]
            fail_after: None,
        }
    }

    fn reserve<T: Clone>(&mut self, count: usize, value: T) -> Result<Vec<T>> {
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

impl ReusableCategoricalMapAbsorber<'_> {
    #[inline(never)]
    pub(super) fn try_checkpoint_three_fe_schur(
        &mut self,
        weights: ValidatedWeights<'_>,
        values: &DenseMatrix,
        config: MapConfig,
    ) -> Result<(Option<CertifiedBorrowedMap>, CheckpointSchurCost)> {
        let mut cost = CheckpointSchurCost::default();
        let levels = std::array::from_fn(|e| self.design.effects()[e].n_levels());
        let Some(plan) = Plan::new(
            values.nrows(),
            values.ncols(),
            levels,
            rayon::current_num_threads(),
        ) else {
            return Ok((None, cost));
        };
        // This caller has already admitted checkpoint LSMR's entire transient
        // frame. These banks replace it and are dropped before LSMR fallback.
        let allowance = estimate_fe_solver_memory(
            plan.rows,
            plan.rhs,
            &levels,
            rayon::current_num_threads(),
            Some(4),
        )?
        .lsmr_schwarz_bytes;
        let Some(mut budget) = plan.admit(allowance) else {
            return Ok((None, cost));
        };
        let cap = config.max_iterations.min(plan.reduced.saturating_mul(2));
        #[cfg(test)]
        let cap = self
            .checkpoint_schur_iteration_cap
            .take()
            .unwrap_or(cap)
            .min(cap);
        let candidate = solve(
            &self.design,
            weights,
            values,
            &self.workspace.reference_norms,
            plan,
            cap,
            config.tolerance,
            &mut budget,
            &mut cost,
        )?;
        if let Some(candidate) = candidate {
            let started = Instant::now();
            let certificate = certify(
                &self.design,
                weights,
                &candidate,
                config.tolerance,
                &mut self.workspace,
            )?;
            cost.certification_seconds = started.elapsed().as_secs_f64();
            #[cfg(not(test))]
            let rejected = false;
            #[cfg(test)]
            let rejected = std::mem::take(&mut self.checkpoint_schur_certificate_failure);
            if certificate.passed && !rejected {
                let mut result = certified_result(
                    candidate,
                    SolveStopReason::Certified,
                    cost.iterations,
                    certificate,
                    cost.setup_seconds,
                    cost.solve_seconds,
                )?;
                result.report.requested_solver = FeSolverKind::Lsmr;
                result.report.certification_seconds = cost.certification_seconds;
                cost.trace(values, "three_fe_certified");
                return Ok((Some(result), cost));
            }
        }
        cost.rejected = true;
        cost.trace(values, "three_fe_rejected");
        Ok((None, cost))
    }
}

#[allow(clippy::too_many_arguments)]
fn solve(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    input: &DenseMatrix,
    norms: &[f64],
    plan: Plan,
    cap: usize,
    tolerance: f64,
    budget: &mut AllocationBudget,
    cost: &mut CheckpointSchurCost,
) -> Result<Option<DenseMatrix>> {
    let setup_started = Instant::now();
    let [a, b, c] = plan.order.map(|e| &design.effects()[e]);
    let q = plan.reduced;
    let mut w = budget.reserve(plan.rows, 0.)?;
    for (row, value) in w.iter_mut().enumerate() {
        *value = weights.normalized(row);
    }
    if w.iter().any(|&value| value <= 0. || !value.is_finite()) {
        cost.setup_seconds = setup_started.elapsed().as_secs_f64();
        return Ok(None);
    }
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
            cost.setup_seconds = setup_started.elapsed().as_secs_f64();
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
            cost.setup_seconds = setup_started.elapsed().as_secs_f64();
            return Ok(None);
        }
        scales[j] = if diagonal == 0. { 1. } else { diagonal.sqrt() };
    }
    for i in 0..q {
        for j in i..q {
            let v = matrix[i * q + j] / scales[i] / scales[j];
            if !v.is_finite() {
                cost.setup_seconds = setup_started.elapsed().as_secs_f64();
                return Ok(None);
            }
            matrix[i * q + j] = v;
            matrix[j * q + i] = v;
        }
    }
    let mut columns = budget.reserve(plan.outputs, 0.)?;
    let mut workspace = budget.reserve(plan.workspace, 0.)?;
    let mut steps = budget.reserve(plan.rhs, 0_usize)?;
    cost.setup_seconds = setup_started.elapsed().as_secs_f64();
    let started = Instant::now();
    let width = plan.columns_per_worker;
    // Fixed disjoint workspace slices bound live Krylov storage even with a
    // nested Rayon caller. Each column stops independently; no converged RHS
    // participates in another column's recurrence.
    columns
        .par_chunks_mut(plan.rows * width)
        .zip(workspace.par_chunks_mut(plan.worker_width))
        .zip(steps.par_chunks_mut(width))
        .enumerate()
        .for_each(|(block, ((outputs, work), column_steps))| {
            let (arhs, rest) = work.split_at_mut(plan.eliminated);
            let (solution, rest) = rest.split_at_mut(q);
            let (residual, rest) = rest.split_at_mut(q);
            let (direction, rest) = rest.split_at_mut(q);
            let (ax, rhs) = rest.split_at_mut(q);
            for (local, out) in outputs.chunks_mut(plan.rows).enumerate() {
                let column = block * width + local;
                let norm = norms[column];
                arhs.fill(0.);
                rhs.fill(0.);
                solution.fill(0.);
                for row in 0..plan.rows {
                    let wy = w[row]
                        * if norm == 0. {
                            0.
                        } else {
                            input[(row, column)] / norm
                        };
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
                column_steps[local] = iterations;
                for j in 0..q {
                    solution[j] /= scales[j];
                }
                for g in 0..plan.eliminated {
                    for &(j, v) in &cross[offsets[g]..offsets[g + 1]] {
                        arhs[g] -= v * solution[j];
                    }
                    arhs[g] /= mass[g];
                }
                for row in 0..plan.rows {
                    let fit = arhs[a.codes()[row] as usize]
                        + solution[b.codes()[row] as usize]
                        + solution[plan.split + c.codes()[row] as usize];
                    out[row] = input[(row, column)] - norm * fit;
                }
            }
        });
    cost.iterations = steps.iter().copied().max().unwrap_or(0);
    if columns.iter().any(|v| !v.is_finite()) {
        cost.solve_seconds = started.elapsed().as_secs_f64();
        return Ok(None);
    }
    let mut output = budget.reserve(plan.outputs, 0.)?;
    for row in 0..plan.rows {
        for column in 0..plan.rhs {
            output[row * plan.rhs + column] = columns[column * plan.rows + row];
        }
    }
    cost.solve_seconds = started.elapsed().as_secs_f64();
    Ok(Some(DenseMatrix::from_row_major(
        plan.rows, plan.rhs, output,
    )?))
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
                        owner.enable_checkpoint_lsmr();
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
                            assert_eq!(fit.report().used_solver, FeSolverKind::Map);
                            assert_eq!(fit.report().checkpoint_retries, 0);
                            assert!(fit.report().certified());
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
                            for capped in [false, true] {
                                if capped {
                                    owner.checkpoint_schur_iteration_cap = Some(0);
                                } else {
                                    owner.checkpoint_schur_certificate_failure = true;
                                }
                                let fallback = owner
                                    .absorb(
                                        BorrowedMapWeights::Provided(&weights),
                                        raw.clone(),
                                        config,
                                    )
                                    .unwrap();
                                assert!(fallback.report().certified());
                                assert!(fallback.report().checkpoint_retries >= 1);
                                assert!(fallback
                                    .report()
                                    .warnings
                                    .iter()
                                    .any(|w| w.contains("untouched authoritative inputs")));
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
        let input =
            DenseMatrix::from_columns(&[(0..n).map(|r| (r as f64).sin()).collect(), vec![0.; n]])
                .unwrap();
        let original = input.clone();
        let weights = validate_weights(BorrowedMapWeights::Unit, n).unwrap();
        let plan = Plan::new(n, 2, [5, 4, 3], 4).unwrap();
        let allowance = estimate_fe_solver_memory(n, 2, &[5, 4, 3], 4, Some(4))
            .unwrap()
            .lsmr_schwarz_bytes;
        assert!(plan.bytes <= allowance);
        let norms = [10., 0.];
        for failed in 0..13 {
            let mut budget = AllocationBudget::new(plan.bytes);
            budget.fail_after = Some(failed);
            let error = solve(
                &design,
                weights,
                &input,
                &norms,
                plan,
                100,
                1e-12,
                &mut budget,
                &mut CheckpointSchurCost::default(),
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
            assert_eq!(input, original);
        }
        let mut budget = plan.admit(plan.bytes).unwrap();
        let result = solve(
            &design,
            weights,
            &input,
            &norms,
            plan,
            100,
            1e-12,
            &mut budget,
            &mut CheckpointSchurCost::default(),
        )
        .unwrap();
        assert!(result.is_some());
        assert!(budget.remaining > 0);
        // The exact boundary is admission, not a license to attempt partial
        // construction when the declared allowance is a byte short.
        assert!(plan.admit(plan.bytes - 1).is_none());
        let extreme: Vec<_> = (0..n)
            .map(|r| if r == 0 { f64::MIN_POSITIVE } else { f64::MAX })
            .collect();
        assert_eq!(
            validate_weights(BorrowedMapWeights::Provided(&extreme), n)
                .unwrap_err()
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(input, original);
    }
}
