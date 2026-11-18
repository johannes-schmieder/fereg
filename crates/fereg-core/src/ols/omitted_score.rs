//! A numerical omission is never evidence that the original data are exactly
//! aliased. Check well-resolved directional scores on the final fitted sample.
use super::centered::exact_location_difference;
use super::prepared_fit_view::{prepared_continuous_range, PreparedOlsFitView};
use super::{PreparedOlsColumnKind, PreparedOlsFitStatus, PreparedOlsModel};
use crate::data::PreparedWeights;
use crate::error::{ErrorCode, FeregError, Result};
use crate::linalg::neumaier_add;

#[derive(Clone, Copy)]
enum Column<'a> {
    Continuous(&'a [f64]),
    Indicator(&'a [u32], u32, usize),
}

impl<'a> Column<'a> {
    fn read<P: PreparedOlsFitView + ?Sized>(data: &'a P, j: usize) -> Result<Option<Self>> {
        Ok(match data.fit_model().columns[j].kind {
            PreparedOlsColumnKind::Continuous { .. } => {
                prepared_continuous_range(data, j, 0..data.fit_sample().n_active())
                    .map(Self::Continuous)
            }
            PreparedOlsColumnKind::Categorical {
                categorical_source_index,
                level_code: Some(code),
                ..
            } => Some(Self::Indicator(
                data.fit_categorical_codes(categorical_source_index)?,
                code,
                categorical_source_index,
            )),
            _ => None,
        })
    }
    fn at(self, row: usize) -> f64 {
        match self {
            Self::Continuous(x) => x[row],
            Self::Indicator(codes, level, _) => f64::from(u8::from(codes[row] == level)),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum FittedResponse<'a> {
    Residual(&'a [f64]),
    Mean(&'a [f64]),
}

struct Scores<'a> {
    y: &'a [f64],
    fit: FittedResponse<'a>,
    weights: Option<&'a [f64]>,
    w_max: f64,
    scale: f64,
    norm: f64,
}

impl Scores<'_> {
    fn weight(&self, i: usize) -> f64 {
        self.weights.map_or(1., |w| w[i] / self.w_max)
    }
    fn response(&self, i: usize) -> (f64, f64) {
        match self.fit {
            FittedResponse::Residual(r) => (r[i], self.y[i].abs().max(r[i].abs())),
            FittedResponse::Mean(mu) => (self.y[i] - mu[i], self.y[i].max(mu[i])),
        }
    }
}

/// An optional witness has bounded streaming work and no row-sized scratch.
/// A skipped or inconclusive witness does not choose a different rank.
const MAX_WITNESS_ROW_VISITS: usize = 32 << 20;

pub(crate) fn check<P: PreparedOlsFitView + ?Sized>(
    data: &P,
    slots: impl Iterator<Item = (Option<usize>, PreparedOlsFitStatus)>,
    kept: &[usize],
    y: &[f64],
    fit: FittedResponse<'_>,
    tolerance: f64,
) -> Result<()> {
    let mut omitted = slots
        .filter_map(|(index, status)| match status {
            PreparedOlsFitStatus::CollinearWithFixedEffects
            | PreparedOlsFitStatus::NumericallyRankDeficient => index,
            _ => None,
        })
        .peekable();
    if omitted.peek().is_none()
        || y.len() > MAX_WITNESS_ROW_VISITS / 2
        || !data
            .fit_model()
            .columns
            .iter()
            .any(|c| matches!(c.kind, PreparedOlsColumnKind::Continuous { .. }))
    {
        return Ok(());
    }
    let weights = match &data.fit_sample().weights {
        PreparedWeights::None => None,
        PreparedWeights::Provided { values, .. } => Some(values.as_slice()),
    };
    let mut score = Scores {
        y,
        fit,
        weights,
        w_max: weights.map_or(1., |w| w.iter().copied().fold(0., f64::max)),
        scale: 0.,
        norm: 0.,
    };
    score.scale = (0..y.len()).map(|i| score.response(i).1).fold(0., f64::max);
    if score.scale == 0. {
        return Ok(());
    }
    score.norm = (0..y.len())
        .map(|i| score.weight(i) * (score.response(i).1 / score.scale).powi(2))
        .sum::<f64>()
        .sqrt();
    let design = data.fit_borrowed_intercept_map_design()?;
    let mut budget = MAX_WITNESS_ROW_VISITS / y.len().max(1);
    let center = data.fit_structural_intercept() || data.fit_has_absorbed_intercept();
    for j in omitted {
        // When pivoting drops the structural intercept, the nearly dependent
        // relation can involve two retained columns and a constant. Centering
        // at a reference row still yields a direction of the requested model.
        if matches!(
            data.fit_model().columns[j].kind,
            PreparedOlsColumnKind::Intercept
        ) {
            for (pos, &left) in kept.iter().enumerate() {
                let Some(x) = Column::read(data, left)? else {
                    continue;
                };
                for &right in &kept[pos + 1..] {
                    // Charge every pair, including unsupported kinds. A large
                    // factor expansion must never turn this into an unbounded
                    // quadratic metadata scan.
                    if budget < 3 {
                        return Ok(());
                    }
                    budget -= 3;
                    if let Some(z) = Column::read(data, right)? {
                        paired_direction(
                            data.fit_model(),
                            &score,
                            tolerance,
                            x,
                            z,
                            true,
                            &mut budget,
                        )?;
                    }
                }
            }
            continue;
        }
        let Some(x) = Column::read(data, j)? else {
            continue;
        };
        // x minus any one absorbed level function is in the full design.
        for effect in design.iter().flat_map(|d| d.effects()) {
            if budget < 2 {
                return Ok(());
            }
            budget -= 2;
            direction(&score, tolerance, |i| {
                let first = effect.level_index().members(effect.codes()[i] as usize)[0];
                let base = x.at(first);
                (x.at(i) - base, x.at(i).abs().max(base.abs()), true)
            })?;
        }
        for &k in kept {
            if budget < 3 {
                return Ok(());
            }
            budget -= 3;
            if let Some(other) = Column::read(data, k)? {
                paired_direction(
                    data.fit_model(),
                    &score,
                    tolerance,
                    x,
                    other,
                    center,
                    &mut budget,
                )?;
            }
        }
    }
    Ok(())
}

fn paired_direction(
    model: &PreparedOlsModel,
    scores: &Scores<'_>,
    tolerance: f64,
    x: Column<'_>,
    other: Column<'_>,
    center: bool,
    budget: &mut usize,
) -> Result<()> {
    match (x, other) {
        (Column::Indicator(..), Column::Indicator(..)) => return Ok(()),
        (Column::Continuous(values), Column::Indicator(codes, _, source))
        | (Column::Indicator(codes, _, source), Column::Continuous(values)) => {
            let cost = 3 + model.columns.len().div_ceil(values.len());
            if cost <= *budget {
                *budget -= cost;
                within_factor(model, source, codes, values, center, scores, tolerance)?;
            }
        }
        _ => {}
    }
    let (x0, z0) = if center {
        (x.at(0), other.at(0))
    } else {
        (0., 0.)
    };
    let pivot = (0..scores.y.len())
        .max_by(|&i, &j| {
            (other.at(i) - z0)
                .abs()
                .total_cmp(&(other.at(j) - z0).abs())
        })
        .unwrap();
    let alpha = (x.at(pivot) - x0) / (other.at(pivot) - z0);
    if !alpha.is_finite() {
        return Ok(());
    }
    // The proposed coefficient only chooses a full-model direction. The
    // derivative independently decides whether that direction contradicts the fit.
    direction(scores, tolerance, |i| {
        let d = (-alpha).mul_add(other.at(i) - z0, x.at(i) - x0);
        let raw = x
            .at(i)
            .abs()
            .max(x0.abs())
            .max(alpha.abs() * other.at(i).abs().max(z0.abs()));
        let exact_origins =
            exact_location_difference(x.at(i), x0) && exact_location_difference(other.at(i), z0);
        (d, raw, exact_origins)
    })
}

/// Subtraction of a separate reference value in each observed category is a
/// legal full-model direction only if the requested indicators span all such
/// functions. With a constant, one missing indicator can be reconstructed.
/// Scratch is bounded by one page of reference values plus a presence bitmap.
fn within_factor(
    model: &PreparedOlsModel,
    source: usize,
    codes: &[u32],
    x: &[f64],
    center: bool,
    scores: &Scores<'_>,
    tolerance: f64,
) -> Result<()> {
    const LEVELS: usize = 4096 / std::mem::size_of::<f64>();
    let mut explicit = [false; LEVELS];
    for column in &model.columns {
        if let PreparedOlsColumnKind::Categorical {
            categorical_source_index,
            level_code: Some(code),
            ..
        } = column.kind
        {
            if categorical_source_index == source && (code as usize) < LEVELS {
                explicit[code as usize] = true;
            }
        }
    }
    let mut first = [f64::NAN; LEVELS];
    let mut missing = 0;
    for (&code, &value) in codes.iter().zip(x) {
        let level = code as usize;
        if level >= LEVELS {
            return Ok(());
        }
        if first[level].is_nan() {
            first[level] = value;
            missing += usize::from(!explicit[level]);
            if missing > usize::from(center) {
                return Ok(());
            }
        }
    }
    direction(scores, tolerance, |i| {
        let base = first[codes[i] as usize];
        (x[i] - base, x[i].abs().max(base.abs()), true)
    })
}

/// Evaluate the directional derivative in units of the direction itself. The
/// enclosure covers two origin subtractions and the final fused multiply-add,
/// including a subnormal absolute error floor. Differences at source-rounding
/// scale alone are inconclusive.
fn direction(
    scores: &Scores<'_>,
    tolerance: f64,
    value: impl Fn(usize) -> (f64, f64, bool),
) -> Result<()> {
    let radius = |raw: f64| 8. * f64::EPSILON * raw + 8. * f64::from_bits(1);
    let (mut largest, mut resolved) = (0_f64, false);
    for i in 0..scores.y.len() {
        let (d, raw, _) = value(i);
        if !d.is_finite() || !raw.is_finite() {
            return Ok(());
        }
        largest = largest.max(d.abs());
        resolved |= d.abs() > radius(raw);
    }
    if !resolved {
        return Ok(());
    }
    let (mut sum, mut compensation, mut uncertainty, mut norm) = (0., 0., 0., 0.);
    for i in 0..scores.y.len() {
        let (d, raw, exact_origins) = value(i);
        let a = d / largest;
        // Once a direction is resolved on the supplied binary64 inputs, its
        // arithmetic error is one rounding of the difference or fused
        // multiply-add if the origins were subtracted exactly. Treat each
        // unresolved row's entire contrast as uncertainty as well: rounding
        // of a source-scale multiple alone must never trigger the gate.
        let eval_error = if exact_origins {
            f64::EPSILON * d.abs() + f64::from_bits(1)
        } else {
            radius(raw)
        };
        let uncertain_row = if d.abs() <= radius(raw) { d.abs() } else { 0. };
        let err = (eval_error + uncertain_row) / largest;
        let w = scores.weight(i);
        let r = scores.response(i).0 / scores.scale;
        neumaier_add(w * r * a, &mut sum, &mut compensation);
        uncertainty += w * r.abs() * err;
        norm += w * (a.abs() + err).powi(2);
    }
    // The nonnegative reductions include at most N rounded additions. Cover
    // their forward error and the normalized products. Subnormal products
    // can incur an absolute rounding error even when their relative error
    // is unbounded, so require a positive normal lower bound on each norm.
    let n = scores.y.len() as f64;
    let round = (8. * n + 32.) * f64::EPSILON;
    let underflow = (8. * n + 32.) * f64::MIN_POSITIVE;
    let scale = (norm + underflow).sqrt() * (scores.norm + underflow.sqrt());
    let lower =
        (((sum + compensation).abs() - uncertainty - underflow) / scale) * (1. - round) - round;
    if lower.is_finite() && lower > tolerance {
        return Err(FeregError::new(ErrorCode::NonConvergence, format!(
            "an omitted regressor has a resolved score along a full-model direction (relative score lower bound {lower:e}, limit {tolerance:e}). Re-express the nearly collinear regressors using their differences."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_multiple_with_correlated_roundoff_is_inconclusive() {
        let x: [f64; 4] = [0.1, 0.2, 0.3, 0.4];
        let z = x.map(|v| 3. * v);
        let r = std::array::from_fn::<_, 4, _>(|i| (-3_f64).mul_add(x[i], z[i]).signum());
        let scores = Scores {
            y: &r,
            fit: FittedResponse::Residual(&r),
            weights: None,
            w_max: 1.,
            scale: 1.,
            norm: 2.,
        };
        direction(&scores, 1e-10, |i| {
            ((-3_f64).mul_add(x[i], z[i]), z[i].abs(), true)
        })
        .unwrap();
        let resolved = direction(&scores, 1e-10, |i| {
            (r[i] * 2_f64.powi(-38), z[i].abs(), true)
        });
        assert_eq!(resolved.unwrap_err().code, ErrorCode::NonConvergence);
    }
}
