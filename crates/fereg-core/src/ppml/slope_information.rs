//! A large inverse shear can magnify final-Hessian roundoff into original
//! covariance entries, even when the covariance mapping itself is lossless.
use super::family::CompensatedSum;
use super::prepared_irls::PreparedPpmlIrlsFit;
use super::prepared_model::PreparedPpmlModelGeneration;
use super::PpmlInferenceDesign;
use super::{PpmlCoefficientOutput, PpmlInferenceResult};
use crate::data::WeightKind;
use crate::error::{ErrorCode, FeregError, Result};
use crate::interrupt::row_checkpoint;
use crate::memory::fallible_filled;
use crate::slope_recovery::mapping::PairMap;

fn unresolved() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "the original-unit covariance is sensitive to rounding of the final Poisson information",
    )
}

pub(super) fn check(
    map: PairMap,
    data: &PreparedPpmlModelGeneration,
    fit: &PreparedPpmlIrlsFit,
    coefficients: &PpmlCoefficientOutput,
    inference: &PpmlInferenceResult,
) -> Result<()> {
    let slots = coefficients.kept_full_slots();
    let k = slots.len();
    let width = coefficients.values().len();
    let stable = &inference.vce.covariance;
    let bread = fit.linear().identified_bread();
    let target = slots
        .iter()
        .position(|&j| j == map.target)
        .ok_or_else(unresolved)?;
    let source = slots
        .iter()
        .position(|&j| j == map.source)
        .ok_or_else(unresolved)?;
    // Screen in the Fisher metric. A coefficient has units sqrt(B_jj);
    // stable score variability determines the scale of covariance uncertainty.
    let mut score_scale = 0.0;
    for (j, &slot) in slots.iter().enumerate() {
        let ratio = stable[slot * width + slot] / bread[j * k + j];
        if !ratio.is_finite() || ratio < 0.0 {
            return Err(unresolved());
        }
        score_scale += ratio.sqrt();
    }
    let mapped_scale = |j: usize| {
        let weak = map.scale.abs() * bread[target * k + target].sqrt();
        if j == target {
            weak
        } else if j == source {
            bread[source * k + source].sqrt() + map.multiple.abs() * weak
        } else {
            bread[j * k + j].sqrt()
        }
    };
    let mut inspect = false;
    for i in [source, target] {
        for (j, &slot) in slots.iter().enumerate() {
            let variance = |row: usize| {
                map.projected_covariance(stable, width, row, row)
                    .abs()
                    .sqrt()
            };
            let value = map.projected_covariance(stable, width, slots[i], slot);
            let sensitivity = score_scale
                * (mapped_scale(i) * variance(slot) + mapped_scale(j) * variance(slots[i]));
            inspect |= !sensitivity.is_finite()
                || (32.0 * k as f64 * f64::EPSILON) * sensitivity > 1e-10 + 1e-6 * value.abs();
        }
    }
    let profiled = !data.fit_view().effect_specs().is_empty();
    // Holding the within transformation fixed cannot adjudicate material
    // sensitivity to a change in the profiled FE projection.
    if inspect && profiled || k.saturating_mul(k).saturating_mul(fit.mean().len()) > 32 << 20 {
        return Err(unresolved());
    }
    let frame = Frame {
        data,
        fit,
        k,
        columns: fit.linear().basis().kept_generated_columns(),
        profiled,
    };
    let mut x = fallible_filled(k, 0.0, "unable to reserve Poisson information row")?;
    let delta = frame.newton_step(bread, &mut x)?;
    let refreshed = frame.refreshed_inverse(bread, &delta, &mut x)?;
    let mut covariance = fallible_filled(
        k * k,
        CompensatedSum::default(),
        "unable to reserve Poisson sensitivity covariance",
    )?;
    let groups = inference
        .vce
        .cluster_dimension_groups
        .first()
        .copied()
        .unwrap_or(0);
    let codes = if groups == 0 {
        None
    } else {
        Some(data.cluster_codes(0)?)
    };
    let mut sums = fallible_filled(
        groups.checked_mul(k).ok_or_else(unresolved)?,
        CompensatedSum::default(),
        "unable to reserve one-way Poisson influence sums",
    )?;
    let mut influence = fallible_filled(k, 0.0, "unable to reserve Poisson sensitivity influence")?;
    frame.visit(&mut x, |row, x| {
        let mu = fit.mean()[row];
        let r = (data.outcome()[row] - mu) - mu * dot(x, &delta).exp_m1();
        let q = data.prior_weights()[row];
        let multiplier =
            if codes.is_none() && data.weight_report().kind == Some(WeightKind::Frequency) {
                q.sqrt() * r
            } else {
                q * r
            };
        for (i, b) in refreshed.chunks(k).enumerate() {
            influence[i] = dot(b, x);
        }
        influence[target] *= map.scale;
        influence[source] = (-map.multiple).mul_add(influence[target], influence[source]);
        if let Some(codes) = codes {
            let start = codes[row] as usize * k;
            let group = sums.get_mut(start..start + k).ok_or_else(unresolved)?;
            for (s, &v) in group.iter_mut().zip(&influence) {
                s.add(v * multiplier);
            }
        } else {
            for v in &mut influence {
                *v *= multiplier;
            }
            accumulate_outer(&mut covariance, &influence);
        }
        Ok(())
    })?;
    for (group_index, group) in sums.chunks(k).enumerate() {
        row_checkpoint(group_index)?;
        for (v, &s) in influence.iter_mut().zip(group) {
            *v = s.total();
        }
        accumulate_outer(&mut covariance, &influence);
    }
    for i in 0..k {
        for j in 0..=i {
            let reference = covariance[i * k + j].total() * inference.vce.correction;
            let actual = map.projected_covariance(stable, width, slots[i], slots[j]);
            if !reference.is_finite() || (actual - reference).abs() > 1e-10 + 1e-6 * reference.abs()
            {
                return Err(unresolved());
            }
        }
    }
    Ok(())
}

