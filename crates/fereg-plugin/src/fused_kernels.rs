//! Hardware-FMA instantiations of fereg-core's fused kernels.
//!
//! Each entry compiles the same core body with FMA enabled. The table is
//! installed only after runtime detection, so processors without FMA keep the
//! portable path. `mul_add` is exactly rounded on both paths, so results are
//! bit-identical; only the instruction selection changes.

/// Install the hardware table for this process when the processor supports it.
pub(crate) fn install() {
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("fma") {
        fereg_core::kernels::install(x86_64::HARDWARE);
    }
}

#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use fereg_core::kernels::{weighted_upper_rank_one, FusedKernels};

    pub(super) const HARDWARE: FusedKernels = FusedKernels {
        name: "x86_64-fma",
        upper_rank_one,
    };

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
        // SAFETY: `HARDWARE` is installed only after runtime FMA detection.
        unsafe { upper_rank_one_fma(weight, row, response, left_start, rhs, gram) }
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
            if !std::is_x86_feature_detected!("fma") {
                return;
            }
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
                        (HARDWARE.upper_rank_one)(
                            weight,
                            &row,
                            response,
                            left_start,
                            &mut hardware_rhs,
                            &mut hardware_gram,
                        );
                    }
                    let bits =
                        |values: &[f64]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
                    assert_eq!(bits(&portable_rhs), bits(&hardware_rhs));
                    assert_eq!(bits(&portable_gram), bits(&hardware_gram));
                }
            }
        }
    }
}
