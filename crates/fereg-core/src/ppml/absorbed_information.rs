//! Independent small-model replay of the finite and categorical-dummy PPML
//! likelihood. A re-expanded slope can magnify a rounding error in a near-zero
//! covariance. After an original omission, the remaining design can still be
//! nearly dependent. Rebuild the fitted coefficients and sandwich in guard
//! precision before publishing either representation.
use super::prepared_irls::PreparedPpmlIrlsFit;
use super::prepared_model::PreparedPpmlModelGeneration;
use super::{PpmlCoefficientOutput, PpmlInferenceResult};
use crate::data::WeightKind;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::BorrowedInterceptFeDesign;
use crate::interrupt::row_checkpoint;
use crate::memory::fallible_filled;
use crate::ols::prepared_fit_view::PreparedOlsFitView;
use crate::slope_recovery::absorbed::AbsorbedMap;
mod wide;
use wide::Wide;

const MAX_WIDTH: usize = 64;
const MAX_PRODUCTS: usize = 8 << 20;
fn unverified() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "the original PPML slope coefficients and covariance cannot be independently certified",
    )
}

pub(super) fn workspace_bytes(slots: usize, levels: usize, groups: usize) -> u128 {
    let p = slots.saturating_add(levels).min(MAX_WIDTH) as u128;
    // One full Fisher factor, one finite covariance, several vectors, and
    // the finite one-way scores. Reuse across the bounded Newton updates.
    (2 * p * p + 10 * p + groups as u128 * p) * std::mem::size_of::<Wide>() as u128
}

struct Design<'a> {
    data: &'a PreparedPpmlModelGeneration,
    columns: &'a [usize],
    effects: BorrowedInterceptFeDesign<'a>,
    p: usize,
}
impl Design<'_> {
    fn row(&self, r: usize, x: &mut [Wide]) -> Result<()> {
        row_checkpoint(r)?;
        x.fill(Wide::ZERO);
        for (dest, &c) in x.iter_mut().zip(self.columns) {
            *dest = Wide::from(self.data.fit_view().column_value(c, r)?);
        }
        let mut start = self.columns.len();
        for (j, e) in self.effects.effects().iter().enumerate() {
            let skip = usize::from(j > 0);
            let code = e.codes()[r] as usize;
            if code >= skip {
                x[start + code - skip] = Wide::ONE;
            }
            start += e.n_levels() - skip;
        }
        Ok(())
    }
    fn equations(
        &self,
        b: &[Wide],
        eta: &[f64],
        initial: bool,
        h: &mut [Wide],
        rhs: &mut [Wide],
        x: &mut [Wide],
    ) -> Result<()> {
        h.fill(Wide::ZERO);
        rhs.fill(Wide::ZERO);
        for (r, &fit_eta) in eta.iter().enumerate() {
            self.row(r, x)?;
            let q = Wide::from(self.data.prior_weights()[r]);
            let offset = Wide::from(self.data.offset()[r]);
            let (mass, score) = if initial {
                // A weighted projection of the fitted log means gives an
                // initial feasible coefficient vector in the explicit basis.
                let w = q * Wide::from(fit_eta.exp());
                (w, w * (Wide::from(fit_eta) - offset))
            } else {
                let mu = (dot(b, x) + offset).exp().ok_or_else(unverified)?;
                (q * mu, q * (Wide::from(self.data.outcome()[r]) - mu))
            };
            for i in 0..self.p {
                if x[i] == Wide::ZERO {
                    continue;
                }
                rhs[i] += score * x[i];
                let wx = mass * x[i];
                for j in 0..=i {
                    h[i * self.p + j] += wx * x[j];
                }
            }
        }
        Ok(())
    }
    fn fit(&self, eta: &[f64], h: &mut [Wide], x: &mut [Wide]) -> Result<Vec<Wide>> {
        let p = self.p;
        let mut b = zeros(p)?;
        let mut rhs = zeros(p)?;
        for step in 0..5 {
            self.equations(&b, eta, step == 0, h, &mut rhs, x)?;
            factor(h, p)?;
            solve(h, &mut rhs)?;
            let mut max_delta = 0f64;
            for r in 0..eta.len() {
                self.row(r, x)?;
                max_delta = max_delta.max(dot(x, &rhs).value().abs());
            }
            // Check the residual Newton step in log-mean units. Return the
            // current b and its own final Fisher factor, from the same frame.
            if step > 0 && max_delta < 1e-24 {
                return Ok(b);
            }
            for (v, &d) in b.iter_mut().zip(&rhs) {
                *v += d;
            }
        }
        Err(unverified())
    }
}

