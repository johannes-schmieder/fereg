use super::*;
use crate::fe::absorb_borrowed_intercept_stable_cg_retaining_certified;

/// A fitted-model target may contain subtraction roundoff outside the FE span.
/// Project one fresh RHS, keep its complete coefficient product, and certify
/// against the original model scale before constructing an exactly in-span
/// target. The ordinary strict span certificate remains unchanged.
#[allow(clippy::too_many_arguments)]
pub(crate) fn recover_finalized_model_effects_certified(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    residuals: &[f64],
    model_scale: f64,
    requested: &[bool],
    known_connected: bool,
    materialize_absorbed_sum: bool,
    map: MapConfig,
) -> Result<RecoveredCategoricalEffects> {
    validate_recovery_inputs(topology, effects, target, requested)?;
    if effects.is_empty()
        || effects
            .iter()
            .any(|effect| !effect.has_intercept() || !effect.slopes().is_empty())
        || residuals.len() != target.len()
        || residuals.iter().any(|value| !value.is_finite())
        || !model_scale.is_finite()
        || model_scale < 0.0
    {
        return Err(invalid_argument("model FE recovery inputs are invalid"));
    }
    let sources = topology_source_table(topology)?;
    let design = borrow_categorical_topology_design(
        target.len(),
        topology.registry(),
        &sources,
        topology.effect_to_topology(),
    )?;
    let mut raw = fallible_copy(target, "model recovery fresh RHS")?;
    for (value, &residual) in raw.iter_mut().zip(residuals) {
        *value -= residual;
    }
    let rhs_scale = raw.iter().map(|value| value.abs()).fold(0.0_f64, f64::max);
    let rhs_scale = if rhs_scale == 0.0 { 1.0 } else { rhs_scale };
    // Project the FE discrepancy so estimator noise does not dominate stopping.
    // Normalize every RHS to remove inherited absolute CG energy scales.
    for value in &mut raw {
        *value /= rhs_scale;
    }
    let projection = absorb_borrowed_intercept_stable_cg_retaining_certified(
        &design,
        weights,
        DenseMatrix::from_row_major(target.len(), 1, raw)?,
        map,
    )?;
    let (mut reconstructed, report, coefficients) = projection.into_full_parts();
    let coefficients = coefficients.ok_or_else(|| {
        invalid_data("model FE recovery did not retain a complete coefficient product")
    })?;
    let mut normalized = fallible_copy(
        coefficients.rhs(0)?,
        "model recovery normalized coefficients",
    )?;
    drop(coefficients);
    let validated = validate_weights(weights, target.len())?;
    normalize_intercept_components(
        topology,
        effects,
        validated,
        &mut normalized,
        known_connected,
    )?;
    for value in &mut normalized {
        *value *= rhs_scale;
    }
    let offsets = channel_major_effect_offsets(topology, effects)?;
    let fitted = |row: usize| -> Result<f64> {
        let mut value = 0.0;
        for (effect, &source) in topology.effect_to_topology().iter().enumerate() {
            let codes = topology.registry().source(source)?.codes(&sources)?;
            value += normalized[offsets[effect] + codes[row] as usize];
        }
        Ok(value)
    };
    // Reconstruct from normalized coefficients, independently of the CG
    // recurrence, then check the accepted estimator residual without changing it.
    let (mut norm_scale, mut norm_sum) = (0.0, 1.0);
    for row in 0..target.len() {
        let raw = target[row];
        let contribution = fitted(row)?;
        certify_model_match(
            raw - residuals[row],
            contribution,
            model_scale,
            map.tolerance,
        )?;
        reconstructed[(row, 0)] = raw - contribution;
        update_scaled_sum(
            validated.value(row).sqrt() * raw,
            &mut norm_scale,
            &mut norm_sum,
        );
    }
    let mut reservation = SystemSlopeWorkspaceReservation;
    let mut workspace = SlopeWorkspace::new(1, 1, &mut reservation)?;
    workspace.reference_norms[0] = norm_scale * norm_sum.sqrt();
    if !workspace.reference_norms[0].is_finite()
        || !certify(
            topology,
            effects,
            validated,
            &reconstructed,
            map.tolerance,
            &mut workspace,
        )?
        .passed
    {
        return Err(invalid_data(
            "model FE recovery failed its reconstructed FE certificate",
        ));
    }
    for row in 0..target.len() {
        reconstructed[(row, 0)] = fitted(row)?;
    }
    finish_channel_major_recovery(
        topology,
        effects,
        validated,
        reconstructed.as_slice(),
        requested,
        &normalized,
        report.iterations,
        map.tolerance,
        CategoricalRecoveryMethod::Cg,
        materialize_absorbed_sum,
    )
}

