//! Refit in a verified finite basis, then score the original data before
//! publishing coefficients or any observation outputs.
mod interval;
mod replay;
use super::ols_pipeline::{infer_ols, OwnedInference};
use super::*;
use fereg_core::slope_recovery::{absorbed::AbsorbedRecovery, RecoveredSlope as Basis};

pub(crate) struct Reported {
    pub(crate) b: Vec<f64>,
    pub(crate) v: Vec<f64>,
    pub(super) absorbed: bool,
    pub(crate) wald: Option<f64>,
    pub(crate) clustered_minimum: Option<f64>,
}

impl Reported {
    pub(super) fn owned_bytes(&self) -> u128 {
        (self.b.capacity() as u128 + self.v.capacity() as u128) * 8
    }
}

pub(super) fn infer_and_derive(
    prepared: &mut PreparedOlsData,
    numerical_action: &mut EstimateAction,
    loaded_rows: Option<LoadedOlsRows>,
) -> Result<(
    OwnedInference,
    Option<Reported>,
    DerivedObservationOutputs,
    f64,
    f64,
)> {
    numerical_action.admit_prepared_parent(
        prepared.execution_admission_bytes(),
        prepared.owned_buffer_bytes(),
    )?;
    let inference_started = Instant::now();
    let (inference, reported) = infer(prepared, numerical_action, loaded_rows)?;
    numerical_action.admit_output_parent(
        prepared.execution_admission_bytes(),
        inference.view().owned_buffer_bytes() + reported.as_ref().map_or(0, Reported::owned_bytes),
    )?;
    let inference_seconds = inference_started.elapsed().as_secs_f64();
    let outputs_started = Instant::now();
    let derived = derive_observation_outputs(
        prepared,
        inference.view(),
        numerical_action,
        reported.as_ref(),
    )?;
    let outputs_seconds = outputs_started.elapsed().as_secs_f64();
    Ok((
        inference,
        reported,
        derived,
        inference_seconds,
        outputs_seconds,
    ))
}

