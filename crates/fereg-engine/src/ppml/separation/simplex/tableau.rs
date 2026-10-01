//! Retained phase-I tableau with Bland pivoting; candidates need separate checks.
use crate::error::{ErrorCode, FeregError, Result};
use crate::execution::{self, Poll, WorkPoll};
use crate::ppml::{Interrupt, InterruptPoint};

use super::{filled, reserve};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum PhaseOneConeOutcome {
    Feasible {
        coordinates: Vec<f64>,
        iterations: usize,
    },
    /// Candidate Farkas multiplier for the *original* cone-feasibility
    /// equations.  The tableau is not trusted to certify this vector; the
    /// caller independently rechecks it against the immutable cone.
    Infeasible { dual: Vec<f64>, iterations: usize },
}

pub(super) fn phase_one_cone(
    cone: &[Vec<f64>],
    max_iterations: usize,
    tolerance: f64,
    interrupt: &mut Interrupt<'_>,
    poll: Poll<'_>,
) -> Result<PhaseOneConeOutcome> {
    let mut state = Tableau::new(cone, poll)?;
    for iteration in 0..max_iterations {
        interrupt(InterruptPoint::SimplexPivot)?;
        let Some(entering) = state.entering(tolerance, poll)? else {
            return state.outcome(iteration, tolerance, poll);
        };
        let leaving = state.leaving(entering, tolerance, poll)?;
        pivot(
            &mut state.values,
            state.basis.len(),
            state.width,
            leaving,
            entering,
            tolerance,
            poll,
        )?;
        state.basis[leaving] = entering;
    }
    Err(FeregError::new(
        ErrorCode::NonConvergence,
        format!("simplex phase-I exceeded {max_iterations} pivots"),
    ))
}
struct Tableau {
    values: Vec<f64>,
    basis: Vec<usize>,
    costs: Vec<f64>,
    width: usize,
    dimension: usize,
    original_variables: usize,
}
impl Tableau {
    fn new(cone: &[Vec<f64>], poll: Poll<'_>) -> Result<Self> {
        let mut work = WorkPoll::new(poll)?;
        let boundary = cone.len();
        let dimension = cone.first().map_or(0, Vec::len);
        if boundary == 0
            || dimension == 0
            || execution::any(cone, |row| row.len() != dimension, poll)?
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "simplex phase-I requires an aligned nonempty boundary cone",
            ));
        }
        let rows = boundary.checked_add(1).ok_or_else(super::overflow)?;
        let original_variables = dimension
            .checked_mul(2)
            .and_then(|v| v.checked_add(boundary))
            .ok_or_else(super::overflow)?;
        let total_variables = original_variables
            .checked_add(rows)
            .ok_or_else(super::overflow)?;
        let width = total_variables.checked_add(1).ok_or_else(super::overflow)?;
        let cells = rows.checked_mul(width).ok_or_else(super::overflow)?;
        let mut values = filled(cells, 0., "simplex phase-I tableau", poll)?;
        let mut basis = Vec::new();
        reserve(&mut basis, rows, "simplex phase-I basis")?;
        for (row, constraint) in cone.iter().enumerate() {
            work.step()?;
            for (column, &value) in constraint.iter().enumerate() {
                work.step()?;
                values[row * width + column] = value;
                values[row * width + dimension + column] = -value;
            }
            values[row * width + 2 * dimension + row] = -1.;
            let artificial = original_variables + row;
            values[row * width + artificial] = 1.;
            basis.push(artificial);
        }
        for slack in 0..boundary {
            work.step()?;
            values[boundary * width + 2 * dimension + slack] = 1.;
        }
        values[boundary * width + original_variables + boundary] = 1.;
        values[boundary * width + total_variables] = 1.;
        basis.push(original_variables + boundary);
        let mut costs = filled(total_variables, 0., "simplex phase-I costs", poll)?;
        for cost in &mut costs[original_variables..] {
            work.step()?;
            *cost = -1.;
        }
        execution::check(poll)?;
        Ok(Self {
            values,
            basis,
            costs,
            width,
            dimension,
            original_variables,
        })
    }
    fn entering(&self, tolerance: f64, poll: Poll<'_>) -> Result<Option<usize>> {
        let mut work = WorkPoll::new(poll)?;
        for (column, &cost) in self.costs.iter().enumerate() {
            work.step()?;
            let mut basic_column = false;
            for &basic in &self.basis {
                work.step()?;
                if basic == column {
                    basic_column = true;
                    break;
                }
            }
            if basic_column {
                continue;
            }
            let mut reduced = cost;
            for (row, &basic) in self.basis.iter().enumerate() {
                work.step()?;
                reduced -= self.costs[basic] * self.values[row * self.width + column];
            }
            if reduced > tolerance {
                return Ok(Some(column)); // Bland's rule
            }
        }
        Ok(None)
    }
    fn leaving(&self, entering: usize, tolerance: f64, poll: Poll<'_>) -> Result<usize> {
        let mut work = WorkPoll::new(poll)?;
        let mut leaving = None;
        let mut best_ratio = f64::INFINITY;
        let mut best_basic = usize::MAX;
        for (row, &basic) in self.basis.iter().enumerate() {
            work.step()?;
            let coefficient = self.values[row * self.width + entering];
            if coefficient <= tolerance {
                continue;
            }
            let rhs = self.values[row * self.width + self.costs.len()].max(0.);
            let ratio = rhs / coefficient;
            if ratio < best_ratio - tolerance
                || ((ratio - best_ratio).abs() <= tolerance && basic < best_basic)
            {
                best_ratio = ratio;
                best_basic = basic;
                leaving = Some(row);
            }
        }
        leaving.ok_or_else(|| super::failed("simplex phase-I tableau became unbounded"))
    }
    fn outcome(
        &self,
        iterations: usize,
        tolerance: f64,
        poll: Poll<'_>,
    ) -> Result<PhaseOneConeOutcome> {
        let mut work = WorkPoll::new(poll)?;
        let rhs_column = self.costs.len();
        let objective = execution::fold(
            self.basis.iter().enumerate(),
            0_f64,
            poll,
            |sum, (row, &basic)| {
                self.costs[basic].mul_add(self.values[row * self.width + rhs_column], sum)
            },
        )?;
        if objective < -100. * tolerance {
            // Original artificial columns are an identity block, hence their
            // canonical tableau columns are B^-1. Extract c_B' B^-1; the caller
            // separately verifies this candidate against the immutable cone.
            let mut dual = filled(self.basis.len(), 0., "simplex phase-I dual", poll)?;
            for (constraint, value) in dual.iter_mut().enumerate() {
                let artificial = self.original_variables + constraint;
                for (row, &basic) in self.basis.iter().enumerate() {
                    work.step()?;
                    *value = self.costs[basic]
                        .mul_add(self.values[row * self.width + artificial], *value);
                }
            }
            if execution::any(&dual, |v| !v.is_finite(), poll)? {
                return Err(super::failed("simplex phase-I dual became nonfinite"));
            }
            execution::check(poll)?;
            return Ok(PhaseOneConeOutcome::Infeasible { dual, iterations });
        }
        let mut solution = filled(
            self.original_variables,
            0.,
            "simplex phase-I solution",
            poll,
        )?;
        for (row, &basic) in self.basis.iter().enumerate() {
            work.step()?;
            if basic < self.original_variables {
                solution[basic] = self.values[row * self.width + rhs_column].max(0.);
            }
        }
        let mut coordinates = filled(self.dimension, 0., "simplex phase-I coordinates", poll)?;
        for (column, value) in coordinates.iter_mut().enumerate() {
            work.step()?;
            *value = solution[column] - solution[self.dimension + column];
        }
        execution::check(poll)?;
        Ok(PhaseOneConeOutcome::Feasible {
            coordinates,
            iterations,
        })
    }
}

fn pivot(
    tableau: &mut [f64],
    rows: usize,
    width: usize,
    pivot_row: usize,
    pivot_column: usize,
    tolerance: f64,
    poll: Poll<'_>,
) -> Result<()> {
    let mut work = WorkPoll::new(poll)?;
    let pivot = tableau[pivot_row * width + pivot_column];
    if !pivot.is_finite() || pivot.abs() <= tolerance {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "simplex selected a numerically zero pivot",
        ));
    }
    for column in 0..width {
        work.step()?;
        tableau[pivot_row * width + column] /= pivot;
    }
    for row in 0..rows {
        work.step()?;
        if row == pivot_row {
            continue;
        }
        let factor = tableau[row * width + pivot_column];
        if factor == 0.0 {
            continue;
        }
        for column in 0..width {
            work.step()?;
            tableau[row * width + column] -= factor * tableau[pivot_row * width + column];
        }
    }
    if execution::any(tableau.iter(), |value| !value.is_finite(), poll)? {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "simplex tableau became nonfinite",
        ));
    }
    execution::check(poll)?;
    Ok(())
}
