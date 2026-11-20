//! A bounded alternative for prepared models whose boundary-square tableau
//! cannot fit. Both outcomes require an independent raw-design proof.
use super::{combine_basis, compact_tableau, exact_terminal, reserve};
use super::{fallible_collect, no_separation_certificate, nullspace_basis};
use super::{fallible_filled, verify_indexed_farkas_certificate};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result};
use crate::ppml::ExactNullspace;
use crate::ppml_separation::{
    DetectionProposal, MethodStatus, SeparationCertificate, SeparationCertificateProof,
    SeparationMethod,
};

pub(crate) enum CompactModel {
    Direction {
        coefficients: Vec<i64>,
        iterations: usize,
    },
    Terminal(DetectionProposal),
}

mod positive_rows;
mod projection;

const MAX_WORK: usize = 128 << 20;

pub(crate) struct BinaryDesign {
    offsets: Vec<usize>,
    indices: Vec<u32>,
    columns: usize,
}

impl BinaryDesign {
    pub(crate) fn from_rows(
        rows: usize,
        columns: usize,
        limit: usize,
        mut read: impl FnMut(usize, &mut [f64]) -> Result<()>,
    ) -> Result<Option<Self>> {
        if u32::try_from(columns).is_err()
            || rows as u128 > (1u128 << 53)
            || rows.saturating_mul(columns).saturating_mul(2) > MAX_WORK
            || rows.saturating_add(1).saturating_add(columns) > limit
        {
            return Ok(None);
        }
        let mut offsets = fallible_filled(rows + 1, 0, "unable to reserve compact binary rows")?;
        let mut row = fallible_filled(columns, 0., "unable to reserve compact binary input row")?;
        let max_ones = (limit - offsets.len() - row.len()).saturating_mul(2);
        for i in 0..rows {
            crate::interrupt::row_checkpoint(i)?;
            read(i, &mut row)?;
            let mut count = offsets[i];
            for &x in &row {
                if x != 0. && x != 1. {
                    return Ok(None);
                }
                count += usize::from(x == 1.);
            }
            if count > max_ones {
                return Ok(None);
            }
            offsets[i + 1] = count;
        }
        let mut indices = Vec::new();
        reserve(
            &mut indices,
            offsets[rows],
            "unable to reserve compact binary indices",
        )?;
        for i in 0..rows {
            crate::interrupt::row_checkpoint(i)?;
            read(i, &mut row)?;
            indices.extend(
                row.iter()
                    .enumerate()
                    .filter_map(|(j, &x)| (x == 1.).then_some(j as u32)),
            );
        }
        Ok(Some(Self {
            offsets,
            indices,
            columns,
        }))
    }

    pub(crate) fn ncols(&self) -> usize {
        self.columns
    }

    pub(crate) fn row(&self, row: usize) -> &[u32] {
        &self.indices[self.offsets[row]..self.offsets[row + 1]]
    }

    fn cells(&self) -> usize {
        self.offsets.capacity() + self.indices.capacity().div_ceil(2)
    }
}

pub(crate) fn compact_model_admitted(
    rows: usize,
    positive: usize,
    width: usize,
    limit: usize,
) -> bool {
    width > 0
        && positive > 0
        && positive < rows
        && width.saturating_mul(width).saturating_add(rows) <= limit
}

fn dense_positive_admitted(rows: usize, positive: usize, width: usize, limit: usize) -> bool {
    let positive_cells = positive.saturating_mul(width);
    let elimination = positive_cells.saturating_mul(positive.min(width));
    let scratch = positive_cells
        .saturating_mul(2)
        .saturating_add(width.saturating_mul(width.saturating_add(8)))
        .saturating_add(rows.saturating_mul(3));
    elimination <= MAX_WORK && scratch <= limit
}