fn infer(
    prepared: &mut PreparedOlsData,
    action: &mut EstimateAction,
    rows: Option<LoadedOlsRows>,
) -> Result<(OwnedInference, Option<Reported>)> {
    let ceiling = action
        .memory_plan
        .usable_bytes
        .unwrap_or(u128::MAX)
        .saturating_sub(action.metadata_owner_bytes + action.loaded_fit_workspace_bytes);
    let Some(recovery) = Basis::prepare(prepared, ceiling)? else {
        return Ok((infer_ols(prepared, action, rows)?, None));
    };
    // The speculative cache contains the original columns, including any
    // precomputed moment summaries. It cannot cross a basis change.
    drop(rows);
    let k = prepared.model().coefficients.len() as u128;
    let paired = match &recovery {
        Basis::Pair(r) => [r.source_column, r.target_column],
        Basis::Absorbed(r) => [r.target_column, r.target_column],
    };
    let recover_bytes = recovery.owned_bytes();
    let extra = (96 << 10) + recover_bytes + 8 * (5 * k * k + 16 * k);
    action.admit_prepared_parent(
        prepared.execution_admission_bytes() + extra,
        prepared.owned_buffer_bytes() + extra,
    )?;
    let inference = infer_ols(prepared, action, None).map_err(|error| {
        if error.code == ErrorCode::InvalidData
            && error.message == "sandwich covariance diagonal is invalid"
        {
            refusal("the recovered regression cannot certify the sandwich covariance diagonal")
        } else {
            error
        }
    })?;
    let view = inference.view();
    let fit = view.fit();
    let aliases = !paired
        .iter()
        .all(|col| fit.kept_generated_columns().contains(col));
    let additional_alias = !fit.dropped_generated_columns().is_empty();
    if let OwnedInference::Clustered(value) = &inference {
        if value.covariance().psd().full_adjusted {
            // Euclidean eigenvalue clipping does not commute with a shear.
            return Err(refusal(
                "the clustered covariance requires a coordinate-dependent PSD repair",
            ));
        }
    }
    let result = if aliases {
        Err(refusal("a working coefficient is aliased"))
    } else {
        restore(prepared, view, &recovery)
    };
    recovery.restore_design(prepared)?;
    let (mapped, error) = match result {
        Ok(result) if !additional_alias => return Ok((inference, Some(result))),
        Ok(result) => (
            Some(result),
            refusal("the original alias basis cannot be certified"),
        ),
        Err(error) if error.code == ErrorCode::NonConvergence => (None, error),
        Err(error) => return Err(error),
    };
    // A third regressor may be the exact difference of the pair. The raw
    // estimator then owns which public slot to omit. Accept a raw omission
    // only after both certified fits reproduce the same row predictions.
    let rank = fit.finite_rank();
    let mut kept = reserved_empty(
        fit.kept_generated_columns().len(),
        "original OLS alias check",
    )?;
    kept.extend_from_slice(fit.kept_generated_columns());
    let residuals = inference.into_residuals();
    let original = match infer_ols(prepared, action, None) {
        Ok(value) => value,
        Err(e) if matches!(e.code, ErrorCode::NonConvergence | ErrorCode::InvalidData) => {
            return Err(error)
        }
        Err(e) => return Err(e),
    };
    let raw = original.view().fit();
    let both_kept = paired
        .iter()
        .all(|col| raw.kept_generated_columns().contains(col));
    if raw.finite_rank() > rank
        || (additional_alias && raw.finite_rank() != rank)
        || (both_kept && (mapped.is_none() || kept != raw.kept_generated_columns()))
    {
        return Err(error);
    }
    let mut error_norm = 0.0_f64;
    let mut residual_norm = 0.0_f64;
    let mut outcome_norm = 0.0_f64;
    for (row, ((&a, &b), &y)) in residuals
        .iter()
        .zip(raw.residuals())
        .zip(&prepared.sample().outcome)
        .enumerate()
    {
        if row % 1024 == 0 {
            fereg_core::interrupt::checkpoint()?;
        }
        error_norm = error_norm.max((a - b).abs());
        residual_norm = residual_norm.max(a.abs()).max(b.abs());
        outcome_norm = outcome_norm.max(y.abs());
    }
    // A zero-residual row can acquire a few ulps of fitted-model arithmetic.
    // The original-direction score and the independent whole-sample residual
    // must both certify, including on the exactly fitted case.
    if error_norm > 5e-10 * residual_norm + 64. * f64::EPSILON * outcome_norm {
        return Err(error);
    }
    if !both_kept {
        // A pivot change can replace the omitted pair member with a different
        // weak column. Equal ranks and residuals do not certify inference on
        // that remaining basis. Replay the covariance that would be posted.
        let view = original.view();
        let (b, v) = scatter_working(&view)?;
        replay::enclose(prepared, &view, None, &b)?.check(&v)?;
    }
    Ok((original, if both_kept { mapped } else { None }))
}

fn restore(
    data: &PreparedOlsData,
    view: CertifiedOlsInference<'_>,
    recovery: &Basis,
) -> Result<Reported> {
    let fit = view.fit();
    let (mut b, mut v) = scatter_working(&view)?;
    map_report(data, fit, recovery, &mut b, &mut v)?;
    let raw = replay::enclose(data, &view, Some(recovery), &b)?;
    let mut corrected = None;
    if let Err(initial) = raw.check(&v) {
        // Recompute in the well-scaled basis before applying the congruence
        // again. Every final raw entry and all original restrictions must
        // then pass the *same* independent checks.
        let (mut refined_b, mut refined_v) = scatter_working(&view)?;
        replay::enclose(data, &view, None, &refined_b)?.refine(&mut refined_v)?;
        let compact = compact_covariance(&view, &refined_v)?;
        fereg_core::ols::recovered_clustered_minimum(
            &compact,
            view.covariance_coordinates().len(),
            false,
        )?;
        map_report(data, fit, recovery, &mut refined_b, &mut refined_v)?;
        raw.check(&refined_v).map_err(|_| initial)?;
        b = refined_b;
        v = refined_v;
        corrected = Some(compact);
    }
    let clustered_minimum = if let CertifiedOlsInference::Clustered(c) = &view {
        Some(fereg_core::ols::recovered_clustered_minimum(
            &compact_covariance(&view, &v)?,
            view.covariance_coordinates().len(),
            c.cluster_counts().len() > 1,
        )?)
    } else {
        None
    };
    let wald = fereg_core::ols::recovered_wald(
        data.model(),
        fit,
        corrected.as_deref().unwrap_or(view.covariance_values()),
    )?;
    Ok(Reported {
        b,
        v,
        absorbed: matches!(recovery, Basis::Absorbed(_)),
        wald,
        clustered_minimum,
    })
}

