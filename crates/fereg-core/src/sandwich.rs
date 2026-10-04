//! Estimator-neutral sandwich reduction mechanisms.
//!
//! Estimators own the scientific meaning of the final design, scalar score
//! multiplier, degrees of freedom, and finite-sample correction.  This module
//! owns only deterministic score-meat and bread--meat--bread reductions.  The
//! traits are statically dispatched: the hot observation loops contain no
//! trait objects or per-row heap allocation.

use rayon::prelude::*;
use std::cmp::Ordering;

use crate::error::{ErrorCode, FeregError, Result};

mod influence;
mod qualified_cluster;
pub(crate) use qualified_cluster::{
    prepare_cluster_score_batch, reduce_qualified_cluster_subset, QualifiedClusterCertificateRoute,
    QualifiedClusterRoutePolicy, QualifiedClusterScoreBatch,
};

/// Reservation boundary shared by certified inference reducers.
///
/// Keeping reservation explicit preserves fail-closed allocation injection in
/// the OLS qualification suite and lets future estimators account for the same
/// workspaces before entering a hot reduction.
pub(crate) trait SandwichReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

pub(crate) struct SystemSandwichReservation;

impl SandwichReservation for SystemSandwichReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }
}

/// A final, reported-coordinate design used to form score moments.
///
/// Implementations may borrow packed rows, reconstruct strided rows, or stream
/// bounded scratch storage.  Parallel reducers additionally require `Copy +
/// Sync`; each reduction block may still own an immutable scan cursor.
pub(crate) trait SandwichDesign {
    fn nrows(&self) -> usize;
    fn dimension(&self) -> usize;
    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()>;

    /// Scratch sources can amortize positional I/O across an immutable scan.
    fn prefers_sequential_scan(&self) -> bool {
        false
    }

    /// A reducer owns its row buffer and order; a source may retain a bounded
    /// reader for this immutable scan without sharing cursor state with workers.
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

/// Estimator-provided scalar multiplying one final-design row.
///
/// Production and certificate values are separate so a certified estimator
/// can retain independent construction without duplicating the design pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SandwichScorePair {
    pub production: f64,
    pub certificate: f64,
}

pub(crate) trait SandwichScoreMultiplier {
    fn nrows(&self) -> usize;
    fn score_pair(&self, row: usize) -> Result<SandwichScorePair>;
}

/// Zero-copy row-major design adapter, principally for estimator-owned dense
/// final designs.
#[derive(Debug, Clone, Copy)]
#[cfg(test)]
pub(crate) struct RowMajorSandwichDesign<'a> {
    values: &'a [f64],
    nrows: usize,
    dimension: usize,
}

#[cfg(test)]
impl<'a> RowMajorSandwichDesign<'a> {
    pub(crate) fn new(values: &'a [f64], nrows: usize, dimension: usize) -> Result<Self> {
        let elements = nrows
            .checked_mul(dimension)
            .ok_or_else(|| allocation("sandwich design dimensions overflow usize"))?;
        if nrows == 0 || values.len() != elements || values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument(
                "sandwich row-major design is empty, nonfinite, or dimensionally inconsistent",
            ));
        }
        Ok(Self {
            values,
            nrows,
            dimension,
        })
    }
}

#[cfg(test)]
impl SandwichDesign for RowMajorSandwichDesign<'_> {
    fn nrows(&self) -> usize {
        self.nrows
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        if row >= self.nrows || output.len() != self.dimension {
            return Err(invalid_argument(
                "sandwich row-major design row is dimensionally inconsistent",
            ));
        }
        let start = row
            .checked_mul(self.dimension)
            .ok_or_else(|| allocation("sandwich row-major design offset overflows usize"))?;
        output.copy_from_slice(&self.values[start..start + self.dimension]);
        Ok(())
    }
}

/// Zero-copy multiplier adapter when production and certificate scores are
/// already materialized by an estimator.
#[derive(Debug, Clone, Copy)]
#[cfg(test)]
pub(crate) struct SliceSandwichScoreMultiplier<'a> {
    production: &'a [f64],
    certificate: &'a [f64],
}

#[cfg(test)]
impl<'a> SliceSandwichScoreMultiplier<'a> {
    pub(crate) fn new(production: &'a [f64], certificate: &'a [f64]) -> Result<Self> {
        if production.is_empty()
            || production.len() != certificate.len()
            || production.iter().any(|value| !value.is_finite())
            || certificate.iter().any(|value| !value.is_finite())
        {
            return Err(invalid_argument(
                "sandwich score multipliers are empty, nonfinite, or misaligned",
            ));
        }
        Ok(Self {
            production,
            certificate,
        })
    }

    pub(crate) fn same(values: &'a [f64]) -> Result<Self> {
        Self::new(values, values)
    }
}

#[cfg(test)]
impl SandwichScoreMultiplier for SliceSandwichScoreMultiplier<'_> {
    fn nrows(&self) -> usize {
        self.production.len()
    }

    fn score_pair(&self, row: usize) -> Result<SandwichScorePair> {
        let production = *self
            .production
            .get(row)
            .ok_or_else(|| invalid_argument("sandwich score row is out of range"))?;
        let certificate = *self
            .certificate
            .get(row)
            .ok_or_else(|| invalid_argument("sandwich certificate score row is out of range"))?;
        if production.is_finite() && certificate.is_finite() {
            Ok(SandwichScorePair {
                production,
                certificate,
            })
        } else {
            Err(invalid_data("sandwich score multiplier is not finite"))
        }
    }
}

/// Validated borrowed cluster topology on the final sample axis.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SandwichClusters<'a> {
    dimensions: &'a [&'a [u32]],
    nrows: usize,
}

impl<'a> SandwichClusters<'a> {
    pub(crate) fn new(
        dimensions: &'a [&'a [u32]],
        nrows: usize,
        maximum_dimensions: usize,
    ) -> Result<Self> {
        if nrows == 0
            || dimensions.is_empty()
            || dimensions.len() > maximum_dimensions
            || dimensions.iter().any(|codes| codes.len() != nrows)
        {
            return Err(invalid_argument(
                "sandwich cluster topology is empty or not final-sample aligned",
            ));
        }
        Ok(Self { dimensions, nrows })
    }

    pub(crate) fn dimensions(self) -> &'a [&'a [u32]] {
        self.dimensions
    }

    pub(crate) fn nrows(self) -> usize {
        self.nrows
    }
}

/// Estimator-owned correction applied after the uncorrected sandwich is
/// formed.  The kernel validates but never derives this value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SandwichCorrectionPolicy {
    None,
    Scale(f64),
}

impl SandwichCorrectionPolicy {
    fn factor(self) -> Result<f64> {
        let factor = match self {
            Self::None => 1.0,
            Self::Scale(value) => value,
        };
        if factor.is_finite() && factor > 0.0 {
            Ok(factor)
        } else {
            Err(invalid_argument(
                "sandwich correction must be finite and positive",
            ))
        }
    }
}

/// Floating-point path for the final bread--meat--bread product.  These two
/// modes intentionally preserve the distinct accepted OLS robust and
/// clustered accumulation orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SandwichProductMode {
    OrdinaryUpperTriangle,
    CompensatedFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FixedBlockReductionPolicy {
    pub rows_per_block: usize,
    pub maximum_blocks: usize,
    pub partial_budget_bytes: usize,
}

#[derive(Debug, PartialEq)]
pub(crate) struct IndependentScoreMeats {
    pub production: Vec<f64>,
    pub certificate: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClusterSubsetReduction {
    pub mask: u16,
    pub dimensions: usize,
    pub sign: i8,
    pub groups: usize,
}

#[derive(Debug, PartialEq)]
pub(crate) struct ClusteredScoreMeats {
    pub production: Vec<f64>,
    pub certificate: Vec<f64>,
    pub subsets: Vec<ClusterSubsetReduction>,
}

/// Estimator-neutral provider for one already ordered/certified cluster
/// subset. Estimators retain their qualified row-access and grouping route;
/// this shared boundary owns the common cardinality/mask traversal,
/// inclusion--exclusion merge, independent certificate merge, and report
/// publication. Static dispatch leaves the accepted estimator hot loop
/// unchanged.
pub(crate) trait QualifiedClusterSubsetMeatProvider<R: SandwichReservation> {
    type Report: Copy;