pub(crate) fn compact_model(
    outcome: &[f64],
    design: &BinaryDesign,
    tolerance: f64,
    certificate_tolerance: f64,
    max_iterations: usize,
    max_cells: usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<CompactModel>> {
    let width = design.ncols();
    let positive_count = outcome.iter().filter(|&&y| y > 0.).count();
    let Some(available) = max_cells.checked_sub(design.cells()) else {
        return Ok(None);
    };
    if !compact_model_admitted(outcome.len(), positive_count, width, available) {
        return Ok(None);
    }
    let mut pivots = 0;
    let search = CompactSearch {
        outcome,
        design,
        tolerance,
        certificate_tolerance,
        max_iterations,
        available,
    };
    if dense_positive_admitted(outcome.len(), positive_count, width, available) {
        let positive = fallible_collect(
            outcome
                .iter()
                .enumerate()
                .filter_map(|(i, &y)| (y > 0.).then_some(i)),
            positive_count,
            "unable to reserve compact simplex positive rows",
        )?;
        return search.solve(positive, &mut pivots, interrupt);
    }

    let Some(selected) = positive_rows::select(design, outcome, available, interrupt)? else {
        return Ok(None);
    };
    let rank = selected.len();
    let proposal = search.solve(selected, &mut pivots, interrupt)?;
    if proposal.is_some() || rank == width {
        return Ok(proposal);
    }
    let Some(odd) = positive_rows::select_odd(design, outcome, available, interrupt)? else {
        return Ok(None);
    };
    if odd.len() > rank {
        search.solve(odd, &mut pivots, interrupt)
    } else {
        Ok(None)
    }
}

struct CompactSearch<'a> {
    outcome: &'a [f64],
    design: &'a BinaryDesign,
    tolerance: f64,
    certificate_tolerance: f64,
    max_iterations: usize,
    available: usize,
}