fn scatter_working(view: &CertifiedOlsInference<'_>) -> Result<(Vec<f64>, Vec<f64>)> {
    let fit = view.fit();
    let p = fit.coefficients().len();
    let coords = view.covariance_coordinates();
    let mut b = reserved_filled(p, 0., "OLS recovery coefficients")?;
    let mut v = reserved_filled(
        p.checked_mul(p)
            .ok_or_else(|| allocation("OLS recovery covariance overflow"))?,
        0.,
        "OLS recovery covariance",
    )?;
    for c in fit.coefficients() {
        b[c.coefficient_slot.get() as usize - 1] = c.value.unwrap_or(0.);
    }
    for (i, left) in coords.iter().enumerate() {
        for (j, right) in coords.iter().enumerate() {
            let row = left.coefficient_slot.get() as usize - 1;
            let col = right.coefficient_slot.get() as usize - 1;
            v[row * p + col] = view.covariance_values()[i * coords.len() + j];
        }
    }
    Ok((b, v))
}

fn map_report(
    data: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    recovery: &Basis,
    b: &mut [f64],
    v: &mut [f64],
) -> Result<()> {
    match recovery {
        Basis::Pair(r) => {
            let check = r.map.restore(b, v)?;
            r.check_full_predictions(data, &check, b)?;
            Ok(())
        }
        Basis::Absorbed(r) => {
            let cons = fit
                .coefficients()
                .iter()
                .find(|c| c.status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept);
            if let Some(c) = cons {
                r.map
                    .normalized_ols_map(c.coefficient_slot.get() as usize - 1)
                    .restore(b, v)?;
            } else {
                r.map.restore(b, v)?;
            }
            check_absorbed_predictions(data, fit, b, r, cons.is_some())?;
            Ok(())
        }
    }
}

fn compact_covariance(view: &CertifiedOlsInference<'_>, v: &[f64]) -> Result<Vec<f64>> {
    let p = view.fit().coefficients().len();
    let coords = view.covariance_coordinates();
    let n = coords.len();
    let mut block = reserved_filled(n * n, 0., "recovered OLS compact covariance")?;
    for (i, l) in coords.iter().enumerate() {
        for (j, r) in coords.iter().enumerate() {
            block[i * n + j] = v[(l.coefficient_slot.get() as usize - 1) * p
                + r.coefficient_slot.get() as usize
                - 1];
        }
    }
    Ok(block)
}

/// The mapped finite predictor can be very large even at moderate response
/// values. Check its *difference* from the original absorbed contribution,
/// and the resulting fitted values, in the units of the working regression.
fn check_absorbed_predictions(
    data: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    b: &[f64],
    r: &AbsorbedRecovery,
    center: bool,
) -> Result<()> {
    for row in 0..data.sample().n_active() {
        if row % 1024 == 0 {
            fereg_core::interrupt::checkpoint()?;
        }
        let (mut stable, mut serial, mut fused) = (0., 0., 0.);
        for c in fit.coefficients() {
            let (Some(j), Some(gamma)) = (c.generated_column_index, c.value) else {
                continue;
            };
            let z = data.column_value(j, row)?;
            let raw = if j == r.target_column {
                r.original_target(row)
            } else {
                z
            };
            let beta = b[c.coefficient_slot.get() as usize - 1];
            stable = gamma.mul_add(z, stable);
            serial += beta * raw;
            fused = beta.mul_add(raw, fused);
        }
        let shift = r.row_adjustment(row, b[r.map.slot], center);
        let y = data.sample().outcome[row];
        let expected = y - fit.residuals()[row];
        let d = (expected - stable) - shift;
        let tol = (stable
            .abs()
            .max(expected.abs())
            .max(fit.residuals()[row].abs()))
            * 5e-10;
        for error in [
            (serial - shift) - stable,
            (fused - shift) - stable,
            (serial + d) - expected,
            (fused + d) - expected,
        ] {
            if !error.is_finite() || error.abs() > tol {
                return Err(refusal(
                    "the original slope and absorbed effects lose their fitted sum",
                ));
            }
        }
    }
    Ok(())
}

fn refusal(detail: &str) -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        format!(
            "nearly collinear slopes: {detail}. Re-express the regressors using their difference."
        ),
    )
}