    fn dimension(&self) -> usize;
    fn cluster_dimensions(&self) -> usize;
    fn reduce_subset(
        &self,
        mask: usize,
        reservation: &mut R,
    ) -> Result<QualifiedClusterSubsetMeats>;
    fn report(
        &self,
        mask: usize,
        dimensions: usize,
        sign: i8,
        groups: usize,
    ) -> Result<Self::Report>;
}

#[derive(Debug, PartialEq)]
pub(crate) struct QualifiedClusterSubsetMeats {
    pub production: Vec<f64>,
    pub certificate: Vec<f64>,
    pub groups: usize,
}

#[derive(Debug, PartialEq)]
pub(crate) struct QualifiedClusteredScoreMeats<T> {
    pub production: Vec<f64>,
    pub certificate: Vec<f64>,
    pub subsets: Vec<T>,
    pub certificate_subsets: Vec<T>,
}

/// Combine estimator-qualified cluster-subset meats without changing their
/// construction order or allocation profile. The two report vectors are
/// deliberately reserved independently because OLS certification compares
/// independently published production and certificate subset ledgers.
pub(crate) fn reduce_qualified_clustered_score_meats<P, R>(
    provider: &P,
    reservation: &mut R,
) -> Result<QualifiedClusteredScoreMeats<P::Report>>
where
    P: QualifiedClusterSubsetMeatProvider<R>,
    R: SandwichReservation,
{
    let dimension = provider.dimension();
    let cluster_dimensions = provider.cluster_dimensions();
    if cluster_dimensions == 0 || cluster_dimensions > u16::BITS as usize {
        return Err(invalid_argument(
            "qualified clustered sandwich dimensions are invalid",
        ));
    }
    let square = checked_square(
        dimension,
        "qualified clustered sandwich meat dimensions overflow usize",
    )?;
    let subset_count = 1_usize
        .checked_shl(
            u32::try_from(cluster_dimensions)
                .map_err(|_| allocation("qualified cluster dimension count overflows"))?,
        )
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| allocation("qualified cluster subset count overflows usize"))?;
    let mut production = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve qualified clustered production meat",
    )?;
    let mut production_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve qualified clustered production corrections",
    )?;
    let mut certificate = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve qualified clustered certificate meat",
    )?;
    let mut certificate_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve qualified clustered certificate corrections",
    )?;
    let mut subsets = reserved_empty(
        reservation,
        subset_count,
        "unable to reserve qualified clustered subset reports",
    )?;
    let mut certificate_subsets = reserved_empty(
        reservation,
        subset_count,
        "unable to reserve qualified clustered certificate subset reports",
    )?;

    for cardinality in 1..=cluster_dimensions {
        for mask in 1..=subset_count {
            if mask.count_ones() as usize != cardinality {
                continue;
            }
            let reduced = provider.reduce_subset(mask, reservation)?;
            if reduced.production.len() != square || reduced.certificate.len() != square {
                return Err(invalid_argument(
                    "qualified clustered subset meat dimensions are inconsistent",
                ));
            }
            let sign = if cardinality % 2 == 1 { 1_i8 } else { -1_i8 };
            for position in 0..square {
                compensated_add(
                    f64::from(sign) * reduced.production[position],
                    &mut production[position],
                    &mut production_corrections[position],
                )?;
                compensated_add(
                    f64::from(sign) * reduced.certificate[position],
                    &mut certificate[position],
                    &mut certificate_corrections[position],
                )?;
            }
            let report = provider.report(mask, cardinality, sign, reduced.groups)?;
            push_reserved(
                &mut subsets,
                report,
                "qualified clustered subset report capacity exhausted",
            )?;
            push_reserved(
                &mut certificate_subsets,
                report,
                "qualified clustered certificate subset report capacity exhausted",
            )?;
        }
    }
    for (value, correction) in production.iter_mut().zip(production_corrections) {
        *value += correction;
        if !value.is_finite() {
            return Err(invalid_data(
                "qualified clustered production meat is not finite",
            ));
        }
    }
    for (value, correction) in certificate.iter_mut().zip(certificate_corrections) {
        *value += correction;
        if !value.is_finite() {
            return Err(invalid_data(
                "qualified clustered certificate meat is not finite",
            ));
        }
    }
    Ok(QualifiedClusteredScoreMeats {
        production,
        certificate,
        subsets,
        certificate_subsets,
    })
}

/// Dense observation-level score-meat reduction with separate contiguous
/// production and certificate triangles.  Production blocks merge forward;
/// certificate blocks merge backward, matching ADR 0014.
pub(crate) fn reduce_dense_score_meats<D, M, R>(
    design: D,
    multipliers: M,
    policy: FixedBlockReductionPolicy,
    influence_bread: Option<&[f64]>,
    reservation: &mut R,
) -> Result<IndependentScoreMeats>
where
    D: SandwichDesign + Copy + Sync,
    M: SandwichScoreMultiplier + Copy + Sync,
    R: SandwichReservation,
{
    let nrows = design.nrows();
    let dimension = design.dimension();
    if nrows == 0
        || dimension == 0
        || multipliers.nrows() != nrows
        || policy.rows_per_block == 0
        || policy.maximum_blocks == 0
        || policy.partial_budget_bytes == 0
    {
        return Err(invalid_argument(
            "dense sandwich reduction dimensions or policy are invalid",
        ));
    }
    let square = checked_square(dimension, "dense sandwich meat dimensions overflow usize")?;
    influence::validate_bread(influence_bread, dimension)?;
    let workspace = dimension
        .checked_mul(2)
        .ok_or_else(|| allocation("dense sandwich score workspace overflows usize"))?;
    let partial_stride = square
        .checked_mul(2)
        .and_then(|meats| meats.checked_add(workspace))
        .ok_or_else(|| allocation("dense sandwich partial stride overflows usize"))?;
    let chunks = reduction_chunks(nrows, partial_stride, policy)?;
    let partial_elements = chunks
        .checked_mul(partial_stride)
        .ok_or_else(|| allocation("dense sandwich partials overflow usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve dense sandwich partials",
    )?;
    let fixed_triangle = fixed_dense_triangle_update(dimension);
    partials
        .par_chunks_mut(partial_stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let (production_meat, tail) = partial.split_at_mut(square);
            let (certificate_meat, workspace) = tail.split_at_mut(square);
            let (production_score, certificate_score) = workspace.split_at_mut(dimension);
            let rows_per_chunk = nrows / chunks;
            let remainder = nrows % chunks;
            let start = chunk * rows_per_chunk + chunk.min(remainder);
            let end = start + rows_per_chunk + usize::from(chunk < remainder);
            design.visit_range(start..end, production_score, |row, production_score| {
                let score = multipliers.score_pair(row)?;
                if let Some(bread) = influence_bread {
                    return influence::dense_row(
                        bread,
                        production_score,
                        certificate_score,
                        score,
                        production_meat,
                        certificate_meat,
                    );
                }
                for coordinate in 0..dimension {
                    let raw = production_score[coordinate];
                    production_score[coordinate] = score.production * raw;
                    certificate_score[coordinate] = score.certificate * raw;
                    if !production_score[coordinate].is_finite()
                        || !certificate_score[coordinate].is_finite()
                    {
                        return Err(invalid_data("dense sandwich scaled score is not finite"));
                    }
                }
                if let Some(update) = fixed_triangle {
                    update(
                        production_meat,
                        certificate_meat,
                        production_score,
                        certificate_score,
                    )?;
                } else {
                    for left in 0..dimension {
                        let production_left = production_score[left];
                        let certificate_left = certificate_score[left];
                        let range = left * dimension + left..(left + 1) * dimension;
                        // Contiguous triangle rows expose independent coordinates to
                        // vectorization while preserving each sum's observation order.
                        for (((production, certificate), &production_right), &certificate_right) in
                            production_meat[range.clone()]
                                .iter_mut()
                                .zip(&mut certificate_meat[range])
                                .zip(&production_score[left..])
                                .zip(&certificate_score[left..])
                        {
                            *production += production_left * production_right;
                            *certificate += certificate_right * certificate_left;
                        }
                    }
                }
                Ok(())
            })?;
            if production_meat.iter().all(|value| value.is_finite())
                && certificate_meat.iter().all(|value| value.is_finite())
            {
                Ok(())
            } else {
                Err(invalid_data(
                    "dense sandwich partial contains a nonfinite value",
                ))
            }
        })?;

    let mut production = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve dense sandwich production meat",
    )?;
    let mut production_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve dense sandwich production corrections",
    )?;
    let mut certificate = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve dense sandwich certificate meat",
    )?;
    let mut certificate_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve dense sandwich certificate corrections",
    )?;
    for partial in partials.chunks_exact(partial_stride) {
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                compensated_add(
                    partial[position],
                    &mut production[position],
                    &mut production_corrections[position],
                )?;
            }
        }
    }
    for partial in partials.rchunks_exact(partial_stride) {
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                compensated_add(
                    partial[square + position],
                    &mut certificate[position],
                    &mut certificate_corrections[position],
                )?;
            }
        }
    }
    finalize_symmetric_triangle(&mut production, production_corrections, dimension)?;
    finalize_symmetric_triangle(&mut certificate, certificate_corrections, dimension)?;
    Ok(IndependentScoreMeats {
        production,
        certificate,
    })
}

type DenseTriangleUpdate = fn(&mut [f64], &mut [f64], &[f64], &[f64]) -> Result<()>;

