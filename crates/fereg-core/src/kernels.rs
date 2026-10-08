//! Hot loops that a host may compile for the processor it detects at runtime.
//!
//! `f64::mul_add` is exactly rounded, so a hardware FMA instruction and the
//! portable library call return identical bits. Baseline x86-64 code cannot
//! inline the instruction and pays an out-of-line call per element; it also
//! cannot use AVX vectors. A host that detects FMA and AVX may install
//! instantiations of these same bodies compiled with them enabled. Every
//! kernel uses only exactly rounded IEEE operations in a fixed per-element
//! order, so results are unchanged wherever the platform `fma` is exactly
//! rounded; without an installed table the portable bodies run inline.
use crate::linalg::neumaier_add;
use std::sync::OnceLock;

/// One observation's contribution to a partition of the weighted upper Gram
/// triangle and its right-hand side: `(weight, row, response, left_start,
/// rhs, gram)`; see [`weighted_upper_rank_one`].
pub type UpperRankOne = fn(f64, &[f64], f64, usize, &mut [f64], &mut [f64]);

/// Column scales and compensated weight sum over one group's members:
/// `(weights, raw_scale, data, rows, members, scales)`; see [`group_scales`].
pub type GroupScales =
    fn(Option<&[f64]>, f64, &[f64], usize, &[usize], &mut [f64]) -> Option<(f64, f64)>;

/// Scaled compensated column sums over one group's members:
/// `(weights, raw_scale, data, members, scales, sums, corrections)`; see
/// [`scaled_group_sums`].
pub type ScaledGroupSums =
    fn(Option<&[f64]>, f64, &[f64], &[usize], &[f64], &mut [f64], &mut [f64]);

/// Hardware-compiled instantiations of the kernels below.
#[derive(Clone, Copy, Debug)]
pub struct FusedKernels {
    /// Doctor label for the arithmetic path these entries compile to.
    pub name: &'static str,
    pub upper_rank_one: UpperRankOne,
    pub group_scales: GroupScales,
    pub scaled_group_sums: ScaledGroupSums,
}

const PORTABLE_NAME: &str = if cfg!(target_arch = "aarch64") {
    "aarch64-fma"
} else if cfg!(all(target_arch = "x86_64", target_feature = "fma")) {
    "x86_64-fma"
} else if cfg!(target_arch = "x86_64") {
    "x86_64-portable"
} else {
    "portable"
};

static INSTALLED: OnceLock<FusedKernels> = OnceLock::new();

/// Install host-compiled instantiations of these kernels for the process.
/// The first installation wins. A host must install only entries whose
/// required processor features it has detected.
pub fn install(kernels: FusedKernels) {
    let _ = INSTALLED.set(kernels);
}

/// The installed hardware table, if any. Without one, callers run the
/// portable bodies inline rather than through a per-row function pointer.
#[must_use]
pub fn installed() -> Option<&'static FusedKernels> {
    INSTALLED.get()
}

/// Doctor label for the arithmetic path currently in use.
#[must_use]
pub fn active_name() -> &'static str {
    installed().map_or(PORTABLE_NAME, |kernels| kernels.name)
}

/// Run an installed hardware entry, or the portable body inline. Callers read
/// `installed()` once per loop so the choice is loop-invariant.
#[inline(always)]
pub(crate) fn upper_rank_one(
    hardware: Option<UpperRankOne>,
    weight: f64,
    row: &[f64],
    response: f64,
    left_start: usize,
    rhs: &mut [f64],
    gram: &mut [f64],
) {
    match hardware {
        Some(update) => update(weight, row, response, left_start, rhs, gram),
        None => weighted_upper_rank_one(weight, row, response, left_start, rhs, gram),
    }
}

/// Add one weighted observation to partition rows of the upper Gram triangle.
///
/// `row` holds the standardized observation through the full dimension.
/// Partition row `local` is column `left_start + local`: its RHS entry gains
/// `weight * row[left] * response` and its Gram row gains
/// `weight * row[left] * row[right]` for `right >= left`, each by one
/// `mul_add` in canonical column order. `gram` stores one dimension-wide row
/// per partition row.
#[inline(always)]
pub fn weighted_upper_rank_one(
    weight: f64,
    row: &[f64],
    response: f64,
    left_start: usize,
    rhs: &mut [f64],
    gram: &mut [f64],
) {
    let dimension = row.len();
    for (local_left, destination) in rhs.iter_mut().enumerate() {
        let left = left_start + local_left;
        let weighted_left = weight * row[left];
        *destination = weighted_left.mul_add(response, *destination);
        let gram_row = &mut gram[local_left * dimension..(local_left + 1) * dimension];
        for (entry, &right_value) in gram_row[left..].iter_mut().zip(&row[left..]) {
            *entry = weighted_left.mul_add(right_value, *entry);
        }
    }
}

#[inline(always)]
fn normalized_weight(weights: Option<&[f64]>, raw_scale: f64, observation: usize) -> f64 {
    weights.map_or(1.0, |values| values[observation] / raw_scale)
}

/// First group-mean pass over `members` of a row-major `data` matrix with
/// `scales.len()` columns and `rows` rows: raise each column scale to the
/// largest absolute member value and Neumaier-sum the normalized weights
/// (`weights[row] / raw_scale`, or one when unweighted) in member order.
/// Returns the weight sum and correction, or `None` at the first member
/// outside the row axis.
#[inline(always)]
pub fn group_scales(
    weights: Option<&[f64]>,
    raw_scale: f64,
    data: &[f64],
    rows: usize,
    members: &[usize],
    scales: &mut [f64],
) -> Option<(f64, f64)> {
    let columns = scales.len();
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= rows {
            return None;
        }
        neumaier_add(
            normalized_weight(weights, raw_scale, observation),
            &mut weight_sum,
            &mut weight_correction,
        );
        let row = &data[observation * columns..(observation + 1) * columns];
        for (scale, value) in scales.iter_mut().zip(row) {
            *scale = scale.max(value.abs());
        }
    }
    Some((weight_sum, weight_correction))
}

/// Second group-mean pass: Neumaier-sum `weight * (value / scale)` for every
/// column with a nonzero scale, each column in member order. Members must
/// already be validated by [`group_scales`].
#[inline(always)]
pub fn scaled_group_sums(
    weights: Option<&[f64]>,
    raw_scale: f64,
    data: &[f64],
    members: &[usize],
    scales: &[f64],
    sums: &mut [f64],
    corrections: &mut [f64],
) {
    let columns = scales.len();
    for &observation in members {
        let weight = normalized_weight(weights, raw_scale, observation);
        let row = &data[observation * columns..(observation + 1) * columns];
        for (((value, &scale), sum), correction) in row
            .iter()
            .zip(scales)
            .zip(sums.iter_mut())
            .zip(corrections.iter_mut())
        {
            if scale != 0.0 {
                neumaier_add(weight * (value / scale), sum, correction);
            }
        }
    }
}