impl CompactSearch<'_> {
    fn solve(
        &self,
        mut positive: Vec<usize>,
        pivots: &mut usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Option<CompactModel>> {
        let Self {
            outcome,
            design,
            tolerance,
            certificate_tolerance,
            max_iterations,
            available,
        } = *self;
        let width = design.ncols();
        if !dense_positive_admitted(outcome.len(), positive.len(), width, available) {
            return Ok(None);
        }
        // The pivot subset only reduces proposal work. A numerical direction or
        // dual still has to balance the full raw design before we can certify it.
        let Some(mut basis) = positive_nullspace(design, &mut positive, tolerance, interrupt)?
        else {
            return Ok(None);
        };
        let boundary_count = outcome.iter().filter(|&&y| y == 0.).count();
        if basis.is_empty() {
            return Ok(exact_terminal::compact_full_positive_rank(
                design, outcome, &positive, MAX_WORK,
            )?
            .then(|| {
                terminal(
                    no_separation_certificate(
                        SeparationCertificateProof::SimplexFullPositiveRank { columns: width },
                    ),
                    *pivots,
                    "finite regressors have full column rank on the positive-outcome sample",
                )
            }));
        }

        let dimension = basis.len();
        let boundary = fallible_collect(
            outcome
                .iter()
                .enumerate()
                .filter_map(|(i, &y)| (y == 0.).then_some(i)),
            boundary_count,
            "unable to reserve compact simplex zero rows",
        )?;
        let fixed_cells = boundary
            .capacity()
            .saturating_add(positive.capacity())
            .saturating_add(width);
        let Some(cone_budget) = available.checked_sub(fixed_cells) else {
            return Ok(None);
        };
        let Some(projected) =
            projection::build(design, &boundary, &mut basis, tolerance, cone_budget)?
        else {
            return Ok(None);
        };
        let retained = basis.len();
        let remaining = cone_budget - projected.live_cells;
        if remaining <= boundary_count.saturating_mul(2) {
            return Ok(None); // The numerical dual and its Farkas replay coexist.
        }
        let proposal = projected.propose(
            boundary_count,
            (tolerance, max_iterations),
            pivots,
            remaining,
            interrupt,
        )?;
        if crate::diagnostics::solver_trace_enabled() {
            let branch = match &proposal {
                Some(compact_tableau::Proposal::Direction { .. }) => "direction",
                Some(compact_tableau::Proposal::PositiveWeights { .. }) => "dual",
                None => "unresolved",
            };
            eprintln!("FEREG_SOLVER_TRACE|phase=compact_tableau|dimension={dimension}|retained={retained}|boundary={boundary_count}|branch={branch}");
        }
        match proposal {
            Some(compact_tableau::Proposal::Direction {
                coordinates,
                iterations,
            }) => {
                drop(projected);
                let coefficients = combine_basis(&basis, &coordinates)?;
                drop(basis);
                let scratch = available.saturating_sub(fixed_cells + retained);
                Ok(lift_ray(
                    design,
                    outcome,
                    &coefficients,
                    &positive,
                    tolerance,
                    scratch,
                )?
                .map(|coefficients| CompactModel::Direction {
                    coefficients,
                    iterations,
                }))
            }
            Some(compact_tableau::Proposal::PositiveWeights {
                weights,
                iterations,
            }) => {
                let dual = fallible_collect(
                    weights.iter().map(|w| -*w).chain(std::iter::once(-1.)),
                    weights.len() + 1,
                    "unable to reserve compact Farkas certificate",
                )?;
                let certificate = verify_indexed_farkas_certificate(
                    &projected.rows,
                    &projected.index,
                    &dual,
                    certificate_tolerance,
                )?;
                drop(dual);
                drop(projected);
                drop(basis);
                let Some(certificate) = certificate else {
                    if crate::diagnostics::solver_trace_enabled() {
                        eprintln!(
                            "FEREG_SOLVER_TRACE|phase=compact_terminal|failed=numerical_farkas"
                        );
                    }
                    return Ok(None);
                };
                let scratch = available.saturating_sub(fixed_cells + weights.len());
                Ok(exact_terminal::compact_no_recession(
                    design, outcome, &boundary, &weights, &positive, MAX_WORK, scratch,
                )?
                .then(|| {
                    terminal(
                    certificate,
                    iterations,
                    "model simplex phase-I infeasibility passed an independent Farkas certificate",
                )
                }))
            }
            None => Ok(None),
        }
    }
}

fn positive_nullspace(
    design: &BinaryDesign,
    positive: &mut Vec<usize>,
    tolerance: f64,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Vec<Vec<f64>>>> {
    let width = design.ncols();
    let mut rows = fallible_filled(
        positive.len() * width,
        0.,
        "unable to reserve compact positive design",
    )?;
    for (i, &origin) in positive.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        for &j in design.row(origin) {
            rows[i * width + j as usize] = 1.;
        }
    }
    let matrix = DenseMatrix::from_row_major(positive.len(), width, rows)?;
    let mut order = fallible_collect(
        0..positive.len(),
        positive.len(),
        "unable to reserve compact pivot order",
    )?;
    let basis = match nullspace_basis(&matrix, &mut order, tolerance, interrupt) {
        Ok(basis) => basis,
        Err(error) if error.code == ErrorCode::NonConvergence => return Ok(None),
        Err(error) => return Err(error),
    };
    let rank = width - basis.len();
    let pivots = fallible_collect(
        order[..rank].iter().map(|&i| positive[i]),
        rank,
        "unable to reserve compact pivot rows",
    )?;
    *positive = pivots;
    Ok(Some(basis))
}

fn terminal(
    certificate: SeparationCertificate,
    iterations: usize,
    note: &'static str,
) -> CompactModel {
    CompactModel::Terminal(DetectionProposal {
        method: SeparationMethod::Simplex,
        dropped: Vec::new(),
        iterations,
        status: MethodStatus::NoSeparation,
        certificate: Some(certificate),
        exact_fe_certificate: None,
        direction: None,
        note: note.into(),
    })
}

