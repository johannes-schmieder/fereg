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
        multiply_add,
        subtract_products,
        basis_projection_tile,
        selected_residual,
        normalized_selected_residual,
        normalized_selected_scores,
        incidence_schur_row,
        joint_step_energy,
        ordered_dot,
        normalized_product,
        normalized_products,
        gram_panel,
        scaled_add,
        coefficient_add,
        grouped_cross,
        affine_direction,
        joint_step,
        subtract_projection,
        weighted_square_sum,
        weighted_product,
        normalized_add,
        normalized_dot,
        standardized_residual,
        basis_forward,
        basis_transpose,
        basis_project,
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
    unsafe fn incidence_schur_row_fma(
        coefficients: &[f64],
        columns: &[usize],
        direction: &[f64],
        product: &mut [f64],
    ) {
        fereg_core::kernels::incidence_schur_row(coefficients, columns, direction, product)
    }
    fn incidence_schur_row(
        coefficients: &[f64],
        columns: &[usize],
        direction: &[f64],
        product: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { incidence_schur_row_fma(coefficients, columns, direction, product) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn joint_step_energy_fma(
        alpha: f64,
        direction: &[f64],
        product: &[f64],
        values: &mut [f64],
        residual: &mut [f64],
    ) -> f64 {
        fereg_core::kernels::joint_step_energy(alpha, direction, product, values, residual)
    }
    fn joint_step_energy(
        alpha: f64,
        direction: &[f64],
        product: &[f64],
        values: &mut [f64],
        residual: &mut [f64],
    ) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { joint_step_energy_fma(alpha, direction, product, values, residual) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn normalized_selected_residual_fma(
        residual: f64,
        values: &[f64],
        selected: &[usize],
        scales: &[f64],
        coefficients: &[f64],
        root: f64,
    ) -> f64 {
        fereg_core::kernels::normalized_selected_residual(
            residual,
            values,
            selected,
            scales,
            coefficients,
            root,
        )
    }
    fn normalized_selected_residual(
        residual: f64,
        values: &[f64],
        selected: &[usize],
        scales: &[f64],
        coefficients: &[f64],
        root: f64,
    ) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe {
            normalized_selected_residual_fma(residual, values, selected, scales, coefficients, root)
        }
    }

    #[target_feature(enable = "fma")]
    unsafe fn normalized_selected_scores_fma(
        values: &[f64],
        selected: &[usize],
        scales: &[f64],
        root: f64,
        observation: f64,
        scores: &mut [f64],
    ) {
        fereg_core::kernels::normalized_selected_scores(
            values,
            selected,
            scales,
            root,
            observation,
            scores,
        )
    }
    fn normalized_selected_scores(
        values: &[f64],
        selected: &[usize],
        scales: &[f64],
        root: f64,
        observation: f64,
        scores: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe {
            normalized_selected_scores_fma(values, selected, scales, root, observation, scores)
        }
    }

    #[target_feature(enable = "fma")]
    unsafe fn multiply_add_fma(left: f64, right: f64, sum: f64) -> f64 {
        fereg_core::kernels::multiply_add(left, right, sum)
    }
    fn multiply_add(left: f64, right: f64, sum: f64) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { multiply_add_fma(left, right, sum) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn subtract_products_fma(
        left: &[f64],
        left_stride: usize,
        right: &[f64],
        right_stride: usize,
        count: usize,
        value: f64,
    ) -> f64 {
        fereg_core::kernels::subtract_products(left, left_stride, right, right_stride, count, value)
    }
    fn subtract_products(
        left: &[f64],
        left_stride: usize,
        right: &[f64],
        right_stride: usize,
        count: usize,
        value: f64,
    ) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { subtract_products_fma(left, left_stride, right, right_stride, count, value) }
    }

    #[allow(clippy::too_many_arguments)]
    #[target_feature(enable = "fma")]
    unsafe fn basis_projection_tile_fma(
        members: &[usize],
        basis: &[f64],
        stride: usize,
        weights: &[f64],
        residuals: &[f64],
        width: usize,
        scales: &[f64],
        first: usize,
        projected: &mut [f64],
    ) {
        fereg_core::kernels::basis_projection_tile(
            members, basis, stride, weights, residuals, width, scales, first, projected,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn basis_projection_tile(
        members: &[usize],
        basis: &[f64],
        stride: usize,
        weights: &[f64],
        residuals: &[f64],
        width: usize,
        scales: &[f64],
        first: usize,
        projected: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe {
            basis_projection_tile_fma(
                members, basis, stride, weights, residuals, width, scales, first, projected,
            )
        }
    }

    #[target_feature(enable = "fma")]
    unsafe fn selected_residual_fma(
        response: f64,
        values: &[f64],
        selected: &[usize],
        coefficients: &[Option<f64>],
    ) -> f64 {
        fereg_core::kernels::selected_residual(response, values, selected, coefficients)
    }
    fn selected_residual(
        response: f64,
        values: &[f64],
        selected: &[usize],
        coefficients: &[Option<f64>],
    ) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { selected_residual_fma(response, values, selected, coefficients) }
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

    #[target_feature(enable = "fma")]
    unsafe fn normalized_products_fma(
        columns: &[f64],
        n: usize,
        count: usize,
        first: usize,
        rows: usize,
        output: &mut [f64],
    ) {
        fereg_core::kernels::normalized_products(columns, n, count, first, rows, output)
    }

    fn normalized_products(
        columns: &[f64],
        n: usize,
        count: usize,
        first: usize,
        rows: usize,
        output: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { normalized_products_fma(columns, n, count, first, rows, output) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn gram_panel_fma(
        columns: &[f64],
        n: usize,
        count: usize,
        start: usize,
        end: usize,
        lanes: &mut [f64],
    ) {
        fereg_core::kernels::gram_panel(columns, n, count, start, end, lanes)
    }

    fn gram_panel(
        columns: &[f64],
        n: usize,
        count: usize,
        start: usize,
        end: usize,
        lanes: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { gram_panel_fma(columns, n, count, start, end, lanes) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn weighted_product_fma(left: &[f64], roots: &[f64], right: &[f64]) -> f64 {
        fereg_core::kernels::weighted_product(left, roots, right)
    }

    fn weighted_product(left: &[f64], roots: &[f64], right: &[f64]) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { weighted_product_fma(left, roots, right) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn normalized_add_fma(
        coefficient: f64,
        source: &[f64],
        roots: &[f64],
        scale: f64,
        destination: &mut [f64],
    ) {
        fereg_core::kernels::normalized_add(coefficient, source, roots, scale, destination)
    }

    fn normalized_add(
        coefficient: f64,
        source: &[f64],
        roots: &[f64],
        scale: f64,
        destination: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { normalized_add_fma(coefficient, source, roots, scale, destination) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn normalized_dot_fma(
        source: &[f64],
        roots: &[f64],
        scale: f64,
        residual: &[f64],
    ) -> f64 {
        fereg_core::kernels::normalized_dot(source, roots, scale, residual)
    }

    fn normalized_dot(source: &[f64], roots: &[f64], scale: f64, residual: &[f64]) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { normalized_dot_fma(source, roots, scale, residual) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn basis_forward_fma(
        rows: &[usize],
        basis: &[f64],
        coefficients: &[f64],
        output: &mut [f64],
    ) {
        fereg_core::kernels::basis_forward(rows, basis, coefficients, output)
    }

    fn basis_forward(rows: &[usize], basis: &[f64], coefficients: &[f64], output: &mut [f64]) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { basis_forward_fma(rows, basis, coefficients, output) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn basis_transpose_fma(
        rows: &[usize],
        basis: &[f64],
        rank: usize,
        values: &[f64],
        output: Option<&mut [f64]>,
        certify: bool,
    ) -> f64 {
        fereg_core::kernels::basis_transpose(rows, basis, rank, values, output, certify)
    }

    fn basis_transpose(
        rows: &[usize],
        basis: &[f64],
        rank: usize,
        values: &[f64],
        output: Option<&mut [f64]>,
        certify: bool,
    ) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { basis_transpose_fma(rows, basis, rank, values, output, certify) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn basis_project_fma(
        rows: &[usize],
        basis: &[f64],
        values: &mut [f64],
        scratch: &mut [f64],
    ) {
        fereg_core::kernels::basis_project(rows, basis, values, scratch)
    }

    fn basis_project(rows: &[usize], basis: &[f64], values: &mut [f64], scratch: &mut [f64]) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { basis_project_fma(rows, basis, values, scratch) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn standardized_residual_fma(
        response: f64,
        values: &[f64],
        positions: &[usize],
        norms: &[f64],
        coefficients: &[f64],
        root: Option<f64>,
    ) -> f64 {
        fereg_core::kernels::standardized_residual(
            response,
            values,
            positions,
            norms,
            coefficients,
            root,
        )
    }

    fn standardized_residual(
        response: f64,
        values: &[f64],
        positions: &[usize],
        norms: &[f64],
        coefficients: &[f64],
        root: Option<f64>,
    ) -> f64 {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { standardized_residual_fma(response, values, positions, norms, coefficients, root) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn coefficient_add_fma(
        coefficient: f64,
        source: Option<&[f64]>,
        destination: &mut [f64],
    ) {
        fereg_core::kernels::coefficient_add(coefficient, source, destination)
    }

    fn coefficient_add(coefficient: f64, source: Option<&[f64]>, destination: &mut [f64]) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { coefficient_add_fma(coefficient, source, destination) }
    }

    #[target_feature(enable = "fma")]
    unsafe fn grouped_cross_fma(
        members: &[usize],
        weights: &[f64],
        source: Option<&[f64]>,
        scale: f64,
        center: f64,
        codes: &[u32],
        cross: &mut [f64],
    ) {
        fereg_core::kernels::grouped_cross(members, weights, source, scale, center, codes, cross)
    }

    fn grouped_cross(
        members: &[usize],
        weights: &[f64],
        source: Option<&[f64]>,
        scale: f64,
        center: f64,
        codes: &[u32],
        cross: &mut [f64],
    ) {
        // SAFETY: reachable only through `hardware()`, after FMA/AVX detection.
        unsafe { grouped_cross_fma(members, weights, source, scale, center, codes, cross) }
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
                let columns: Vec<_> = left.iter().chain(&right).chain(&left).copied().collect();
                for (first, rows) in [(0, 3), (1, 2)] {
                    let (mut hardware, mut portable) = (vec![0.0; 9], vec![0.0; 9]);
                    (h.normalized_products)(&columns, length, 3, first, rows, &mut hardware);
                    p::normalized_products(&columns, length, 3, first, rows, &mut portable);
                    assert_eq!(
                        hardware.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        portable.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                    );
                }
                let (mut hardware, mut portable) = (vec![0.0; 48], vec![0.0; 48]);
                (h.gram_panel)(&columns, length, 3, 0, length, &mut hardware);
                p::gram_panel(&columns, length, 3, 0, length, &mut portable);
                assert_eq!(
                    hardware.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    portable.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                );
                let weights: Vec<_> = right.iter().map(|v| v.abs() + 0.5).collect();
                assert_eq!(
                    (h.weighted_product)(&left, &weights, &right).to_bits(),
                    p::weighted_product(&left, &weights, &right).to_bits()
                );
                for scale in [0.25, 3.0] {
                    assert_eq!(
                        (h.normalized_dot)(&left, &weights, scale, &right).to_bits(),
                        p::normalized_dot(&left, &weights, scale, &right).to_bits()
                    );
                    let mut actual = right.clone();
                    let mut expected = right.clone();
                    (h.normalized_add)(-0.7, &left, &weights, scale, &mut actual);
                    p::normalized_add(-0.7, &left, &weights, scale, &mut expected);
                    assert_eq!(bits(&actual), bits(&expected));
                }
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
                    for source in [None, Some(right.as_slice())] {
                        (h.coefficient_add)(scale, source, &mut actual);
                        p::coefficient_add(scale, source, &mut expected);
                        assert_eq!(bits(&actual), bits(&expected));
                    }
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
        fn hardware_group_operators_and_retained_residuals_match_portable_bits() {
            let Some(h) = hardware() else {
                eprintln!("skipped: this processor has no FMA/AVX hardware path");
                return;
            };
            use fereg_core::kernels as p;
            let rows = [8, 1, 5, 0];
            let mut input = values(113, 11);
            input[..3].copy_from_slice(&[-0.0, 0.0, f64::from_bits(1)]);
            for rank in [0, 1, 3] {
                let basis = values(127, rank * rows.len() + 3);
                let coefficients = values(131, rank);
                let mut actual = input.clone();
                let mut expected = input.clone();
                (h.basis_forward)(&rows, &basis, &coefficients, &mut actual);
                p::basis_forward(&rows, &basis, &coefficients, &mut expected);
                assert_eq!(bits(&actual), bits(&expected));
                for certify in [false, true] {
                    let mut actual = vec![73.0; rank + 2];
                    let mut expected = actual.clone();
                    let actual_square = (h.basis_transpose)(
                        &rows,
                        &basis,
                        rank,
                        &input,
                        Some(&mut actual[1..rank + 1]),
                        certify,
                    );
                    let expected_square = p::basis_transpose(
                        &rows,
                        &basis,
                        rank,
                        &input,
                        Some(&mut expected[1..rank + 1]),
                        certify,
                    );
                    assert_eq!(actual_square.to_bits(), expected_square.to_bits());
                    assert_eq!(bits(&actual), bits(&expected));
                    assert_eq!(actual[0], 73.0);
                    assert_eq!(actual[rank + 1], 73.0);
                    assert_eq!(
                        (h.basis_transpose)(&rows, &basis, rank, &input, None, certify).to_bits(),
                        expected_square.to_bits()
                    );
                }
                let mut actual = input.clone();
                let mut expected = input.clone();
                let mut actual_scratch = vec![73.0; rank + 2];
                let mut expected_scratch = actual_scratch.clone();
                (h.basis_project)(&rows, &basis, &mut actual, &mut actual_scratch[1..rank + 1]);
                p::basis_project(
                    &rows,
                    &basis,
                    &mut expected,
                    &mut expected_scratch[1..rank + 1],
                );
                assert_eq!(bits(&actual), bits(&expected));
                assert_eq!(bits(&actual_scratch), bits(&expected_scratch));
                for row in 0..input.len() {
                    if !rows.contains(&row) {
                        assert_eq!(actual[row].to_bits(), input[row].to_bits());
                    }
                }
                assert_eq!(actual_scratch[0], 73.0);
                assert_eq!(actual_scratch[rank + 1], 73.0);
            }
            let positions = [8, 1, 5, 0, 1];
            let norms: Vec<_> = values(137, input.len())
                .iter()
                .map(|v| v.abs() + 0.5)
                .collect();
            let coefficients = values(139, positions.len());
            let codes: Vec<_> = (0..input.len()).map(|row| (row % 4) as u32).collect();
            for source in [None, Some(input.as_slice())] {
                let mut actual = vec![73.0; 7];
                let mut expected = actual.clone();
                (h.grouped_cross)(&rows, &norms, source, 3.0, -0.7, &codes, &mut actual[1..6]);
                p::grouped_cross(
                    &rows,
                    &norms,
                    source,
                    3.0,
                    -0.7,
                    &codes,
                    &mut expected[1..6],
                );
                assert_eq!(bits(&actual), bits(&expected));
                for index in [0, 5, 6] {
                    assert_eq!(actual[index], 73.0);
                }
            }
            for root in [None, Some(0.125), Some(1.0), Some(3.7)] {
                for response in [-0.0, 0.0, 3.0] {
                    assert_eq!(
                        (h.standardized_residual)(
                            response,
                            &input,
                            &positions,
                            &norms,
                            &coefficients,
                            root
                        )
                        .to_bits(),
                        p::standardized_residual(
                            response,
                            &input,
                            &positions,
                            &norms,
                            &coefficients,
                            root
                        )
                        .to_bits()
                    );
                }
            }
        }

        #[test]
        fn hardware_incidence_and_reduced_cg_preserve_bits() {
            let Some(h) = hardware() else {
                eprintln!("skipped: this processor has no FMA/AVX hardware path");
                return;
            };
            use fereg_core::kernels as p;
            let direction = [1.0, -0.0, f64::from_bits(1), -2.3, 0.0, 7.1];
            // Repeated, nonmonotonic coordinates make scatter order observable.
            for columns in [&[][..], &[4, 1, 4, 0, 2, 5, 3][..]] {
                let coefficients = values(181, columns.len());
                let mut actual = vec![0.3, -0.0, 1.1, 2.0, 0.0, -4.0, 913.0];
                let mut expected = actual.clone();
                (h.incidence_schur_row)(&coefficients, columns, &direction, &mut actual);
                p::incidence_schur_row(&coefficients, columns, &direction, &mut expected);
                assert_eq!(bits(&actual), bits(&expected));
                assert_eq!(actual[6], 913.0);
            }
            for n in [0, 1, 7, 19] {
                for alpha in [-1.7, -0.0, 0.0, 0.3, 8.1] {
                    let mut direction = values(183, n);
                    let product = values(185, n);
                    let mut actual_values = values(187, n + 2);
                    let mut actual_residual = values(189, n + 2);
                    if n > 0 {
                        direction[0] = f64::from_bits(1);
                        actual_residual[0] = -0.0;
                    }
                    let mut expected_values = actual_values.clone();
                    let mut expected_residual = actual_residual.clone();
                    let actual_energy = (h.joint_step_energy)(
                        alpha,
                        &direction,
                        &product,
                        &mut actual_values[..n],
                        &mut actual_residual[..n],
                    );
                    let expected_energy = p::joint_step_energy(
                        alpha,
                        &direction,
                        &product,
                        &mut expected_values[..n],
                        &mut expected_residual[..n],
                    );
                    assert_eq!(actual_energy.to_bits(), expected_energy.to_bits());
                    assert_eq!(bits(&actual_values), bits(&expected_values));
                    assert_eq!(bits(&actual_residual), bits(&expected_residual));
                }
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
                        for (level, row) in direction.chunks_exact(stride).enumerate() {
                            for (column, &value) in row.iter().enumerate() {
                                let index = level * stride + column;
                                if column >= rhs {
                                    assert_eq!(value.to_bits(), initial_direction[index].to_bits());
                                    assert_eq!(
                                        solution[index].to_bits(),
                                        initial_solution[index].to_bits()
                                    );
                                    assert_eq!(
                                        residual[index].to_bits(),
                                        initial_residual[index].to_bits()
                                    );
                                } else if active[column] == 0.0 {
                                    assert_eq!(value.to_bits(), before_direction[index].to_bits());
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
        fn hardware_certificate_tiles_and_triangular_products_preserve_bits() {
            let Some(h) = hardware() else {
                eprintln!("skipped: this processor has no FMA/AVX hardware path");
                return;
            };
            use fereg_core::kernels as p;
            for count in [0, 1, 7, 19] {
                for left_stride in [1, 3] {
                    for right_stride in [1, 5] {
                        let left = values(101, count * left_stride);
                        let right = values(107, count * right_stride);
                        for initial in [-0.0, 0.0, 1.25] {
                            assert_eq!(
                                (h.subtract_products)(
                                    &left,
                                    left_stride,
                                    &right,
                                    right_stride,
                                    count,
                                    initial
                                )
                                .to_bits(),
                                p::subtract_products(
                                    &left,
                                    left_stride,
                                    &right,
                                    right_stride,
                                    count,
                                    initial
                                )
                                .to_bits()
                            );
                        }
                    }
                }
            }
            let members = [8, 1, 5, 0];
            for width in [1, 3, 17, 33] {
                let residuals = values(109, 9 * width);
                let weights: Vec<_> = values(113, 9).iter().map(|v| v.abs() + 0.1).collect();
                let scales: Vec<_> = (0..width)
                    .map(|i| if i % 3 == 0 { 0.0 } else { 3.7 })
                    .collect();
                for rank in [0, 1, 3] {
                    for stride in [members.len(), members.len() + 3] {
                        let basis = values(127, rank * stride);
                        for first in (0..width).step_by(16) {
                            let count = (width - first).min(16);
                            let mut actual = [f64::MAX; 18];
                            let mut expected = actual;
                            (h.basis_projection_tile)(
                                &members,
                                &basis,
                                stride,
                                &weights,
                                &residuals,
                                width,
                                &scales,
                                first,
                                &mut actual[1..1 + count],
                            );
                            p::basis_projection_tile(
                                &members,
                                &basis,
                                stride,
                                &weights,
                                &residuals,
                                width,
                                &scales,
                                first,
                                &mut expected[1..1 + count],
                            );
                            assert_eq!(bits(&actual), bits(&expected));
                        }
                    }
                }
            }
            let mut empty_projection = [1.0];
            (h.basis_projection_tile)(&[], &[], 0, &[], &[], 1, &[1.0], 0, &mut empty_projection);
            assert_eq!(empty_projection[0].to_bits(), 0.0_f64.to_bits());
            for selected in [&[][..], &[8, 2, 5, 0, 2][..]] {
                let mut values = values(131, 9);
                values[..3].copy_from_slice(&[-0.0, 0.0, f64::from_bits(1)]);
                let scales = [0.125, 1.0, 3.7, 1.0e-100, 1.0e100];
                let coefficients = [0.3, -0.0, 3.7, 1.0e-100, 1.0e100];
                for root in [0.125, 1.0, 3.7] {
                    for initial in [-0.0, 0.0, 1.25] {
                        assert_eq!(
                            (h.normalized_selected_residual)(
                                initial,
                                &values,
                                selected,
                                &scales,
                                &coefficients,
                                root
                            )
                            .to_bits(),
                            p::normalized_selected_residual(
                                initial,
                                &values,
                                selected,
                                &scales,
                                &coefficients,
                                root
                            )
                            .to_bits()
                        );
                        let mut actual = [-0.0, 0.0, 0.25, f64::from_bits(1), -3.7, f64::MAX];
                        let mut expected = actual;
                        (h.normalized_selected_scores)(
                            &values,
                            selected,
                            &scales,
                            root,
                            initial,
                            &mut actual[..selected.len()],
                        );
                        p::normalized_selected_scores(
                            &values,
                            selected,
                            &scales,
                            root,
                            initial,
                            &mut expected[..selected.len()],
                        );
                        assert_eq!(bits(&actual), bits(&expected));
                    }
                }
            }
            let selected = [3, 0, 2];
            let coefficients = [Some(-0.0), None, Some(3.7), Some(-1.3)];
            let row = [f64::from_bits(1), f64::NAN, -0.0, 1.7];
            for initial in [-0.0, 0.0, 1.25] {
                assert_eq!(
                    (h.selected_residual)(initial, &row, &selected, &coefficients).to_bits(),
                    p::selected_residual(initial, &row, &selected, &coefficients).to_bits()
                );
            }
            // A known fused-versus-separate rounding witness, plus signed zero.
            for (left, right, sum) in [
                (1.0 + f64::EPSILON, 1.0 - f64::EPSILON, -1.0),
                (-0.0, 1.0, -0.0),
                (f64::from_bits(1), 0.5, f64::from_bits(1)),
            ] {
                assert_eq!(
                    (h.multiply_add)(left, right, sum).to_bits(),
                    left.mul_add(right, sum).to_bits()
                );
            }
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
