#![allow(clippy::needless_range_loop)]

use crate::data::DenseMatrix;
use crate::data::WeightKind;
use crate::error::{ErrorCode, FeregError, Result};
use crate::ols::CertifiedLinearWithinDesign;
#[cfg(test)]
use crate::ols::{ClusterSubsetReport, VceKind, VceReport};
use crate::options::ThreadConfig;
use crate::parallel::ThreadTeam;
use crate::sandwich::{
    bread_meat_bread, reduce_clustered_score_meats, reduce_dense_score_meats, standard_errors,
    FixedBlockReductionPolicy, SandwichClusters, SandwichCorrectionPolicy, SandwichDesign,
    SandwichProductMode, SandwichScoreMultiplier, SandwichScorePair, SystemSandwichReservation,
};

use super::prepared::{checked_frequency_effective_observations, PreparedPpmlWeightReport};
use super::psd::{apply_ppml_psd_policy, PpmlPsdReport};

#[path = "factor_covariance.rs"]
mod factor_covariance;

const PPML_SANDWICH_REDUCTION: FixedBlockReductionPolicy = FixedBlockReductionPolicy {
    rows_per_block: 16_384,
    maximum_blocks: 64,
    partial_budget_bytes: 32 << 20,
};

// Count both independently accumulated triangles. Partitioning depends on
// matrix work, not the requested team, so every team retains the same merge tree.
const DENSE_SCORE_PRODUCTS_PER_BLOCK: u128 = 1 << 20;

fn dense_score_reduction(dimension: usize) -> FixedBlockReductionPolicy {
    let k = dimension as u128;
    let products_per_row = (k * (k + 1)).max(1);
    FixedBlockReductionPolicy {
        rows_per_block: (DENSE_SCORE_PRODUCTS_PER_BLOCK / products_per_row)
            .clamp(1, PPML_SANDWICH_REDUCTION.rows_per_block as u128)
            as usize,
        ..PPML_SANDWICH_REDUCTION
    }
}

/// Bound every possible final rank below the admitted finite width. The byte
/// cap can change the number of blocks after rank drops, so bound its complete
/// budget rather than assuming the maximum-rank block count stays authoritative.
pub(super) fn dense_score_partials_bound(rows: usize, dimension: usize) -> Result<u128> {
    if rows == 0 || dimension == 0 {
        return Ok(0);
    }
    let k = dimension as u128;
    let stride = k
        .checked_mul(k + 1)
        .and_then(|n| n.checked_mul(16))
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "PPML score partial dimensions overflow",
            )
        })?;
    let policy = dense_score_reduction(dimension);
    let blocks = rows
        .div_ceil(policy.rows_per_block)
        .min(policy.maximum_blocks) as u128;
    let requested = stride.checked_mul(blocks).ok_or_else(|| {
        FeregError::new(ErrorCode::Allocation, "PPML score partial bank overflows")
    })?;
    Ok(requested.min((policy.partial_budget_bytes as u128).max(stride)))
}

/// Bound ordered and paired clustered reducers at every possible final rank.
/// Paired tables replace the ordering banks and are explicitly bounded by two
/// observation-length word banks. The retained original covariance during a
/// factor check occupies the already-admitted final result covariance bank.
pub(super) fn clustered_score_workspace_bound(rows: usize, dimension: usize) -> Result<u128> {
    let k = dimension as u128;
    let square = k
        .checked_mul(k)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "PPML clustered square overflows"))?;
    let stride = square
        .checked_mul(2)
        .and_then(|value| value.checked_add(5 * k + 1))
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "PPML clustered partial stride overflows",
            )
        })?;
    let blocks = rows
        .div_ceil(PPML_SANDWICH_REDUCTION.rows_per_block)
        .min(PPML_SANDWICH_REDUCTION.maximum_blocks) as u128;
    let partials = stride
        .checked_mul(blocks)
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "PPML clustered partial bank overflows",
            )
        })?
        .min((PPML_SANDWICH_REDUCTION.partial_budget_bytes as u128).max(stride));
    let words = std::mem::size_of::<usize>() as u128;
    let order = square
        .checked_mul(32)
        .and_then(|value| value.checked_add(3 * rows as u128 * words))
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "PPML clustered ordering bound overflows",
            )
        })?;
    let reduction = square
        .checked_mul(64)
        .and_then(|value| value.checked_add(rows as u128 * words + partials + (blocks + 1) * words))
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "PPML clustered reduction bound overflows",
            )
        })?;
    Ok(order.max(reduction))
}

/// Translate only numerical failures emitted by the estimator-neutral
/// sandwich kernels into PPML's failed-publication-certificate code. The
/// shared kernels retain their historical OLS error behavior, and arbitrary
/// InvalidData errors returned by a malformed estimator design are preserved.
fn ppml_sandwich_result<T>(result: Result<T>) -> Result<T> {
    result.map_err(|error| {
        let numerical = error.code == ErrorCode::InvalidData
            && matches!(
                error.message.as_str(),
                "dense sandwich scaled score is not finite"
                    | "dense sandwich partial contains a nonfinite value"
                    | "qualified clustered ordered partial is nonfinite"
                    | "qualified clustered meat is nonfinite"
                    | "qualified clustered production meat is not finite"
                    | "qualified clustered certificate meat is not finite"
                    | "qualified clustered group count is invalid"
                    | "clustered sandwich subset partial contains a nonfinite value"
                    | "clustered sandwich group meat contains a nonfinite value"
                    | "clustered sandwich subset group count is invalid"
                    | "ordinary sandwich bread-meat product is not finite"
                    | "ordinary sandwich product is not finite"
                    | "compensated sandwich product is not finite"
                    | "sandwich covariance diagonal is invalid"
                    | "sandwich meat contains a nonfinite value"
                    | "sandwich symmetrized value is not finite"
                    | "sandwich compensated accumulator is not finite"
            );
        if numerical {
            FeregError::new(
                ErrorCode::NonConvergence,
                format!(
                    "PPML numerical sandwich certificate failed: {}",
                    error.message
                ),
            )
        } else {
            error
        }
    })
}

/// User-facing PPML covariance request. `Unadjusted` remains an accepted
/// compatibility spelling but maps to robust covariance before computation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PpmlVceRequest {
    Unadjusted,
    #[default]
    Robust,
    Clustered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlVceKind {
    Robust,
    Clustered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlTestDistribution {
    StandardNormal,
    ChiSquared,
}

/// PPML-specific degrees of freedom. No OLS correction policy enters this
/// calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpmlDofReport {
    pub effective_observations: usize,
    /// Numerical rank of the compact within design used by the sandwich.
    pub sandwich_rank: usize,
    /// Reported PPML finite rank. This additionally counts the one normalized
    /// structural-constant coordinate in an absorbed-intercept model.
    pub finite_within_rank: usize,
    pub absorbed_rank: usize,
    pub model_rank: usize,
    pub structural_residual: usize,
    pub reported: usize,
    pub minimum_cluster_groups: Option<usize>,
    pub connected_components: usize,
    pub absorbed_rank_is_approximate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpmlClusterSubsetReport {
    pub mask: u16,
    pub dimensions: Vec<usize>,
    pub groups: usize,
    pub sign: i8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlInferenceCertificate {
    pub bread_symmetry_relative_infinity_norm: f64,
    pub meat_relative_infinity_norm: f64,
    pub covariance_relative_infinity_norm: f64,
    pub covariance_symmetry_relative_infinity_norm: f64,
    pub wald_solve_relative_infinity_norm: f64,
    pub tolerance: f64,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlWaldReport {
    pub statistic: Option<f64>,
    pub degrees_of_freedom: usize,
    pub tested_columns: Vec<usize>,
    pub distribution: PpmlTestDistribution,
    pub solve_relative_infinity_norm: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlVceResult {
    pub requested: PpmlVceRequest,
    pub effective: PpmlVceKind,
    pub covariance: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub z_statistics: Vec<f64>,
    pub coefficient_distribution: PpmlTestDistribution,
    pub correction: f64,
    /// Group counts in declared cluster-dimension order. Empty for robust
    /// covariance; `min()` is the one correction/DoF authority.
    pub cluster_dimension_groups: Vec<usize>,
    pub cluster_subsets: Vec<PpmlClusterSubsetReport>,
    pub psd: PpmlPsdReport,
    pub certificate: PpmlInferenceCertificate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlInferenceResult {
    pub dof: PpmlDofReport,
    pub vce: PpmlVceResult,
    pub wald: PpmlWaldReport,
}

/// Static final-within row source for PPML inference. Implementations may be
/// resident or scratch-backed; `Copy + Sync` on the concrete adapter keeps the
/// shared sandwich hot loops free of trait objects and per-row allocation.
pub trait PpmlInferenceDesign {
    fn nrows(&self) -> usize;
    fn dimension(&self) -> usize;
    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()>;

    /// Whether bounded consecutive scans avoid expensive pointwise access.
    fn prefers_sequential_scan(&self) -> bool {
        false
    }

    /// Visit an immutable contiguous block using caller-owned row storage.
    /// Sources may keep bounded reader state local to this call; the callback
    /// may overwrite the row after consuming its unscaled values.
    fn visit_range(
        &self,
        rows: std::ops::Range<usize>,
        output: &mut [f64],
        mut visit: impl FnMut(usize, &mut [f64]) -> Result<()>,
    ) -> Result<()>
    where
        Self: Sized,
    {
        for row in rows {
            self.fill_row(row, output)?;
            visit(row, output)?;
        }
        Ok(())
    }
}

impl PpmlInferenceDesign for &DenseMatrix {
    fn nrows(&self) -> usize {
        DenseMatrix::nrows(self)
    }

    fn dimension(&self) -> usize {
        DenseMatrix::ncols(self)
    }

    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        let rows = DenseMatrix::nrows(self);
        let dimension = DenseMatrix::ncols(self);
        if row >= rows || output.len() != dimension {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML dense inference row is dimensionally inconsistent",
            ));
        }
        let start = row.checked_mul(dimension).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "PPML dense inference row overflowed")
        })?;
        output.copy_from_slice(&self.as_slice()[start..start + dimension]);
        Ok(())
    }
}