fn certify_model_match(target: f64, product: f64, scale: f64, tolerance: f64) -> Result<()> {
    let error = (target - product).abs();
    let relative = if scale == 0.0 {
        if error == 0.0 {
            0.0
        } else {
            f64::INFINITY
        }
    } else {
        error / scale
    };
    if !product.is_finite() || !relative.is_finite() || relative > tolerance {
        return Err(invalid_data(
            "saved FE product failed the original model reconstruction certificate",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::certify_model_match;
    #[test]
    fn complete_model_products_recover_tiny_effects_on_disconnected_scaled_designs() {
        use super::super::*;
        let n = 96;
        let g: Vec<_> = (0..n)
            .map(|row| (row % 4 + 4 * (row / 48)) as u32)
            .collect();
        let h: Vec<_> = (0..n)
            .map(|row| ((row / 4) % 3 + 3 * (row / 48)) as u32)
            .collect();
        let topology = crate::fe::finalize_categorical_topology(
            &[
                crate::fe::CategoricalCodeView::new(&g, 8),
                crate::fe::CategoricalCodeView::new(&h, 6),
            ],
            &[&[0], &[1]],
            &vec![true; n],
        )
        .unwrap();
        let effects = [
            CategoricalAbsorbedEffectView::new(true, vec![]).unwrap(),
            CategoricalAbsorbedEffectView::new(true, vec![]).unwrap(),
        ];
        let weights: Vec<_> = (0..n)
            .map(|row| 0.25 + (g[row] + 1) as f64 / 4.0 + (h[row] + 1) as f64 / 8.0)
            .collect();
        for scale in [1e-100, 1.0, 1e100] {
            let noise: Vec<_> = (0..n)
                .map(|row| {
                    let replicate = (row % 48) / 12;
                    4.0 * if replicate == 0 || replicate == 3 {
                        scale
                    } else {
                        -scale
                    }
                })
                .collect();
            let truth: Vec<_> = (0..n)
                .map(|row| scale * (2e-9 * (g[row] + 1) as f64 + 1e-9 * (h[row] + 1) as f64))
                .collect();
            let target: Vec<_> = truth.iter().zip(&noise).map(|(fe, r)| fe + r).collect();
            let contaminated: Vec<_> = noise
                .iter()
                .enumerate()
                .map(|(row, r)| r + 1e-4 * scale * (g[row] + 1) as f64)
                .collect();
            let rejected = super::recover_finalized_model_effects_certified(
                &topology,
                &effects,
                BorrowedMapWeights::Unit,
                &target,
                &contaminated,
                5.0 * scale,
                &[true, true],
                false,
                true,
                MapConfig {
                    tolerance: 1e-12,
                    max_iterations: 256,
                    check_every: 1,
                    symmetric: true,
                },
            )
            .unwrap_err();
            assert!(rejected.message.contains("reconstructed FE certificate"));
            for prior in [
                BorrowedMapWeights::Unit,
                BorrowedMapWeights::Provided(&weights),
            ] {
                for saved_only in [false, true] {
                    let recovered = super::recover_finalized_model_effects_certified(
                        &topology,
                        &effects,
                        prior,
                        &target,
                        &noise,
                        5.0 * scale,
                        &[true, true],
                        false,
                        !saved_only,
                        MapConfig {
                            tolerance: 1e-12,
                            max_iterations: 256,
                            check_every: 1,
                            symmetric: true,
                        },
                    )
                    .unwrap();
                    for (row, expected) in truth.iter().enumerate() {
                        let fitted = recovered.channels[0].as_ref().unwrap()[row]
                            + recovered.channels[1].as_ref().unwrap()[row];
                        assert!((fitted - expected).abs() < 1e-14 * scale,
                            "scale={scale:e} prior={prior:?} row={row} actual={fitted:e} expected={:e} method={:?} iterations={}", expected, recovered.recovery_method, recovered.iterations);
                    }
                    assert_eq!(recovered.absorbed_sum.is_none(), saved_only);
                }
            }
        }
    }

    #[test]
    fn model_certificate_is_scale_invariant_and_rejects_missing_products() {
        for scale in [1e-100, 1.0, 1e100] {
            assert!(certify_model_match(
                4e-9 * scale + 6e-16 * scale,
                4e-9 * scale,
                5.0 * scale,
                1e-12
            )
            .is_ok());
            assert!(certify_model_match(4e-9 * scale, 0.0, 5.0 * scale, 1e-12).is_err());
        }
        assert!(certify_model_match(0.0, 0.0, 0.0, 1e-12).is_ok());
        assert!(certify_model_match(0.0, 1e-100, 0.0, 1e-12).is_err());
    }
}