/// Recover an integer ray from the proposed support and its binding rows.
/// Neither the support nor numerical rank determines a sample deletion.
fn lift_ray(
    design: &BinaryDesign,
    outcome: &[f64],
    coefficients: &[f64],
    pivots: &[usize],
    tolerance: f64,
    scratch: usize,
) -> Result<Option<Vec<i64>>> {
    let scale = coefficients.iter().map(|v| v.abs()).fold(0., f64::max);
    if !scale.is_finite() || scale == 0. {
        return Ok(None);
    }
    let threshold = tolerance * scale;
    let active = fallible_collect(
        coefficients
            .iter()
            .enumerate()
            .filter_map(|(j, c)| (c.abs() > threshold).then_some(j)),
        coefficients.len(),
        "unable to reserve compact ray support",
    )?;
    let width = coefficients.len();
    let k = active.len();
    if k == 0 || k.saturating_mul(k + 8).saturating_add(3 * width) > scratch {
        return Ok(None);
    }
    let mut result = fallible_filled(width, 0i64, "unable to reserve compact raw ray")?;
    let smallest = active
        .iter()
        .map(|&j| coefficients[j].abs())
        .fold(scale, f64::min);
    for divisor in [1., 2., 3., 4., 6., 8., 12., 16.] {
        if scale / smallest * divisor > (1u64 << 42) as f64 {
            break;
        }
        for &j in &active {
            result[j] = (coefficients[j] / smallest * divisor).round() as i64;
        }
        if verify_ray(design, outcome, &result)? {
            return Ok(Some(result));
        }
    }

    let mut positions = fallible_filled(width, usize::MAX, "unable to reserve compact ray map")?;
    for (i, &column) in active.iter().enumerate() {
        positions[column] = i;
    }
    let mut space = ExactNullspace::new(k)?;
    let mut row = fallible_filled(k, 0., "unable to reserve compact binding row")?;
    let mut work = MAX_WORK;
    for i in pivots.iter().copied().chain(0..outcome.len()) {
        crate::interrupt::row_checkpoint(i)?;
        if space.rank() >= k - 1 {
            break;
        }
        row.fill(0.);
        let mut score = 0.;
        for &column in design.row(i) {
            let j = column as usize;
            if positions[j] != usize::MAX {
                row[positions[j]] = 1.;
                score += coefficients[j];
            }
        }
        if outcome[i] > 0. || score.abs() <= threshold * k as f64 {
            let Some(remaining) = work.checked_sub((space.rank() + 1).saturating_mul(k)) else {
                return Ok(None);
            };
            work = remaining;
            space.insert(&row)?;
        }
    }
    let mut free = fallible_filled(k, 0i64, "unable to reserve compact free coordinates")?;
    for resolution in [65536., 16_777_216., 4_294_967_296.] {
        for (&j, dest) in active.iter().zip(&mut free) {
            *dest = (coefficients[j] / scale * resolution).round() as i64;
        }
        if let Some(lift) = space.reconstruct(&free)? {
            for (&j, value) in active.iter().zip(lift) {
                result[j] = value;
            }
            if verify_ray(design, outcome, &result)? {
                return Ok(Some(result));
            }
        }
    }
    Ok(None)
}

/// Entries have already been checked to be binary. With at most usize::MAX
/// coordinates of type i64, their sum is exactly representable in i128.
fn verify_ray(design: &BinaryDesign, outcome: &[f64], coefficients: &[i64]) -> Result<bool> {
    let mut strict = false;
    for (i, &y) in outcome.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let score: i128 = design
            .row(i)
            .iter()
            .map(|&j| i128::from(coefficients[j as usize]))
            .sum();
        if score > 0 || (y > 0. && score != 0) {
            return Ok(false);
        }
        strict |= score < 0;
    }
    Ok(strict)
}

#[cfg(test)]
mod tests;
