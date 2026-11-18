//! Publish original-coordinate inference after the stable likelihood, score,
//! sandwich, joint Wald and sample have passed their independent replay.
use super::super::slope_recovery::ReportMap;
use super::*;

impl CertifiedPpmlResult {
    pub(in crate::ppml) fn restore_slope_coordinates(&mut self, map: ReportMap) -> Result<()> {
        let wald = &self.inference.wald;
        let slots = match map {
            ReportMap::Pair(pair) => [Some(pair.source), Some(pair.target)],
            ReportMap::Absorbed(fe) => [Some(fe.slot), None],
        };
        for slot in slots.into_iter().flatten() {
            // The jointly tested slopes must span the same zero restriction
            // before and after the invertible map. An implicit constant with
            // nonzero loadings on these coordinates requires a distinct map.
            if !wald.tested_columns.iter().enumerate().any(|(i, &s)| {
                slot == s
                    && wald
                        .reference
                        .as_ref()
                        .is_none_or(|r| r.loadings()[i] == 0.0)
            }) {
                return Err(FeregError::new(ErrorCode::NonConvergence,
                    "original-coordinate PPML slope recovery cannot certify the model Wald restriction"));
            }
        }
        let tolerance = self.certificate.tolerance;
        let stable_wald = super::super::wald::slope_wald(
            self.coefficients.values(),
            &self.inference.vce.covariance,
            &wald.tested_columns,
            wald.reference.as_ref(),
            tolerance,
        )?;
        let vce = &mut self.inference.vce;
        match map {
            ReportMap::Pair(pair) => {
                pair.restore(&mut self.coefficients.values, &mut vce.covariance)?;
            }
            ReportMap::Absorbed(fe) => {
                fe.restore(&mut self.coefficients.values, &mut vce.covariance)?
            }
        }
        let width = self.coefficients.values.len();
        let slots = &self.coefficients.kept_full_slots;
        let rank = slots.len();
        let mut identified = fallible_filled(
            rank.checked_mul(rank).ok_or_else(|| {
                crate::error::allocation("original PPML covariance dimensions overflow")
            })?,
            0.0,
            "unable to reserve original PPML covariance inspection",
        )?;
        for (row, &i) in slots.iter().enumerate() {
            let variance = vce.covariance[i * width + i];
            if !variance.is_finite() || variance < 0.0 {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "original PPML slope variance is negative or nonfinite",
                ));
            }
            let se = variance.sqrt();
            vce.standard_errors[i] = se;
            vce.z_statistics[i] = if se == 0.0 {
                f64::NAN
            } else {
                let z = self.coefficients.values[i] / se;
                if !z.is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::NonConvergence,
                        "original PPML slope z statistic is nonfinite",
                    ));
                }
                z
            };
            for (column, &j) in slots.iter().enumerate() {
                identified[row * rank + column] = vce.covariance[i * width + j];
            }
        }
        let (_, psd) = super::super::psd::apply_ppml_psd_policy(
            &identified,
            rank,
            vce.cluster_dimension_groups.len(),
            self.certificate.tolerance,
        )?;
        vce.psd = psd;
        // The restoration constructs equal off-diagonal entries. The stable
        // joint Wald was replayed above and its null space is unchanged.
        vce.certificate.covariance_symmetry_relative_infinity_norm = 0.0;
        drop(identified);
        let original_wald = super::super::wald::slope_wald(
            self.coefficients.values(),
            &vce.covariance,
            &wald.tested_columns,
            wald.reference.as_ref(),
            tolerance,
        )?;
        let agrees = match (stable_wald.statistic, original_wald.statistic) {
            (Some(a), Some(b)) => relative_scalar_error(a, b) <= 1e-6,
            (None, None) => true,
            _ => false,
        };
        if !agrees
            || stable_wald.solve_relative_infinity_norm > tolerance
            || original_wald.solve_relative_infinity_norm > tolerance
        {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "the original-unit covariance cannot reproduce the certified joint PPML Wald test",
            ));
        }
        vce.certificate.wald_solve_relative_infinity_norm =
            original_wald.solve_relative_infinity_norm;
        self.inference.wald = original_wald;
        validate_covariance_and_certificates(
            &self.coefficients,
            &self.inference,
            self.certificate.tolerance,
        )
    }
}
