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

/// One observation's upper-Gram tile: weighted left values, right values,
/// their global column starts, full dimension and dimension-strided Gram rows.
pub type UpperTile = fn(&[f64], &[f64], usize, usize, usize, &mut [f64]);

/// Column scales and compensated weight sum over one group's members:
/// `(weights, raw_scale, data, rows, members, scales)`; see [`group_scales`].
pub type GroupScales =
    fn(Option<&[f64]>, f64, &[f64], usize, &[usize], &mut [f64]) -> Option<(f64, f64)>;

/// Scaled compensated column sums over one group's members:
/// `(weights, raw_scale, data, members, scales, sums, corrections)`; see
/// [`scaled_group_sums`].
pub type ScaledGroupSums =
    fn(Option<&[f64]>, f64, &[f64], &[usize], &[f64], &mut [f64], &mut [f64]);

/// One-pass compensated weighted column sums, column maxima and weight sum
/// over one group's members:
/// `(weights, raw_scale, data, rows, members, sums, corrections, maxima)`;
/// see [`group_sums`].
pub type GroupSums = fn(
    Option<&[f64]>,
    f64,
    &[f64],
    usize,
    &[usize],
    &mut [f64],
    &mut [f64],
    &mut [f64],
) -> Option<(f64, f64)>;

