use crate::error::{ErrorCode, FeregError, Result};

use super::family::poisson_deviance_contribution;

/// Public PPML residual definitions fixed by SPEC §12.
///
/// These are deliberately independent of the prior-weight experiment.  Prior
/// weights enter the estimator and its sandwich scores, but the values exposed
/// to users are defined on the raw outcome scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlResidualKind {
    Response,
    Score,
    Pearson,
    Working,
    /// The nonnegative per-observation deviance contribution used by the
    /// pinned comparator, not a signed square-root residual.
    Deviance,
    Anscombe,
}

/// Compute one public residual vector on the final compact PPML sample.
pub fn ppml_residuals(outcome: &[f64], mean: &[f64], kind: PpmlResidualKind) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    values.try_reserve_exact(outcome.len()).map_err(|_| {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve the PPML residual output",
        )
    })?;
    values.resize(outcome.len(), 0.0);
    fill_ppml_residuals(outcome, mean, kind, &mut values)?;
    Ok(values)
}

/// Fill an already allocated public residual vector.
///
/// Keeping the fill operation separate lets the fit-lifetime output planner
/// reuse a transaction-owned destination without adding another O(N)
/// allocation after estimation.
pub fn fill_ppml_residuals(
    outcome: &[f64],
    mean: &[f64],
    kind: PpmlResidualKind,
    destination: &mut [f64],
) -> Result<()> {
    if outcome.len() != mean.len() || destination.len() != outcome.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML residual inputs are not aligned on the final sample",
        ));
    }
    for row in 0..outcome.len() {
        let y = outcome[row];
        let mu = mean[row];
        if !y.is_finite() || y < 0.0 || !mu.is_finite() || mu <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("PPML residual inputs are outside the Poisson domain at compact row {row}"),
            ));
        }
        let raw = y - mu;
        let value = match kind {
            PpmlResidualKind::Response | PpmlResidualKind::Score => raw,
            PpmlResidualKind::Pearson => raw / mu.sqrt(),
            PpmlResidualKind::Working => raw / mu,
            PpmlResidualKind::Deviance => poisson_deviance_contribution(y, mu)?,
            PpmlResidualKind::Anscombe => {
                // cbrt(x)^2 and sqrt(cbrt(mu)) implement x^(2/3) and
                // mu^(1/6) without forming a ratio that can overflow first.
                let y_two_thirds = y.cbrt().powi(2);
                let mu_cuberoot = mu.cbrt();
                let mu_two_thirds = mu_cuberoot.powi(2);
                1.5 * (y_two_thirds - mu_two_thirds) / mu_cuberoot.sqrt()
            }
        };
        if !value.is_finite() {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                format!("PPML {kind:?} residual is not representable at compact row {row}"),
            ));
        }
        destination[row] = value;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f64, right: f64) {
        assert!((left - right).abs() <= 2e-14 * left.abs().max(right.abs()).max(1.0));
    }

    #[test]
    fn all_public_formulas_match_the_independent_oracle_fixture() {
        let y = [0.0_f64, 1.0, 4.0];
        let mu = [0.5_f64, 1.5, 3.0];
        let raw = [-0.5_f64, -0.5, 1.0];

        assert_eq!(
            ppml_residuals(&y, &mu, PpmlResidualKind::Response).expect("response"),
            raw
        );
        assert_eq!(
            ppml_residuals(&y, &mu, PpmlResidualKind::Score).expect("score"),
            raw
        );
        for (actual, expected) in ppml_residuals(&y, &mu, PpmlResidualKind::Pearson)
            .expect("pearson")
            .iter()
            .zip(
                raw.iter()
                    .zip(mu)
                    .map(|(residual, mean)| residual / mean.sqrt()),
            )
        {
            assert_close(*actual, expected);
        }
        for (actual, expected) in ppml_residuals(&y, &mu, PpmlResidualKind::Working)
            .expect("working")
            .iter()
            .zip(raw.iter().zip(mu).map(|(residual, mean)| residual / mean))
        {
            assert_close(*actual, expected);
        }
        let deviance = ppml_residuals(&y, &mu, PpmlResidualKind::Deviance).expect("deviance");
        assert_close(deviance[0], 1.0);
        assert_close(deviance[1], 2.0 * (1.5 - 1.0 + (1.0_f64 / 1.5).ln()));
        assert_close(deviance[2], 2.0 * (3.0 - 4.0 + 4.0 * (4.0_f64 / 3.0).ln()));
        for (actual, (outcome, mean)) in ppml_residuals(&y, &mu, PpmlResidualKind::Anscombe)
            .expect("anscombe")
            .iter()
            .zip(y.iter().zip(mu))
        {
            let expected =
                1.5 * (outcome.powf(2.0 / 3.0) - mean.powf(2.0 / 3.0)) / mean.powf(1.0 / 6.0);
            assert_close(*actual, expected);
        }
    }

    #[test]
    fn public_residuals_do_not_accept_weights_or_change_with_weight_kind() {
        let y = [0.0_f64, 2.0];
        let mu = [1.0_f64, 1.5];
        let first = ppml_residuals(&y, &mu, PpmlResidualKind::Score).expect("score");
        let second = ppml_residuals(&y, &mu, PpmlResidualKind::Response).expect("response");
        assert_eq!(first, second);
    }

    #[test]
    fn deviance_contribution_stays_nonnegative_at_near_equality() {
        let mean = 1.0_f64;
        let outcome = f64::from_bits(mean.to_bits() + 1);
        let value = ppml_residuals(&[outcome], &[mean], PpmlResidualKind::Deviance)
            .expect("stable deviance")[0];
        assert!(value > 0.0);
        assert!(value.is_finite());
    }

    #[test]
    fn invalid_domain_and_nonrepresentable_values_fail_closed() {
        assert_eq!(
            ppml_residuals(&[-1.0], &[1.0], PpmlResidualKind::Response)
                .expect_err("negative outcome")
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            ppml_residuals(&[1.0], &[0.0], PpmlResidualKind::Response)
                .expect_err("zero mean")
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            ppml_residuals(&[f64::MAX], &[f64::MIN_POSITIVE], PpmlResidualKind::Working)
                .expect_err("overflowing working residual")
                .code,
            ErrorCode::NonConvergence
        );
    }

    #[test]
    fn fill_requires_one_exact_final_sample_axis() {
        let mut destination = [0.0_f64; 1];
        assert_eq!(
            fill_ppml_residuals(
                &[1.0, 2.0],
                &[1.0, 2.0],
                PpmlResidualKind::Response,
                &mut destination,
            )
            .expect_err("misaligned destination")
            .code,
            ErrorCode::InvalidArgument
        );
    }
}