fn fixed_dense_triangle_update(dimension: usize) -> Option<DenseTriangleUpdate> {
    // Fixed small widths eliminate repeated triangle-offset and slice-bound
    // bookkeeping. Bound specialization size; wider designs retain their loop.
    Some(match dimension {
        1 => update_fixed_dense_triangle::<1, 1>,
        2 => update_fixed_dense_triangle::<2, 4>,
        3 => update_fixed_dense_triangle::<3, 9>,
        4 => update_fixed_dense_triangle::<4, 16>,
        5 => update_fixed_dense_triangle::<5, 25>,
        6 => update_fixed_dense_triangle::<6, 36>,
        7 => update_fixed_dense_triangle::<7, 49>,
        8 => update_fixed_dense_triangle::<8, 64>,
        9 => update_fixed_dense_triangle::<9, 81>,
        10 => update_fixed_dense_triangle::<10, 100>,
        11 => update_fixed_dense_triangle::<11, 121>,
        12 => update_fixed_dense_triangle::<12, 144>,
        13 => update_fixed_dense_triangle::<13, 169>,
        14 => update_fixed_dense_triangle::<14, 196>,
        15 => update_fixed_dense_triangle::<15, 225>,
        16 => update_fixed_dense_triangle::<16, 256>,
        _ => return None,
    })
}

fn update_fixed_dense_triangle<const DIMENSION: usize, const SQUARE: usize>(
    production: &mut [f64],
    certificate: &mut [f64],
    production_score: &[f64],
    certificate_score: &[f64],
) -> Result<()> {
    let shape_error =
        || invalid_argument("fixed dense sandwich triangle dimensions are inconsistent");
    let production: &mut [f64; SQUARE] = production.try_into().map_err(|_| shape_error())?;
    let certificate: &mut [f64; SQUARE] = certificate.try_into().map_err(|_| shape_error())?;
    let production_score: &[f64; DIMENSION] =
        production_score.try_into().map_err(|_| shape_error())?;
    let certificate_score: &[f64; DIMENSION] =
        certificate_score.try_into().map_err(|_| shape_error())?;
    for left in 0..DIMENSION {
        for right in left..DIMENSION {
            production[left * DIMENSION + right] +=
                production_score[left] * production_score[right];
            certificate[left * DIMENSION + right] +=
                certificate_score[right] * certificate_score[left];
        }
    }
    Ok(())
}

/// Serial observation-influence covariance for a scratch-backed design.
///
/// The estimator supplies the complete scalar multiplier, including any
/// finite-sample correction it wants applied before the bread transform.  The
/// reducer deliberately forms `bread * row` with compensated accumulation,
/// scales that influence, and then accumulates its upper-triangle outer
/// product in row order.  This is distinct from forming a raw score meat and
/// applying bread afterward: the order here is the accepted low-memory OLS
/// floating-point route and must remain stable.
///
/// Unlike the fixed-block reducer, this function accepts a non-`Sync` design
/// so callers may stream rows through bounded mutable scratch storage.  Every
/// owned workspace is obtained through `SandwichReservation`; no allocation
/// occurs in the observation loop.
pub(crate) fn reduce_serial_influence_covariance<D, M, R>(
    design: &D,
    multipliers: M,
    bread: &[f64],
    reservation: &mut R,
) -> Result<Vec<f64>>
where
    D: SandwichDesign + ?Sized,
    M: SandwichScoreMultiplier,
    R: SandwichReservation,
{
    let nrows = design.nrows();
    let dimension = design.dimension();
    let square = checked_square(
        dimension,
        "serial sandwich covariance dimensions overflow usize",
    )?;
    if nrows == 0 || multipliers.nrows() != nrows || bread.len() != square {
        return Err(invalid_argument(
            "serial sandwich design, multiplier, and bread dimensions are inconsistent",
        ));
    }
    let mut covariance = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve serial sandwich covariance",
    )?;
    let mut corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve serial sandwich covariance corrections",
    )?;
    let mut influence = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve serial sandwich influence row",
    )?;
    let mut score = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve serial sandwich score row",
    )?;

    for row in 0..nrows {
        design.fill_row(row, &mut score)?;
        let scale = multipliers.score_pair(row)?.production;
        for (left, influence_value) in influence.iter_mut().enumerate() {
            let mut product = 0.0_f64;
            let mut correction = 0.0_f64;
            for (right, score_value) in score.iter().copied().enumerate() {
                compensated_add(
                    bread[left * dimension + right] * score_value,
                    &mut product,
                    &mut correction,
                )?;
            }
            *influence_value = scale * (product + correction);
            if !influence_value.is_finite() {
                return Err(invalid_data(
                    "serial sandwich influence row contains a nonfinite value",
                ));
            }
        }
        for left in 0..dimension {
            let start = left * dimension + left;
            let end = (left + 1) * dimension;
            compensated_add_scaled_row(
                influence[left],
                &influence[left..],
                &mut covariance[start..end],
                &mut corrections[start..end],
            )?;
        }
    }
    finalize_symmetric_triangle(&mut covariance, corrections, dimension)?;
    Ok(covariance)
}

/// Multiway clustered score-meat reduction over all nonempty cluster subsets.
///
/// Subsets are visited by cardinality and then ascending bit mask.  Within a
/// subset, stable radix ordering (or a deterministic tuple-order fallback)
/// exposes whole groups to fixed row blocks.  Production blocks merge forward
/// and certificate blocks backward.  The result is the uncorrected CGM
/// inclusion--exclusion meat; estimators derive and apply their own single
/// common correction after the full combination.
pub(crate) fn reduce_clustered_score_meats<D, M, R>(
    design: D,
    multipliers: M,
    clusters: SandwichClusters<'_>,
    policy: FixedBlockReductionPolicy,
    influence_bread: Option<&[f64]>,
    reservation: &mut R,
) -> Result<ClusteredScoreMeats>
where
    D: SandwichDesign + Copy + Sync,
    M: SandwichScoreMultiplier + Copy + Sync,
    R: SandwichReservation,
{
    let nrows = design.nrows();
    let dimension = design.dimension();
    let cluster_dimensions = clusters.dimensions();
    if nrows == 0
        || dimension == 0
        || multipliers.nrows() != nrows
        || clusters.nrows() != nrows
        || cluster_dimensions.len() > u16::BITS as usize
    {
        return Err(invalid_argument(
            "clustered sandwich inputs are not final-sample aligned",
        ));
    }
    let square = checked_square(
        dimension,
        "clustered sandwich meat dimensions overflow usize",
    )?;
    influence::validate_bread(influence_bread, dimension)?;
    let subset_count = 1_usize
        .checked_shl(
            u32::try_from(cluster_dimensions.len())
                .map_err(|_| allocation("clustered sandwich dimension count overflows"))?,
        )
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| allocation("clustered sandwich subset count overflows usize"))?;
    let mut production = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve clustered sandwich production meat",
    )?;
    let mut production_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve clustered sandwich production corrections",
    )?;
    let mut certificate = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve clustered sandwich certificate meat",
    )?;
    let mut certificate_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve clustered sandwich certificate corrections",
    )?;
    let mut subsets = reserved_empty(
        reservation,
        subset_count,
        "unable to reserve clustered sandwich subset reports",
    )?;

    for cardinality in 1..=cluster_dimensions.len() {
        for mask in 1..=subset_count {
            if mask.count_ones() as usize != cardinality {
                continue;
            }
            let reduced = reduce_qualified_cluster_subset(
                &design,
                Some(design),
                multipliers,
                cluster_dimensions,
                mask,
                QualifiedClusterCertificateRoute::PairedIndependent(policy, influence_bread),
                QualifiedClusterRoutePolicy {
                    maximum_dimensions: u16::BITS as usize,
                    reduction_maximum_blocks: policy.maximum_blocks,
                    partial_budget_bytes: policy.partial_budget_bytes,
                    ordered_rows_per_block: policy.rows_per_block,
                    dense_groups_per_block: policy.rows_per_block,
                },
                reservation,
            )?;
            let sign = if cardinality % 2 == 1 { 1_i8 } else { -1_i8 };
            for position in 0..square {
                compensated_add(
                    f64::from(sign) * reduced.production[position],
                    &mut production[position],
                    &mut production_corrections[position],
                )?;
                compensated_add(
                    f64::from(sign) * reduced.certificate[position],
                    &mut certificate[position],
                    &mut certificate_corrections[position],
                )?;
            }
            push_reserved(
                &mut subsets,
                ClusterSubsetReduction {
                    mask: u16::try_from(mask)
                        .map_err(|_| allocation("clustered sandwich subset mask exceeds u16"))?,
                    dimensions: cardinality,
                    sign,
                    groups: reduced.groups,
                },
                "clustered sandwich subset report capacity exhausted",
            )?;
        }
    }
    finalize_symmetric_triangle(&mut production, production_corrections, dimension)?;
    finalize_symmetric_triangle(&mut certificate, certificate_corrections, dimension)?;
    Ok(ClusteredScoreMeats {
        production,
        certificate,
        subsets,
    })
}

#[derive(Debug)]
struct ClusterSubsetMeats {
    production: Vec<f64>,
    certificate: Vec<f64>,
    groups: usize,
}

/// Extend a boundary to the end of its group in an already sorted row order.
/// A giant group may cover many provisional boundaries; binary search avoids
/// rescanning its tail while preserving every chunk and reduction order.
fn extend_cluster_boundary(
    order: &[usize],
    boundary: usize,
    mut same_group: impl FnMut(usize, usize) -> bool,
) -> usize {
    if boundary == 0 || boundary >= order.len() {
        return boundary;
    }
    let prior = order[boundary - 1];
    if !same_group(prior, order[boundary]) {
        return boundary;
    }
    boundary + 1 + order[boundary + 1..].partition_point(|row| same_group(prior, *row))
}