impl PpmlInferenceDesign for CertifiedLinearWithinDesign<'_> {
    fn prefers_sequential_scan(&self) -> bool {
        SandwichDesign::prefers_sequential_scan(self)
    }

    fn nrows(&self) -> usize {
        SandwichDesign::nrows(self)
    }

    fn dimension(&self) -> usize {
        SandwichDesign::dimension(self)
    }

    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        SandwichDesign::fill_row(self, row, output)
    }

    fn visit_range(
        &self,
        rows: std::ops::Range<usize>,
        output: &mut [f64],
        visit: impl FnMut(usize, &mut [f64]) -> Result<()>,
    ) -> Result<()> {
        SandwichDesign::visit_range(self, rows, output, visit)
    }
}

/// Final strict-state inputs for PPML inference.
///
/// `within_design` is the compact, unscaled canonical within design in frozen
/// solve order. `identified_bread` is the matching rank-by-rank inverse of
/// `sum q*mu*x_tilde*x_tilde'`; neither input expands to `N * full_dimension`.
/// `kept_full_slots` maps compact columns into reported coefficient slots and
/// may have any order, but must be unique. `score_residuals` are the public raw
/// values `y-mu`; the adapter applies estimator- and weight-specific scalars.
pub struct PpmlInferenceInput<'a, D = &'a DenseMatrix> {
    /// Full reported coefficient vector, including omitted slots.
    pub coefficients: &'a [f64],
    pub full_dimension: usize,
    pub within_design: D,
    /// Public finite rank. Equal to the compact design dimension, except that
    /// an absorbed structural constant contributes one reported coordinate
    /// without entering the sandwich design.
    pub reported_finite_rank: usize,
    pub score_residuals: &'a [f64],
    pub prior_weights: &'a [f64],
    pub weight_report: PreparedPpmlWeightReport,
    pub identified_bread: &'a [f64],
    pub kept_full_slots: &'a [usize],
    pub absorbed_rank: usize,
    pub connected_components: usize,
    pub absorbed_rank_is_approximate: bool,
    /// Borrowed compact cluster codes in declared dimension order. Only the
    /// `Q <= 5` slice table is materialized; observation-scale codes remain
    /// owned by the prepared model generation.
    pub clusters: &'a [&'a [u32]],
    pub requested_vce: PpmlVceRequest,
    /// Explicit model-test coordinates, normally identified slopes excluding
    /// the structural constant normalization.
    pub wald_columns: &'a [usize],
    pub certificate_tolerance: f64,
}

pub fn compute_ppml_inference<D>(input: PpmlInferenceInput<'_, D>) -> Result<PpmlInferenceResult>
where
    D: PpmlInferenceDesign + Copy + Sync,
{
    compute_ppml_inference_interruptible_with_threads(
        input,
        ThreadConfig::default(),
        &mut || Ok(()),
    )
}

/// Configured-team PPML inference boundary. The caller-owned interrupt probe
/// remains on the caller thread; only complete robust or clustered sandwich
/// reductions enter the bounded Rayon team.
pub(crate) fn compute_ppml_inference_interruptible_with_threads<D>(
    input: PpmlInferenceInput<'_, D>,
    threads: ThreadConfig,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<PpmlInferenceResult>
where
    D: PpmlInferenceDesign + Copy + Sync,
{
    interrupt()?;
    validate_ppml_inference(&input)?;
    let rows = input.within_design.nrows();
    let full_dimension = input.full_dimension;
    let rank = input.within_design.dimension();
    let effective_kind = match input.requested_vce {
        PpmlVceRequest::Unadjusted | PpmlVceRequest::Robust => PpmlVceKind::Robust,
        PpmlVceRequest::Clustered => PpmlVceKind::Clustered,
    };
    let bread_symmetry = matrix_symmetry_error(input.identified_bread, rank)?;
    let mut reservation = SystemSandwichReservation;
    let (production_meat, certificate_meat, cluster_subsets) = if rank == 0 {
        let reports = if effective_kind == PpmlVceKind::Clustered {
            // The shared reducer intentionally rejects a zero-dimensional
            // design.  This is the only path that factors cluster tuples
            // independently; nonzero-rank inference obtains group counts from
            // the same shared pass that builds the meat.
            cluster_subset_inventory_interruptible(input.clusters, rows, interrupt)?
        } else {
            Vec::new()
        };
        (Vec::new(), Vec::new(), reports)
    } else {
        let design = PpmlSandwichDesign {
            source: input.within_design,
        };
        let mode = if effective_kind == PpmlVceKind::Robust {
            PpmlScoreMode::Robust
        } else {
            PpmlScoreMode::Clustered
        };
        let multipliers = PpmlScoreMultipliers {
            residuals: input.score_residuals,
            prior_weights: input.prior_weights,
            weight_kind: input.weight_report.kind,
            mode,
        };
        interrupt()?;
        match effective_kind {
            PpmlVceKind::Robust => {
                let meats = ThreadTeam::new(threads).install(|| {
                    ppml_sandwich_result(reduce_dense_score_meats(
                        design,
                        multipliers,
                        dense_score_reduction(rank),
                        None,
                        &mut reservation,
                    ))
                })??;
                interrupt()?;
                (meats.production, meats.certificate, Vec::new())
            }
            PpmlVceKind::Clustered => {
                let mut cluster_slices = Vec::new();
                cluster_slices
                    .try_reserve_exact(input.clusters.len())
                    .map_err(|_| {
                        FeregError::new(
                            ErrorCode::Allocation,
                            "unable to reserve PPML cluster slice views",
                        )
                    })?;
                cluster_slices.extend(input.clusters.iter().copied());
                let clusters = SandwichClusters::new(&cluster_slices, rows, 5)?;
                let meats = ThreadTeam::new(threads).install(|| {
                    ppml_sandwich_result(reduce_clustered_score_meats(
                        design,
                        multipliers,
                        clusters,
                        PPML_SANDWICH_REDUCTION,
                        None,
                        &mut reservation,
                    ))
                })??;
                interrupt()?;
                let mut reports = Vec::new();
                reports
                    .try_reserve_exact(meats.subsets.len())
                    .map_err(|_| {
                        FeregError::new(
                            ErrorCode::Allocation,
                            "unable to reserve PPML cluster subset reports",
                        )
                    })?;
                for report in &meats.subsets {
                    interrupt()?;
                    reports.push(PpmlClusterSubsetReport {
                        mask: report.mask,
                        dimensions: mask_dimensions(report.mask, input.clusters.len())?,
                        groups: report.groups,
                        sign: report.sign,
                    });
                }
                validate_cluster_subset_reports(&reports, input.clusters.len())?;
                (meats.production, meats.certificate, reports)
            }
        }
    };

    let minimum_cluster_groups = cluster_subsets
        .iter()
        .filter(|report| report.dimensions.len() == 1)
        .map(|report| report.groups)
        .min();
    let cluster_dimension_groups = if effective_kind == PpmlVceKind::Clustered {
        let mut groups = filled(
            input.clusters.len(),
            0_usize,
            "unable to reserve PPML cluster-dimension group counts",
        )?;
        for report in cluster_subsets
            .iter()
            .filter(|report| report.dimensions.len() == 1)
        {
            interrupt()?;
            groups[report.dimensions[0]] = report.groups;
        }
        if groups.iter().any(|groups| *groups < 2) {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML cluster-dimension group counts are incomplete",
            ));
        }
        groups
    } else {
        Vec::new()
    };
    let dof = compute_ppml_dof(
        input.weight_report.effective_observations,
        rank,
        input.reported_finite_rank,
        input.absorbed_rank,
        minimum_cluster_groups,
        input.connected_components,
        input.absorbed_rank_is_approximate,
    )?;
    interrupt()?;

    let correction = match effective_kind {
        PpmlVceKind::Robust => {
            dof.effective_observations as f64 / (dof.effective_observations - 1) as f64
        }
        PpmlVceKind::Clustered => {
            let groups = dof.minimum_cluster_groups.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "clustered PPML inference lacks a minimum group count",
                )
            })?;
            groups as f64 / (groups - 1) as f64
        }
    };
    if !correction.is_finite() || correction <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML covariance correction is not finite and positive",
        ));
    }

    let (mut raw_covariance, meat_error, mut covariance_error, factor_check) = if rank == 0 {
        (Vec::new(), 0.0, 0.0, false)
    } else {
        let meat_error = matrix_relative_difference(&production_meat, &certificate_meat, rank)?;
        let production_covariance = ppml_sandwich_result(bread_meat_bread(
            input.identified_bread,
            &production_meat,
            rank,
            SandwichCorrectionPolicy::Scale(correction),
            SandwichProductMode::CompensatedFull,
            &mut reservation,
        ))?;
        interrupt()?;
        let mut certificate_covariance = ppml_sandwich_result(bread_meat_bread(
            input.identified_bread,
            &certificate_meat,
            rank,
            SandwichCorrectionPolicy::Scale(correction),
            SandwichProductMode::CompensatedFull,
            &mut reservation,
        ))?;
        interrupt()?;
        let covariance_error =
            matrix_relative_difference(&production_covariance, &certificate_covariance, rank)?;
        let factor_check = (effective_kind == PpmlVceKind::Robust || input.clusters.len() <= 1)
            && factor_covariance::needs_factor_check(
                &input,
                &production_meat,
                &production_covariance,
                &mut certificate_covariance,
                minimum_cluster_groups.unwrap_or(rows),
                correction,
            );
        (
            production_covariance,
            meat_error,
            covariance_error,
            factor_check,
        )
    };
    drop(production_meat);
    drop(certificate_meat);

    let cluster_dimensions = if effective_kind == PpmlVceKind::Clustered {
        input.clusters.len()
    } else {
        0
    };
    if factor_check {
        let (reconstructed, error) =
            factor_covariance::reconstruct(&input, effective_kind, threads, correction, interrupt)?;
        if error > input.certificate_tolerance {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML influence covariance failed independent certification",
            ));
        }
        let disagreement = matrix_relative_difference(&raw_covariance, &reconstructed, rank)?;
        if disagreement > input.certificate_tolerance
            || (0..rank).any(|row| raw_covariance[row * rank + row] < 0.0)
        {
            raw_covariance = reconstructed;
            covariance_error = error;
        }
    }
    let (identified_covariance, psd) = apply_ppml_psd_policy(
        &raw_covariance,
        rank,
        cluster_dimensions,
        input.certificate_tolerance,
    )?;
    drop(raw_covariance);
    interrupt()?;
    let covariance_symmetry = matrix_symmetry_error(&identified_covariance, rank)?;
    let mut reservation = SystemSandwichReservation;
    let identified_standard_errors = ppml_sandwich_result(standard_errors(
        &identified_covariance,
        rank,
        &mut reservation,
    ))?;
    let covariance = expand_identified_square(
        &identified_covariance,
        full_dimension,
        input.kept_full_slots,
    )?;
    let standard_errors = expand_identified_vector(
        &identified_standard_errors,
        full_dimension,
        input.kept_full_slots,
        f64::NAN,
    )?;
    let mut z_statistics = filled(
        full_dimension,
        f64::NAN,
        "unable to reserve PPML z statistics",
    )?;
    for slot in 0..full_dimension {
        let standard_error = standard_errors[slot];
        if standard_error.is_finite() && standard_error > 0.0 {
            z_statistics[slot] = input.coefficients[slot] / standard_error;
        }
    }
    let wald = compute_ppml_wald(
        input.coefficients,
        &covariance,
        full_dimension,
        input.wald_columns,
        input.certificate_tolerance,
    )?;
    interrupt()?;
    let certificate_passed = bread_symmetry <= input.certificate_tolerance
        && meat_error <= input.certificate_tolerance
        && covariance_error <= input.certificate_tolerance
        && covariance_symmetry <= input.certificate_tolerance
        && wald.solve_relative_infinity_norm <= input.certificate_tolerance
        && psd.passed;
    if !certificate_passed {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!(
                "PPML inference certificate failed: bread={bread_symmetry:.3e}, meat={meat_error:.3e}, covariance={covariance_error:.3e}, symmetry={covariance_symmetry:.3e}, wald={:.3e}",
                wald.solve_relative_infinity_norm,
            ),
        ));
    }
    Ok(PpmlInferenceResult {
        dof,
        vce: PpmlVceResult {
            requested: input.requested_vce,
            effective: effective_kind,
            covariance,
            standard_errors,
            z_statistics,
            coefficient_distribution: PpmlTestDistribution::StandardNormal,
            correction,
            cluster_dimension_groups,
            cluster_subsets,
            psd,
            certificate: PpmlInferenceCertificate {
                bread_symmetry_relative_infinity_norm: bread_symmetry,
                meat_relative_infinity_norm: meat_error,
                covariance_relative_infinity_norm: covariance_error,
                covariance_symmetry_relative_infinity_norm: covariance_symmetry,
                wald_solve_relative_infinity_norm: wald.solve_relative_infinity_norm,
                tolerance: input.certificate_tolerance,
                passed: certificate_passed,
            },
        },
        wald,
    })
}