pub(super) fn check(
    map: Option<AbsorbedMap>,
    data: &PreparedPpmlModelGeneration,
    fit: &PreparedPpmlIrlsFit,
    coefficients: &PpmlCoefficientOutput,
    inference: &PpmlInferenceResult,
) -> Result<()> {
    check_frame(map, data, fit, coefficients, inference, false)
}

pub(super) fn check_alternate(
    data: &PreparedPpmlModelGeneration,
    fit: &PreparedPpmlIrlsFit,
    coefficients: &PpmlCoefficientOutput,
    inference: &PpmlInferenceResult,
) -> Result<()> {
    for r in 0..fit.mean().len() {
        row_checkpoint(r)?;
        let mut xb = 0.;
        for (j, col) in data.model().columns.iter().enumerate() {
            let slot = col.coefficient_slot.get() as usize - 1;
            xb += data.fit_view().column_value(j, r)? * coefficients.values()[slot];
        }
        let mu = ((xb + data.offset()[r]) + fit.linear().absorbed_predictor()[r]).exp();
        if !mu.is_finite() || (fit.mean()[r] - mu).abs() > 1e-8 * fit.mean()[r] {
            return Err(unverified());
        }
    }
    check_frame(None, data, fit, coefficients, inference, true)
}

fn check_frame(
    map: Option<AbsorbedMap>,
    data: &PreparedPpmlModelGeneration,
    fit: &PreparedPpmlIrlsFit,
    coefficients: &PpmlCoefficientOutput,
    inference: &PpmlInferenceResult,
    exact_exchange: bool,
) -> Result<()> {
    let columns = fit.linear().basis().kept_generated_columns();
    if data.fit_view().has_absorbed_slope() || data.fit_view().effect_specs().len() > MAX_WIDTH {
        return Err(unverified());
    }
    let effects = data
        .fit_view()
        .fit_borrowed_intercept_map_design()?
        .ok_or_else(unverified)?;
    let p = columns
        .len()
        .checked_add(
            effects
                .total_term_slots()
                .saturating_sub(effects.effects().len().saturating_sub(1)),
        )
        .ok_or_else(unverified)?;
    if p > MAX_WIDTH || p.saturating_mul(p).saturating_mul(fit.mean().len()) > MAX_PRODUCTS {
        return Err(unverified());
    }
    let design = Design {
        data,
        columns,
        effects,
        p,
    };
    let k = columns.len();
    if k == 0 || coefficients.kept_full_slots().len() != k {
        return Err(unverified());
    }
    let mut h = zeros(p * p)?;
    let mut x = zeros(p)?;
    let b = design.fit(fit.eta(), &mut h, &mut x)?;
    if map.is_none() {
        // For a separately proven exact exchange the original Fisher can be
        // weak while the reported sandwich is well conditioned. Bound the
        // accumulated guard-precision error before using the refined solution.
        let limit = if exact_exchange {
            (1e-15 / (f64::EPSILON.powi(2) * (p as f64).powi(3) * fit.mean().len() as f64))
                .min(1e10)
        } else {
            1e4
        };
        check_condition(&h, k, &mut x, limit)?;
    }
    let public_scale = |s| map.filter(|m| m.slot == s).map_or(1., |m| m.scale);
    for (i, &s) in coefficients.kept_full_slots().iter().enumerate() {
        let est = coefficients.values()[s] * public_scale(s);
        let oracle = (b[i] * Wide::from(public_scale(s))).value();
        certify(est, oracle, 1e-7)?;
    }
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
    let mut sums = zeros(groups.checked_mul(k).ok_or_else(unverified)?)?;
    let mut covariance = zeros(k * k)?;
    for r in 0..fit.mean().len() {
        design.row(r, &mut x)?;
        let mu = (dot(&b, &x) + Wide::from(data.offset()[r]))
            .exp()
            .ok_or_else(unverified)?;
        if (fit.mean()[r] - mu.value()).abs() > 1e-8 * mu.value() {
            return Err(unverified());
        }
        let e = Wide::from(data.outcome()[r]) - mu;
        let q = Wide::from(data.prior_weights()[r]);
        solve(&h, &mut x)?;
        if let Some(c) = codes {
            let start = c[r] as usize * k;
            let group = sums.get_mut(start..start + k).ok_or_else(unverified)?;
            for (dest, &v) in group.iter_mut().zip(&x) {
                *dest += (q * e) * v;
            }
        } else {
            let q2 = if data.weight_report().kind == Some(WeightKind::Frequency) {
                q
            } else {
                q * q
            };
            outer(&mut covariance, &x[..k], q2 * e * e);
        }
    }
    for (r, g) in sums.chunks(k).enumerate() {
        row_checkpoint(r)?;
        outer(&mut covariance, g, Wide::ONE);
    }
    let slots = coefficients.kept_full_slots();
    let width = coefficients.values().len();
    let mut standardized_error = 0.;
    for (i, &s) in slots.iter().enumerate() {
        for (j, &t) in slots[..=i].iter().enumerate() {
            let a = public_scale(s);
            let d = public_scale(t);
            let reference = (covariance[i * k + j]
                * Wide::from(inference.vce.correction)
                * Wide::from(a)
                * Wide::from(d))
            .value();
            let posted = (inference.vce.covariance[s * width + t] * a) * d;
            certify(posted, reference, 1e-6)?;
            if map.is_none() {
                let scale = covariance[i * k + i].value().sqrt()
                    * covariance[j * k + j].value().sqrt()
                    * inference.vce.correction;
                let multiplicity = if i == j { 1. } else { 2. };
                standardized_error += multiplicity * (posted - reference).abs() / scale;
            }
        }
    }
    drop(h);
    if map.is_none() {
        if exact_exchange {
            test_original_wald(&b, &covariance, k, slots, inference)?;
        }
        let minimum_variance = (0..k)
            .map(|j| covariance[j * k + j].value() * inference.vce.correction)
            .fold(f64::INFINITY, f64::min);
        factor(&mut covariance, k)?;
        let inflation = check_condition(&covariance, k, &mut x[..k], 1e4)?;
        if exact_exchange && minimum_variance / inflation < 1e-19 {
            return Err(unverified());
        }
        // For every contrast t, |t_i| <= sqrt((V^-1)_ii * t' V t).
        // Applying that inequality to each error cell bounds the relative
        // quadratic-form error for every original-unit linear contrast.
        let error = inflation * standardized_error;
        if !error.is_finite() || error > 1e-6 {
            return Err(unverified());
        }
    }
    Ok(())
}

