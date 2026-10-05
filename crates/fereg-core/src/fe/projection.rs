use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{EffectTerm, FeDesign};
use crate::linalg::solve_linear_system;
use rayon::prelude::*;

/// Reusable buffers for one weighted fixed-effect projection.
#[derive(Debug, Clone)]
pub struct ProjectionWorkspace {
    max_channels: usize,
    rhs_count: usize,
    gram: Vec<f64>,
    right_hand_sides: Vec<f64>,
}

impl ProjectionWorkspace {
    pub fn new(max_channels: usize, rhs_count: usize) -> Result<Self> {
        let plan = super::workspace_capacity::ProjectionCapacity::new(
            max_channels as u128,
            rhs_count as u128,
        )?;
        plan.gram.len("projection Gram dimensions overflow")?;
        plan.rhs.len("projection RHS dimensions overflow")?;
        Ok(Self {
            max_channels,
            rhs_count,
            gram: plan
                .gram
                .filled(0.0, "unable to reserve generic FE projection workspace")?,
            right_hand_sides: plan
                .rhs
                .filled(0.0, "unable to reserve generic FE projection workspace")?,
        })
    }

    pub(crate) fn storage_capacities(&self) -> (usize, usize) {
        (self.gram.capacity(), self.right_hand_sides.capacity())
    }
}

fn fallible_zeros(elements: usize) -> Result<Vec<f64>> {
    crate::memory::Capacity::new(elements as u128)
        .filled(0.0, "unable to reserve generic FE projection workspace")
}

/// Subtract the weighted least-squares projection onto one effect term.
pub fn project_effect_in_place(
    effect: &EffectTerm,
    weights: &[f64],
    values: &mut DenseMatrix,
    workspace: &mut ProjectionWorkspace,
) -> Result<()> {
    validate_projection(effect, weights, values, workspace)?;
    let channels = effect.channels();
    let rhs_count = values.ncols();

    for level in 0..effect.n_levels() {
        let gram_length = channels * channels;
        let rhs_length = channels * rhs_count;
        group_coefficients(
            effect,
            weights,
            values,
            level,
            &mut workspace.gram[..gram_length],
            &mut workspace.right_hand_sides[..rhs_length],
        )?;

        // Batch independent RHSs while keeping each fitted value's channel FMA
        // order and its single final subtraction. The tile adds no heap state.
        for observation in effect.level_index().members(level) {
            let row_start = *observation * rhs_count;
            let row = &mut values.as_mut_slice()[row_start..row_start + rhs_count];
            subtract_row(
                effect,
                *observation,
                row,
                &workspace.right_hand_sides,
                rhs_count,
            );
        }
    }
    Ok(())
}

#[inline]
fn group_coefficients(
    effect: &EffectTerm,
    weights: &[f64],
    values: &DenseMatrix,
    level: usize,
    gram: &mut [f64],
    coefficients: &mut [f64],
) -> Result<()> {
    let channels = effect.channels();
    let rhs_count = values.ncols();
    gram.fill(0.0);
    coefficients.fill(0.0);

    for observation in effect.level_index().members(level) {
        let weight = weights[*observation];
        let row_start = *observation * rhs_count;
        let row = &values.as_slice()[row_start..row_start + rhs_count];
        for left_channel in 0..channels {
            let left_basis = effect.basis_value(left_channel, *observation);
            for right_channel in 0..channels {
                let right_basis = effect.basis_value(right_channel, *observation);
                let index = left_channel * channels + right_channel;
                gram[index] = (weight * left_basis).mul_add(right_basis, gram[index]);
            }
            let start = left_channel * rhs_count;
            let moments = &mut coefficients[start..start + rhs_count];
            for (moment, value) in moments.iter_mut().zip(row) {
                *moment = (weight * left_basis).mul_add(*value, *moment);
            }
        }
    }

    solve_linear_system(gram, coefficients, channels, rhs_count, 1e-12)?;

    Ok(())
}