pub fn compute_ppml_dof(
    effective_observations: usize,
    sandwich_rank: usize,
    finite_within_rank: usize,
    absorbed_rank: usize,
    minimum_cluster_groups: Option<usize>,
    connected_components: usize,
    absorbed_rank_is_approximate: bool,
) -> Result<PpmlDofReport> {
    let model_rank = finite_within_rank
        .checked_add(absorbed_rank)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "PPML model rank overflowed"))?;
    if model_rank >= effective_observations {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            format!(
                "PPML model has no positive structural residual degrees of freedom: N={effective_observations}, rank={model_rank}"
            ),
        ));
    }
    let structural_residual = effective_observations - model_rank;
    let reported = if let Some(groups) = minimum_cluster_groups {
        if groups < 2 {
            return Err(FeregError::new(
                ErrorCode::InsufficientObservations,
                "PPML clustered inference requires at least two groups in every dimension",
            ));
        }
        structural_residual.min(groups - 1)
    } else {
        structural_residual
    };
    if reported == 0 {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "PPML reported degrees of freedom are not positive",
        ));
    }
    Ok(PpmlDofReport {
        effective_observations,
        sandwich_rank,
        finite_within_rank,
        absorbed_rank,
        model_rank,
        structural_residual,
        reported,
        minimum_cluster_groups,
        connected_components,
        absorbed_rank_is_approximate,
    })
}

#[derive(Debug, Clone, Copy)]
struct PpmlSandwichDesign<D> {
    source: D,
}

impl<D> SandwichDesign for PpmlSandwichDesign<D>
where
    D: PpmlInferenceDesign,
{
    fn prefers_sequential_scan(&self) -> bool {
        self.source.prefers_sequential_scan()
    }

    fn nrows(&self) -> usize {
        self.source.nrows()
    }

    fn dimension(&self) -> usize {
        self.source.dimension()
    }

    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        self.source.fill_row(row, output)
    }

    fn visit_range(
        &self,
        rows: std::ops::Range<usize>,
        output: &mut [f64],
        visit: impl FnMut(usize, &mut [f64]) -> Result<()>,
    ) -> Result<()> {
        self.source.visit_range(rows, output, visit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PpmlScoreMode {
    Robust,
    Clustered,
}

#[derive(Debug, Clone, Copy)]
struct PpmlScoreMultipliers<'a> {
    residuals: &'a [f64],
    prior_weights: &'a [f64],
    weight_kind: Option<WeightKind>,
    mode: PpmlScoreMode,
}

impl SandwichScoreMultiplier for PpmlScoreMultipliers<'_> {
    fn nrows(&self) -> usize {
        self.residuals.len()
    }

    fn score_pair(&self, row: usize) -> Result<SandwichScorePair> {
        let residual = *self.residuals.get(row).ok_or_else(|| {
            FeregError::new(ErrorCode::InvalidArgument, "PPML score row is out of range")
        })?;
        let weight = *self.prior_weights.get(row).ok_or_else(|| {
            FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML weight row is out of range",
            )
        })?;
        let (production, certificate) = match (self.mode, self.weight_kind) {
            (PpmlScoreMode::Robust, Some(WeightKind::Frequency)) => {
                (weight.sqrt() * residual, residual * weight.sqrt())
            }
            (PpmlScoreMode::Robust, None) => (residual, residual),
            (PpmlScoreMode::Robust, Some(WeightKind::Probability))
            | (PpmlScoreMode::Clustered, _) => (weight * residual, residual * weight),
            (_, Some(WeightKind::Analytic | WeightKind::Importance)) => {
                return Err(FeregError::new(
                    ErrorCode::Unsupported,
                    "PPML sandwich does not support analytic or importance weights",
                ));
            }
        };
        if production.is_finite() && certificate.is_finite() {
            Ok(SandwichScorePair {
                production,
                certificate,
            })
        } else {
            Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML sandwich score is not finitely representable",
            ))
        }
    }
}

fn validate_ppml_inference<D>(input: &PpmlInferenceInput<'_, D>) -> Result<()>
where
    D: PpmlInferenceDesign,
{
    let rows = input.within_design.nrows();
    let rank = input.within_design.dimension();
    let bread_elements = rank.checked_mul(rank).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Allocation,
            "PPML bread dimensions overflow usize",
        )
    })?;
    if rows == 0
        || input.coefficients.len() != input.full_dimension
        || input.kept_full_slots.len() != rank
        || input.reported_finite_rank < rank
        || input.reported_finite_rank > rank.saturating_add(1)
        || input.reported_finite_rank > input.full_dimension
        || input.score_residuals.len() != rows
        || input.prior_weights.len() != rows
        || input.identified_bread.len() != bread_elements
        || !input.certificate_tolerance.is_finite()
        || input.certificate_tolerance <= 0.0
        || input.certificate_tolerance >= 1.0
        || input
            .coefficients
            .iter()
            .chain(input.score_residuals)
            .chain(input.prior_weights)
            .chain(input.identified_bread)
            .any(|value| !value.is_finite())
        || input.prior_weights.iter().any(|weight| *weight <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML inference inputs are nonfinite or dimensionally inconsistent",
        ));
    }
    let mut kept_slots = filled(
        input.full_dimension,
        false,
        "unable to reserve PPML kept-slot validation state",
    )?;
    for &slot in input.kept_full_slots {
        if slot >= input.full_dimension || kept_slots[slot] {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML compact-to-full coefficient mapping is duplicated or out of range",
            ));
        }
        kept_slots[slot] = true;
    }
    if input
        .wald_columns
        .iter()
        .enumerate()
        .any(|(position, column)| {
            *column >= input.full_dimension
                || !kept_slots[*column]
                || (position > 0 && input.wald_columns[position - 1] >= *column)
        })
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML Wald slots must be a strictly increasing subset of identified full slots",
        ));
    }
    if input.clusters.len() > 5
        || input.clusters.iter().any(|cluster| cluster.len() != rows)
        || (input.requested_vce == PpmlVceRequest::Clustered && input.clusters.is_empty())
        || (input.requested_vce != PpmlVceRequest::Clustered && !input.clusters.is_empty())
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML cluster inputs do not match the requested covariance",
        ));
    }
    validate_weight_report(input.prior_weights, input.weight_report, rows)
}

