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

mod grouped;
pub(crate) use grouped::CompactInput;
mod positive_rows;
mod projection;

const MAX_WORK: usize = 128 << 20;

pub(crate) struct BinaryDesign {
    offsets: Vec<usize>,
    indices: Vec<u32>,
    columns: usize,
}

impl BinaryDesign {
    #[cfg(test)]
    pub(crate) fn from_rows(
        rows: usize,
        columns: usize,
        limit: usize,
        mut read: impl FnMut(usize, &mut [f64]) -> Result<()>,
    ) -> Result<Option<Self>> {
        let mut dense = vec![0.; columns];
        let mut support = |i, row: &mut Vec<u32>| {
            read(i, &mut dense)?;
            for (j, &x) in dense.iter().enumerate() {
                if x != 0. {
                    if x != 1. {
                        return Ok(false);
                    }
                    row.push(j as u32);
                }
            }
            Ok(true)
        };
        grouped::RawSupport::new((columns, columns, columns), &mut support).pack(
            rows,
            &[],
            None,
            limit.saturating_sub(columns),
        )
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
    elimination <= MAX_WORK && dense_positive_cells(rows, positive, width) <= limit
}

fn dense_positive_cells(rows: usize, positive: usize, width: usize) -> usize {
    positive
        .saturating_mul(width)
        .saturating_mul(2)
        .saturating_add(width.saturating_mul(width.saturating_add(8)))
        .saturating_add(rows.saturating_mul(3))
}

pub(crate) fn compact_model(
    outcome: &[f64],
    design: &BinaryDesign,
    config: crate::ppml_separation::SeparationConfig,
    pivots: &mut usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<CompactModel>> {
    CompactSearch::new(
        outcome,
        design,
        config,
        None,
        config.simplex_max_tableau_cells,
    )
    .run(pivots, interrupt, &mut |_| None)
}

struct CompactSearch<'a> {
    outcome: &'a [f64],
    design: &'a BinaryDesign,
    groups: Option<&'a grouped::RowGroups>,
    tolerance: f64,
    certificate_tolerance: f64,
    max_iterations: usize,
    available: usize,
}

impl<'a> CompactSearch<'a> {
    fn new(
        outcome: &'a [f64],
        design: &'a BinaryDesign,
        config: crate::ppml_separation::SeparationConfig,
        groups: Option<&'a grouped::RowGroups>,
        limit: usize,
    ) -> Self {
        Self {
            outcome,
            design,
            groups,
            tolerance: config.projection_tolerance,
            certificate_tolerance: config.certificate_tolerance,
            max_iterations: config.simplex_max_iterations,
            available: limit.saturating_sub(design.cells()),
        }
    }

    fn require(&mut self, needed: usize, grow: &mut dyn FnMut(usize) -> Option<usize>) -> bool {
        if needed > self.available {
            let Some(granted) = grow(needed).filter(|&grant| grant >= needed) else {
                return false;
            };
            self.available = granted;
        }
        true
    }

    fn run(
        &mut self,
        pivots: &mut usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<CompactModel>> {
        let width = self.design.ncols();
        let outcome = self.outcome;
        let positive_count = outcome.iter().filter(|&&y| y > 0.).count();
        if width == 0
            || positive_count == 0
            || positive_count == outcome.len()
            || !self.require(
                width.saturating_mul(width).saturating_add(outcome.len()),
                grow,
            )
        {
            return Ok(None);
        }
        if dense_positive_admitted(outcome.len(), positive_count, width, self.available) {
            let positive = fallible_collect(
                outcome
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &y)| (y > 0.).then_some(i)),
                positive_count,
                "unable to reserve compact simplex positive rows",
            )?;
            return self.solve(positive, pivots, interrupt, grow);
        }
        let Some(selected) =
            positive_rows::select(self.design, outcome, self.available, interrupt)?
        else {
            return Ok(None);
        };
        let rank = selected.len();
        let proposal = self.solve(selected, pivots, interrupt, grow)?;
        if proposal.is_some() || rank == width {
            return Ok(proposal);
        }
        let Some(odd) = positive_rows::select_odd(self.design, outcome, self.available, interrupt)?
        else {
            return Ok(None);
        };
        if odd.len() > rank {
            self.solve(odd, pivots, interrupt, grow)
        } else {
            Ok(None)
        }
    }
}

impl CompactSearch<'_> {
    fn solve(
        &mut self,
        mut positive: Vec<usize>,
        pivots: &mut usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<CompactModel>> {
        let Self {
            outcome,
            design,
            tolerance,
            ..
        } = *self;
        let width = design.ncols();
        if !dense_positive_admitted(outcome.len(), positive.len(), width, usize::MAX)
            || !self.require(
                dense_positive_cells(outcome.len(), positive.len(), width),
                grow,
            )
        {
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
        let basis_workspace = projection::minimum_basis_cells(dimension, width);
        if !self.require(fixed_cells.saturating_add(basis_workspace), grow) {
            return Ok(None);
        }
        let cone_budget = self.available - fixed_cells;
        let Some(projected) =
            projection::build(design, &boundary, &mut basis, tolerance, cone_budget)?
        else {
            return Ok(None);
        };
        let retained = basis.len();
        let live = fixed_cells.saturating_add(projected.live_cells);
        let proposal = self.propose(&projected, (boundary_count, live), pivots, interrupt, grow)?;
        let remaining = self.available.saturating_sub(live);
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
                let scratch = self.available.saturating_sub(fixed_cells + retained);
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
                let certificate = self.farkas(
                    &projected,
                    &boundary,
                    &weights,
                    remaining.saturating_sub(weights.capacity()),
                )?;
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
                let scratch = self.available.saturating_sub(fixed_cells + weights.len());
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
    /// Try the already funded cone first. Extend only when its exact direct
    /// tableau size was refused and the caller can admit the physical index.
    fn propose(
        &mut self,
        projected: &projection::BoundaryCone,
        (boundary, live): (usize, usize),
        pivots: &mut usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<compact_tableau::Proposal>> {
        let dual_cells = boundary.saturating_mul(2).saturating_add(1);
        if !self.require(live.saturating_add(dual_cells), grow) {
            return Ok(None);
        }
        let policy = (self.tolerance, self.max_iterations);
        let remaining = self.available - live;
        let initial = projected.propose(boundary, policy, pivots, remaining, interrupt)?;
        if initial.is_some() || *pivots >= self.max_iterations {
            return Ok(initial);
        }
        let dimension = projected.rows.first().map_or(0, Vec::len);
        let needed = compact_tableau::workspace_cells(projected.rows.len(), dimension);
        if needed.is_some_and(|words| {
            words > remaining && self.require(live.saturating_add(words), grow)
        }) {
            return projected.propose(boundary, policy, pivots, self.available - live, interrupt);
        }
        Ok(None)
    }

    fn farkas(
        &self,
        cone: &projection::BoundaryCone,
        boundary: &[usize],
        weights: &[f64],
        available: usize,
    ) -> Result<Option<SeparationCertificate>> {
        if let Some(groups) = self.groups {
            return groups.farkas(
                cone,
                boundary,
                weights,
                self.certificate_tolerance,
                available,
            );
        }
        let dual = fallible_collect(
            weights.iter().map(|w| -*w).chain(std::iter::once(-1.)),
            weights.len() + 1,
            "unable to reserve compact Farkas certificate",
        )?;
        verify_indexed_farkas_certificate(
            &cone.rows,
            &cone.index,
            &dual,
            self.certificate_tolerance,
        )
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