fn test_original_wald(
    b: &[Wide],
    covariance: &[Wide],
    k: usize,
    slots: &[usize],
    inference: &PpmlInferenceResult,
) -> Result<()> {
    let w = &inference.wald;
    let p = w.tested_columns.len();
    if p == 0 || w.reference.is_some() || w.degrees_of_freedom != p {
        return Err(unverified());
    }
    let reported = w.statistic.ok_or_else(unverified)?;
    let mut a = zeros(p * p)?;
    let mut v = zeros(p)?;
    for (i, &s) in w.tested_columns.iter().enumerate() {
        let u = slots.iter().position(|&x| x == s).ok_or_else(unverified)?;
        v[i] = b[u];
        for (j, &t) in w.tested_columns[..=i].iter().enumerate() {
            let w = slots.iter().position(|&x| x == t).ok_or_else(unverified)?;
            a[i * p + j] =
                covariance[u.max(w) * k + u.min(w)] * Wide::from(inference.vce.correction);
        }
    }
    factor(&mut a, p)?;
    let mut rhs =
        crate::memory::fallible_copy(&v, "unable to reserve alternate model Wald restrictions")?;
    solve(&a, &mut rhs)?;
    certify(reported, dot(&v, &rhs).value(), 1e-6)
}

fn certify(posted: f64, reference: f64, relative: f64) -> Result<()> {
    if !reference.is_finite()
        || !posted.is_finite()
        || (posted - reference).abs() > 1e-10 + relative * reference.abs()
    {
        return Err(unverified());
    }
    Ok(())
}