fn reduce_one_cluster_subset<D, M, R>(
    design: D,
    multipliers: M,
    dimensions: &[&[u32]],
    mask: usize,
    policy: FixedBlockReductionPolicy,
    influence_bread: Option<&[f64]>,
    reservation: &mut R,
) -> Result<ClusterSubsetMeats>
where
    D: SandwichDesign + Copy + Sync,
    M: SandwichScoreMultiplier + Copy + Sync,
    R: SandwichReservation,
{
    let nrows = design.nrows();
    let dimension = design.dimension();
    let square = checked_square(
        dimension,
        "clustered sandwich subset dimensions overflow usize",
    )?;
    let order = build_cluster_order(dimensions, mask, nrows, reservation)?;
    let workspace = dimension
        .checked_mul(5)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| allocation("clustered sandwich subset workspace overflows usize"))?;
    let partial_stride = square
        .checked_mul(2)
        .and_then(|value| value.checked_add(workspace))
        .ok_or_else(|| allocation("clustered sandwich subset partial stride overflows usize"))?;
    let chunks = reduction_chunks(nrows, partial_stride, policy)?;
    let mut boundaries = reserved_filled(
        reservation,
        chunks + 1,
        0_usize,
        "unable to reserve clustered sandwich subset boundaries",
    )?;
    boundaries[chunks] = nrows;
    for (chunk, target) in boundaries.iter_mut().enumerate().take(chunks).skip(1) {
        *target = extend_cluster_boundary(&order, chunk * nrows / chunks, |left, right| {
            compare_cluster_rows(dimensions, mask, left, right) == Ordering::Equal
        });
    }
    for chunk in 1..chunks {
        boundaries[chunk] = boundaries[chunk].max(boundaries[chunk - 1]);
    }
    let partial_elements = chunks
        .checked_mul(partial_stride)
        .ok_or_else(|| allocation("clustered sandwich subset partials overflow usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve clustered sandwich subset partials",
    )?;
    partials
        .par_chunks_mut(partial_stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let (production_meat, tail) = partial.split_at_mut(square);
            let (certificate_meat, workspace) = tail.split_at_mut(square);
            let (production_group, workspace) = workspace.split_at_mut(dimension);
            let (production_correction, workspace) = workspace.split_at_mut(dimension);
            let (certificate_group, workspace) = workspace.split_at_mut(dimension);
            let (certificate_correction, workspace) = workspace.split_at_mut(dimension);
            let (row_values, count) = workspace.split_at_mut(dimension);
            let start = boundaries[chunk];
            let end = boundaries[chunk + 1];
            let mut previous = None;
            for row in order[start..end].iter().copied() {
                if previous.is_some_and(|prior| {
                    compare_cluster_rows(dimensions, mask, prior, row) != Ordering::Equal
                }) {
                    finish_cluster_group(
                        production_group,
                        production_correction,
                        certificate_group,
                        certificate_correction,
                        production_meat,
                        certificate_meat,
                        row_values,
                        influence_bread,
                    )?;
                    production_group.fill(0.0);
                    production_correction.fill(0.0);
                    certificate_group.fill(0.0);
                    certificate_correction.fill(0.0);
                    count[0] += 1.0;
                }
                design.fill_row(row, row_values)?;
                let score = multipliers.score_pair(row)?;
                for coordinate in 0..dimension {
                    compensated_add(
                        score.production * row_values[coordinate],
                        &mut production_group[coordinate],
                        &mut production_correction[coordinate],
                    )?;
                    compensated_add(
                        score.certificate * row_values[coordinate],
                        &mut certificate_group[coordinate],
                        &mut certificate_correction[coordinate],
                    )?;
                }
                previous = Some(row);
            }
            if previous.is_some() {
                finish_cluster_group(
                    production_group,
                    production_correction,
                    certificate_group,
                    certificate_correction,
                    production_meat,
                    certificate_meat,
                    row_values,
                    influence_bread,
                )?;
                count[0] += 1.0;
            }
            if partial.iter().all(|value| value.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "clustered sandwich subset partial contains a nonfinite value",
                ))
            }
        })?;

    let mut production = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve clustered sandwich subset production meat",
    )?;
    let mut production_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve clustered sandwich subset production corrections",
    )?;
    let mut certificate = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve clustered sandwich subset certificate meat",
    )?;
    let mut certificate_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve clustered sandwich subset certificate corrections",
    )?;
    let mut groups = 0_usize;
    for partial in partials.chunks_exact(partial_stride) {
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                compensated_add(
                    partial[position],
                    &mut production[position],
                    &mut production_corrections[position],
                )?;
            }
        }
        let count = partial[2 * square + 5 * dimension];
        if count.fract() != 0.0 || count < 0.0 || count > usize::MAX as f64 {
            return Err(invalid_data(
                "clustered sandwich subset group count is invalid",
            ));
        }
        groups = groups
            .checked_add(count as usize)
            .ok_or_else(|| allocation("clustered sandwich group count overflows usize"))?;
    }
    for partial in partials.rchunks_exact(partial_stride) {
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                compensated_add(
                    partial[square + position],
                    &mut certificate[position],
                    &mut certificate_corrections[position],
                )?;
            }
        }
    }
    finalize_symmetric_triangle(&mut production, production_corrections, dimension)?;
    finalize_symmetric_triangle(&mut certificate, certificate_corrections, dimension)?;
    Ok(ClusterSubsetMeats {
        production,
        certificate,
        groups,
    })
}

#[allow(clippy::too_many_arguments)]
fn finish_cluster_group(
    production_score: &mut [f64],
    production_corrections: &mut [f64],
    certificate_score: &mut [f64],
    certificate_corrections: &mut [f64],
    production_meat: &mut [f64],
    certificate_meat: &mut [f64],
    row_workspace: &mut [f64],
    influence_bread: Option<&[f64]>,
) -> Result<()> {
    let dimension = production_score.len();
    if production_corrections.len() != dimension
        || certificate_score.len() != dimension
        || certificate_corrections.len() != dimension
        || production_meat.len() != dimension * dimension
        || certificate_meat.len() != production_meat.len()
    {
        return Err(invalid_argument(
            "clustered sandwich group dimensions are inconsistent",
        ));
    }
    if let Some(bread) = influence_bread {
        influence::transform_group(
            production_score,
            production_corrections,
            row_workspace,
            bread,
            false,
        )?;
        influence::transform_group(
            certificate_score,
            certificate_corrections,
            row_workspace,
            bread,
            true,
        )?;
    }
    for left in 0..dimension {
        let production_left = production_score[left] + production_corrections[left];
        let certificate_left = certificate_corrections[left] + certificate_score[left];
        for right in left..dimension {
            let position = left * dimension + right;
            production_meat[position] +=
                production_left * (production_score[right] + production_corrections[right]);
            certificate_meat[position] +=
                (certificate_corrections[right] + certificate_score[right]) * certificate_left;
        }
    }
    if production_meat.iter().all(|value| value.is_finite())
        && certificate_meat.iter().all(|value| value.is_finite())
    {
        Ok(())
    } else {
        Err(invalid_data(
            "clustered sandwich group meat contains a nonfinite value",
        ))
    }
}

fn build_cluster_order<R: SandwichReservation>(
    dimensions: &[&[u32]],
    mask: usize,
    nrows: usize,
    reservation: &mut R,
) -> Result<Vec<usize>> {
    if nrows == 0
        || dimensions.is_empty()
        || mask == 0
        || mask >= (1_usize << dimensions.len())
        || dimensions.iter().any(|codes| codes.len() != nrows)
    {
        return Err(invalid_argument(
            "clustered sandwich ordering dimensions are inconsistent",
        ));
    }
    let mut maximum_radix = 0_usize;
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let radix = codes
            .iter()
            .copied()
            .max()
            .and_then(|value| usize::try_from(value).ok())
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| invalid_data("clustered sandwich dimension has no groups"))?;
        maximum_radix = maximum_radix.max(radix);
    }
    let ordering_bytes = nrows
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("clustered sandwich ordering bytes overflow usize"))?;
    let count_bytes = maximum_radix
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("clustered sandwich count bytes overflow usize"))?;
    if count_bytes > ordering_bytes.saturating_mul(2).max(1 << 20) {
        let mut order = reserved_empty(
            reservation,
            nrows,
            "unable to reserve clustered sandwich tuple ordering",
        )?;
        for row in 0..nrows {
            push_reserved(
                &mut order,
                row,
                "clustered sandwich tuple ordering capacity exhausted",
            )?;
        }
        order.sort_unstable_by(|left, right| {
            compare_cluster_rows(dimensions, mask, *left, *right).then_with(|| left.cmp(right))
        });
        return Ok(order);
    }
    let mut order = reserved_filled(
        reservation,
        nrows,
        0_usize,
        "unable to reserve clustered sandwich radix ordering",
    )?;
    for (row, target) in order.iter_mut().enumerate() {
        *target = row;
    }
    let mut scratch = reserved_filled(
        reservation,
        nrows,
        0_usize,
        "unable to reserve clustered sandwich radix scratch",
    )?;
    let mut counts = reserved_filled(
        reservation,
        maximum_radix,
        0_usize,
        "unable to reserve clustered sandwich radix counts",
    )?;
    for dimension in (0..dimensions.len()).rev() {
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let radix = dimensions[dimension]
            .iter()
            .copied()
            .max()
            .and_then(|value| usize::try_from(value).ok())
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| invalid_data("clustered sandwich dimension has no groups"))?;
        let active_counts = &mut counts[..radix];
        active_counts.fill(0);
        for row in order.iter().copied() {
            let key = usize::try_from(dimensions[dimension][row])
                .map_err(|_| invalid_data("clustered sandwich code exceeds usize"))?;
            active_counts[key] = active_counts[key]
                .checked_add(1)
                .ok_or_else(|| allocation("clustered sandwich radix count overflows usize"))?;
        }
        let mut offset = 0_usize;
        for count in active_counts.iter_mut() {
            let frequency = *count;
            *count = offset;
            offset = offset
                .checked_add(frequency)
                .ok_or_else(|| allocation("clustered sandwich radix offset overflows usize"))?;
        }
        if offset != nrows {
            return Err(invalid_data(
                "clustered sandwich radix counts do not cover the sample",
            ));
        }
        for row in order.iter().copied() {
            let key = usize::try_from(dimensions[dimension][row])
                .map_err(|_| invalid_data("clustered sandwich code exceeds usize"))?;
            let position = active_counts[key];
            scratch[position] = row;
            active_counts[key] = position
                .checked_add(1)
                .ok_or_else(|| allocation("clustered sandwich radix cursor overflows usize"))?;
        }
        std::mem::swap(&mut order, &mut scratch);
    }
    Ok(order)
}