fn accumulate_outer(covariance: &mut [CompensatedSum], influence: &[f64]) {
    let k = influence.len();
    for i in 0..k {
        for j in 0..=i {
            covariance[i * k + j].add(influence[i] * influence[j]);
        }
    }
}

fn dot(x: &[f64], y: &[f64]) -> f64 {
    let mut s = CompensatedSum::default();
    for (&a, &b) in x.iter().zip(y) {
        let p = a * b;
        s.add(p);
        s.add(a.mul_add(b, -p));
    }
    s.total()
}

struct Frame<'a> {
    data: &'a PreparedPpmlModelGeneration,
    fit: &'a PreparedPpmlIrlsFit,
    k: usize,
    columns: &'a [usize],
    profiled: bool,
}
impl Frame<'_> {
    fn visit(
        &self,
        x: &mut [f64],
        mut callback: impl FnMut(usize, &[f64]) -> Result<()>,
    ) -> Result<()> {
        let rows = 0..self.fit.mean().len();
        if self.profiled {
            return self
                .fit
                .linear()
                .finite_within_design()
                .visit_range(rows, x, |r, v| {
                    row_checkpoint(r)?;
                    callback(r, v)
                });
        }
        for row in rows {
            row_checkpoint(row)?;
            for (v, &column) in x.iter_mut().zip(self.columns) {
                *v = self.data.fit_view().column_value(column, row)?;
            }
            callback(row, x)?;
        }
        Ok(())
    }
    /// Correct the mean by the unresolved score. Keeping delta-mu separate
    /// from mu preserves sub-ulp movements in an almost orthogonal Hessian.
    fn newton_step(&self, bread: &[f64], x: &mut [f64]) -> Result<Vec<f64>> {
        let mut score = fallible_filled(
            self.k,
            CompensatedSum::default(),
            "unable to reserve Poisson score replay",
        )?;
        self.visit(x, |row, x| {
            let r = self.data.outcome()[row] - self.fit.mean()[row];
            let q = self.data.prior_weights()[row];
            for (s, a) in score.iter_mut().zip(x.iter()) {
                let qr = q * r;
                s.add(qr * a);
                s.add(qr.mul_add(*a, -(qr * a)));
                s.add(q.mul_add(r, -qr) * a);
            }
            Ok(())
        })?;
        let mut delta = fallible_filled(self.k, 0.0, "unable to reserve Poisson score update")?;
        for (i, b) in bread.chunks(self.k).enumerate() {
            let mut s = CompensatedSum::default();
            for (&a, v) in b.iter().zip(&score) {
                s.add(a * v.total());
            }
            delta[i] = s.total();
        }
        Ok(delta)
    }
    fn refreshed_inverse(&self, bread: &[f64], delta: &[f64], x: &mut [f64]) -> Result<Vec<f64>> {
        let k = self.k;
        let mut h = fallible_filled(
            k * k,
            CompensatedSum::default(),
            "unable to reserve Poisson Fisher replay",
        )?;
        self.visit(x, |row, x| {
            let w = self.data.prior_weights()[row] * self.fit.mean()[row];
            let change = dot(x, delta).exp_m1();
            if !change.is_finite() || change.abs() > 1e-6 {
                return Err(unresolved());
            }
            for i in 0..k {
                for j in 0..k {
                    let p = x[i] * x[j];
                    let term = w * p;
                    let sum = &mut h[i * k + j];
                    sum.add(term);
                    sum.add(w.mul_add(p, -term));
                    sum.add(w * x[i].mul_add(x[j], -p));
                    sum.add(term * change);
                }
            }
            Ok(())
        })?;
        let mut error = fallible_filled(k * k, 0.0, "unable to reserve Poisson inverse residual")?;
        // One Newton-Schulz inverse correction: B' = B + B (I − H' B).
        for i in 0..k {
            for j in 0..k {
                let mut s = CompensatedSum::default();
                s.add(f64::from(i == j));
                for t in 0..k {
                    let b = bread[t * k + j];
                    for part in h[i * k + t].components() {
                        let p = -part * b;
                        s.add(p);
                        s.add((-part).mul_add(b, -p));
                    }
                }
                error[i * k + j] = s.total();
            }
        }
        drop(h);
        let mut inverse = fallible_filled(k * k, 0.0, "unable to reserve refined Poisson inverse")?;
        for i in 0..k {
            for j in 0..k {
                let mut s = CompensatedSum::default();
                s.add(bread[i * k + j]);
                for t in 0..k {
                    s.add(bread[i * k + t] * error[t * k + j]);
                }
                inverse[i * k + j] = s.total();
            }
        }
        Ok(inverse)
    }
}