/// Hardware-compiled instantiations of the kernels below.
#[derive(Clone, Copy, Debug)]
pub struct FusedKernels {
    /// Doctor label for the arithmetic path these entries compile to.
    pub name: &'static str,
    pub upper_rank_one: UpperRankOne,
    pub upper_tile: UpperTile,
    pub group_scales: GroupScales,
    pub scaled_group_sums: ScaledGroupSums,
    pub group_sums: GroupSums,
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

/// Update an already standardized upper-Gram tile without changing row order.
/// Left values already include the observation weight. Only entries on or
/// above the diagonal are touched; the caller retains all input validation.
/// `gram` contains at least `weighted_left.len()` dimension-strided rows.
/// Every left column must lie before the right tile's endpoint, and that
/// endpoint must not exceed `dimension`.
#[inline(always)]
pub fn weighted_upper_tile(
    weighted_left: &[f64],
    right: &[f64],
    left_start: usize,
    right_start: usize,
    dimension: usize,
    gram: &mut [f64],
) {
    for (offset, &left_value) in weighted_left.iter().enumerate() {
        let first_right = (left_start + offset).max(right_start);
        let gram_row = &mut gram[offset * dimension..(offset + 1) * dimension];
        for (entry, &right_value) in gram_row[first_right..right_start + right.len()]
            .iter_mut()
            .zip(&right[first_right - right_start..])
        {
            *entry = left_value.mul_add(right_value, *entry);
        }
    }
}

#[inline(always)]
pub(crate) fn upper_tile(
    hardware: Option<UpperTile>,
    left: &[f64],
    right: &[f64],
    left_start: usize,
    right_start: usize,
    dimension: usize,
    gram: &mut [f64],
) {
    match hardware {
        Some(update) => update(left, right, left_start, right_start, dimension, gram),
        None => weighted_upper_tile(left, right, left_start, right_start, dimension, gram),
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

/// One-pass group-mean sums over `members` of a row-major `data` matrix with
/// `sums.len()` columns and `rows` rows. Every column is shifted by the first
/// member's value: Neumaier-sum `weight * (value - shift)` for every column
/// and the normalized weights (`weights[row] / raw_scale`, or one when
/// unweighted) in member order, and raise each column maximum to the largest
/// absolute member value. The group mean is `shift + sum / weight_sum`, which
/// is exactly the shared value when all members are equal. The caller falls
/// back to the scaled two-pass kernels when a sum is not finite or a column is
/// so small that its products may underflow. Returns `None` for an empty group
/// or at the first member outside the row axis.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn group_sums(
    weights: Option<&[f64]>,
    raw_scale: f64,
    data: &[f64],
    rows: usize,
    members: &[usize],
    sums: &mut [f64],
    corrections: &mut [f64],
    maxima: &mut [f64],
) -> Option<(f64, f64)> {
    let columns = sums.len();
    let first = *members.first()?;
    if first >= rows {
        return None;
    }
    let shifts = &data[first * columns..(first + 1) * columns];
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= rows {
            return None;
        }
        let weight = normalized_weight(weights, raw_scale, observation);
        neumaier_add(weight, &mut weight_sum, &mut weight_correction);
        let row = &data[observation * columns..(observation + 1) * columns];
        for ((((value, shift), sum), correction), maximum) in row
            .iter()
            .zip(shifts)
            .zip(sums.iter_mut())
            .zip(corrections.iter_mut())
            .zip(maxima.iter_mut())
        {
            *maximum = maximum.max(value.abs());
            neumaier_add(weight * (value - shift), sum, correction);
        }
    }
    Some((weight_sum, weight_correction))
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    // Each row is [a, b, c, R53(a*b+c), R53(R53(a*b)+c), R53(R64(a*b+c))]
    // in IEEE binary64 bits. R53/R64 use round-to-nearest, ties-to-even;
    // R64 has a 64-bit significand and an extended exponent range.
    // Independently derived and checked without host floating point by
    // scripts/check_fma_rounding_vectors.py (Fraction + integer rounding).
    const EXACT_FMA_VECTORS: [[u64; 6]; 6] = [
        // (1+2^-52)*1.5 - 2^-105 lies just below an odd/even midpoint.
        // R64 loses the tiny subtraction, so the second rounding goes up.
        [
            0x3ff0_0000_0000_0001,
            0x3ff8_0000_0000_0000,
            0xb960_0000_0000_0000,
            0x3ff8_0000_0000_0001,
            0x3ff8_0000_0000_0002,
            0x3ff8_0000_0000_0002,
        ],
        // (1+3*2^-52)*1.5 + 2^-105 lies just above an even/odd midpoint.
        [
            0x3ff0_0000_0000_0003,
            0x3ff8_0000_0000_0000,
            0x3960_0000_0000_0000,
            0x3ff8_0000_0000_0005,
            0x3ff8_0000_0000_0004,
            0x3ff8_0000_0000_0004,
        ],
        // Negative counterparts exercise both rounding directions by sign.
        [
            0xbff0_0000_0000_0001,
            0x3ff8_0000_0000_0000,
            0x3960_0000_0000_0000,
            0xbff8_0000_0000_0001,
            0xbff8_0000_0000_0002,
            0xbff8_0000_0000_0002,
        ],
        [
            0xbff0_0000_0000_0003,
            0x3ff8_0000_0000_0000,
            0xb960_0000_0000_0000,
            0xbff8_0000_0000_0005,
            0xbff8_0000_0000_0004,
            0xbff8_0000_0000_0004,
        ],
        // Exact cancellation: (1+2^-27)*(1-2^-27)-1 = -2^-54.
        [
            0x3ff0_0000_0200_0000,
            0x3fef_ffff_fc00_0000,
            0xbff0_0000_0000_0000,
            0xbc90_0000_0000_0000,
            0x0000_0000_0000_0000,
            0xbc90_0000_0000_0000,
        ],
        // Exact cancellation all the way down to the product's low bit.
        // (1+2^-52)*(1-2^-52)-1 = -2^-104.
        [
            0x3ff0_0000_0000_0001,
            0x3fef_ffff_ffff_fffe,
            0xbff0_0000_0000_0000,
            0xb970_0000_0000_0000,
            0x0000_0000_0000_0000,
            0xb970_0000_0000_0000,
        ],
    ];

    #[test]
    fn platform_mul_add_is_exactly_rounded() {
        for (index, [a, b, c, expected, nonfused, _double]) in
            EXACT_FMA_VECTORS.into_iter().enumerate()
        {
            let a = black_box(f64::from_bits(a));
            let b = black_box(f64::from_bits(b));
            let c = black_box(f64::from_bits(c));
            assert_eq!(
                a.mul_add(b, c).to_bits(),
                expected,
                "platform mul_add failed exact-rounding vector {index}"
            );
            // Prevent contraction of the deliberately non-fused comparison.
            let product = black_box(a * b);
            assert_eq!(
                (product + c).to_bits(),
                nonfused,
                "non-fused vector {index}"
            );
            assert_ne!(expected, nonfused);
        }
    }
}