#[inline]
fn subtract_row(
    effect: &EffectTerm,
    observation: usize,
    row: &mut [f64],
    coefficients: &[f64],
    rhs_count: usize,
) {
    let channels = effect.channels();
    for (tile_index, tile) in row.chunks_mut(32).enumerate() {
        let mut fitted = [0.0_f64; 32];
        let rhs_start = tile_index * 32;
        for channel in 0..channels {
            let basis = effect.basis_value(channel, observation);
            let start = channel * rhs_count + rhs_start;
            let coefficients = &coefficients[start..start + tile.len()];
            for (value, coefficient) in fitted[..tile.len()].iter_mut().zip(coefficients) {
                *value = basis.mul_add(*coefficient, *value);
            }
        }
        for (value, fit) in tile.iter_mut().zip(fitted) {
            *value -= fit;
        }
    }
}

fn validate_projection(
    effect: &EffectTerm,
    weights: &[f64],
    values: &DenseMatrix,
    workspace: &ProjectionWorkspace,
) -> Result<()> {
    if weights.len() != values.nrows() || effect.codes().len() != values.nrows() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "projection inputs have inconsistent observation counts",
        ));
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "projection weights must be finite and positive",
        ));
    }
    let channels = effect.channels();
    let rhs_count = values.ncols();
    if channels > workspace.max_channels || rhs_count > workspace.rhs_count {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "projection workspace is smaller than the requested projection",
        ));
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParallelProjectionPlan {
    channels: usize,
    rhs: usize,
    levels: usize,
    workers: usize,
}

struct ProjectionWorker {
    gram: Vec<f64>,
    error: Option<FeregError>,
}

struct ParallelProjectionWorkspace {
    plan: ParallelProjectionPlan,
    coefficients: Vec<f64>,
    workers: Vec<ProjectionWorker>,
}

pub(super) struct AutoProjectionWorkspace {
    serial: ProjectionWorkspace,
    parallel: Option<ParallelProjectionWorkspace>,
}

impl ParallelProjectionPlan {
    pub(super) fn for_design(design: &FeDesign, rhs: usize) -> Result<Option<Self>> {
        if rayon::current_thread_index().is_none() {
            return Ok(None);
        }
        Self::for_design_and_threads(design, rhs, rayon::current_num_threads())
    }

    /// Reusable callers bind this capacity to their explicitly resolved team.
    pub(crate) fn for_design_and_threads(
        design: &FeDesign,
        rhs: usize,
        threads: usize,
    ) -> Result<Option<Self>> {
        if design.nobs() < 16_384
            || rhs == 0
            || threads <= 1
            || !design.effects().iter().any(|e| !e.slopes().is_empty())
        {
            return Ok(None);
        }
        let plan = Self {
            channels: design.max_channels(),
            rhs,
            levels: design
                .effects()
                .iter()
                .map(|e| e.n_levels())
                .max()
                .unwrap_or(0),
            workers: threads,
        };
        plan.bytes()?;
        Ok(Some(plan))
    }

    fn cells(self) -> Result<(usize, usize)> {
        let fail = || {
            FeregError::new(
                ErrorCode::Allocation,
                "parallel projection dimensions overflow",
            )
        };
        Ok((
            self.levels
                .checked_mul(self.channels)
                .and_then(|n| n.checked_mul(self.rhs))
                .ok_or_else(fail)?,
            self.channels.checked_mul(self.channels).ok_or_else(fail)?,
        ))
    }

    pub(crate) fn bytes(self) -> Result<u128> {
        let (coefficients, gram) = self.cells()?;
        (gram as u128)
            .checked_mul(self.workers as u128)
            .and_then(|n| n.checked_add(coefficients as u128))
            .and_then(|n| n.checked_mul(8))
            .and_then(|n| {
                n.checked_add(
                    self.workers as u128 * std::mem::size_of::<ProjectionWorker>() as u128,
                )
            })
            .ok_or_else(|| {
                FeregError::new(ErrorCode::Allocation, "parallel projection bytes overflow")
            })
    }

    fn allocate(self) -> Result<ParallelProjectionWorkspace> {
        let (coefficients, gram) = self.cells()?;
        let coefficients = fallible_zeros(coefficients)?;
        let mut workers = Vec::new();
        workers.try_reserve_exact(self.workers).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve parallel projection workers",
            )
        })?;
        for _ in 0..self.workers {
            workers.push(ProjectionWorker {
                gram: fallible_zeros(gram)?,
                error: None,
            });
        }
        let actual = coefficients.capacity() as u128 * 8
            + workers.capacity() as u128 * std::mem::size_of::<ProjectionWorker>() as u128
            + workers
                .iter()
                .map(|w| w.gram.capacity() as u128 * 8)
                .sum::<u128>();
        if actual > self.bytes()? {
            return Err(FeregError::new(
                ErrorCode::Allocation,
                "parallel projection capacity exceeds admission",
            ));
        }
        Ok(ParallelProjectionWorkspace {
            plan: self,
            coefficients,
            workers,
        })
    }
}

