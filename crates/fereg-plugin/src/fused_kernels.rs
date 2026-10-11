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
        group_scales as portable_group_scales, group_sums as portable_group_sums,
        scaled_group_sums as portable_scaled_group_sums, weighted_upper_rank_one,
        weighted_upper_tile, FusedKernels,
    };

    const HARDWARE: FusedKernels = FusedKernels {
        name: "x86_64-fma",
        ordered_dot,
        normalized_product,
        scaled_add,
        affine_direction,
        joint_step,
        subtract_projection,
        weighted_square_sum,
        basis_projection,
        weighted_upper_gram,
        column_products,
        pcg_initialize,
        pcg_step,
        pcg_energy,
        pcg_step_energy,
        pcg_direction,

        upper_rank_one,
        upper_tile,
        group_scales,
        scaled_group_sums,
        group_sums,
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
    unsafe fn upper_tile_fma(
        left: &[f64],
        right: &[f64],
        left_start: usize,
        right_start: usize,
        dimension: usize,
        gram: &mut [f64],
    ) {
        weighted_upper_tile(left, right, left_start, right_start, dimension, gram);
    }

    fn upper_tile(
        left: &[f64],
        right: &[f64],
        left_start: usize,
        right_start: usize,
        dimension: usize,
        gram: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { upper_tile_fma(left, right, left_start, right_start, dimension, gram) }
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

    #[target_feature(enable = "fma")]
    #[allow(clippy::too_many_arguments)]
    unsafe fn group_sums_fma(
        weights: Option<&[f64]>,
        raw_scale: f64,
        data: &[f64],
        rows: usize,
        members: &[usize],
        sums: &mut [f64],
        corrections: &mut [f64],
        maxima: &mut [f64],
    ) -> Option<(f64, f64)> {
        portable_group_sums(
            weights,
            raw_scale,
            data,
            rows,
            members,
            sums,
            corrections,
            maxima,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn group_sums(
        weights: Option<&[f64]>,
        raw_scale: f64,
        data: &[f64],
        rows: usize,
        members: &[usize],
        sums: &mut [f64],
        corrections: &mut [f64],
        maxima: &mut [f64],
    ) -> Option<(f64, f64)> {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe {
            group_sums_fma(
                weights,
                raw_scale,
                data,
                rows,
                members,
                sums,
                corrections,
                maxima,
            )
        }
    }

    #[target_feature(enable = "fma")]
    unsafe fn scaled_add_fma(source: &[f64], scale: f64, destination: &mut [f64]) {
        fereg_core::kernels::scaled_add(source, scale, destination)
    }

    fn scaled_add(source: &[f64], scale: f64, destination: &mut [f64]) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { scaled_add_fma(source, scale, destination) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn affine_direction_fma(scale: f64, source: &[f64], destination: &mut [f64]) {
        fereg_core::kernels::affine_direction(scale, source, destination)
    }

    fn affine_direction(scale: f64, source: &[f64], destination: &mut [f64]) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { affine_direction_fma(scale, source, destination) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn joint_step_fma(
        step: f64,
        direction: &[f64],
        product: &[f64],
        solution: &mut [f64],
        residual: &mut [f64],
    ) {
        fereg_core::kernels::joint_step(step, direction, product, solution, residual)
    }

    fn joint_step(
        step: f64,
        direction: &[f64],
        product: &[f64],
        solution: &mut [f64],
        residual: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { joint_step_fma(step, direction, product, solution, residual) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn subtract_projection_fma(values: &mut [f64], direction: &[f64]) {
        fereg_core::kernels::subtract_projection(values, direction)
    }

    fn subtract_projection(values: &mut [f64], direction: &[f64]) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { subtract_projection_fma(values, direction) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn weighted_square_sum_fma(response: &[f64], weights: &[f64], reference: f64) -> f64 {
        fereg_core::kernels::weighted_square_sum(response, weights, reference)
    }

    fn weighted_square_sum(response: &[f64], weights: &[f64], reference: f64) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { weighted_square_sum_fma(response, weights, reference) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn basis_projection_fma(
        observations: &[usize],
        bases: &[Vec<f64>],
        weights: &[f64],
        residuals: &[f64],
        stride: usize,
        rhs: usize,
        denominator: f64,
    ) -> f64 {
        fereg_core::kernels::basis_projection(
            observations,
            bases,
            weights,
            residuals,
            stride,
            rhs,
            denominator,
        )
    }

    fn basis_projection(
        observations: &[usize],
        bases: &[Vec<f64>],
        weights: &[f64],
        residuals: &[f64],
        stride: usize,
        rhs: usize,
        denominator: f64,
    ) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe {
            basis_projection_fma(
                observations,
                bases,
                weights,
                residuals,
                stride,
                rhs,
                denominator,
            )
        }
    }

    #[target_feature(enable = "fma")]
    unsafe fn weighted_upper_gram_fma(weight: f64, row: &[f64], gram: &mut [f64]) {
        fereg_core::kernels::weighted_upper_gram(weight, row, gram)
    }

    fn weighted_upper_gram(weight: f64, row: &[f64], gram: &mut [f64]) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { weighted_upper_gram_fma(weight, row, gram) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn column_products_fma(left: &[f64], right: &[f64], stride: usize, sums: &mut [f64]) {
        fereg_core::kernels::column_products(left, right, stride, sums)
    }

    fn column_products(left: &[f64], right: &[f64], stride: usize, sums: &mut [f64]) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { column_products_fma(left, right, stride, sums) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn pcg_initialize_fma(
        weights: &[f64],
        scales: Option<&[f64]>,
        stride: usize,
        residual: &[f64],
        direction: &mut [f64],
        energy: &mut [f64],
    ) {
        fereg_core::kernels::pcg_initialize(weights, scales, stride, residual, direction, energy)
    }

    fn pcg_initialize(
        weights: &[f64],
        scales: Option<&[f64]>,
        stride: usize,
        residual: &[f64],
        direction: &mut [f64],
        energy: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { pcg_initialize_fma(weights, scales, stride, residual, direction, energy) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn pcg_step_fma(
        stride: usize,
        steps: &[f64],
        direction: &[f64],
        product: &[f64],
        solution: &mut [f64],
        residual: &mut [f64],
    ) {
        fereg_core::kernels::pcg_step(stride, steps, direction, product, solution, residual)
    }

    fn pcg_step(
        stride: usize,
        steps: &[f64],
        direction: &[f64],
        product: &[f64],
        solution: &mut [f64],
        residual: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { pcg_step_fma(stride, steps, direction, product, solution, residual) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn pcg_energy_fma(
        weights: &[f64],
        scales: Option<&[f64]>,
        stride: usize,
        residual: &[f64],
        energy: &mut [f64],
    ) {
        fereg_core::kernels::pcg_energy(weights, scales, stride, residual, energy)
    }

    fn pcg_energy(
        weights: &[f64],
        scales: Option<&[f64]>,
        stride: usize,
        residual: &[f64],
        energy: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { pcg_energy_fma(weights, scales, stride, residual, energy) }
    }

    #[target_feature(enable = "fma")]
    #[allow(clippy::too_many_arguments)]
    unsafe fn pcg_step_energy_fma(
        weights: &[f64],
        scales: Option<&[f64]>,
        stride: usize,
        steps: &[f64],
        direction: &[f64],
        product: &[f64],
        solution: &mut [f64],
        residual: &mut [f64],
        energy: &mut [f64],
    ) {
        fereg_core::kernels::pcg_step_energy(
            weights, scales, stride, steps, direction, product, solution, residual, energy,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn pcg_step_energy(
        weights: &[f64],
        scales: Option<&[f64]>,
        stride: usize,
        steps: &[f64],
        direction: &[f64],
        product: &[f64],
        solution: &mut [f64],
        residual: &mut [f64],
        energy: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe {
            pcg_step_energy_fma(
                weights, scales, stride, steps, direction, product, solution, residual, energy,
            )
        }
    }

    #[target_feature(enable = "fma")]
    unsafe fn pcg_direction_fma(
        weights: &[f64],
        scales: Option<&[f64]>,
        stride: usize,
        ratios: &[f64],
        active: &[f64],
        residual: &[f64],
        direction: &mut [f64],
    ) {
        fereg_core::kernels::pcg_direction(
            weights, scales, stride, ratios, active, residual, direction,
        )
    }

    fn pcg_direction(
        weights: &[f64],
        scales: Option<&[f64]>,
        stride: usize,
        ratios: &[f64],
        active: &[f64],
        residual: &[f64],
        direction: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { pcg_direction_fma(weights, scales, stride, ratios, active, residual, direction) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn ordered_dot_fma(left: &[f64], right: &[f64]) -> f64 {
        fereg_core::kernels::ordered_dot(left, right)
    }

    fn ordered_dot(left: &[f64], right: &[f64]) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { ordered_dot_fma(left, right) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn normalized_product_fma(left: &[f64], right: &[f64]) -> f64 {
        fereg_core::kernels::normalized_product(left, right)
    }

    fn normalized_product(left: &[f64], right: &[f64]) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { normalized_product_fma(left, right) }
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
        fn hardware_ordered_projection_and_products_match_portable_bits() {
            let Some(h) = hardware() else {
                eprintln!("skipped: this processor has no FMA/AVX hardware path");
                return;
            };
            use fereg_core::kernels as p;
            for length in [0, 1, 7, 8, 9, 17, 65, 129] {
                let mut left = values(31, length);
                let right = values(41, length);
                if length >= 3 {
                    left[..3].copy_from_slice(&[0.0, -0.0, f64::from_bits(1)]);
                }
                assert_eq!(
                    (h.ordered_dot)(&left, &right).to_bits(),
                    p::ordered_dot(&left, &right).to_bits()
                );
                assert_eq!(
                    (h.normalized_product)(&left, &right).to_bits(),
                    p::normalized_product(&left, &right).to_bits()
                );
                let weights: Vec<_> = right.iter().map(|v| v.abs() + 0.5).collect();
                assert_eq!(
                    (h.weighted_square_sum)(&left, &weights, 3.0).to_bits(),
                    p::weighted_square_sum(&left, &weights, 3.0).to_bits()
                );
                let mut actual = left.clone();
                let mut expected = left.clone();
                (h.subtract_projection)(&mut actual, &right);
                p::subtract_projection(&mut expected, &right);
                assert_eq!(bits(&actual), bits(&expected));
                for scale in [-0.0, -0.3, 0.7] {
                    (h.scaled_add)(&right, scale, &mut actual);
                    p::scaled_add(&right, scale, &mut expected);
                    assert_eq!(bits(&actual), bits(&expected));
                    (h.affine_direction)(scale, &right, &mut actual);
                    p::affine_direction(scale, &right, &mut expected);
                    assert_eq!(bits(&actual), bits(&expected));
                    let mut r_actual = left.clone();
                    let mut r_expected = left.clone();
                    (h.joint_step)(scale, &left, &right, &mut actual, &mut r_actual);
                    p::joint_step(scale, &left, &right, &mut expected, &mut r_expected);
                    assert_eq!(bits(&actual), bits(&expected));
                    assert_eq!(bits(&r_actual), bits(&r_expected));
                }
            }
            for width in [1, 3, 8, 17] {
                let rows = 19;
                let residuals = values(47, rows * width);
                let weights: Vec<_> = values(53, rows).iter().map(|v| v.abs() + 0.5).collect();
                let members: Vec<_> = (0..rows).rev().step_by(3).collect();
                let bases = vec![values(59, members.len()), values(61, members.len())];
                for rhs in 0..width {
                    for denominator in [0.0, 1.0, 3.0] {
                        assert_eq!(
                            (h.basis_projection)(
                                &members,
                                &bases,
                                &weights,
                                &residuals,
                                width,
                                rhs,
                                denominator
                            )
                            .to_bits(),
                            p::basis_projection(
                                &members,
                                &bases,
                                &weights,
                                &residuals,
                                width,
                                rhs,
                                denominator
                            )
                            .to_bits()
                        );
                    }
                }
                let mut actual = values(67, width * width);
                let mut expected = actual.clone();
                for (row, &weight) in residuals.chunks(width).zip(&weights) {
                    (h.weighted_upper_gram)(weight, row, &mut actual);
                    p::weighted_upper_gram(weight, row, &mut expected);
                }
                assert_eq!(bits(&actual), bits(&expected));
            }
        }

        #[test]
        fn hardware_pcg_preserves_order_padding_and_inactive_rhs() {
            let Some(h) = hardware() else {
                eprintln!("skipped: this processor has no FMA/AVX hardware path");
                return;
            };
            use fereg_core::kernels as p;
            for levels in [1, 7, 19] {
                for rhs in [1, 3, 8, 17] {
                    let stride = rhs + 3;
                    let weights: Vec<_> =
                        values(71, levels).iter().map(|v| v.abs() + 0.5).collect();
                    let scales: Vec<_> = values(73, levels).iter().map(|v| v.abs() + 0.5).collect();
                    let residual = values(79, levels * stride);
                    let product = values(83, levels * stride);
                    let steps = values(89, rhs);
                    let active: Vec<_> = (0..rhs)
                        .map(|j| {
                            if j % 3 == 0 {
                                -1.0
                            } else if j % 3 == 1 {
                                1.0
                            } else {
                                0.0
                            }
                        })
                        .collect();
                    for scales in [None, Some(scales.as_slice())] {
                        let mut direction = values(97, levels * stride);
                        let mut expected_direction = direction.clone();
                        let initial_direction = direction.clone();
                        let mut energy = vec![0.0; rhs];
                        let mut expected_energy = energy.clone();
                        (h.pcg_initialize)(
                            &weights,
                            scales,
                            stride,
                            &residual,
                            &mut direction,
                            &mut energy,
                        );
                        p::pcg_initialize(
                            &weights,
                            scales,
                            stride,
                            &residual,
                            &mut expected_direction,
                            &mut expected_energy,
                        );
                        assert_eq!(bits(&direction), bits(&expected_direction));
                        assert_eq!(bits(&energy), bits(&expected_energy));
                        (h.column_products)(&direction, &product, stride, &mut energy);
                        p::column_products(&direction, &product, stride, &mut expected_energy);
                        assert_eq!(bits(&energy), bits(&expected_energy));
                        let mut solution = values(101, levels * stride);
                        let mut expected_solution = solution.clone();
                        let mut residual = residual.clone();
                        let mut expected_residual = residual.clone();
                        let initial_solution = solution.clone();
                        let initial_residual = residual.clone();
                        (h.pcg_step_energy)(
                            &weights,
                            scales,
                            stride,
                            &steps,
                            &direction,
                            &product,
                            &mut solution,
                            &mut residual,
                            &mut energy,
                        );
                        p::pcg_step_energy(
                            &weights,
                            scales,
                            stride,
                            &steps,
                            &expected_direction,
                            &product,
                            &mut expected_solution,
                            &mut expected_residual,
                            &mut expected_energy,
                        );
                        assert_eq!(bits(&solution), bits(&expected_solution));
                        assert_eq!(bits(&residual), bits(&expected_residual));
                        assert_eq!(bits(&energy), bits(&expected_energy));
                        (h.pcg_step)(
                            stride,
                            &steps,
                            &direction,
                            &product,
                            &mut solution,
                            &mut residual,
                        );
                        p::pcg_step(
                            stride,
                            &steps,
                            &expected_direction,
                            &product,
                            &mut expected_solution,
                            &mut expected_residual,
                        );
                        (h.pcg_energy)(&weights, scales, stride, &residual, &mut energy);
                        p::pcg_energy(
                            &weights,
                            scales,
                            stride,
                            &expected_residual,
                            &mut expected_energy,
                        );
                        assert_eq!(bits(&solution), bits(&expected_solution));
                        assert_eq!(bits(&residual), bits(&expected_residual));
                        assert_eq!(bits(&energy), bits(&expected_energy));
                        let before_direction = direction.clone();
                        (h.pcg_direction)(
                            &weights,
                            scales,
                            stride,
                            &steps,
                            &active,
                            &residual,
                            &mut direction,
                        );
                        p::pcg_direction(
                            &weights,
                            scales,
                            stride,
                            &steps,
                            &active,
                            &expected_residual,
                            &mut expected_direction,
                        );
                        assert_eq!(bits(&direction), bits(&expected_direction));
                        for level in 0..levels {
                            for column in 0..stride {
                                let index = level * stride + column;
                                if column >= rhs {
                                    assert_eq!(
                                        direction[index].to_bits(),
                                        initial_direction[index].to_bits()
                                    );
                                    assert_eq!(
                                        solution[index].to_bits(),
                                        initial_solution[index].to_bits()
                                    );
                                    assert_eq!(
                                        residual[index].to_bits(),
                                        initial_residual[index].to_bits()
                                    );
                                } else if active[column] == 0.0 {
                                    assert_eq!(
                                        direction[index].to_bits(),
                                        before_direction[index].to_bits()
                                    );
                                }
                            }
                        }
                    }
                }
            }
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
        fn hardware_upper_tiles_match_scalar_order_and_preserve_other_entries() {
            let Some(hardware) = hardware() else {
                eprintln!("skipped: this processor has no FMA/AVX hardware path");
                return;
            };
            for dimension in [65, 129, 257] {
                for left_start in [0, 1, 63, dimension - 1] {
                    let count = (dimension - left_start).min(64);
                    let mut expected = values(3, count * dimension);
                    let mut actual = expected.clone();
                    for observation in 0..17 {
                        let row = values(7 + observation, dimension);
                        let weight = values(11 + observation, 1)[0].abs();
                        let left: Vec<_> = row[left_start..left_start + count]
                            .iter()
                            .map(|value| weight * value)
                            .collect();
                        for (offset, &value) in left.iter().enumerate() {
                            for right in left_start + offset..dimension {
                                let entry = &mut expected[offset * dimension + right];
                                *entry = value.mul_add(row[right], *entry);
                            }
                        }
                        for right_start in (left_start..dimension).step_by(64) {
                            (hardware.upper_tile)(
                                &left,
                                &row[right_start..(right_start + 64).min(dimension)],
                                left_start,
                                right_start,
                                dimension,
                                &mut actual,
                            );
                        }
                    }
                    assert_eq!(bits(&actual), bits(&expected));
                }
            }
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
                    let mut one_pass = [vec![0.0; columns], vec![0.0; columns], vec![0.0; columns]];
                    let mut hardware_pass =
                        [vec![0.0; columns], vec![0.0; columns], vec![0.0; columns]];
                    let [sums, corrections, maxima] = &mut one_pass;
                    let portable_weight = portable_group_sums(
                        weights,
                        3.0,
                        &data,
                        rows,
                        &members,
                        sums,
                        corrections,
                        maxima,
                    )
                    .expect("valid members");
                    let [sums, corrections, maxima] = &mut hardware_pass;
                    let hardware_weight = (hardware.group_sums)(
                        weights,
                        3.0,
                        &data,
                        rows,
                        &members,
                        sums,
                        corrections,
                        maxima,
                    )
                    .expect("valid members");
                    assert_eq!(
                        [portable_weight.0.to_bits(), portable_weight.1.to_bits()],
                        [hardware_weight.0.to_bits(), hardware_weight.1.to_bits()]
                    );
                    for (portable, hardware) in one_pass.iter().zip(&hardware_pass) {
                        assert_eq!(bits(portable), bits(hardware));
                    }
                }
            }
            let mut scales = [0.0];
            assert!((hardware.group_scales)(None, 1.0, &[1.0], 1, &[1], &mut scales).is_none());
        }
    }
}
