//! A sign claim is untrusted until a fresh projection verifies the model span.
mod cone;
mod projection;
use super::{invalid, Commit, Method, Status, View};
use crate::{
    data::DenseMatrix, error::Result, fe::RhsDiagnostics, spec::Options, threads::ThreadTeam,
    wls::Role,
};

/// Reconstruct the complete fitted model rather than subtracting a nearly
/// equal response and residual, which can erase a small separation direction.
pub(super) fn fitted(
    raw: &DenseMatrix,
    design: &crate::fe::Design<'_>,
    fit: &crate::wls::AbsorbedFit,
    team: &ThreadTeam,
) -> Result<DenseMatrix> {
    if raw.nrows() != design.nobs() || raw.ncols() != fit.finite.coefficients.len() {
        return Err(invalid("separation fitted-model dimensions differ"));
    }
    let mut result = design.multiply(&fit.absorbed_coefficients, team)?;
    for row in 0..raw.nrows() {
        result[(row, 0)] = raw.row_combination(row, &fit.finite.coefficients, result[(row, 0)]);
    }
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controls {
    pub tolerance: f64,
    pub certificate_tolerance: f64,
    pub projection_tolerance: f64,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            tolerance: 1e-5,
            certificate_tolerance: 1e-7,
            projection_tolerance: 1e-10,
        }
    }
}
impl Controls {
    pub fn validate(self) -> Result<()> {
        if [
            self.tolerance,
            self.certificate_tolerance,
            self.projection_tolerance,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0. || *v >= 1.)
        {
            return Err(invalid(
                "separation tolerances must be finite and strictly between zero and one",
            ));
        }
        Ok(())
    }
}

pub struct Model<'a> {
    pub raw: &'a DenseMatrix,
    pub roles: &'a [Role],
    pub options: Options,
    pub controls: Controls,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectionCertificate {
    pub interior_zero_relative: f64,
    pub boundary_violation_relative: f64,
    pub negative_margin_relative: f64,
    pub direction_scale: f64,
    pub positive_maximum: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct DirectionProposal {
    pub generation: usize,
    pub sample_identity: u64,
    pub method: Method,
    pub status: Status,
    pub iterations: usize,
    pub rows: Vec<usize>,
    pub direction: Vec<f64>,
    pub certificate: DirectionCertificate,
}
#[derive(Debug, PartialEq)]
pub struct SpanValidation {
    pub certificate: DirectionCertificate,
    pub span_residual_relative: f64,
    pub projection_diagnostics: Vec<RhsDiagnostics>,
    pub recovery_diagnostics: Vec<RhsDiagnostics>,
    pub setup_diagnostics: Vec<crate::fe::SetupDiagnostics>,
    pub work: crate::wls::Work,
}
#[derive(Debug, PartialEq)]
pub struct NumericalCommit {
    pub commit: Commit,
    pub validation: SpanValidation,
}

impl View<'_> {
    /// Construct a detector claim only; this does not establish membership in the model span.
    pub fn direction_proposal(
        self,
        method: Method,
        iterations: usize,
        direction: Vec<f64>,
        controls: Controls,
    ) -> Result<DirectionProposal> {
        if method == Method::FixedEffects || iterations == 0 {
            return Err(invalid(
                "invalid numerical separation method or iteration count",
            ));
        }
        let (rows, certificate) = cone::claim(self, &direction, controls)?;
        Ok(DirectionProposal {
            generation: self.generation,
            sample_identity: self.numerical_identity(),
            method,
            status: Status::Certified,
            iterations,
            rows,
            direction,
            certificate,
        })
    }

    pub fn certify_direction(
        self,
        proposal: &DirectionProposal,
        model: &Model<'_>,
        team: &ThreadTeam,
    ) -> Result<SpanValidation> {
        if proposal.generation != self.generation
            || proposal.sample_identity != self.numerical_identity()
            || proposal.method == Method::FixedEffects
            || proposal.iterations == 0
            || proposal.status != Status::Certified
        {
            return Err(invalid(
                "stale or inconsistent numerical separation proposal",
            ));
        }
        let (rows, certificate) = cone::claim(self, &proposal.direction, model.controls)?;
        if proposal.rows != rows || proposal.certificate != certificate {
            return Err(invalid(
                "numerical proposal rows or certificate disagree with its direction",
            ));
        }
        projection::certify(self, proposal, model, team)
    }

    pub(super) fn numerical_identity(self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for (&row, &y) in self.sample.retained_rows().iter().zip(&self.sample.outcome) {
            hash = (hash ^ row as u64).wrapping_mul(0x100000001b3);
            hash = (hash ^ y.to_bits()).wrapping_mul(0x100000001b3);
        }
        hash
    }
}

impl Model<'_> {
    pub(super) fn validate(&self, view: View<'_>) -> Result<()> {
        self.controls.validate()?;
        self.options.validate()?;
        if self.raw.nrows() != view.sample.n_active()
            || self.raw.ncols() != self.roles.len()
            || self.raw.as_slice().iter().any(|v| !v.is_finite())
        {
            return Err(invalid(
                "separation model is malformed or not aligned to this generation",
            ));
        }
        Ok(())
    }
    pub(super) fn projection_options(&self) -> Options {
        Options {
            tolerance: self
                .options
                .tolerance
                .min(self.controls.projection_tolerance),
            ..self.options
        }
    }
    pub(super) fn span_options(&self) -> Result<Options> {
        let mut options = self.projection_options();
        // The fresh unit-metric whole-model distance needs a stricter FE score
        // than the weighted rectifier recurrence's registered projection target.
        options.tolerance = crate::ppml::control::Controls::new(&options)?.strict_tolerance();
        Ok(options)
    }
    pub(super) fn linear_policy(&self) -> crate::wls::Policy {
        crate::wls::Policy {
            // An estimator's FE-omission threshold cannot remove a finite
            // cone direction. Shared QR still owns numerical span rank at its
            // unchanged original-column scales; this guard only trims roundoff.
            fe_collinearity: f64::EPSILON.powi(2),
            score_tolerance: crate::wls::Policy::default()
                .score_tolerance
                .min(self.controls.projection_tolerance),
            ..crate::wls::Policy::default()
        }
    }
}