impl AutoProjectionWorkspace {
    /// The caller admits the optional bank beside the existing serial workspace.
    pub(super) fn new(
        design: &FeDesign,
        rhs: usize,
        plan: Option<ParallelProjectionPlan>,
    ) -> Result<Self> {
        let serial = ProjectionWorkspace::new(design.max_channels().max(1), rhs)?;
        let mut workspace = Self {
            serial,
            parallel: None,
        };
        if let Some(plan) = plan {
            workspace.enable_parallel(plan)?;
        }
        Ok(workspace)
    }

    pub(super) fn enable_parallel(&mut self, plan: ParallelProjectionPlan) -> Result<()> {
        self.parallel = match plan.allocate() {
            Ok(workspace) => Some(workspace),
            Err(error) if error.code == ErrorCode::Allocation => None,
            Err(error) => return Err(error),
        };
        Ok(())
    }

    pub(super) fn storage_capacities(&self) -> (usize, usize, usize) {
        let (gram, rhs) = self.serial.storage_capacities();
        self.parallel.as_ref().map_or((gram, rhs, 0), |parallel| {
            (
                gram + parallel
                    .workers
                    .iter()
                    .map(|worker| worker.gram.capacity())
                    .sum::<usize>(),
                rhs + parallel.coefficients.capacity(),
                parallel.workers.capacity(),
            )
        })
    }

    pub(super) fn project(
        &mut self,
        effect: &EffectTerm,
        weights: &[f64],
        values: &mut DenseMatrix,
    ) -> Result<()> {
        if let Some(parallel) = self.parallel.as_mut() {
            validate_projection(effect, weights, values, &self.serial)?;
            if values.ncols() > 0 && effect.n_levels() > 0 {
                return project_parallel(effect, weights, values, parallel);
            }
        }
        project_effect_in_place(effect, weights, values, &mut self.serial)
    }
}

