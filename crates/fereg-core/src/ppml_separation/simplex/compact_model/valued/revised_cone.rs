//! The valued cone through the revised phase I when the plan refuses its row
//! bank at both the full and the retained dimension.
use super::super::super::super::SeparationCertificate;
use super::super::super::{combine_basis, compact_tableau::Proposal, farkas_residual_certificate};
use super::super::revised::{self, Rows};
use super::super::sparse_echelon::Echelon;
use super::super::{fallible_filled, trace_refusal};
use super::{Columns, ExactHooks, ValuedModel, ValuedSearch, Verifiers};
use crate::error::Result;

impl ValuedSearch<'_, '_> {
    /// Phase I without the bank: the basis inverse and the transposed null
    /// basis are the only stored arrays; rows are projected on demand.
    pub(super) fn revised_cone(
        &mut self,
        basis: Vec<Vec<f64>>,
        (boundary, representatives, echelon): (&[usize], &[usize], &Echelon),
        pivots: &mut usize,
        (interrupt, grow): ExactHooks<'_, '_>,
        verify: &mut Verifiers<'_>,
    ) -> Result<Option<ValuedModel>> {
        let (width, m) = (self.design.width, basis.len());
        let tolerance = self.config.projection_tolerance;
        let fixed = boundary
            .len()
            .saturating_add(representatives.len())
            .saturating_add(width);
        let words = revised::workspace_words(boundary.len(), m)
            .and_then(|w| w.checked_add(m.checked_mul(width)?))
            .and_then(|w| w.checked_add(width.checked_mul(2)?))
            .unwrap_or(usize::MAX);
        let needed = fixed.saturating_add(words);
        if m == 0 || !self.require(needed, grow) {
            trace_refusal("valued_revised_cells", needed, self.available);
            return Ok(None);
        }
        let columns = Columns::new(&basis, width)?;
        let (read, terms) = (self.design.read, self.design.row_terms);
        let mut work = self.work.stage;
        let mut project = |i: usize, out: &mut [f64]| {
            columns.project(read, boundary[i], out, (tolerance, terms), &mut work)
        };
        let mut combined = fallible_filled(width, 0., "unable to reserve revised prices")?;
        let mut scores = |y: &[f64], emit: &mut dyn FnMut(usize, f64)| -> Result<bool> {
            columns.combine(y, &mut combined);
            for (n, &row) in boundary.iter().enumerate() {
                crate::interrupt::row_checkpoint(n)?;
                let mut score = 0.;
                read(row, &mut |j, v| {
                    if j < width {
                        score += v * combined[j];
                    }
                })?;
                emit(n, score);
            }
            Ok(true)
        };
        let policy = (
            tolerance,
            self.config.simplex_max_iterations,
            self.work.pivots_per_dimension,
        );
        let mut rows = Rows {
            count: boundary.len(),
            dimension: m,
            project: &mut project,
            scores: &mut scores,
        };
        let proposal = revised::propose(&mut rows, policy, pivots, interrupt)?;
        drop(columns);
        if crate::diagnostics::solver_trace_enabled() {
            let branch = super::super::tableau_branch(&proposal);
            eprintln!(
                "FEREG_SOLVER_TRACE|phase=valued_revised|dimension={m}|boundary={}|pivots={pivots}|words={words}|branch={branch}",
                boundary.len()
            );
        }
        match proposal {
            Some(Proposal::Direction {
                coordinates,
                iterations,
            }) => {
                let coefficients = combine_basis(&basis, &coordinates)?;
                drop(basis);
                let sets = (coefficients, representatives, iterations);
                self.direction_model(sets, (interrupt, grow), verify)
            }
            Some(Proposal::PositiveWeights {
                weights,
                iterations,
            }) => {
                let Some(certificate) = self.streamed_certificate(&basis, boundary, &weights)?
                else {
                    trace_refusal("valued_numerical_farkas", boundary.len(), 0);
                    return Ok(None);
                };
                drop(basis);
                let sets = (boundary, &weights[..], representatives);
                let target = representatives.len() + m;
                let terminal = self.exact_terminal(
                    sets,
                    target,
                    (certificate, iterations),
                    (&mut *interrupt, &mut *grow),
                    verify,
                )?;
                if let Some(model) = terminal {
                    return Ok(Some(ValuedModel::Terminal(model)));
                }
                self.dual(boundary, &weights, representatives, echelon, grow)
            }
            None => Ok(None),
        }
    }

    /// The projected Farkas residual of positive weights, streamed through
    /// the raw rows as `N' (sum_i w_i x_i)`.
    fn streamed_certificate(
        &self,
        basis: &[Vec<f64>],
        boundary: &[usize],
        weights: &[f64],
    ) -> Result<Option<SeparationCertificate>> {
        let width = self.design.width;
        let mut total = fallible_filled(width, 0., "unable to reserve revised residual")?;
        for (n, (&row, &w)) in boundary.iter().zip(weights).enumerate() {
            crate::interrupt::row_checkpoint(n)?;
            (self.design.read)(row, &mut |j, v| {
                if j < width {
                    total[j] += w * v;
                }
            })?;
        }
        let stationarity = basis
            .iter()
            .map(|vector| {
                vector
                    .iter()
                    .zip(&total)
                    .map(|(&a, &b)| a * b)
                    .sum::<f64>()
                    .abs()
            })
            .fold(0., f64::max);
        let scale = weights.iter().copied().fold(1., f64::max);
        if !(stationarity.is_finite() && scale.is_finite()) {
            return Ok(None);
        }
        Ok(farkas_residual_certificate(
            boundary.len(),
            basis.len(),
            scale,
            -1.,
            stationarity,
            0.,
            self.config.certificate_tolerance,
        ))
    }
}
