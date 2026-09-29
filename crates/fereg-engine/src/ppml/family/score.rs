//! Fresh outer scores, independent of projected controls and working-state caches.
use super::{invalid, mean_from_eta, numerical, Family};
use crate::{
    data::DenseMatrix,
    error::{reserved, Result},
    fe::Design,
    linalg::scale_ratio,
    ppml::control::Scores,
    threads::{CompensatedSum, ThreadTeam},
};
use rayon::prelude::*;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ScoreNorms {
    pub absolute_max: f64,
    pub relative_max: f64,
    pub directions: usize,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoreCertificate {
    pub finite: ScoreNorms,
    pub fixed_effects: ScoreNorms,
    pub scale: f64,
    pub relative_rms: f64,
    pub passed: bool,
}
impl ScoreCertificate {
    #[must_use]
    pub fn convergence_scores(self) -> Scores {
        Scores {
            finite: self.finite.relative_max,
            fixed_effects: self.fixed_effects.relative_max,
        }
    }
}
#[derive(Default)]
struct Row {
    score: f64,
    root_h: f64,
}
struct Direction {
    absolute: f64,
    relative: f64,
}
impl Family<'_> {
    /// `raw` contains original generated columns, including the structural constant.
    /// Only FE membership/bases are borrowed: current h is always rebuilt from eta.
    /// This is score evidence, not a model-space or final-Hessian certificate.
    pub fn score_certificate(
        &self,
        eta: &[f64],
        raw: &DenseMatrix,
        fe: &Design<'_>,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<ScoreCertificate> {
        let n = self.outcome.len();
        if eta.len() != n || raw.nrows() != n || fe.nobs() != n {
            return Err(invalid("PPML score-frame lengths differ"));
        }
        if !tolerance.is_finite() || tolerance <= 0. {
            return Err(invalid("PPML score tolerance must be finite and positive"));
        }
        let (rows, scale) = self.score_rows(eta)?;
        let mut finite = reserved(raw.ncols())?;
        finite.resize_with(raw.ncols(), || None);
        team.install(|| {
            finite
                .par_iter_mut()
                .enumerate()
                .for_each(|(column, result)| {
                    *result = Some(direction(
                        (0..n).map(|row| (&rows[row], Ok(raw[(row, column)]))),
                        scale,
                    ));
                });
        });
        let mut fixed_effects = reserved(fe.layout().total)?;
        fixed_effects.resize_with(fe.layout().total, || None);
        team.install(|| {
            fixed_effects
                .par_iter_mut()
                .enumerate()
                .for_each(|(index, result)| {
                    // Preserve the published effect/level/channel direction order.
                    let effect = fe.layout().effects.partition_point(|e| e.offset <= index) - 1;
                    let layout = &fe.layout().effects[effect];
                    let local = index - layout.offset;
                    let channel = local % layout.channels;
                    let level = local / layout.channels;
                    *result = Some((|| {
                        let members = fe
                            .topology()
                            .effect_partition(effect)?
                            .membership()
                            .members(level)?;
                        direction(
                            members
                                .iter()
                                .map(|&row| (&rows[row], fe.basis_value(effect, channel, row))),
                            scale,
                        )
                    })());
                });
        });
        let mut square = CompensatedSum::default();
        let finite = summarize(finite, &mut square)?;
        let fixed_effects = summarize(fixed_effects, &mut square)?;
        let count = finite.directions + fixed_effects.directions;
        let relative_rms = if count == 0 {
            0.
        } else {
            (square.total() / count as f64).sqrt()
        };
        if !relative_rms.is_finite() {
            return Err(numerical("PPML score RMS is not representable"));
        }
        Ok(ScoreCertificate {
            finite,
            fixed_effects,
            scale,
            relative_rms,
            passed: finite.relative_max.max(fixed_effects.relative_max) <= tolerance
                && relative_rms <= tolerance,
        })
    }

    fn score_rows(&self, eta: &[f64]) -> Result<(Vec<Row>, f64)> {
        let mut rows = reserved(eta.len())?;
        let mut residual_norm = 0_f64;
        let mut reference_norm = 0_f64;
        for (row, &eta) in eta.iter().enumerate() {
            let mu = mean_from_eta(eta)?;
            let q = self.prior.value(row);
            let y = self.outcome[row];
            let residual = y - mu;
            let h = q * mu;
            let score = q * residual;
            if !h.is_finite() || h <= 0. || !score.is_finite() || score == 0. && residual != 0. {
                return Err(numerical(
                    "PPML score or information weight is not representable",
                ));
            }
            // The pinned scale is max(||sqrt(q/mu)(y-mu)||, ||sqrt(q/mu)y||).
            // Scaled products and hypot avoid false zero/overflow from squaring.
            residual_norm = residual_norm.hypot(scale_ratio(residual, q.sqrt(), mu.sqrt(), 1.));
            reference_norm = reference_norm.hypot(scale_ratio(y, q.sqrt(), mu.sqrt(), 1.));
            rows.push(Row {
                score,
                root_h: h.sqrt(),
            });
        }
        if !residual_norm.is_finite() || !reference_norm.is_finite() {
            return Err(numerical("PPML score scale is not representable"));
        }
        let scale = residual_norm.max(reference_norm);
        if scale == 0. {
            return Err(numerical("positive-outcome PPML score scale underflowed"));
        }
        Ok((rows, scale))
    }
}

fn direction<'a>(
    values: impl Iterator<Item = (&'a Row, Result<f64>)>,
    scale: f64,
) -> Result<Direction> {
    let mut moment = CompensatedSum::default();
    let mut norm = 0_f64;
    for (row, basis) in values {
        let basis = basis?;
        if !basis.is_finite() {
            return Err(invalid("PPML score basis contains a nonfinite value"));
        }
        let score = row.score * basis;
        let weighted_basis = row.root_h * basis;
        if !score.is_finite()
            || !weighted_basis.is_finite()
            || basis != 0. && (weighted_basis == 0. || score == 0. && row.score != 0.)
        {
            return Err(numerical("PPML score direction is not representable"));
        }
        moment.add(score);
        norm = norm.hypot(weighted_basis);
    }
    let absolute = moment.total().abs();
    if !absolute.is_finite() || !norm.is_finite() {
        return Err(numerical("PPML score direction norm is not representable"));
    }
    let relative = if norm > 0. {
        scale_ratio(absolute, 1., norm, scale)
    } else if absolute == 0. {
        0.
    } else {
        return Err(numerical("nonzero PPML score has a zero basis norm"));
    };
    if !relative.is_finite() {
        return Err(numerical("normalized PPML score is not representable"));
    }
    Ok(Direction { absolute, relative })
}
fn summarize(
    directions: Vec<Option<Result<Direction>>>,
    square: &mut CompensatedSum,
) -> Result<ScoreNorms> {
    let mut norms = ScoreNorms {
        directions: directions.len(),
        ..ScoreNorms::default()
    };
    for result in directions {
        let value = result.expect("visited score direction")?;
        norms.absolute_max = norms.absolute_max.max(value.absolute);
        norms.relative_max = norms.relative_max.max(value.relative);
        square.add(value.relative * value.relative);
    }
    Ok(norms)
}