fn compare_cluster_rows(dimensions: &[&[u32]], mask: usize, left: usize, right: usize) -> Ordering {
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let ordering = codes[left].cmp(&codes[right]);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

/// Deterministic bread--meat--bread product.  Estimator correction policy is
/// explicit and applied only after the uncorrected product has been formed.
pub(crate) fn bread_meat_bread<R: SandwichReservation>(
    bread: &[f64],
    meat: &[f64],
    dimension: usize,
    correction: SandwichCorrectionPolicy,
    mode: SandwichProductMode,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let square = checked_square(dimension, "sandwich product dimensions overflow usize")?;
    if bread.len() != square || meat.len() != square {
        return Err(invalid_argument(
            "sandwich bread and meat dimensions are inconsistent",
        ));
    }
    let factor = correction.factor()?;
    let mut left = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve sandwich bread-meat product",
    )?;
    match mode {
        SandwichProductMode::OrdinaryUpperTriangle => {
            for row in 0..dimension {
                for column in 0..dimension {
                    let mut value = 0.0_f64;
                    for inner in 0..dimension {
                        value += bread[row * dimension + inner] * meat[inner * dimension + column];
                    }
                    if !value.is_finite() {
                        return Err(invalid_data(
                            "ordinary sandwich bread-meat product is not finite",
                        ));
                    }
                    left[row * dimension + column] = value;
                }
            }
        }
        SandwichProductMode::CompensatedFull => {
            for row in 0..dimension {
                for column in 0..dimension {
                    let mut value = 0.0_f64;
                    let mut compensation = 0.0_f64;
                    for inner in 0..dimension {
                        compensated_add(
                            bread[row * dimension + inner] * meat[inner * dimension + column],
                            &mut value,
                            &mut compensation,
                        )?;
                    }
                    left[row * dimension + column] = value + compensation;
                }
            }
        }
    }

    let mut covariance = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve sandwich covariance",
    )?;
    match mode {
        SandwichProductMode::OrdinaryUpperTriangle => {
            for row in 0..dimension {
                for column in row..dimension {
                    let mut value = 0.0_f64;
                    for inner in 0..dimension {
                        value += left[row * dimension + inner] * bread[column * dimension + inner];
                    }
                    value *= factor;
                    if !value.is_finite() {
                        return Err(invalid_data("ordinary sandwich product is not finite"));
                    }
                    covariance[row * dimension + column] = value;
                    covariance[column * dimension + row] = value;
                }
            }
        }
        SandwichProductMode::CompensatedFull => {
            for row in 0..dimension {
                for column in 0..dimension {
                    let mut value = 0.0_f64;
                    let mut compensation = 0.0_f64;
                    for inner in 0..dimension {
                        compensated_add(
                            left[row * dimension + inner] * bread[column * dimension + inner],
                            &mut value,
                            &mut compensation,
                        )?;
                    }
                    covariance[row * dimension + column] = factor * (value + compensation);
                    if !covariance[row * dimension + column].is_finite() {
                        return Err(invalid_data("compensated sandwich product is not finite"));
                    }
                }
            }
            symmetrize(&mut covariance, dimension)?;
        }
    }
    Ok(covariance)
}

pub(crate) fn standard_errors<R: SandwichReservation>(
    covariance: &[f64],
    dimension: usize,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let square = checked_square(
        dimension,
        "sandwich standard-error dimensions overflow usize",
    )?;
    if covariance.len() != square {
        return Err(invalid_argument(
            "sandwich covariance dimensions are inconsistent",
        ));
    }
    let mut output = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve sandwich standard errors",
    )?;
    for position in 0..dimension {
        let diagonal = covariance[position * dimension + position];
        if !diagonal.is_finite() || diagonal < 0.0 {
            return Err(invalid_data("sandwich covariance diagonal is invalid"));
        }
        output[position] = diagonal.sqrt();
    }
    Ok(output)
}

fn reduction_chunks(
    nrows: usize,
    partial_stride: usize,
    policy: FixedBlockReductionPolicy,
) -> Result<usize> {
    let partial_bytes = partial_stride
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or_else(|| allocation("sandwich reduction partial bytes overflow usize"))?;
    let memory_blocks = (policy.partial_budget_bytes / partial_bytes.max(1)).max(1);
    Ok(nrows
        .div_ceil(policy.rows_per_block)
        .clamp(1, policy.maximum_blocks)
        .min(memory_blocks))
}

fn finalize_symmetric_triangle(
    matrix: &mut [f64],
    corrections: Vec<f64>,
    dimension: usize,
) -> Result<()> {
    let square = checked_square(dimension, "sandwich meat dimensions overflow usize")?;
    if matrix.len() != square || corrections.len() != square {
        return Err(invalid_argument(
            "sandwich meat dimensions are inconsistent",
        ));
    }
    for left in 0..dimension {
        for right in left..dimension {
            let position = left * dimension + right;
            matrix[position] += corrections[position];
            if !matrix[position].is_finite() {
                return Err(invalid_data("sandwich meat contains a nonfinite value"));
            }
            matrix[right * dimension + left] = matrix[position];
        }
    }
    Ok(())
}

fn symmetrize(matrix: &mut [f64], dimension: usize) -> Result<()> {
    if matrix.len() != dimension * dimension {
        return Err(invalid_argument(
            "sandwich symmetry dimensions are inconsistent",
        ));
    }
    for row in 0..dimension {
        for column in (row + 1)..dimension {
            let value = 0.5 * (matrix[row * dimension + column] + matrix[column * dimension + row]);
            if !value.is_finite() {
                return Err(invalid_data("sandwich symmetrized value is not finite"));
            }
            matrix[row * dimension + column] = value;
            matrix[column * dimension + row] = value;
        }
    }
    Ok(())
}

fn reserved_filled<T: Clone, R: SandwichReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "sandwich reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, value);
    Ok(output)
}

fn reserved_empty<T, R: SandwichReservation>(
    reservation: &mut R,
    elements: usize,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "sandwich reservation returned insufficient capacity",
        ));
    }
    Ok(output)
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn checked_square(dimension: usize, message: &'static str) -> Result<usize> {
    dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation(message))
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "sandwich compensated accumulator is not finite",
        ))
    }
}

