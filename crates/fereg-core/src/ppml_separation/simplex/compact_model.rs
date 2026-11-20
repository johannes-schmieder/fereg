//! A bounded alternative for prepared models whose boundary-square tableau
//! cannot fit. Both outcomes require an independent raw-design proof.
use super::{combine_basis, compact_tableau, exact_terminal, reserve};
use super::{fallible_collect, no_separation_certificate, nullspace_basis};
use super::{fallible_filled, verify_phase_one_farkas_certificate};
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

const MAX_WORK: usize = 128 << 20;

pub(crate) struct BinaryDesign {
    offsets: Vec<usize>,
    indices: Vec<usize>,
    columns: usize,
}

impl BinaryDesign {
    pub(crate) fn from_rows(
        rows: usize,
        columns: usize,
        limit: usize,
        mut read: impl FnMut(usize, &mut [f64]) -> Result<()>,
    ) -> Result<Option<Self>> {
        if rows as u128 > (1u128 << 53)
            || rows.saturating_mul(columns).saturating_mul(2) > MAX_WORK
            || rows.saturating_add(1).saturating_add(columns) > limit
        {
            return Ok(None);
        }
        let mut offsets = fallible_filled(rows + 1, 0, "unable to reserve compact binary rows")?;
        let mut row = fallible_filled(columns, 0., "unable to reserve compact binary input row")?;
        let max_ones = limit - offsets.len() - row.len();
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
                    .filter_map(|(j, &x)| (x == 1.).then_some(j)),
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

    pub(crate) fn row(&self, row: usize) -> &[usize] {
        &self.indices[self.offsets[row]..self.offsets[row + 1]]
    }

    fn cells(&self) -> usize {
        self.offsets.len() + self.indices.len()
    }
}

pub(crate) fn compact_model_admitted(
    rows: usize,
    positive: usize,
    width: usize,
    limit: usize,
) -> bool {
    let positive_cells = positive.saturating_mul(width);
    let elimination = positive_cells.saturating_mul(positive.min(width));
    let scratch = positive_cells
        .saturating_mul(2)
        .saturating_add(width.saturating_mul(width.saturating_add(8)))
        .saturating_add(rows.saturating_mul(3));
    positive > 0 && positive < rows && elimination <= MAX_WORK && scratch <= limit
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
    let mut positive = fallible_collect(
        outcome
            .iter()
            .enumerate()
            .filter_map(|(i, &y)| (y > 0.).then_some(i)),
        outcome.len(),
        "unable to reserve compact simplex positive rows",
    )?;
    let width = design.ncols();
    if !compact_model_admitted(outcome.len(), positive.len(), width, max_cells) {
        return Ok(None);
    }
    let Some(available) = max_cells.checked_sub(design.cells()) else {
        return Ok(None);
    };
    // Preserve an original-row map while factoring only the positive rows.
    // The numerical nullspace proposes directions; exact checks use original
    // binary rows, including every zero outcome.
    if !compact_model_admitted(outcome.len(), positive.len(), width, available) {
        return Ok(None);
    }
    let Some(basis) = positive_nullspace(design, &mut positive, tolerance, interrupt)? else {
        return Ok(None);
    };
    let mut basis = basis;
    let boundary_count = outcome.iter().filter(|&&y| y == 0.).count();
    if basis.is_empty() {
        return Ok(exact_terminal::compact_full_positive_rank(
            design, outcome, &positive, MAX_WORK,
        )?
        .then(|| {
            terminal(
                no_separation_certificate(SeparationCertificateProof::SimplexFullPositiveRank {
                    columns: width,
                }),
                0,
                "finite regressors have full column rank on the positive-outcome sample",
            )
        }));
    }

    let dimension = basis.len();
    let cone_cells = boundary_count.saturating_mul(dimension + 3);
    let live = cone_cells
        .saturating_add(dimension.saturating_mul(width + 3))
        .saturating_add(outcome.len().saturating_mul(2))
        .saturating_add(width);
    if live > available {
        return Ok(None);
    }
    let boundary = fallible_collect(
        outcome
            .iter()
            .enumerate()
            .filter_map(|(i, &y)| (y == 0.).then_some(i)),
        boundary_count,
        "unable to reserve compact simplex zero rows",
    )?;
    let Some(mut cone) = binary_cone(design, &boundary, &basis, tolerance)? else {
        return Ok(None);
    };
    let reduction = live.saturating_add(dimension.saturating_mul(dimension + 8));
    if reduction > available || !reduce_cone(&mut cone, &mut basis, tolerance)? {
        return Ok(None);
    }
    let retained = basis.len();
    let live = live.saturating_sub((dimension - retained).saturating_mul(width + 3));
    let proposal = compact_tableau::propose(
        &cone,
        tolerance,
        max_iterations,
        available - live,
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
            drop(cone);
            let coefficients = combine_basis(&basis, &coordinates)?;
            drop(basis);
            let scratch = available.saturating_sub(2 * outcome.len() + width + dimension);
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
            let certificate =
                verify_phase_one_farkas_certificate(&cone, &dual, certificate_tolerance)?;
            drop(dual);
            drop(cone);
            drop(basis);
            let Some(certificate) = certificate else {
                if crate::diagnostics::solver_trace_enabled() {
                    eprintln!("FEREG_SOLVER_TRACE|phase=compact_terminal|failed=numerical_farkas");
                }
                return Ok(None);
            };
            let scratch = available.saturating_sub(2 * outcome.len() + weights.len());
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
            rows[i * width + j] = 1.;
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

/// Visit the original binary rows once, accumulating only their nonzero
/// coordinates into the numerical cone. Ignore small roundoff in this
/// proposal; the coefficients and dual still have to balance raw rows.
fn binary_cone(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &[Vec<f64>],
    tolerance: f64,
) -> Result<Option<Vec<Vec<f64>>>> {
    let mut remaining = MAX_WORK;
    let mut cone = Vec::new();
    reserve(
        &mut cone,
        boundary.len(),
        "unable to reserve compact cone rows",
    )?;
    for &row in boundary {
        crate::interrupt::row_checkpoint(row)?;
        let mut values = fallible_filled(basis.len(), 0., "unable to reserve compact cone row")?;
        let Some(work) = remaining.checked_sub(design.row(row).len().saturating_mul(basis.len()))
        else {
            return Ok(None);
        };
        remaining = work;
        for &j in design.row(row) {
            for (score, direction) in values.iter_mut().zip(basis) {
                *score += direction[j];
            }
        }
        for value in &mut values {
            if value.abs() <= tolerance {
                *value = 0.;
            }
        }
        cone.push(values);
    }
    Ok(Some(cone))
}

/// Aliased effects often leave many more coordinates in the positive-row
/// nullspace than directions observable on the zero rows. A proposed column
/// basis gives the small transposed program independent equations.
fn reduce_cone(cone: &mut [Vec<f64>], basis: &mut Vec<Vec<f64>>, tolerance: f64) -> Result<bool> {
    let dimension = basis.len();
    let maximum = cone.iter().flatten().fold(0.0_f64, |m, v| m.max(v.abs()));
    let threshold = tolerance * maximum;
    let mut rows: Vec<(usize, Vec<f64>)> = Vec::new();
    reserve(
        &mut rows,
        dimension,
        "unable to reserve compact cone pivots",
    )?;
    let mut work = fallible_filled(dimension, 0., "unable to reserve compact cone contrast")?;
    let mut remaining = MAX_WORK;
    for (i, row) in cone.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let Some(left) = remaining.checked_sub((1 + rows.len()).saturating_mul(dimension)) else {
            return Ok(false);
        };
        remaining = left;
        work.copy_from_slice(row);
        for (column, direction) in &rows {
            let multiplier = work[*column];
            for (value, &prior) in work.iter_mut().zip(direction) {
                *value -= multiplier * prior;
            }
            work[*column] = 0.;
        }
        let Some((column, &pivot)) = work
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .filter(|(_, value)| value.abs() > threshold)
        else {
            continue;
        };
        let direction = fallible_collect(
            work.iter().map(|x| *x / pivot),
            dimension,
            "unable to reserve compact cone direction",
        )?;
        rows.push((column, direction));
        if rows.len() == dimension {
            return Ok(true);
        }
    }
    let mut keep = fallible_filled(dimension, false, "unable to reserve compact cone mask")?;
    for (column, _) in rows {
        keep[column] = true;
    }
    // Represent the zero-dimensional cone by one zero coordinate for the
    // normalized Farkas check. The exact raw proof still handles every alias.
    if !keep.contains(&true) {
        keep[0] = true;
    }
    for row in cone {
        let mut selected = keep.iter();
        row.retain(|_| *selected.next().expect("cone width checked at construction"));
    }
    let mut selected = keep.iter();
    basis.retain(|_| {
        *selected
            .next()
            .expect("basis width checked at construction")
    });
    Ok(true)
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
        for &j in design.row(i) {
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
            .map(|&j| i128::from(coefficients[j]))
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