fn project_parallel(
    effect: &EffectTerm,
    weights: &[f64],
    values: &mut DenseMatrix,
    workspace: &mut ParallelProjectionWorkspace,
) -> Result<()> {
    let channels = effect.channels();
    let rhs = values.ncols();
    if channels > workspace.plan.channels
        || rhs > workspace.plan.rhs
        || effect.n_levels() > workspace.plan.levels
        || workspace.workers.is_empty()
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "parallel projection workspace is smaller than the request",
        ));
    }
    let group_width = channels * rhs;
    let chunk_levels = effect.n_levels().div_ceil(workspace.workers.len());
    let coefficients = &mut workspace.coefficients[..effect.n_levels() * group_width];
    // Contiguous level chunks preserve first-error order. No input row is
    // modified until all groups have completed the original small-system solve.
    for worker in &mut workspace.workers {
        worker.error = None;
    }
    coefficients
        .par_chunks_mut(chunk_levels * group_width)
        .zip(workspace.workers.par_iter_mut())
        .enumerate()
        .for_each(|(chunk, (output, worker))| {
            for (local, coefficient) in output.chunks_exact_mut(group_width).enumerate() {
                if let Err(error) = group_coefficients(
                    effect,
                    weights,
                    values,
                    chunk * chunk_levels + local,
                    &mut worker.gram[..channels * channels],
                    coefficient,
                ) {
                    worker.error = Some(error);
                    break;
                }
            }
        });
    for worker in &mut workspace.workers {
        if let Some(error) = worker.error.take() {
            return Err(error);
        }
    }
    values
        .as_mut_slice()
        .par_chunks_mut(rhs)
        .enumerate()
        .for_each(|(row, values)| {
            let start = effect.codes()[row] as usize * group_width;
            subtract_row(
                effect,
                row,
                values,
                &coefficients[start..start + group_width],
                rhs,
            );
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rhs_tiles_preserve_scalar_projection_bits() {
        let rows = 97;
        let codes: Vec<u32> = (0..rows)
            .map(|row| ((19 * row + row / 7) % 11) as u32)
            .collect();
        for intercept in [false, true] {
            for slope_count in 0..=5 {
                if !intercept && slope_count == 0 {
                    continue;
                }
                for scale in [1e-60, 1.0, 1e60] {
                    let slopes = (0..slope_count)
                        .map(|channel| {
                            (0..rows)
                                .map(|row| {
                                    let x = ((row * 13 % 37) as f64 - 18.0) / 7.0;
                                    scale
                                        * match channel {
                                            0 => x,
                                            1 => 2.0 * x,
                                            2 => 0.0,
                                            3 => (row % 5) as f64 - 2.0,
                                            _ => 1.0,
                                        }
                                })
                                .collect()
                        })
                        .collect();
                    // Includes empty levels, dependent and zero channels, and
                    // projection Gram thresholds dominated by the intercept.
                    let effect = EffectTerm::new(codes.clone(), 13, intercept, slopes).unwrap();
                    for weighted in [false, true] {
                        let weights: Vec<_> = (0..rows)
                            .map(|row| {
                                if weighted {
                                    0.25 + (row % 9) as f64 / 4.0
                                } else {
                                    1.0
                                }
                            })
                            .collect();
                        for rhs_count in [0, 1, 2, 7, 31, 32, 33, 64, 65, 129] {
                            let mut actual = DenseMatrix::zeros(rows, rhs_count).unwrap();
                            for row in 0..rows {
                                for rhs in 0..rhs_count {
                                    actual[(row, rhs)] = match rhs % 4 {
                                        0 => ((row * 17 + rhs * 3) % 41) as f64 - 20.0,
                                        1 => 0.0,
                                        2 => -0.0,
                                        _ => 1e100 * ((row % 7) as f64 - 3.0),
                                    };
                                }
                            }
                            let mut expected = actual.clone();
                            let mut actual_work =
                                ProjectionWorkspace::new(7, rhs_count + 3).unwrap();
                            let mut expected_work = actual_work.clone();
                            for _ in 0..3 {
                                project_effect_in_place(
                                    &effect,
                                    &weights,
                                    &mut actual,
                                    &mut actual_work,
                                )
                                .unwrap();
                                scalar_project_effect_in_place(
                                    &effect,
                                    &weights,
                                    &mut expected,
                                    &mut expected_work,
                                )
                                .unwrap();
                                for (a, b) in actual.as_slice().iter().zip(expected.as_slice()) {
                                    assert_eq!(a.to_bits(), b.to_bits(), "intercept={intercept} slopes={slope_count} scale={scale} weighted={weighted} rhs={rhs_count}");
                                }
                                assert_eq!(actual_work.gram, expected_work.gram);
                                assert_eq!(
                                    actual_work.right_hand_sides,
                                    expected_work.right_hand_sides
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn scalar_project_effect_in_place(
        effect: &EffectTerm,
        weights: &[f64],
        values: &mut DenseMatrix,
        workspace: &mut ProjectionWorkspace,
    ) -> Result<()> {
        if weights.len() != values.nrows() || effect.codes().len() != values.nrows() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "projection inputs have inconsistent observation counts",
            ));
        }
        if weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "projection weights must be finite and positive",
            ));
        }
        let channels = effect.channels();
        let rhs_count = values.ncols();
        if channels > workspace.max_channels || rhs_count > workspace.rhs_count {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "projection workspace is smaller than the requested projection",
            ));
        }

        for level in 0..effect.n_levels() {
            let gram_length = channels * channels;
            let rhs_length = channels * rhs_count;
            workspace.gram[..gram_length].fill(0.0);
            workspace.right_hand_sides[..rhs_length].fill(0.0);

            for observation in effect.level_index().members(level) {
                let weight = weights[*observation];
                for left_channel in 0..channels {
                    let left_basis = effect.basis_value(left_channel, *observation);
                    for right_channel in 0..channels {
                        let right_basis = effect.basis_value(right_channel, *observation);
                        let index = left_channel * channels + right_channel;
                        workspace.gram[index] =
                            (weight * left_basis).mul_add(right_basis, workspace.gram[index]);
                    }
                    for rhs in 0..rhs_count {
                        let index = left_channel * rhs_count + rhs;
                        workspace.right_hand_sides[index] = (weight * left_basis).mul_add(
                            values[(*observation, rhs)],
                            workspace.right_hand_sides[index],
                        );
                    }
                }
            }

            solve_linear_system(
                &mut workspace.gram[..gram_length],
                &mut workspace.right_hand_sides[..rhs_length],
                channels,
                rhs_count,
                1e-12,
            )?;

            for observation in effect.level_index().members(level) {
                for rhs in 0..rhs_count {
                    let mut fitted = 0.0;
                    for channel in 0..channels {
                        fitted = effect.basis_value(channel, *observation).mul_add(
                            workspace.right_hand_sides[channel * rhs_count + rhs],
                            fitted,
                        );
                    }
                    values[(*observation, rhs)] -= fitted;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn parallel_groups_preserve_scalar_bits_across_rank_units_and_teams() {
        for workers in [2, 4, 8] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    let n = 257;
                    for intercept in [false, true] {
                        for scale in [1e-60, 1.0, 1e60] {
                            for rhs in [1, 7, 32, 33, 65] {
                                let x: Vec<_> = (0..n)
                                    .map(|r| scale * ((r * 13 % 37) as f64 - 18.0))
                                    .collect();
                                let effect = EffectTerm::new(
                                    (0..n).map(|r| (r % 11) as u32).collect(),
                                    13,
                                    intercept,
                                    vec![
                                        x.clone(),
                                        x.iter().map(|x| 2.0 * x).collect(),
                                        vec![0.0; n],
                                    ],
                                )
                                .unwrap();
                                let weights: Vec<_> =
                                    (0..n).map(|r| 0.25 + (r % 9) as f64 / 4.0).collect();
                                let mut expected = DenseMatrix::zeros(n, rhs).unwrap();
                                for r in 0..n {
                                    for j in 0..rhs {
                                        expected[(r, j)] = ((r * 17 + j * 3) % 41) as f64 - 20.0;
                                    }
                                }
                                let mut actual = expected.clone();
                                let mut serial = ProjectionWorkspace::new(4, rhs).unwrap();
                                let mut parallel = ParallelProjectionPlan {
                                    channels: 4,
                                    rhs,
                                    levels: 13,
                                    workers,
                                }
                                .allocate()
                                .unwrap();
                                for _ in 0..3 {
                                    scalar_project_effect_in_place(
                                        &effect,
                                        &weights,
                                        &mut expected,
                                        &mut serial,
                                    )
                                    .unwrap();
                                    project_parallel(&effect, &weights, &mut actual, &mut parallel)
                                        .unwrap();
                                    for (a, b) in actual.as_slice().iter().zip(expected.as_slice())
                                    {
                                        assert_eq!(a.to_bits(), b.to_bits());
                                    }
                                }
                            }
                        }
                    }
                });
        }
    }

    #[test]
    fn parallel_weighted_projection_matches_independent_two_column_formula() {
        let n = 311;
        let x: Vec<_> = (0..n).map(|r| (r % 13) as f64 - 6.0).collect();
        let weights: Vec<_> = (0..n).map(|r| 0.5 + (r % 7) as f64).collect();
        let y: Vec<_> = (0..n)
            .map(|r| ((r * 11) % 23) as f64 + 0.7 * x[r])
            .collect();
        let effect = EffectTerm::new(
            (0..n).map(|r| (r % 5) as u32).collect(),
            7,
            true,
            vec![x.clone()],
        )
        .unwrap();
        let mut expected = y.clone();
        for level in 0..5 {
            let rows: Vec<_> = (0..n).filter(|r| r % 5 == level).collect();
            let sw: f64 = rows.iter().map(|&r| weights[r]).sum();
            let mx: f64 = rows.iter().map(|&r| weights[r] * x[r]).sum::<f64>() / sw;
            let my: f64 = rows.iter().map(|&r| weights[r] * y[r]).sum::<f64>() / sw;
            let slope = rows
                .iter()
                .map(|&r| weights[r] * (x[r] - mx) * (y[r] - my))
                .sum::<f64>()
                / rows
                    .iter()
                    .map(|&r| weights[r] * (x[r] - mx).powi(2))
                    .sum::<f64>();
            for r in rows {
                expected[r] -= my + slope * (x[r] - mx);
            }
        }
        for workers in [1, 2, 4, 8] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    let mut actual = DenseMatrix::from_columns(&[y.clone()]).unwrap();
                    let mut workspace = ParallelProjectionPlan {
                        channels: 2,
                        rhs: 1,
                        levels: 7,
                        workers,
                    }
                    .allocate()
                    .unwrap();
                    project_parallel(&effect, &weights, &mut actual, &mut workspace).unwrap();
                    for (a, b) in actual.as_slice().iter().zip(&expected) {
                        assert!((a - b).abs() < 2e-13, "{a} {b}");
                    }
                });
        }
    }

    #[test]
    fn optional_projection_checks_capacity_overflow_and_validation_before_mutation() {
        let plan = ParallelProjectionPlan {
            channels: 2,
            rhs: 3,
            levels: 7,
            workers: 4,
        };
        let workspace = plan.allocate().unwrap();
        let actual = workspace.coefficients.capacity() * 8
            + workspace.workers.capacity() * std::mem::size_of::<ProjectionWorker>()
            + workspace
                .workers
                .iter()
                .map(|w| w.gram.capacity() * 8)
                .sum::<usize>();
        assert_eq!(actual as u128, plan.bytes().unwrap());
        assert!(ParallelProjectionPlan {
            levels: usize::MAX,
            ..plan
        }
        .bytes()
        .is_err());
        let design = FeDesign::new(
            6,
            vec![EffectTerm::new(
                vec![0, 0, 0, 1, 1, 1],
                2,
                true,
                vec![vec![0., 1., 2., 0., 1., 2.]],
            )
            .unwrap()],
        )
        .unwrap();
        let input = DenseMatrix::from_columns(&[vec![1., 4., 2., 5., 3., 7.]]).unwrap();
        for bad_plan in [Some(ParallelProjectionPlan { levels: 1, ..plan }), None] {
            let mut w = AutoProjectionWorkspace::new(&design, 1, bad_plan).unwrap();
            let mut values = input.clone();
            assert!(w
                .project(
                    &design.effects()[0],
                    &[1., 1., -1., 1., 1., 1.],
                    &mut values
                )
                .is_err());
            assert_eq!(values, input);
            if bad_plan.is_some() {
                assert!(w
                    .project(&design.effects()[0], &[1.; 6], &mut values)
                    .is_err());
                assert_eq!(values, input);
            }
        }
        let mut fallback = AutoProjectionWorkspace::new(
            &design,
            1,
            Some(ParallelProjectionPlan {
                levels: usize::MAX,
                ..plan
            }),
        )
        .unwrap();
        assert!(fallback.parallel.is_none());
        let mut values = input;
        fallback
            .project(&design.effects()[0], &[1.; 6], &mut values)
            .unwrap();
    }

    #[test]
    fn automatic_projection_plan_respects_team_size_and_design_scope() {
        for workers in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    for n in [16_383, 16_384] {
                        for slopes in [false, true] {
                            let effect = EffectTerm::new(
                                vec![0; n],
                                1,
                                true,
                                if slopes { vec![vec![1.; n]] } else { vec![] },
                            )
                            .unwrap();
                            let design = FeDesign::new(n, vec![effect]).unwrap();
                            assert_eq!(
                                ParallelProjectionPlan::for_design(&design, 1)
                                    .unwrap()
                                    .is_some(),
                                workers > 1 && n >= 16_384 && slopes
                            );
                            assert!(ParallelProjectionPlan::for_design(&design, 0)
                                .unwrap()
                                .is_none());
                        }
                    }
                });
        }
    }

    #[test]
    fn removes_weighted_group_intercepts_and_slopes() {
        let effect = EffectTerm::new(
            vec![0, 0, 0, 1, 1, 1],
            2,
            true,
            vec![vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0]],
        )
        .expect("effect");
        let weights = vec![1.0, 2.0, 1.0, 1.0, 1.0, 2.0];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 3.0, 5.0, 2.0, 1.0, 0.0]]).expect("matrix");
        let mut workspace = ProjectionWorkspace::new(2, 1).expect("workspace");
        project_effect_in_place(&effect, &weights, &mut values, &mut workspace)
            .expect("projection");
        assert!(values.as_slice().iter().all(|value| value.abs() < 1e-10));
    }
}