pub(crate) fn validate_weight_report(
    weights: &[f64],
    report: PreparedPpmlWeightReport,
    rows: usize,
) -> Result<()> {
    if report.physical_observations != rows
        || !report.prepared_weight_sum.is_finite()
        || report.prepared_weight_sum <= 0.0
        || !report.log_raw_to_prepared_multiplier.is_finite()
        || report
            .raw_weight_sum
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        || report
            .raw_to_prepared_multiplier
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML inference weight report is inconsistent with the final sample",
        ));
    }
    let frequency_effective = if report.kind == Some(WeightKind::Frequency) {
        Some(checked_frequency_effective_observations(weights)?)
    } else {
        None
    };
    let sum = compensated_positive_sum(weights)?;
    let tolerance = 128.0 * f64::EPSILON * sum.abs().max(report.prepared_weight_sum.abs()).max(1.0);
    if !sum.is_finite() || (sum - report.prepared_weight_sum).abs() > tolerance {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML prepared weights disagree with their reported sum",
        ));
    }
    match report.kind {
        None => {
            if report.effective_observations != rows
                || report.raw_weight_sum.is_some()
                || report.log_raw_weight_sum.is_some()
                || report.raw_to_prepared_multiplier != Some(1.0)
                || report.log_raw_to_prepared_multiplier != 0.0
                || weights.iter().any(|weight| *weight != 1.0)
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "unweighted PPML inference does not have unit-weight metadata",
                ));
            }
        }
        Some(WeightKind::Frequency) => {
            let effective = frequency_effective.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "PPML frequency-weight validation lost its effective N",
                )
            })?;
            let raw_sum = report.raw_weight_sum.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::InvalidArgument,
                    "PPML frequency-weight report lacks its raw multiplicity sum",
                )
            })?;
            let expected_sum = effective as f64;
            if !raw_sum.is_finite()
                || raw_sum <= 0.0
                || effective != report.effective_observations
                || report.raw_to_prepared_multiplier != Some(1.0)
                || report.log_raw_to_prepared_multiplier != 0.0
                || !log_matches_positive(
                    report.log_raw_weight_sum,
                    raw_sum,
                    "frequency-weight raw sum",
                )?
                || (raw_sum - expected_sum).abs()
                    > 128.0 * f64::EPSILON * raw_sum.abs().max(expected_sum.abs()).max(1.0)
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "PPML frequency-weight effective N is inconsistent",
                ));
            }
        }
        Some(WeightKind::Probability) => {
            let scale = rows as f64;
            let log_raw_sum = report.log_raw_weight_sum.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::InvalidArgument,
                    "PPML probability-weight report lacks its log raw weight sum",
                )
            })?;
            let expected_log_multiplier = scale.ln() - log_raw_sum;
            if report.effective_observations != rows
                || !log_raw_sum.is_finite()
                || !expected_log_multiplier.is_finite()
                || (sum - scale).abs() > 128.0 * f64::EPSILON * scale.max(1.0)
                || (report.log_raw_to_prepared_multiplier - expected_log_multiplier).abs()
                    > 256.0 * f64::EPSILON * expected_log_multiplier.abs().max(1.0)
                || !optional_log_representation_is_consistent(report.raw_weight_sum, log_raw_sum)?
                || !optional_log_representation_is_consistent(
                    report.raw_to_prepared_multiplier,
                    report.log_raw_to_prepared_multiplier,
                )?
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "PPML probability weights are not normalized to physical N",
                ));
            }
        }
        Some(WeightKind::Analytic | WeightKind::Importance) => {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "PPML inference supports only fweights and pweights",
            ));
        }
    }
    Ok(())
}

fn compensated_positive_sum(values: &[f64]) -> Result<f64> {
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for &value in values {
        if !value.is_finite() || value <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "PPML prepared weight is not finite and positive",
            ));
        }
        let next = sum + value;
        if sum.abs() >= value.abs() {
            correction += (sum - next) + value;
        } else {
            correction += (value - next) + sum;
        }
        sum = next;
        if !sum.is_finite() || !correction.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "PPML prepared weight sum overflowed",
            ));
        }
    }
    let total = sum + correction;
    if total.is_finite() && total > 0.0 {
        Ok(total)
    } else {
        Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML prepared weight sum is not positive and finite",
        ))
    }
}

fn log_matches_positive(log_value: Option<f64>, value: f64, label: &'static str) -> Result<bool> {
    let log_value = log_value.ok_or_else(|| {
        FeregError::new(
            ErrorCode::InvalidArgument,
            format!("PPML {label} log metadata is absent"),
        )
    })?;
    Ok(log_value.is_finite()
        && (log_value - value.ln()).abs() <= 256.0 * f64::EPSILON * log_value.abs().max(1.0))
}

fn optional_log_representation_is_consistent(
    represented: Option<f64>,
    log_value: f64,
) -> Result<bool> {
    if !log_value.is_finite() {
        return Ok(false);
    }
    if let Some(value) = represented {
        return Ok(value.is_finite()
            && value > 0.0
            && (value.ln() - log_value).abs() <= 256.0 * f64::EPSILON * log_value.abs().max(1.0));
    }
    let exponentiated = log_value.exp();
    Ok(!exponentiated.is_finite() || exponentiated == 0.0)
}

fn cluster_subset_inventory_interruptible(
    clusters: &[&[u32]],
    rows: usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Vec<PpmlClusterSubsetReport>> {
    if clusters.is_empty() || clusters.len() > 5 || clusters.iter().any(|value| value.len() != rows)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML cluster inventory is empty or misaligned",
        ));
    }
    let subset_count = (1_usize << clusters.len()) - 1;
    let mut reports = Vec::new();
    reports.try_reserve_exact(subset_count).map_err(|_| {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve PPML cluster subset inventory",
        )
    })?;
    for cardinality in 1..=clusters.len() {
        for mask in 1..=subset_count {
            if mask.count_ones() as usize != cardinality {
                continue;
            }
            interrupt()?;
            let expected_mask = u16::try_from(mask).map_err(|_| {
                FeregError::new(ErrorCode::Allocation, "PPML cluster mask exceeds u16")
            })?;
            let dimensions = mask_dimensions(expected_mask, clusters.len())?;
            let groups = cluster_subset_group_count(clusters, &dimensions, rows)?;
            if groups < 2 {
                return Err(FeregError::new(
                    ErrorCode::InsufficientObservations,
                    format!("PPML cluster subset {dimensions:?} has fewer than two groups"),
                ));
            }
            reports.push(PpmlClusterSubsetReport {
                mask: expected_mask,
                dimensions,
                groups,
                sign: if cardinality % 2 == 1 { 1 } else { -1 },
            });
        }
    }
    Ok(reports)
}

fn cluster_subset_group_count(
    clusters: &[&[u32]],
    dimensions: &[usize],
    rows: usize,
) -> Result<usize> {
    if rows == 0
        || dimensions.is_empty()
        || dimensions
            .iter()
            .any(|dimension| *dimension >= clusters.len())
        || clusters.iter().any(|codes| codes.len() != rows)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML cluster subset ordering is empty or misaligned",
        ));
    }
    // Rank-zero inference needs only the exact group count, not N owned
    // cluster tuples and their duplicated level table. One fallibly reserved
    // row ordering is sufficient; unstable slice sorting is in-place.
    let mut order = Vec::new();
    order.try_reserve_exact(rows).map_err(|_| {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve PPML rank-zero cluster subset ordering",
        )
    })?;
    for row in 0..rows {
        order.push(row);
    }
    order.sort_unstable_by(|left, right| {
        for &dimension in dimensions {
            let ordering = clusters[dimension][*left].cmp(&clusters[dimension][*right]);
            if ordering != std::cmp::Ordering::Equal {
                return ordering;
            }
        }
        left.cmp(right)
    });
    let mut groups = 1_usize;
    for adjacent in order.windows(2) {
        if dimensions
            .iter()
            .any(|dimension| clusters[*dimension][adjacent[0]] != clusters[*dimension][adjacent[1]])
        {
            groups = groups.checked_add(1).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "PPML rank-zero cluster group count overflows usize",
                )
            })?;
        }
    }
    Ok(groups)
}

fn validate_cluster_subset_reports(
    reports: &[PpmlClusterSubsetReport],
    cluster_dimensions: usize,
) -> Result<()> {
    let expected_count = (1_usize << cluster_dimensions) - 1;
    if cluster_dimensions == 0 || cluster_dimensions > 5 || reports.len() != expected_count {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "shared PPML cluster reduction returned an invalid subset inventory",
        ));
    }
    let mut position = 0_usize;
    for cardinality in 1..=cluster_dimensions {
        for mask in 1..=expected_count {
            if mask.count_ones() as usize != cardinality {
                continue;
            }
            let report = &reports[position];
            let expected_mask = u16::try_from(mask).map_err(|_| {
                FeregError::new(ErrorCode::Allocation, "PPML cluster mask exceeds u16")
            })?;
            let expected_sign = if cardinality % 2 == 1 { 1 } else { -1 };
            if report.mask != expected_mask
                || report.dimensions.len() != cardinality
                || !report
                    .dimensions
                    .iter()
                    .copied()
                    .eq((0..cluster_dimensions)
                        .filter(|dimension| mask & (1_usize << dimension) != 0))
                || report.groups < 2
                || report.sign != expected_sign
            {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "shared PPML cluster reduction returned inconsistent subset metadata",
                ));
            }
            position += 1;
        }
    }
    Ok(())
}

fn mask_dimensions(mask: u16, dimensions: usize) -> Result<Vec<usize>> {
    let count = (0..dimensions)
        .filter(|dimension| mask & (1_u16 << dimension) != 0)
        .count();
    let mut output = Vec::new();
    output.try_reserve_exact(count).map_err(|_| {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve PPML cluster subset dimensions",
        )
    })?;
    for dimension in 0..dimensions {
        if mask & (1_u16 << dimension) != 0 {
            output.push(dimension);
        }
    }
    Ok(output)
}