pub(crate) fn compensated_add_scaled_row(
    scale: f64,
    values: &[f64],
    sums: &mut [f64],
    corrections: &mut [f64],
) -> Result<()> {
    if values.len() != sums.len() || values.len() != corrections.len() {
        return Err(invalid_argument(
            "prepared compensated row dimensions are inconsistent",
        ));
    }
    // Coordinates are independent. Check the whole private row before its
    // caller continues so finite-value branches do not serialize each update.
    // Each coordinate retains the scalar Neumaier operations and row order.
    let mut finite = true;
    for ((value, sum), correction) in values.iter().zip(sums).zip(corrections) {
        let term = scale * value;
        let updated = *sum + term;
        let adjustment = if sum.abs() >= term.abs() {
            (*sum - updated) + term
        } else {
            (term - updated) + *sum
        };
        *sum = updated;
        *correction += adjustment;
        finite &= term.is_finite() & sum.is_finite() & correction.is_finite();
    }
    if finite {
        Ok(())
    } else {
        Err(invalid_data(
            "prepared compensated row accumulation is not finite",
        ))
    }
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_boundaries_match_linear_group_ends_for_every_subset() {
        for nrows in [0usize, 1, 2, 7, 64, 257, 8193] {
            for skew in [false, true] {
                let first: Vec<u32> = (0..nrows)
                    .map(|row| {
                        if skew && row % 20 != 0 {
                            0
                        } else {
                            1 + (row % 17) as u32
                        }
                    })
                    .collect();
                let second: Vec<u32> = (0..nrows).map(|row| ((row / 7) % 3) as u32).collect();
                let third: Vec<u32> = (0..nrows).map(|row| (row / 23) as u32).collect();
                let dimensions = [first.as_slice(), second.as_slice(), third.as_slice()];
                for mask in 1..8 {
                    let mut order: Vec<usize> = (0..nrows).rev().collect();
                    order.sort_unstable_by(|left, right| {
                        compare_cluster_rows(&dimensions, mask, *left, *right)
                            .then_with(|| left.cmp(right))
                    });
                    for boundary in 0..=nrows {
                        let mut expected = boundary;
                        while expected > 0
                            && expected < nrows
                            && compare_cluster_rows(
                                &dimensions,
                                mask,
                                order[expected - 1],
                                order[expected],
                            ) == Ordering::Equal
                        {
                            expected += 1;
                        }
                        let actual = extend_cluster_boundary(&order, boundary, |left, right| {
                            compare_cluster_rows(&dimensions, mask, left, right) == Ordering::Equal
                        });
                        assert_eq!(
                            actual, expected,
                            "rows={nrows}, skew={skew}, mask={mask}, boundary={boundary}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cluster_boundary_search_bounds_giant_group_work_and_keeps_short_edges_cheap() {
        let nrows = 1usize << 20;
        let order: Vec<usize> = (0..nrows).collect();
        for boundary in [1, nrows / 4, nrows / 2, nrows - 1] {
            let mut comparisons = 0;
            let end = extend_cluster_boundary(&order, boundary, |_, _| {
                comparisons += 1;
                true
            });
            assert_eq!(end, nrows);
            assert!(comparisons <= 22, "{comparisons} comparisons");
            let mut comparisons = 0;
            assert_eq!(
                extend_cluster_boundary(&order, boundary, |_, _| {
                    comparisons += 1;
                    false
                }),
                boundary
            );
            assert_eq!(comparisons, 1);
        }
    }

    struct Reservation;

    impl SandwichReservation for Reservation {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            target
                .try_reserve_exact(elements)
                .map_err(|_| allocation("test sandwich reservation failed"))
        }
    }

    #[derive(Default)]
    struct CountingReservation {
        calls: usize,
        fail_at: Option<usize>,
        undersize: bool,
    }

    impl SandwichReservation for CountingReservation {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            self.calls += 1;
            if self.fail_at == Some(self.calls) {
                return Err(allocation("injected serial sandwich reservation failure"));
            }
            if !self.undersize {
                target
                    .try_reserve_exact(elements)
                    .map_err(|_| allocation("test serial sandwich reservation failed"))?;
            }
            Ok(())
        }
    }

    struct QualifiedProvider;

    impl QualifiedClusterSubsetMeatProvider<Reservation> for QualifiedProvider {
        type Report = ClusterSubsetReduction;

        fn dimension(&self) -> usize {
            2
        }

        fn cluster_dimensions(&self) -> usize {
            2
        }

        fn reduce_subset(
            &self,
            mask: usize,
            _reservation: &mut Reservation,
        ) -> Result<QualifiedClusterSubsetMeats> {
            let (production, certificate, groups) = match mask {
                1 => ([4.0, 1.0, 1.0, 3.0], [5.0, 2.0, 2.0, 4.0], 3),
                2 => ([2.0, 3.0, 3.0, 7.0], [3.0, 4.0, 4.0, 8.0], 4),
                3 => ([1.0, 1.0, 1.0, 2.0], [2.0, 2.0, 2.0, 3.0], 5),
                _ => return Err(invalid_argument("unexpected qualified subset mask")),
            };
            Ok(QualifiedClusterSubsetMeats {
                production: production.to_vec(),
                certificate: certificate.to_vec(),
                groups,
            })
        }

        fn report(
            &self,
            mask: usize,
            dimensions: usize,
            sign: i8,
            groups: usize,
        ) -> Result<Self::Report> {
            Ok(ClusterSubsetReduction {
                mask: u16::try_from(mask).expect("test mask"),
                dimensions,
                sign,
                groups,
            })
        }
    }

    #[test]
    fn qualified_cluster_driver_preserves_mask_order_and_independent_meats() {
        let mut reservation = Reservation;
        let reduced = reduce_qualified_clustered_score_meats(&QualifiedProvider, &mut reservation)
            .expect("qualified cluster reduction");
        assert_eq!(reduced.production, [5.0, 3.0, 3.0, 8.0]);
        assert_eq!(reduced.certificate, [6.0, 4.0, 4.0, 9.0]);
        let reports = [
            ClusterSubsetReduction {
                mask: 1,
                dimensions: 1,
                sign: 1,
                groups: 3,
            },
            ClusterSubsetReduction {
                mask: 2,
                dimensions: 1,
                sign: 1,
                groups: 4,
            },
            ClusterSubsetReduction {
                mask: 3,
                dimensions: 2,
                sign: -1,
                groups: 5,
            },
        ];
        assert_eq!(reduced.subsets, reports);
        assert_eq!(reduced.certificate_subsets, reports);
    }

    struct RankZeroQualifiedProvider;

    impl QualifiedClusterSubsetMeatProvider<Reservation> for RankZeroQualifiedProvider {
        type Report = ClusterSubsetReduction;

        fn dimension(&self) -> usize {
            0
        }
        fn cluster_dimensions(&self) -> usize {
            1
        }

        fn reduce_subset(
            &self,
            mask: usize,
            _reservation: &mut Reservation,
        ) -> Result<QualifiedClusterSubsetMeats> {
            assert_eq!(mask, 1);
            Ok(QualifiedClusterSubsetMeats {
                production: Vec::new(),
                certificate: Vec::new(),
                groups: 3,
            })
        }

        fn report(
            &self,
            mask: usize,
            dimensions: usize,
            sign: i8,
            groups: usize,
        ) -> Result<Self::Report> {
            Ok(ClusterSubsetReduction {
                mask: u16::try_from(mask).expect("test mask"),
                dimensions,
                sign,
                groups,
            })
        }
    }

    #[test]
    fn qualified_cluster_driver_preserves_rank_zero_group_inventory() {
        let mut reservation = Reservation;
        let reduced =
            reduce_qualified_clustered_score_meats(&RankZeroQualifiedProvider, &mut reservation)
                .expect("rank-zero qualified cluster reduction");
        assert!(reduced.production.is_empty());
        assert!(reduced.certificate.is_empty());
        assert_eq!(
            reduced.subsets,
            [ClusterSubsetReduction {
                mask: 1,
                dimensions: 1,
                sign: 1,
                groups: 3
            }]
        );
        assert_eq!(reduced.certificate_subsets, reduced.subsets);
    }

    fn legacy_dense_fixed_block_meats(
        values: &[f64],
        nrows: usize,
        dimension: usize,
        production_multiplier: &[f64],
        certificate_multiplier: &[f64],
        policy: FixedBlockReductionPolicy,
    ) -> (Vec<f64>, Vec<f64>) {
        let square = dimension * dimension;
        let workspace = 2 * dimension;
        let partial_stride = 2 * square + workspace;
        let chunks = reduction_chunks(nrows, partial_stride, policy).expect("legacy chunks");
        let mut partials = vec![0.0_f64; chunks * partial_stride];
        for (chunk, partial) in partials.chunks_mut(partial_stride).enumerate() {
            let (production_meat, tail) = partial.split_at_mut(square);
            let (certificate_meat, workspace) = tail.split_at_mut(square);
            let (production_score, certificate_score) = workspace.split_at_mut(dimension);
            let rows_per_chunk = nrows / chunks;
            let remainder = nrows % chunks;
            let start = chunk * rows_per_chunk + chunk.min(remainder);
            let end = start + rows_per_chunk + usize::from(chunk < remainder);
            for row in start..end {
                production_score.copy_from_slice(&values[row * dimension..(row + 1) * dimension]);
                for coordinate in 0..dimension {
                    let raw = production_score[coordinate];
                    production_score[coordinate] = production_multiplier[row] * raw;
                    certificate_score[coordinate] = certificate_multiplier[row] * raw;
                }
                for left in 0..dimension {
                    let production_left = production_score[left];
                    let certificate_left = certificate_score[left];
                    for right in left..dimension {
                        let position = left * dimension + right;
                        production_meat[position] += production_left * production_score[right];
                        certificate_meat[position] += certificate_score[right] * certificate_left;
                    }
                }
            }
        }
        let mut production = vec![0.0_f64; square];
        let mut production_corrections = vec![0.0_f64; square];
        let mut certificate = vec![0.0_f64; square];
        let mut certificate_corrections = vec![0.0_f64; square];
        for partial in partials.chunks_exact(partial_stride) {
            for left in 0..dimension {
                for right in left..dimension {
                    let position = left * dimension + right;
                    compensated_add(
                        partial[position],
                        &mut production[position],
                        &mut production_corrections[position],
                    )
                    .expect("legacy production merge");
                }
            }
        }
        for partial in partials.rchunks_exact(partial_stride) {
            for left in 0..dimension {
                for right in left..dimension {
                    let position = left * dimension + right;
                    compensated_add(
                        partial[square + position],
                        &mut certificate[position],
                        &mut certificate_corrections[position],
                    )
                    .expect("legacy certificate merge");
                }
            }
        }
        finalize_symmetric_triangle(&mut production, production_corrections, dimension)
            .expect("legacy production finalization");
        finalize_symmetric_triangle(&mut certificate, certificate_corrections, dimension)
            .expect("legacy certificate finalization");
        (production, certificate)
    }

    fn legacy_serial_influence_covariance(
        values: &[f64],
        nrows: usize,
        dimension: usize,
        multipliers: &[f64],
        bread: &[f64],
    ) -> Vec<f64> {
        let square = dimension * dimension;
        let mut covariance = vec![0.0_f64; square];
        let mut corrections = vec![0.0_f64; square];
        let mut influence = vec![0.0_f64; dimension];
        let mut score = vec![0.0_f64; dimension];
        for row in 0..nrows {
            score.copy_from_slice(&values[row * dimension..(row + 1) * dimension]);
            for (left, influence_value) in influence.iter_mut().enumerate() {
                let mut product = 0.0_f64;
                let mut correction = 0.0_f64;
                for (right, score_value) in score.iter().copied().enumerate() {
                    compensated_add(
                        bread[left * dimension + right] * score_value,
                        &mut product,
                        &mut correction,
                    )
                    .expect("legacy serial bread product");
                }
                *influence_value = multipliers[row] * (product + correction);
            }
            for left in 0..dimension {
                for right in left..dimension {
                    let position = left * dimension + right;
                    compensated_add(
                        influence[left] * influence[right],
                        &mut covariance[position],
                        &mut corrections[position],
                    )
                    .expect("legacy serial influence accumulation");
                }
            }
        }
        finalize_symmetric_triangle(&mut covariance, corrections, dimension)
            .expect("legacy serial covariance finalization");
        covariance
    }

    #[test]
    fn serial_influence_covariance_is_bit_identical_to_qualified_ols_loop() {
        let values = [
            1.0, 0.17, -2.1, 1.0, -0.4, 0.33, 1.0, 1.75, 0.09, 1.0, -3.2, 2.4, 1.0, 0.61, -0.81,
            1.0, 4.7, 1.3, 1.0, -0.02, -2.9,
        ];
        let multipliers = [0.13, -1.7, 2.2, 0.03, -0.9, 1.01, 0.47];
        let bread = [0.73, -0.12, 0.04, -0.12, 0.51, -0.09, 0.04, -0.09, 0.28];
        let expected = legacy_serial_influence_covariance(&values, 7, 3, &multipliers, &bread);
        let design = RowMajorSandwichDesign::new(&values, 7, 3).expect("serial design");
        let scores = SliceSandwichScoreMultiplier::same(&multipliers).expect("serial multipliers");
        let mut reservation = CountingReservation::default();
        let actual = reduce_serial_influence_covariance(&design, scores, &bread, &mut reservation)
            .expect("serial covariance");
        assert_eq!(actual, expected);
        assert_eq!(reservation.calls, 4);
    }

    #[test]
    fn serial_influence_rows_preserve_scalar_bits_across_widths_and_scales() {
        let rows = 67;
        for dimension in [0, 1, 2, 3, 7, 8, 15, 16, 17, 31, 32, 33, 63, 64, 65, 129] {
            let values: Vec<_> = (0..rows * dimension)
                .map(|i| ((i * 17 + 3) as f64).sin() * [0.0, 1e-40, 1.0, 1e40][i % 4])
                .collect();
            let bread: Vec<_> = (0..dimension * dimension)
                .map(|i| ((i * 11 + 7) as f64).cos() * [1e-20, 1.0, 1e20][i % 3])
                .collect();
            let multipliers: Vec<_> = (0..rows)
                .map(|i| [0.0, -0.0, 1e-10, -1.0, 1e10][i % 5])
                .collect();
            let expected =
                legacy_serial_influence_covariance(&values, rows, dimension, &multipliers, &bread);
            let design = RowMajorSandwichDesign::new(&values, rows, dimension).unwrap();
            let scores = SliceSandwichScoreMultiplier::same(&multipliers).unwrap();
            let mut reservation = CountingReservation::default();
            let actual =
                reduce_serial_influence_covariance(&design, scores, &bread, &mut reservation)
                    .unwrap();
            assert_eq!(reservation.calls, 4);
            for (actual, expected) in actual.iter().zip(expected) {
                assert_eq!(actual.to_bits(), expected.to_bits(), "width={dimension}");
            }
        }
    }

    #[test]
    fn serial_influence_rows_reject_nonfinite_bread_and_outer_products() {
        for (values, bread) in [
            (vec![1.0, 2.0], f64::NAN),
            (vec![1.0, 2.0], f64::INFINITY),
            (vec![1.0, 2.0], f64::MAX),
            (vec![1e200, 1.0], 1.0),
            (vec![1e154, 1e154], 1.0),
        ] {
            let design = RowMajorSandwichDesign::new(&values, 2, 1).unwrap();
            let multipliers = [1.0, 1.0];
            let scores = SliceSandwichScoreMultiplier::same(&multipliers).unwrap();
            let error = reduce_serial_influence_covariance(
                &design,
                scores,
                &[bread],
                &mut CountingReservation::default(),
            )
            .expect_err("nonfinite influence or outer accumulator");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn serial_influence_covariance_preserves_rank_zero_and_fallible_workspaces() {
        let empty_design = RowMajorSandwichDesign::new(&[], 3, 0).expect("rank-zero design");
        let multipliers = [1.0, -2.0, 0.5];
        let scores =
            SliceSandwichScoreMultiplier::same(&multipliers).expect("rank-zero multipliers");
        let mut rank_zero_reservation = CountingReservation::default();
        let rank_zero = reduce_serial_influence_covariance(
            &empty_design,
            scores,
            &[],
            &mut rank_zero_reservation,
        )
        .expect("rank-zero serial covariance");
        assert!(rank_zero.is_empty());
        assert_eq!(rank_zero_reservation.calls, 4);

        let values = [1.0, -0.5, 1.0, 2.0, 1.0, 0.25];
        let bread = [0.8, -0.1, -0.1, 0.4];
        let design = RowMajorSandwichDesign::new(&values, 3, 2).expect("allocation design");
        for fail_at in 1..=4 {
            let mut reservation = CountingReservation {
                fail_at: Some(fail_at),
                ..CountingReservation::default()
            };
            let error =
                reduce_serial_influence_covariance(&design, scores, &bread, &mut reservation)
                    .expect_err("each serial workspace reservation must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation, "fail_at={fail_at}");
        }
        let mut undersized = CountingReservation {
            undersize: true,
            ..CountingReservation::default()
        };
        let error = reduce_serial_influence_covariance(&design, scores, &bread, &mut undersized)
            .expect_err("undersized serial reservation must fail closed");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    #[test]
    fn dense_meat_matches_literal_row_outer_products_exactly() {
        let values = [1.0, 2.0, 1.0, -1.0, 1.0, 0.5, 1.0, 3.0];
        let production = [2.0, -1.0, 0.25, 3.0];
        let certificate = [1.5, -0.5, 0.75, 2.0];
        let design = RowMajorSandwichDesign::new(&values, 4, 2).expect("design");
        let multipliers =
            SliceSandwichScoreMultiplier::new(&production, &certificate).expect("scores");
        let mut reservation = Reservation;
        let meats = reduce_dense_score_meats(
            design,
            multipliers,
            FixedBlockReductionPolicy {
                rows_per_block: 1,
                maximum_blocks: 4,
                partial_budget_bytes: 1 << 20,
            },
            None,
            &mut reservation,
        )
        .expect("meats");
        let mut expected_production = vec![0.0; 4];
        let mut expected_certificate = vec![0.0; 4];
        for row in 0..4 {
            for left in 0..2 {
                for right in 0..2 {
                    expected_production[left * 2 + right] += production[row]
                        * values[row * 2 + left]
                        * production[row]
                        * values[row * 2 + right];
                    expected_certificate[left * 2 + right] += certificate[row]
                        * values[row * 2 + left]
                        * certificate[row]
                        * values[row * 2 + right];
                }
            }
        }
        assert_eq!(meats.production, expected_production);
        assert_eq!(meats.certificate, expected_certificate);
    }

    #[test]
    fn dense_meat_is_bit_identical_to_the_qualified_fixed_block_loop() {
        let values = [
            1.0, 0.17, -2.1, 1.0, -0.4, 0.33, 1.0, 1.75, 0.09, 1.0, -3.2, 2.4, 1.0, 0.61, -0.81,
            1.0, 4.7, 1.3, 1.0, -0.02, -2.9, 1.0, 2.25, 3.4, 1.0, -1.1, 0.73,
        ];
        let production = [0.13, -1.7, 2.2, 0.03, -0.9, 1.01, 0.47, -2.3, 0.8];
        let certificate = [0.11, -1.6, 2.0, 0.07, -0.8, 0.99, 0.43, -2.1, 0.77];
        let policy = FixedBlockReductionPolicy {
            rows_per_block: 2,
            maximum_blocks: 4,
            partial_budget_bytes: 1 << 20,
        };
        let (legacy_production, legacy_certificate) =
            legacy_dense_fixed_block_meats(&values, 9, 3, &production, &certificate, policy);
        let design = RowMajorSandwichDesign::new(&values, 9, 3).expect("design");
        let multipliers =
            SliceSandwichScoreMultiplier::new(&production, &certificate).expect("scores");
        let mut reservation = Reservation;
        let shared = reduce_dense_score_meats(design, multipliers, policy, None, &mut reservation)
            .expect("shared meats");
        assert_eq!(shared.production, legacy_production);
        assert_eq!(shared.certificate, legacy_certificate);
    }

    #[test]
    fn dense_triangle_rows_preserve_both_independent_meats_across_widths_and_teams() {
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                for dimension in (1..=17).chain([31, 50, 65]) {
                    let nrows = 257;
                    let values: Vec<f64> = (0..nrows * dimension).map(|index| {
                        ((index * 37 % 101) as f64 - 50.0) / 17.0
                            * 2.0_f64.powi((index % 13) as i32 - 6)
                    }).collect();
                    let production: Vec<f64> = (0..nrows).map(|row| {
                        ((row * 19 % 97) as f64 - 48.0) / 13.0
                    }).collect();
                    let certificate: Vec<f64> = (0..nrows).map(|row| {
                        ((row * 23 % 89) as f64 - 44.0) / 11.0
                    }).collect();
                    for rows_per_block in [1, 32, 512] {
                        let policy = FixedBlockReductionPolicy {
                            rows_per_block,
                            maximum_blocks: 16,
                            partial_budget_bytes: 1 << 20,
                        };
                        let expected = legacy_dense_fixed_block_meats(
                            &values, nrows, dimension, &production, &certificate, policy,
                        );
                        let actual = reduce_dense_score_meats(
                            RowMajorSandwichDesign::new(&values, nrows, dimension).unwrap(),
                            SliceSandwichScoreMultiplier::new(&production, &certificate).unwrap(),
                            policy, None, &mut Reservation,
                        ).unwrap();
                        assert_eq!(actual.production, expected.0,
                            "production dimension={dimension} threads={threads} block={rows_per_block}");
                        assert_eq!(actual.certificate, expected.1,
                            "certificate dimension={dimension} threads={threads} block={rows_per_block}");
                    }
                }
            });
        }
    }

    #[test]
    fn fixed_dense_triangle_rejects_shapes_before_modifying_either_meat() {
        assert!(fixed_dense_triangle_update(0).is_none());
        assert!(fixed_dense_triangle_update(17).is_none());
        let update = fixed_dense_triangle_update(3).unwrap();
        for malformed in 0..4 {
            let mut production = vec![2.0; if malformed == 0 { 8 } else { 9 }];
            let mut certificate = vec![3.0; if malformed == 1 { 8 } else { 9 }];
            let production_score = vec![5.0; if malformed == 2 { 2 } else { 3 }];
            let certificate_score = vec![7.0; if malformed == 3 { 2 } else { 3 }];
            let prior_production = production.clone();
            let prior_certificate = certificate.clone();
            assert!(update(
                &mut production,
                &mut certificate,
                &production_score,
                &certificate_score
            )
            .is_err());
            assert_eq!(production, prior_production);
            assert_eq!(certificate, prior_certificate);
        }
    }

    #[test]
    fn ordinary_product_matches_legacy_loop_bit_for_bit() {
        let bread = [0.75, -0.125, -0.125, 0.5];
        let meat = [4.0, 1.25, 1.25, 2.0];
        let correction = 1.75;
        let mut expected_left = [0.0; 4];
        for row in 0..2 {
            for column in 0..2 {
                let mut value = 0.0;
                for inner in 0..2 {
                    value += bread[row * 2 + inner] * meat[inner * 2 + column];
                }
                expected_left[row * 2 + column] = value;
            }
        }
        let mut expected = vec![0.0; 4];
        for row in 0..2 {
            for column in row..2 {
                let mut value = 0.0;
                for inner in 0..2 {
                    value += expected_left[row * 2 + inner] * bread[column * 2 + inner];
                }
                value *= correction;
                expected[row * 2 + column] = value;
                expected[column * 2 + row] = value;
            }
        }
        let mut reservation = Reservation;
        let actual = bread_meat_bread(
            &bread,
            &meat,
            2,
            SandwichCorrectionPolicy::Scale(correction),
            SandwichProductMode::OrdinaryUpperTriangle,
            &mut reservation,
        )
        .expect("sandwich");
        assert_eq!(actual, expected);
    }

    #[test]
    fn compensated_product_is_symmetric_and_identity_preserving() {
        let bread = [1.0, 0.0, 0.0, 1.0];
        let meat = [2.0, -0.25, -0.25, 4.0];
        let mut reservation = Reservation;
        let actual = bread_meat_bread(
            &bread,
            &meat,
            2,
            SandwichCorrectionPolicy::None,
            SandwichProductMode::CompensatedFull,
            &mut reservation,
        )
        .expect("sandwich");
        assert_eq!(actual, meat);
    }

    #[test]
    fn compensated_product_matches_the_qualified_cluster_loop_bit_for_bit() {
        let bread = [0.73, -0.12, 0.04, -0.12, 0.51, -0.09, 0.04, -0.09, 0.28];
        let meat = [2.3, -0.17, 0.61, -0.17, 1.8, -0.42, 0.61, -0.42, 3.7];
        let mut left = [0.0_f64; 9];
        for row in 0..3 {
            for column in 0..3 {
                let mut value = 0.0_f64;
                let mut correction = 0.0_f64;
                for inner in 0..3 {
                    compensated_add(
                        bread[row * 3 + inner] * meat[inner * 3 + column],
                        &mut value,
                        &mut correction,
                    )
                    .expect("legacy left product");
                }
                left[row * 3 + column] = value + correction;
            }
        }
        let mut expected = vec![0.0_f64; 9];
        for row in 0..3 {
            for column in 0..3 {
                let mut value = 0.0_f64;
                let mut correction = 0.0_f64;
                for inner in 0..3 {
                    compensated_add(
                        left[row * 3 + inner] * bread[column * 3 + inner],
                        &mut value,
                        &mut correction,
                    )
                    .expect("legacy full product");
                }
                expected[row * 3 + column] = value + correction;
            }
        }
        symmetrize(&mut expected, 3).expect("legacy symmetry");

        let mut reservation = Reservation;
        let actual = bread_meat_bread(
            &bread,
            &meat,
            3,
            SandwichCorrectionPolicy::None,
            SandwichProductMode::CompensatedFull,
            &mut reservation,
        )
        .expect("shared cluster sandwich");
        assert_eq!(actual, expected);
    }

    #[test]
    fn two_way_clustered_meat_matches_literal_inclusion_exclusion() {
        let values = [1.0, 0.0, 1.0, 1.0, 1.0, 2.0, 1.0, 3.0];
        let scores = [1.0, -2.0, 0.5, 3.0];
        let cluster_a = [0_u32, 0, 1, 1];
        let cluster_b = [0_u32, 1, 0, 1];
        let dimensions: [&[u32]; 2] = [&cluster_a, &cluster_b];
        let design = RowMajorSandwichDesign::new(&values, 4, 2).expect("design");
        let multipliers = SliceSandwichScoreMultiplier::same(&scores).expect("scores");
        let clusters = SandwichClusters::new(&dimensions, 4, 5).expect("clusters");
        let mut reservation = Reservation;
        let actual = reduce_clustered_score_meats(
            design,
            multipliers,
            clusters,
            FixedBlockReductionPolicy {
                rows_per_block: 1,
                maximum_blocks: 4,
                partial_budget_bytes: 1 << 20,
            },
            None,
            &mut reservation,
        )
        .expect("clustered meat");

        let mut expected = vec![0.0_f64; 4];
        for (codes, sign) in [(&cluster_a[..], 1.0), (&cluster_b[..], 1.0)] {
            for group in 0..=1_u32 {
                let mut group_score = [0.0_f64; 2];
                for row in 0..4 {
                    if codes[row] == group {
                        for coordinate in 0..2 {
                            group_score[coordinate] += scores[row] * values[row * 2 + coordinate];
                        }
                    }
                }
                for left in 0..2 {
                    for right in 0..2 {
                        expected[left * 2 + right] += sign * group_score[left] * group_score[right];
                    }
                }
            }
        }
        for row in 0..4 {
            let row_score = [scores[row], scores[row] * values[row * 2 + 1]];
            for left in 0..2 {
                for right in 0..2 {
                    expected[left * 2 + right] -= row_score[left] * row_score[right];
                }
            }
        }
        assert_eq!(actual.production, expected);
        assert_eq!(actual.certificate, expected);
        assert_eq!(
            actual.subsets,
            vec![
                ClusterSubsetReduction {
                    mask: 1,
                    dimensions: 1,
                    sign: 1,
                    groups: 2,
                },
                ClusterSubsetReduction {
                    mask: 2,
                    dimensions: 1,
                    sign: 1,
                    groups: 2,
                },
                ClusterSubsetReduction {
                    mask: 3,
                    dimensions: 2,
                    sign: -1,
                    groups: 4,
                },
            ]
        );
    }
}
