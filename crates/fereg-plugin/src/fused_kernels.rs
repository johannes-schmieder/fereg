//! Hardware-FMA instantiations of fereg-core's fused kernels.
//!
//! Each entry compiles the same core body with FMA (which implies AVX)
//! enabled. The table exists only after runtime detection, so processors
//! without both keep the portable path inline. The bodies use only exactly
//! rounded operations in a fixed per-element order, so results match
//! wherever the platform `fma` is exactly rounded too; only instruction
//! selection and vector width change.

/// Install the hardware table for this process when the processor supports it.
pub(crate) fn install() {
    #[cfg(target_arch = "x86_64")]
    if let Some(kernels) = x86_64::hardware() {
        fereg_core::kernels::install(kernels);
    }
}

#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use fereg_core::kernels::{
        group_scales as portable_group_scales, scaled_group_sums as portable_scaled_group_sums,
        weighted_upper_rank_one, FusedKernels,
    };

    const HARDWARE: FusedKernels = FusedKernels {
        name: "x86_64-fma",
        upper_rank_one,
        group_scales,
        scaled_group_sums,
    };

    /// The hardware table, only when the processor and OS provide FMA and AVX.
    /// Its safe entries rely on this check, so it is the only way to reach them.
    pub(super) fn hardware() -> Option<FusedKernels> {
        (std::is_x86_feature_detected!("fma") && std::is_x86_feature_detected!("avx"))
            .then_some(HARDWARE)
    }

    #[target_feature(enable = "fma")]
    unsafe fn upper_rank_one_fma(
        weight: f64,
        row: &[f64],
        response: f64,
        left_start: usize,
        rhs: &mut [f64],
        gram: &mut [f64],
    ) {
        weighted_upper_rank_one(weight, row, response, left_start, rhs, gram);
    }

    fn upper_rank_one(
        weight: f64,
        row: &[f64],
        response: f64,
        left_start: usize,
        rhs: &mut [f64],
        gram: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { upper_rank_one_fma(weight, row, response, left_start, rhs, gram) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn group_scales_fma(
        weights: Option<&[f64]>,
        raw_scale: f64,
        data: &[f64],
        rows: usize,
        members: &[usize],
        scales: &mut [f64],
    ) -> Option<(f64, f64)> {
        portable_group_scales(weights, raw_scale, data, rows, members, scales)
    }

    fn group_scales(
        weights: Option<&[f64]>,
        raw_scale: f64,
        data: &[f64],
        rows: usize,
        members: &[usize],
        scales: &mut [f64],
    ) -> Option<(f64, f64)> {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { group_scales_fma(weights, raw_scale, data, rows, members, scales) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn scaled_group_sums_fma(
        weights: Option<&[f64]>,
        raw_scale: f64,
        data: &[f64],
        members: &[usize],
        scales: &[f64],
        sums: &mut [f64],
        corrections: &mut [f64],
    ) {
        portable_scaled_group_sums(weights, raw_scale, data, members, scales, sums, corrections);
    }

    fn scaled_group_sums(
        weights: Option<&[f64]>,
        raw_scale: f64,
        data: &[f64],
        members: &[usize],
        scales: &[f64],
        sums: &mut [f64],
        corrections: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe {
            scaled_group_sums_fma(weights, raw_scale, data, members, scales, sums, corrections)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn values(seed: u64, count: usize) -> Vec<f64> {
            let mut state = seed;
            (0..count)
                .map(|_| {
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    // Signed values with varied exponents exercise rounding.
                    let unit = (state >> 11) as f64 / (1_u64 << 53) as f64 - 0.5;
                    unit * f64::powi(2.0, (state % 41) as i32 - 20)
                })
                .collect()
        }

        #[test]
        fn hardware_upper_rank_one_matches_portable_bits() {
            let Some(hardware) = hardware() else {
                eprintln!("skipped: this processor has no FMA/AVX hardware path");
                return;
            };
            for dimension in [1, 2, 7, 8, 21, 64] {
                for left_start in [0, dimension / 2, dimension - 1] {
                    let rows = dimension - left_start;
                    let mut portable_rhs = values(3, rows);
                    let mut portable_gram = values(5, rows * dimension);
                    let mut hardware_rhs = portable_rhs.clone();
                    let mut hardware_gram = portable_gram.clone();
                    for observation in 0..50 {
                        let row = values(7 + observation, dimension);
                        let weight = values(11 + observation, 1)[0].abs();
                        let response = row[0];
                        weighted_upper_rank_one(
                            weight,
                            &row,
                            response,
                            left_start,
                            &mut portable_rhs,
                            &mut portable_gram,
                        );
                        (hardware.upper_rank_one)(
                            weight,
                            &row,
                            response,
                            left_start,
                            &mut hardware_rhs,
                            &mut hardware_gram,
                        );
                    }
                    assert_eq!(bits(&portable_rhs), bits(&hardware_rhs));
                    assert_eq!(bits(&portable_gram), bits(&hardware_gram));
                }
            }
        }

        fn bits(values: &[f64]) -> Vec<u64> {
            values.iter().map(|value| value.to_bits()).collect()
        }

        #[test]
        fn hardware_group_passes_match_portable_bits() {
            let Some(hardware) = hardware() else {
                eprintln!("skipped: this processor has no FMA/AVX hardware path");
                return;
            };
            let rows = 97;
            for columns in [1, 3, 4, 12, 17] {
                let mut data = values(13, rows * columns);
                // Zero columns and signed zeros exercise the skipped-scale branch.
                for row in 0..rows {
                    data[row * columns] = if row % 2 == 0 { 0.0 } else { -0.0 };
                }
                let raw: Vec<f64> = values(17, rows).iter().map(|v| v.abs() + 0.5).collect();
                let members: Vec<usize> = (0..rows).rev().step_by(3).collect();
                for weights in [None, Some(raw.as_slice())] {
                    let mut portable_scales = vec![0.0; columns];
                    let mut hardware_scales = vec![0.0; columns];
                    let portable_weight = portable_group_scales(
                        weights,
                        3.0,
                        &data,
                        rows,
                        &members,
                        &mut portable_scales,
                    )
                    .expect("valid members");
                    let hardware_weight = (hardware.group_scales)(
                        weights,
                        3.0,
                        &data,
                        rows,
                        &members,
                        &mut hardware_scales,
                    )
                    .expect("valid members");
                    assert_eq!(
                        [portable_weight.0.to_bits(), portable_weight.1.to_bits()],
                        [hardware_weight.0.to_bits(), hardware_weight.1.to_bits()]
                    );
                    assert_eq!(bits(&portable_scales), bits(&hardware_scales));
                    let mut portable_sums = vec![0.0; columns];
                    let mut portable_corrections = vec![0.0; columns];
                    let mut hardware_sums = vec![0.0; columns];
                    let mut hardware_corrections = vec![0.0; columns];
                    portable_scaled_group_sums(
                        weights,
                        3.0,
                        &data,
                        &members,
                        &portable_scales,
                        &mut portable_sums,
                        &mut portable_corrections,
                    );
                    (hardware.scaled_group_sums)(
                        weights,
                        3.0,
                        &data,
                        &members,
                        &hardware_scales,
                        &mut hardware_sums,
                        &mut hardware_corrections,
                    );
                    assert_eq!(bits(&portable_sums), bits(&hardware_sums));
                    assert_eq!(bits(&portable_corrections), bits(&hardware_corrections));
                }
            }
            let mut scales = [0.0];
            assert!((hardware.group_scales)(None, 1.0, &[1.0], 1, &[1], &mut scales).is_none());
        }
    }
}