/// Temporary bridge for the pre-prepared PPML prototype. New production
/// callers already own the compact frozen design and never enter this path.
#[cfg(test)]
fn select_design_columns(matrix: &DenseMatrix, kept: &[usize]) -> Result<DenseMatrix> {
    if kept
        .iter()
        .enumerate()
        .any(|(position, column)| *column >= matrix.ncols() || kept[..position].contains(column))
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "legacy PPML kept columns are duplicated or out of range",
        ));
    }
    let elements = matrix.nrows().checked_mul(kept.len()).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Allocation,
            "legacy PPML compact design dimensions overflow usize",
        )
    })?;
    let mut values = filled(
        elements,
        0.0_f64,
        "unable to reserve legacy PPML compact design",
    )?;
    for row in 0..matrix.nrows() {
        for (compact_column, &source_column) in kept.iter().enumerate() {
            values[row * kept.len() + compact_column] = matrix[(row, source_column)];
        }
    }
    DenseMatrix::from_row_major(matrix.nrows(), kept.len(), values)
}

#[cfg(test)]
fn select_identified_square(
    matrix: &[f64],
    full_dimension: usize,
    kept: &[usize],
) -> Result<Vec<f64>> {
    let mut selected = filled(
        kept.len().checked_mul(kept.len()).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "PPML bread dimensions overflow")
        })?,
        0.0_f64,
        "unable to reserve PPML identified bread",
    )?;
    for (out_row, &row) in kept.iter().enumerate() {
        for (out_column, &column) in kept.iter().enumerate() {
            selected[out_row * kept.len() + out_column] = matrix[row * full_dimension + column];
        }
    }
    Ok(selected)
}

fn expand_identified_square(
    identified: &[f64],
    full_dimension: usize,
    kept: &[usize],
) -> Result<Vec<f64>> {
    if identified.len() != kept.len().saturating_mul(kept.len()) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML identified covariance dimensions are inconsistent",
        ));
    }
    let mut output = filled(
        full_dimension.checked_mul(full_dimension).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "PPML covariance dimensions overflow")
        })?,
        0.0_f64,
        "unable to reserve PPML covariance",
    )?;
    for (out_row, &row) in kept.iter().enumerate() {
        for (out_column, &column) in kept.iter().enumerate() {
            output[row * full_dimension + column] = identified[out_row * kept.len() + out_column];
        }
    }
    Ok(output)
}

fn expand_identified_vector(
    identified: &[f64],
    full_dimension: usize,
    kept: &[usize],
    omitted: f64,
) -> Result<Vec<f64>> {
    if identified.len() != kept.len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML identified vector dimensions are inconsistent",
        ));
    }
    let mut output = filled(
        full_dimension,
        omitted,
        "unable to reserve PPML coefficient inference vector",
    )?;
    for (position, &column) in kept.iter().enumerate() {
        output[column] = identified[position];
    }
    Ok(output)
}

fn matrix_relative_difference(left: &[f64], right: &[f64], dimension: usize) -> Result<f64> {
    if left.len() != dimension.saturating_mul(dimension) || right.len() != left.len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML inference certificate matrices are misaligned",
        ));
    }
    let mut error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = f64::MIN_POSITIVE;
        for column in 0..dimension {
            let position = row * dimension + column;
            row_error += (left[position] - right[position]).abs();
            row_scale += left[position].abs().max(right[position].abs());
        }
        error = error.max(row_error / row_scale);
    }
    if error.is_finite() {
        Ok(error)
    } else {
        Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML inference certificate difference is not finite",
        ))
    }
}

fn matrix_symmetry_error(matrix: &[f64], dimension: usize) -> Result<f64> {
    if matrix.len() != dimension.saturating_mul(dimension) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML symmetry certificate dimensions are inconsistent",
        ));
    }
    let mut error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = f64::MIN_POSITIVE;
        for column in 0..dimension {
            let left = matrix[row * dimension + column];
            let right = matrix[column * dimension + row];
            row_error += (left - right).abs();
            row_scale += left.abs().max(right.abs());
        }
        error = error.max(row_error / row_scale);
    }
    if error.is_finite() {
        Ok(error)
    } else {
        Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML symmetry certificate is not finite",
        ))
    }
}

pub(crate) fn compute_ppml_wald(
    coefficients: &[f64],
    covariance: &[f64],
    full_dimension: usize,
    tested: &[usize],
    tolerance: f64,
) -> Result<PpmlWaldReport> {
    if tested.is_empty() {
        return Ok(PpmlWaldReport {
            statistic: None,
            degrees_of_freedom: 0,
            tested_columns: Vec::new(),
            distribution: PpmlTestDistribution::ChiSquared,
            solve_relative_infinity_norm: 0.0,
        });
    }
    let dimension = tested.len();
    let mut matrix = filled(
        dimension.checked_mul(dimension).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "PPML Wald dimensions overflow")
        })?,
        0.0_f64,
        "unable to reserve PPML Wald covariance",
    )?;
    let mut rhs = filled(
        dimension,
        0.0_f64,
        "unable to reserve PPML Wald right-hand side",
    )?;
    for (row, &source_row) in tested.iter().enumerate() {
        rhs[row] = coefficients[source_row];
        for (column, &source_column) in tested.iter().enumerate() {
            matrix[row * dimension + column] =
                covariance[source_row * full_dimension + source_column];
        }
    }
    let scale = matrix
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);
    let mut lower = filled(
        matrix.len(),
        0.0_f64,
        "unable to reserve PPML Wald Cholesky factor",
    )?;
    for row in 0..dimension {
        for column in 0..=row {
            let mut value = matrix[row * dimension + column];
            for inner in 0..column {
                value -= lower[row * dimension + inner] * lower[column * dimension + inner];
            }
            if row == column {
                if value <= tolerance * scale {
                    return Ok(PpmlWaldReport {
                        statistic: None,
                        degrees_of_freedom: dimension,
                        tested_columns: fallible_copy(
                            tested,
                            "unable to reserve PPML singular-Wald slot identities",
                        )?,
                        distribution: PpmlTestDistribution::ChiSquared,
                        solve_relative_infinity_norm: 0.0,
                    });
                }
                lower[row * dimension + column] = value.sqrt();
            } else {
                lower[row * dimension + column] = value / lower[column * dimension + column];
            }
        }
    }
    let mut solution = fallible_copy(&rhs, "unable to reserve PPML Wald solution")?;
    for row in 0..dimension {
        for column in 0..row {
            solution[row] -= lower[row * dimension + column] * solution[column];
        }
        solution[row] /= lower[row * dimension + row];
    }
    for row in (0..dimension).rev() {
        for column in (row + 1)..dimension {
            solution[row] -= lower[column * dimension + row] * solution[column];
        }
        solution[row] /= lower[row * dimension + row];
    }
    let mut statistic = 0.0_f64;
    let mut residual_norm = 0.0_f64;
    let mut reference_norm = f64::MIN_POSITIVE;
    for row in 0..dimension {
        statistic = rhs[row].mul_add(solution[row], statistic);
        let mut fitted = 0.0_f64;
        for column in 0..dimension {
            fitted = matrix[row * dimension + column].mul_add(solution[column], fitted);
        }
        residual_norm = residual_norm.max((fitted - rhs[row]).abs());
        reference_norm = reference_norm.max(fitted.abs()).max(rhs[row].abs());
    }
    let solve_error = residual_norm / reference_norm;
    if !statistic.is_finite() || statistic < -tolerance || !solve_error.is_finite() {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML Wald chi-squared calculation is not publishable",
        ));
    }
    Ok(PpmlWaldReport {
        statistic: Some(statistic.max(0.0)),
        degrees_of_freedom: dimension,
        tested_columns: fallible_copy(tested, "unable to reserve PPML Wald slot identities")?,
        distribution: PpmlTestDistribution::ChiSquared,
        solve_relative_infinity_norm: solve_error,
    })
}

fn filled<T: Clone>(elements: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    output.resize(elements, value);
    Ok(output)
}

fn fallible_copy<T: Clone>(source: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    output.extend_from_slice(source);
    Ok(output)
}

#[cfg(test)]
pub(crate) struct PpmlVceInput<'a> {
    pub regressors: &'a DenseMatrix,
    pub score_residuals: &'a [f64],
    pub prior_weights: &'a [f64],
    pub bread: &'a [f64],
    pub kept: &'a [usize],
    pub residual_dof: usize,
    pub clusters: &'a [Vec<u32>],
    pub kind: VceKind,
}