// Large variance inflation in another retained linear combination can survive
// the original omission. An elementwise accurate public matrix then need not
// encode stable contrasts. Certify only well-conditioned correlation matrices.
fn check_condition(ld: &[Wide], k: usize, x: &mut [Wide], limit: f64) -> Result<f64> {
    let p = x.len();
    let mut largest = 1f64;
    for i in 0..k {
        let mut diagonal = ld[i * p + i];
        for j in 0..i {
            diagonal += ld[i * p + j] * ld[i * p + j] * ld[j * p + j];
        }
        x.fill(Wide::ZERO);
        x[i] = Wide::ONE;
        solve(ld, x)?;
        let inflation = (diagonal * x[i]).value();
        if !inflation.is_finite() || inflation > limit {
            return Err(unverified());
        }
        largest = largest.max(inflation);
    }
    Ok(largest)
}

fn zeros(n: usize) -> Result<Vec<Wide>> {
    fallible_filled(
        n,
        Wide::ZERO,
        "unable to reserve original-unit Poisson information replay",
    )
}
fn dot(x: &[Wide], y: &[Wide]) -> Wide {
    x.iter().zip(y).fold(Wide::ZERO, |s, (&a, &b)| s + a * b)
}
fn outer(c: &mut [Wide], x: &[Wide], weight: Wide) {
    for (i, &a) in x.iter().enumerate() {
        for (j, &b) in x[..=i].iter().enumerate() {
            c[i * x.len() + j] += (weight * a) * b;
        }
    }
}

/// LDL' factorization of the identified explicit dummy design. A gauge not
/// identified after dropping the redundant intercepts cannot certify a fit.
fn factor(h: &mut [Wide], p: usize) -> Result<()> {
    for i in 0..p {
        let scale = h[i * p + i].value();
        for j in 0..=i {
            let mut v = h[i * p + j];
            for t in 0..j {
                v -= h[i * p + t] * h[t * p + t] * h[j * p + t];
            }
            h[i * p + j] = if i == j {
                if !v.value().is_finite() || v.value() <= scale.abs() * 1e-18 {
                    return Err(unverified());
                }
                v
            } else {
                v / h[j * p + j]
            };
        }
    }
    Ok(())
}
fn solve(h: &[Wide], v: &mut [Wide]) -> Result<()> {
    let p = v.len();
    for i in 0..p {
        let mut a = v[i];
        for j in 0..i {
            a -= h[i * p + j] * v[j];
        }
        v[i] = a;
    }
    for i in 0..p {
        v[i] = v[i] / h[i * p + i];
    }
    for i in (0..p).rev() {
        let mut a = v[i];
        for j in i + 1..p {
            a -= h[j * p + i] * v[j];
        }
        v[i] = a;
    }
    if v.iter().any(|a| !a.value().is_finite()) {
        return Err(unverified());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn another_weak_combination_cannot_bypass_an_original_omission() {
        // First two columns are independent. The third differs from their sum
        // only by a weak direction. No pair alone shows the dependency.
        let mut h = [1., 0., 0., 0., 1., 0., 1., 1., 2. + 2f64.powi(-30)].map(Wide::from);
        factor(&mut h, 3).unwrap();
        let error = check_condition(&h, 3, &mut [Wide::ZERO; 3], 1e4).unwrap_err();
        assert_eq!(error.code, ErrorCode::NonConvergence);
    }
    #[test]
    fn independent_coordinates_may_have_very_different_units() {
        let mut h = [2f64.powi(-70), 0., 0., 2f64.powi(70)].map(Wide::from);
        factor(&mut h, 2).unwrap();
        assert_eq!(
            check_condition(&h, 2, &mut [Wide::ZERO; 2], 1e4).unwrap(),
            1.
        );
    }
}
