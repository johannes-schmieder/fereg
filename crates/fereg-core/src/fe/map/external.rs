//! An external projection of the generic owner's design (OLS Auto on owned
//! slope designs: fedemean's full system), certified by the owner's
//! independent certificate, with the owner's MAP as the continuation.

use super::*;

/// One external projection of [`ReusableMapAbsorber::absorb_external`].
pub(crate) enum ExternalProjection {
    /// The values hold the residuals; the report names the route and work.
    Projected(SolveReport),
    /// The route declined after `iterations` (the values are untouched or
    /// partially residualized with the same projection).
    Declined {
        iterations: usize,
        reason: &'static str,
        fallback: crate::fe::FeFallbackReason,
    },
    /// The route does not support the design; the values are untouched.
    Ineligible,
}

/// The result of [`ReusableMapAbsorber::absorb_external`] (moved once per
/// projection; boxing it would allocate on every call).
#[allow(clippy::large_enum_variant)]
pub(crate) enum External {
    /// Certified residuals, and whether MAP finished them (the fallback).
    Done(CertifiedBorrowedMap, bool),
    /// The external route was not eligible; the untouched values are
    /// returned for the owner's own route.
    Ineligible(DenseMatrix),
}

/// Fold a correction's work and route into the first projection's report.
fn merge_external(first: &mut SolveReport, second: &SolveReport) {
    first.iterations += second.iterations;
    first.setup_seconds += second.setup_seconds;
    first.solve_seconds += second.solve_seconds;
    first.solver_workspace_peak = first
        .solver_workspace_peak
        .max(second.solver_workspace_peak);
    first.merge_route(second, false);
}

impl ReusableMapAbsorber {
    /// Project with an external solver of this owner's design (OLS Auto:
    /// fedemean's full system) and certify its residuals with this owner's
    /// independent certificate against the input's reference norms. `solve`
    /// residualizes the values in place at a tolerance, given the iterations
    /// already spent. A certificate miss earns one correction at a tenth of
    /// the tolerance; a decline, or a correction that still misses, continues
    /// with symmetric MAP from the current values (the same projection) under
    /// the same reference norms, reported as the fallback.
    pub(crate) fn absorb_external(
        &mut self,
        weights: &[f64],
        mut values: DenseMatrix,
        config: MapConfig,
        generation: Option<MapGeometryGeneration>,
        solve: &mut dyn FnMut(
            &FeDesign,
            &mut DenseMatrix,
            f64,
            usize,
        ) -> Result<ExternalProjection>,
    ) -> Result<External> {
        let previous = self
            .certificate_geometry
            .as_mut()
            .and_then(|g| g.generation.take());
        let mut geometry_valid = generation.is_some() && previous == generation;
        validate_config(config)?;
        let rhs = values.ncols();
        if values.nrows() != self.design.nobs()
            || weights.len() != values.nrows()
            || rhs == 0
            || rhs > self.max_rhs
            || self.design.effects().is_empty()
            || self.coefficient_solver.is_some()
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "external projection inputs exceed their fixed session dimensions",
            ));
        }
        if weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
            || values.as_slice().iter().any(|value| !value.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "external projection inputs must be finite with positive weights",
            ));
        }
        let setup_started = Instant::now();
        reusable_weighted_norms(
            &values,
            weights,
            &mut self.reference_norms[..rhs],
            &mut self.norm_scales[..rhs],
            &mut self.norm_sums[..rhs],
            &mut self.certificate_columns,
        )?;
        self.residual_norms[..rhs].copy_from_slice(&self.reference_norms[..rhs]);
        let setup_seconds = setup_started.elapsed().as_secs_f64();
        let solve_started = Instant::now();
        let mut spent = 0_usize;
        let mut note = None;
        let mut fallback = crate::fe::FeFallbackReason::Uncertified;
        let mut current = None;
        for tolerance in [config.tolerance, config.tolerance / 10.0] {
            match solve(&self.design, &mut values, tolerance, spent)? {
                ExternalProjection::Projected(report) => {
                    spent += report.iterations;
                    let certifying = Instant::now();
                    let certificate =
                        self.certify(&values, weights, rhs, config.tolerance, &mut geometry_valid)?;
                    let mut report = match current.take() {
                        Some(mut first) => {
                            merge_external(&mut first, &report);
                            first.warnings.push(
                                "the full-system certificate missed fereg's independent certificate; corrected at a tenth of the tolerance".to_owned(),
                            );
                            first
                        }
                        None => report,
                    };
                    report.certification_seconds += certifying.elapsed().as_secs_f64();
                    if certificate.passed {
                        report.certificate = certificate;
                        report.stop_reason = SolveStopReason::Certified;
                        report.setup_seconds += setup_seconds;
                        let result = CertifiedBorrowedMap::from_certified_parts(values, report)?;
                        if let Some(geometry) = self.certificate_geometry.as_mut() {
                            geometry.generation = generation;
                        }
                        return Ok(External::Done(result, false));
                    }
                    note = Some(format!(
                        "the full system's residuals missed the independent certificate ({:.3e}) after {spent} iterations; MAP continued",
                        certificate.relative_infinity_norm
                    ));
                    current = Some(report);
                }
                ExternalProjection::Declined {
                    iterations,
                    reason,
                    fallback: why,
                } => {
                    spent += iterations;
                    fallback = why;
                    note = Some(format!(
                        "the full system declined after {spent} iterations ({reason}); MAP continued"
                    ));
                    break;
                }
                // Refused before any data was read: the owner's own route
                // runs on the untouched values, under its own authority.
                ExternalProjection::Ineligible if current.is_none() => {
                    if let Some(geometry) = self.certificate_geometry.as_mut() {
                        geometry.generation = previous;
                    }
                    return Ok(External::Ineligible(values));
                }
                ExternalProjection::Ineligible => {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "the external route refused its own correction",
                    ))
                }
            }
        }
        let initial = self.certify(&values, weights, rhs, config.tolerance, &mut geometry_valid)?;
        let result = self.iterate_map(
            values,
            weights,
            rhs,
            config,
            (&mut geometry_valid, initial),
            (setup_seconds, solve_started),
        )?;
        if let Some(geometry) = self.certificate_geometry.as_mut() {
            geometry.generation = generation;
        }
        let (values, mut report) = result.into_parts();
        report.iterations += spent;
        report.note_fallback(fallback);
        if let Some(note) = note {
            report.warnings.push(note);
        }
        Ok(External::Done(
            CertifiedBorrowedMap::from_certified_parts(values, report)?,
            true,
        ))
    }
}