/// Test-oracle adapter for the pre-prepared in-crate PPML prototype.
///
/// The production driver consumes `PpmlInferenceInput` directly, and the
/// production inference engine above is the sole covariance implementation.
/// This compatibility shim remains test-only for stale direct-fit oracle
/// fixtures; it accepts only unit weights rather than guessing whether a
/// nonunit vector meant fweights or normalized pweights.
#[cfg(test)]
pub(crate) fn compute_ppml_vce(input: PpmlVceInput<'_>) -> Result<VceReport> {
    let rows = input.regressors.nrows();
    if input.prior_weights.iter().any(|weight| *weight != 1.0) {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "the stale direct PPML VCE adapter cannot infer a nonunit weight kind",
        ));
    }
    let model_rank = rows.checked_sub(input.residual_dof).ok_or_else(|| {
        FeregError::new(
            ErrorCode::InvalidArgument,
            "stale PPML residual DoF exceeds physical observations",
        )
    })?;
    let absorbed_rank = model_rank.checked_sub(input.kept.len()).ok_or_else(|| {
        FeregError::new(
            ErrorCode::InvalidArgument,
            "stale PPML residual DoF is inconsistent with finite rank",
        )
    })?;
    let coefficients = vec![0.0_f64; input.regressors.ncols()];
    let compact_design = select_design_columns(input.regressors, input.kept)?;
    let identified_bread =
        select_identified_square(input.bread, input.regressors.ncols(), input.kept)?;
    let cluster_slices = input.clusters.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let result = compute_ppml_inference(PpmlInferenceInput {
        coefficients: &coefficients,
        full_dimension: input.regressors.ncols(),
        within_design: &compact_design,
        reported_finite_rank: compact_design.ncols(),
        score_residuals: input.score_residuals,
        prior_weights: input.prior_weights,
        weight_report: PreparedPpmlWeightReport {
            kind: None,
            physical_observations: rows,
            effective_observations: rows,
            raw_weight_sum: None,
            log_raw_weight_sum: None,
            prepared_weight_sum: rows as f64,
            raw_to_prepared_multiplier: Some(1.0),
            log_raw_to_prepared_multiplier: 0.0,
        },
        identified_bread: &identified_bread,
        kept_full_slots: input.kept,
        absorbed_rank,
        connected_components: 0,
        absorbed_rank_is_approximate: false,
        clusters: &cluster_slices,
        requested_vce: match input.kind {
            VceKind::Unadjusted => PpmlVceRequest::Unadjusted,
            VceKind::Robust => PpmlVceRequest::Robust,
            VceKind::Clustered => PpmlVceRequest::Clustered,
        },
        wald_columns: &[],
        certificate_tolerance: 1e-9,
    })?;
    Ok(VceReport {
        kind: input.kind,
        covariance: result.vce.covariance,
        standard_errors: result.vce.standard_errors,
        cluster_subsets: result
            .vce
            .cluster_subsets
            .into_iter()
            .map(|report| ClusterSubsetReport {
                dimensions: report.dimensions,
                groups: report.groups,
                sign: report.sign,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    #[test]
    fn cancelled_cluster_influences_preserve_zero_and_positive_variances() {
        let bread = [0.3, -0.7, -0.7, 1.7];
        for scale in [2.0_f64.powi(-60), 1.0, 2.0_f64.powi(60)] {
            for perturbation in [0.0, 1e-9] {
                let a = (0.7 + perturbation) * scale;
                let b = 0.3 * scale;
                let design =
                    DenseMatrix::from_columns(&[vec![a, a, 1.0, 0.0], vec![b, b, 0.0, 1.0]])
                        .unwrap();
                for threads in [1, 4] {
                    let inference = compute_ppml_inference_interruptible_with_threads(
                        PpmlInferenceInput {
                            coefficients: &[0.0; 2],
                            full_dimension: 2,
                            within_design: &design,
                            reported_finite_rank: 2,
                            score_residuals: &[1.0, -1.0, 0.0, 0.0],
                            prior_weights: &[1.0; 4],
                            weight_report: unweighted_report(4),
                            identified_bread: &bread,
                            kept_full_slots: &[0, 1],
                            absorbed_rank: 0,
                            connected_components: 0,
                            absorbed_rank_is_approximate: false,
                            clusters: &[&[0, 1, 0, 1]],
                            requested_vce: PpmlVceRequest::Clustered,
                            wald_columns: &[],
                            certificate_tolerance: 1e-12,
                        },
                        ThreadConfig::from_requested(threads, true).unwrap(),
                        &mut || Ok(()),
                    )
                    .expect("rank-deficient score factors define nonnegative variances");
                    // Binary 0.3*0.7 and 0.7*0.3 cancel exactly. Compute the
                    // perturbed product with its FMA remainder, independently
                    // of rounded meat, group reduction and bread products.
                    let product = 0.3 * a;
                    let opposite = 0.7 * b;
                    let first = (product - opposite) + 0.3_f64.mul_add(a, -product)
                        - 0.7_f64.mul_add(b, -opposite);
                    let expected = 4.0 * first * first;
                    let actual = inference.vce.covariance[0];
                    if perturbation == 0.0 {
                        assert_eq!(actual, 0.0);
                    } else {
                        assert!(actual > 0.0);
                        assert!((actual - expected).abs() <= 1e-12 * expected,
                            "scale={scale:e} perturbation={perturbation:e} actual={actual:e} expected={expected:e} psd={:?}", inference.vce.psd);
                    }
                    assert!(inference.vce.psd.passed);
                    assert!(inference.vce.certificate.passed);
                }
            }
        }
    }

    #[test]
    fn factor_check_recovers_variance_when_raw_score_squares_underflow() {
        let design = DenseMatrix::from_columns(&[vec![1e-150; 2]]).unwrap();
        let result = compute_ppml_inference(PpmlInferenceInput {
            coefficients: &[0.0],
            full_dimension: 1,
            within_design: &design,
            reported_finite_rank: 1,
            score_residuals: &[1e-50, -1e-50],
            prior_weights: &[1.0; 2],
            weight_report: unweighted_report(2),
            identified_bread: &[1e300],
            kept_full_slots: &[0],
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &[],
            requested_vce: PpmlVceRequest::Robust,
            wald_columns: &[],
            certificate_tolerance: 1e-12,
        })
        .unwrap();
        assert!((result.vce.covariance[0] / 4e200 - 1.0).abs() < 1e-12);
    }

    #[derive(Debug, Clone, Copy)]
    struct InvalidDataInferenceDesign;

    impl PpmlInferenceDesign for InvalidDataInferenceDesign {
        fn nrows(&self) -> usize {
            2
        }

        fn dimension(&self) -> usize {
            1
        }

        fn fill_row(&self, _row: usize, _output: &mut [f64]) -> Result<()> {
            Err(FeregError::new(
                ErrorCode::InvalidData,
                "malformed PPML inference design",
            ))
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct PoolObservedInferenceDesign<'a> {
        rows: usize,
        pool_width: &'a AtomicUsize,
        workers: &'a AtomicU64,
    }

    impl PpmlInferenceDesign for PoolObservedInferenceDesign<'_> {
        fn nrows(&self) -> usize {
            self.rows
        }

        fn dimension(&self) -> usize {
            1
        }

        fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
            if output.len() != 1 || row >= self.rows {
                return Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "observed PPML inference row is out of range",
                ));
            }
            let width = rayon::current_num_threads();
            self.pool_width.fetch_max(width, Ordering::SeqCst);
            if let Some(worker) = rayon::current_thread_index() {
                let bit = 1_u64.checked_shl(worker as u32).unwrap_or(0);
                self.workers.fetch_or(bit, Ordering::SeqCst);
            }
            output[0] = 1.0 + (row % 7) as f64 / 8.0;
            Ok(())
        }
    }

    fn unweighted_report(rows: usize) -> PreparedPpmlWeightReport {
        PreparedPpmlWeightReport {
            kind: None,
            physical_observations: rows,
            effective_observations: rows,
            raw_weight_sum: None,
            log_raw_weight_sum: None,
            prepared_weight_sum: rows as f64,
            raw_to_prepared_multiplier: Some(1.0),
            log_raw_to_prepared_multiplier: 0.0,
        }
    }

    fn frequency_report(weights: &[f64]) -> PreparedPpmlWeightReport {
        let effective = weights.iter().sum::<f64>() as usize;
        PreparedPpmlWeightReport {
            kind: Some(WeightKind::Frequency),
            physical_observations: weights.len(),
            effective_observations: effective,
            raw_weight_sum: Some(effective as f64),
            log_raw_weight_sum: Some((effective as f64).ln()),
            prepared_weight_sum: effective as f64,
            raw_to_prepared_multiplier: Some(1.0),
            log_raw_to_prepared_multiplier: 0.0,
        }
    }

    #[test]
    fn dense_score_work_blocks_bound_all_reduced_ranks() {
        assert_eq!(dense_score_reduction(3).rows_per_block, 16_384);
        assert!(dense_score_reduction(32).rows_per_block < 1024);
        for columns in [1, 8, 17, 32, 64, 129, 512, 1449, 4096] {
            for rows in [1_usize, 8193, 16_384, 1_000_000] {
                let bound = dense_score_partials_bound(rows, columns).unwrap();
                for rank in 1..=columns {
                    let policy = dense_score_reduction(rank);
                    let stride = 16 * rank * (rank + 1);
                    let blocks = rows
                        .div_ceil(policy.rows_per_block)
                        .min(policy.maximum_blocks)
                        .min((policy.partial_budget_bytes / stride).max(1));
                    assert!((blocks * stride) as u128 <= bound);
                }
            }
        }
        assert_eq!(dense_score_partials_bound(10, 0).unwrap(), 0);
        assert!(dense_score_partials_bound(10, usize::MAX).is_err());
    }

    #[test]
    fn wide_robust_work_blocks_match_literal_weighted_sandwich_and_teams() {
        let rows = 8193;
        let columns = 24;
        let columns_data: Vec<Vec<f64>> = (0..columns)
            .map(|column| {
                (0..rows)
                    .map(|row| {
                        ((row * 17 + column * 31) % 103) as f64 / 31.0
                            + ((row + 1) as f64 / (column + 7) as f64).sin()
                    })
                    .collect()
            })
            .collect();
        let design = DenseMatrix::from_columns(&columns_data).unwrap();
        let residuals: Vec<_> = (0..rows)
            .map(|i| ((i * 13 % 37) as f64 - 18.0) / 7.0)
            .collect();
        let bread: Vec<_> = (0..columns * columns)
            .map(|i| f64::from(i / columns == i % columns))
            .collect();
        let slots: Vec<_> = (0..columns).collect();
        for frequency in [false, true] {
            let weights: Vec<_> = (0..rows)
                .map(|i| if frequency { (i % 7 + 1) as f64 } else { 1.0 })
                .collect();
            let weight_report = if frequency {
                frequency_report(&weights)
            } else {
                unweighted_report(rows)
            };
            let correction = weight_report.effective_observations as f64
                / (weight_report.effective_observations - 1) as f64;
            let mut expected = vec![0.; columns * columns];
            for left in 0..columns {
                for right in 0..columns {
                    // Full scalar outer products, independent of block policy,
                    // score multipliers, triangular storage and compensated merge.
                    for row in 0..rows {
                        expected[left * columns + right] += weights[row]
                            * residuals[row].powi(2)
                            * columns_data[left][row]
                            * columns_data[right][row];
                    }
                    expected[left * columns + right] *= correction;
                }
            }
            let mut serial = None;
            for threads in [1, 4] {
                let report = compute_ppml_inference_interruptible_with_threads(
                    PpmlInferenceInput {
                        coefficients: &vec![0.; columns],
                        full_dimension: columns,
                        within_design: &design,
                        reported_finite_rank: columns,
                        score_residuals: &residuals,
                        prior_weights: &weights,
                        weight_report,
                        identified_bread: &bread,
                        kept_full_slots: &slots,
                        absorbed_rank: 0,
                        connected_components: 0,
                        absorbed_rank_is_approximate: false,
                        clusters: &[],
                        requested_vce: PpmlVceRequest::Robust,
                        wald_columns: &[],
                        certificate_tolerance: 1e-10,
                    },
                    ThreadConfig::from_requested(threads, true).unwrap(),
                    &mut || Ok(()),
                )
                .unwrap();
                assert!(report.vce.certificate.passed);
                for (actual, oracle) in report.vce.covariance.iter().zip(&expected) {
                    assert!((actual - oracle).abs() <= 1e-11 * (1.0 + oracle.abs()));
                }
                if let Some(prior) = &serial {
                    assert_eq!(&report.vce.covariance, prior);
                }
                serial = Some(report.vce.covariance);
            }
        }
    }

    #[test]
    fn configured_ppml_sandwich_uses_requested_team_against_global_pool() {
        const CHILD_ENV: &str = "FEREG_PPML_THREAD_TEAM_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let executable = std::env::current_exe().expect("current Rust test executable");
            let output = std::process::Command::new(executable)
                .arg("configured_ppml_sandwich_uses_requested_team_against_global_pool")
                .arg("--nocapture")
                .env(CHILD_ENV, "1")
                .output()
                .expect("spawn isolated Rayon-global child");
            assert!(
                output.status.success(),
                "isolated thread-team child failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            return;
        }

        rayon::ThreadPoolBuilder::new()
            .num_threads(7)
            .build_global()
            .expect("install mismatched seven-thread global pool");
        assert_eq!(rayon::current_num_threads(), 7);

        let rows = 3 * PPML_SANDWICH_REDUCTION.rows_per_block;
        let residuals = (0..rows)
            .map(|row| if row % 2 == 0 { -0.75 } else { 0.5 })
            .collect::<Vec<_>>();
        let prior_weights = vec![1.0; rows];
        let cluster_codes = (0..rows).map(|row| (row % 101) as u32).collect::<Vec<_>>();

        let run = |requested: usize, requested_vce| {
            let pool_width = AtomicUsize::new(0);
            let workers = AtomicU64::new(0);
            let clusters: Vec<&[u32]> = if requested_vce == PpmlVceRequest::Clustered {
                vec![cluster_codes.as_slice()]
            } else {
                Vec::new()
            };
            let result = compute_ppml_inference_interruptible_with_threads(
                PpmlInferenceInput {
                    coefficients: &[0.0],
                    full_dimension: 1,
                    within_design: PoolObservedInferenceDesign {
                        rows,
                        pool_width: &pool_width,
                        workers: &workers,
                    },
                    reported_finite_rank: 1,
                    score_residuals: &residuals,
                    prior_weights: &prior_weights,
                    weight_report: unweighted_report(rows),
                    identified_bread: &[1.0],
                    kept_full_slots: &[0],
                    absorbed_rank: 0,
                    connected_components: 0,
                    absorbed_rank_is_approximate: false,
                    clusters: &clusters,
                    requested_vce,
                    wald_columns: &[],
                    certificate_tolerance: 1e-9,
                },
                ThreadConfig::from_requested(requested, true).expect("thread config"),
                &mut || Ok(()),
            )
            .expect("observed PPML inference");
            assert!(result.vce.certificate.passed);
            (pool_width.into_inner(), workers.into_inner())
        };

        let (serial_width, serial_workers) = run(1, PpmlVceRequest::Robust);
        assert_eq!(
            serial_width,
            1,
            "serial PPML inference escaped configured pool; observed participants={}",
            serial_workers.count_ones(),
        );

        let (robust_width, robust_workers) = run(3, PpmlVceRequest::Robust);
        assert_eq!(
            robust_width,
            3,
            "robust PPML inference escaped configured pool; observed participants={}",
            robust_workers.count_ones(),
        );

        let (clustered_width, clustered_workers) = run(3, PpmlVceRequest::Clustered);
        assert_eq!(
            clustered_width,
            3,
            "clustered PPML inference escaped configured pool; observed participants={}",
            clustered_workers.count_ones(),
        );
    }

    #[test]
    fn weight_report_rejects_forged_raw_scale_metadata() {
        let frequency = [2.0_f64, 1.0, 3.0];
        let mut missing_frequency_sum = frequency_report(&frequency);
        missing_frequency_sum.raw_weight_sum = None;
        assert!(validate_weight_report(&frequency, missing_frequency_sum, 3).is_err());

        let probability = [0.5_f64, 1.0, 1.5, 1.0];
        let mut forged_multiplier = PreparedPpmlWeightReport {
            kind: Some(WeightKind::Probability),
            physical_observations: 4,
            effective_observations: 4,
            raw_weight_sum: Some(8.0),
            log_raw_weight_sum: Some(8.0_f64.ln()),
            prepared_weight_sum: 4.0,
            raw_to_prepared_multiplier: Some(0.5),
            log_raw_to_prepared_multiplier: 0.5_f64.ln(),
        };
        validate_weight_report(&probability, forged_multiplier, 4)
            .expect("canonical pweight scale");
        forged_multiplier.raw_to_prepared_multiplier = Some(0.25);
        assert!(validate_weight_report(&probability, forged_multiplier, 4).is_err());
    }

    #[test]
    fn inference_frequency_weight_domain_failure_preserves_r401() {
        let weights = [1.5_f64, 1.0];
        let error = validate_weight_report(&weights, frequency_report(&weights), 2)
            .expect_err("nonintegral inference fweight");
        assert_eq!(error.code, ErrorCode::InvalidFrequencyWeight);
        assert_eq!(error.return_code(), 401);
    }

    #[test]
    fn valid_finite_ppml_sandwich_overflow_is_nonconvergence() {
        let design = DenseMatrix::from_columns(&[vec![f64::MAX, f64::MAX]]).expect("finite design");
        let error = compute_ppml_inference(PpmlInferenceInput {
            coefficients: &[0.0],
            full_dimension: 1,
            within_design: &design,
            reported_finite_rank: 1,
            score_residuals: &[2.0, 2.0],
            prior_weights: &[1.0; 2],
            weight_report: unweighted_report(2),
            identified_bread: &[1.0],
            kept_full_slots: &[0],
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &[],
            requested_vce: PpmlVceRequest::Robust,
            wald_columns: &[],
            certificate_tolerance: 1e-10,
        })
        .expect_err("finite score-design product overflow must fail publication");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert_eq!(error.return_code(), 430);
        assert!(error.message.contains("sandwich certificate"));
    }

    #[test]
    fn ppml_sandwich_adapter_maps_covariance_failure_but_preserves_malformed_design() {
        let mut reservation = SystemSandwichReservation;
        let covariance_error =
            ppml_sandwich_result(standard_errors(&[-f64::EPSILON], 1, &mut reservation))
                .expect_err("negative covariance diagonal must fail publication");
        assert_eq!(covariance_error.code, ErrorCode::NonConvergence);
        assert_eq!(covariance_error.return_code(), 430);

        let malformed = compute_ppml_inference(PpmlInferenceInput {
            coefficients: &[0.0],
            full_dimension: 1,
            within_design: InvalidDataInferenceDesign,
            reported_finite_rank: 1,
            score_residuals: &[1.0, -1.0],
            prior_weights: &[1.0; 2],
            weight_report: unweighted_report(2),
            identified_bread: &[1.0],
            kept_full_slots: &[0],
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &[],
            requested_vce: PpmlVceRequest::Robust,
            wald_columns: &[],
            certificate_tolerance: 1e-10,
        })
        .expect_err("malformed estimator design must retain data-domain taxonomy");
        assert_eq!(malformed.code, ErrorCode::InvalidData);
        assert_eq!(malformed.return_code(), 459);
    }

    #[test]
    fn unadjusted_is_a_compatibility_spelling_for_robust_z_inference() {
        let design = DenseMatrix::from_columns(&[vec![1.0; 4], vec![-1.5_f64, -0.5, 0.5, 1.5]])
            .expect("design");
        let coefficients = [0.2_f64, -0.4];
        let report = compute_ppml_inference(PpmlInferenceInput {
            coefficients: &coefficients,
            full_dimension: 2,
            within_design: &design,
            reported_finite_rank: 2,
            score_residuals: &[-1.0, 0.5, -0.25, 0.75],
            prior_weights: &[1.0; 4],
            weight_report: unweighted_report(4),
            identified_bread: &[1.0, 0.0, 0.0, 1.0],
            kept_full_slots: &[0, 1],
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &[],
            requested_vce: PpmlVceRequest::Unadjusted,
            wald_columns: &[1],
            certificate_tolerance: 1e-10,
        })
        .expect("inference");
        assert_eq!(report.vce.requested, PpmlVceRequest::Unadjusted);
        assert_eq!(report.vce.effective, PpmlVceKind::Robust);
        assert_eq!(report.vce.correction, 4.0 / 3.0);
        assert_eq!(
            report.vce.coefficient_distribution,
            PpmlTestDistribution::StandardNormal
        );
        assert!(report
            .vce
            .z_statistics
            .iter()
            .all(|value| value.is_finite()));
        assert_eq!(report.wald.distribution, PpmlTestDistribution::ChiSquared);
        assert!(report.wald.statistic.is_some());
        assert!(report.vce.certificate.passed);
    }

    #[test]
    fn compact_design_maps_arbitrary_solve_order_to_full_reported_slots() {
        let compact_design =
            DenseMatrix::from_columns(&[vec![1.0; 4], vec![-1.5_f64, -0.5, 0.5, 1.5]])
                .expect("compact design");
        let report = compute_ppml_inference(PpmlInferenceInput {
            coefficients: &[0.3, 0.0, -0.2],
            full_dimension: 3,
            within_design: &compact_design,
            reported_finite_rank: 2,
            score_residuals: &[-1.0, 0.5, -0.25, 0.75],
            prior_weights: &[1.0; 4],
            weight_report: unweighted_report(4),
            identified_bread: &[1.0, 0.0, 0.0, 1.0],
            // Compact column zero reports in slot two; column one reports in
            // slot zero. This deliberately differs from both generated and
            // full-slot order.
            kept_full_slots: &[2, 0],
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &[],
            requested_vce: PpmlVceRequest::Robust,
            wald_columns: &[0, 2],
            certificate_tolerance: 1e-10,
        })
        .expect("compact mapped inference");
        assert_eq!(report.vce.covariance.len(), 9);
        for position in [1_usize, 3, 4, 5, 7] {
            assert_eq!(report.vce.covariance[position], 0.0);
        }
        assert!(report.vce.standard_errors[0].is_finite());
        assert!(report.vce.standard_errors[1].is_nan());
        assert!(report.vce.standard_errors[2].is_finite());
        assert!(report.vce.z_statistics[1].is_nan());
        assert_eq!(report.wald.tested_columns, vec![0, 2]);
        assert!(report.wald.statistic.is_some());
    }

    #[test]
    fn robust_fweights_equal_literal_row_replication() {
        let compact_design = DenseMatrix::from_columns(&[vec![1.0; 3], vec![-1.0_f64, 0.5, 2.0]])
            .expect("compact design");
        let residuals = [-0.75_f64, 1.25, -0.5];
        let frequency = [2.0_f64, 1.0, 3.0];
        let compact = compute_ppml_inference(PpmlInferenceInput {
            coefficients: &[0.0; 2],
            full_dimension: 2,
            within_design: &compact_design,
            reported_finite_rank: 2,
            score_residuals: &residuals,
            prior_weights: &frequency,
            weight_report: frequency_report(&frequency),
            identified_bread: &[1.0, 0.0, 0.0, 1.0],
            kept_full_slots: &[0, 1],
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &[],
            requested_vce: PpmlVceRequest::Robust,
            wald_columns: &[],
            certificate_tolerance: 1e-10,
        })
        .expect("compact inference");

        let mut expanded_columns = [Vec::new(), Vec::new()];
        let mut expanded_residuals = Vec::new();
        for row in 0..frequency.len() {
            for _ in 0..frequency[row] as usize {
                expanded_columns[0].push(1.0);
                expanded_columns[1].push(compact_design[(row, 1)]);
                expanded_residuals.push(residuals[row]);
            }
        }
        let expanded_design = DenseMatrix::from_columns(&expanded_columns).expect("expanded");
        let expanded = compute_ppml_inference(PpmlInferenceInput {
            coefficients: &[0.0; 2],
            full_dimension: 2,
            within_design: &expanded_design,
            reported_finite_rank: 2,
            score_residuals: &expanded_residuals,
            prior_weights: &[1.0; 6],
            weight_report: unweighted_report(6),
            identified_bread: &[1.0, 0.0, 0.0, 1.0],
            kept_full_slots: &[0, 1],
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &[],
            requested_vce: PpmlVceRequest::Robust,
            wald_columns: &[],
            certificate_tolerance: 1e-10,
        })
        .expect("expanded inference");
        for (left, right) in compact.vce.covariance.iter().zip(&expanded.vce.covariance) {
            assert!((left - right).abs() <= 2e-14 * left.abs().max(right.abs()).max(1.0));
        }
        assert_eq!(compact.dof.effective_observations, 6);
        assert_eq!(compact.vce.correction, expanded.vce.correction);
    }

    #[test]
    fn normalized_pweight_metadata_does_not_reintroduce_raw_scale() {
        let design = DenseMatrix::from_columns(&[vec![1.0; 4], vec![0.0_f64, 1.0, 2.0, 3.0]])
            .expect("design");
        let weights = [0.5_f64, 1.0, 1.5, 1.0];
        let make_report = |raw_sum: f64| PreparedPpmlWeightReport {
            kind: Some(WeightKind::Probability),
            physical_observations: 4,
            effective_observations: 4,
            raw_weight_sum: Some(raw_sum),
            log_raw_weight_sum: Some(raw_sum.ln()),
            prepared_weight_sum: 4.0,
            raw_to_prepared_multiplier: Some(4.0 / raw_sum),
            log_raw_to_prepared_multiplier: (4.0 / raw_sum).ln(),
        };
        let run = |weight_report| {
            compute_ppml_inference(PpmlInferenceInput {
                coefficients: &[0.0; 2],
                full_dimension: 2,
                within_design: &design,
                reported_finite_rank: 2,
                score_residuals: &[-0.5, 1.0, -0.75, 0.25],
                prior_weights: &weights,
                weight_report,
                identified_bread: &[1.0, 0.0, 0.0, 1.0],
                kept_full_slots: &[0, 1],
                absorbed_rank: 0,
                connected_components: 0,
                absorbed_rank_is_approximate: false,
                clusters: &[],
                requested_vce: PpmlVceRequest::Robust,
                wald_columns: &[],
                certificate_tolerance: 1e-10,
            })
            .expect("pweight inference")
        };
        assert_eq!(run(make_report(8.0)).vce, run(make_report(136.0)).vce);
    }

    #[test]
    fn five_way_clusters_use_all_subsets_one_common_correction_and_capped_dof() {
        let rows = 32_usize;
        let design = DenseMatrix::from_columns(&[vec![1.0; rows]]).expect("design");
        let residuals = (0..rows)
            .map(|row| if row % 2 == 0 { -1.0 } else { 1.0 })
            .collect::<Vec<_>>();
        let clusters = (0..5)
            .map(|dimension| {
                (0..rows)
                    .map(|row| ((row >> dimension) & 1) as u32)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let cluster_slices = clusters.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let prior_weights = vec![1.0; rows];
        let report = compute_ppml_inference(PpmlInferenceInput {
            coefficients: &[0.0],
            full_dimension: 1,
            within_design: &design,
            reported_finite_rank: 1,
            score_residuals: &residuals,
            prior_weights: &prior_weights,
            weight_report: unweighted_report(rows),
            identified_bread: &[1.0],
            kept_full_slots: &[0],
            absorbed_rank: 0,
            connected_components: 0,
            absorbed_rank_is_approximate: false,
            clusters: &cluster_slices,
            requested_vce: PpmlVceRequest::Clustered,
            wald_columns: &[],
            certificate_tolerance: 1e-9,
        })
        .expect("five-way inference");
        assert_eq!(report.vce.cluster_subsets.len(), 31);
        assert_eq!(report.vce.cluster_dimension_groups, vec![2; 5]);
        assert_eq!(report.dof.minimum_cluster_groups, Some(2));
        assert_eq!(report.dof.structural_residual, 31);
        assert_eq!(report.dof.reported, 1);
        assert_eq!(report.vce.correction, 2.0);
        assert!(report.vce.psd.passed);
    }

    #[test]
    fn clustered_rank_zero_counts_groups_with_one_fallible_row_ordering() {
        let rows = 6_usize;
        let design = DenseMatrix::zeros(rows, 0).expect("rank-zero design");
        let first = [0_u32, 0, 0, 1, 1, 1];
        let second = [0_u32, 1, 2, 0, 1, 2];
        let report = compute_ppml_inference(PpmlInferenceInput {
            coefficients: &[],
            full_dimension: 0,
            within_design: &design,
            reported_finite_rank: 0,
            score_residuals: &[0.0; 6],
            prior_weights: &[1.0; 6],
            weight_report: unweighted_report(rows),
            identified_bread: &[],
            kept_full_slots: &[],
            absorbed_rank: 1,
            connected_components: 1,
            absorbed_rank_is_approximate: false,
            clusters: &[&first, &second],
            requested_vce: PpmlVceRequest::Clustered,
            wald_columns: &[],
            certificate_tolerance: 1e-10,
        })
        .expect("clustered rank-zero inference");
        assert_eq!(report.vce.cluster_dimension_groups, vec![2, 3]);
        assert_eq!(
            report
                .vce
                .cluster_subsets
                .iter()
                .map(|subset| (subset.mask, subset.groups))
                .collect::<Vec<_>>(),
            vec![(1, 2), (2, 3), (3, 6)],
        );
        assert_eq!(report.dof.sandwich_rank, 0);
        assert!(report.vce.covariance.is_empty());
        assert!(report.vce.certificate.passed);
    }

    #[test]
    fn final_sample_dof_uses_effective_n_and_cluster_cap() {
        let unclustered = compute_ppml_dof(20, 3, 3, 5, None, 2, false).expect("DoF");
        assert_eq!(unclustered.structural_residual, 12);
        assert_eq!(unclustered.reported, 12);
        let clustered = compute_ppml_dof(20, 3, 3, 5, Some(7), 2, false).expect("cluster DoF");
        assert_eq!(clustered.structural_residual, 12);
        assert_eq!(clustered.reported, 6);
        assert_eq!(
            compute_ppml_dof(8, 3, 3, 5, None, 1, false)
                .expect_err("nonpositive DoF")
                .code,
            ErrorCode::InsufficientObservations
        );
    }

    #[test]
    fn reported_structural_normalization_is_separate_from_sandwich_rank() {
        let report = compute_ppml_dof(20, 1, 2, 4, None, 1, false).expect("split rank DoF");
        assert_eq!(report.sandwich_rank, 1);
        assert_eq!(report.finite_within_rank, 2);
        assert_eq!(report.absorbed_rank, 4);
        assert_eq!(report.model_rank, 6);
        assert_eq!(report.structural_residual, 14);
    }

    #[test]
    fn stale_unit_weight_adapter_routes_through_the_new_engine() {
        let design = DenseMatrix::from_columns(&[vec![1.0; 4], vec![0.0_f64, 1.0, 2.0, 3.0]])
            .expect("design");
        let report = compute_ppml_vce(PpmlVceInput {
            regressors: &design,
            score_residuals: &[0.0; 4],
            prior_weights: &[1.0; 4],
            bread: &[0.7, -0.2, -0.2, 0.1],
            kept: &[0, 1],
            residual_dof: 2,
            clusters: &[],
            kind: VceKind::Unadjusted,
        })
        .expect("compatibility adapter");
        assert_eq!(report.covariance, vec![0.0; 4]);
    }
}
