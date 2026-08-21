use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::{EffectTerm, FeDesign};

#[derive(Debug)]
pub(super) struct SubsetProblem {
    pub(super) original_rows: Vec<usize>,
    pub(super) outcome: Vec<f64>,
    pub(super) regressors: DenseMatrix,
    pub(super) fixed_effects: Option<FeDesign>,
}

pub(super) fn subset_problem(
    outcome: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    active: &[bool],
) -> Result<SubsetProblem> {
    let original_rows = active
        .iter()
        .enumerate()
        .filter_map(|(row, keep)| (*keep).then_some(row))
        .collect::<Vec<_>>();
    let outcome = select_rows(outcome, &original_rows);
    let regressors = subset_matrix(regressors, &original_rows)?;
    let fixed_effects = fixed_effects
        .map(|design| subset_fixed_effects(design, &original_rows))
        .transpose()?;
    Ok(SubsetProblem {
        original_rows,
        outcome,
        regressors,
        fixed_effects,
    })
}

pub(super) fn subset_matrix(matrix: &DenseMatrix, rows: &[usize]) -> Result<DenseMatrix> {
    if rows.iter().any(|row| *row >= matrix.nrows()) {
        return Err(StFixestError::new(
            ErrorCode::Internal,
            "matrix subset contains an out-of-range observation",
        ));
    }
    if matrix.ncols() == 0 {
        return DenseMatrix::zeros(rows.len(), 0);
    }
    let columns = (0..matrix.ncols())
        .map(|column| rows.iter().map(|row| matrix[(*row, column)]).collect())
        .collect::<Vec<Vec<f64>>>();
    DenseMatrix::from_columns(&columns)
}

pub(super) fn subset_fixed_effects(design: &FeDesign, rows: &[usize]) -> Result<FeDesign> {
    if rows.iter().any(|row| *row >= design.nobs()) {
        return Err(StFixestError::new(
            ErrorCode::Internal,
            "fixed-effect subset contains an out-of-range observation",
        ));
    }
    let mut effects = Vec::with_capacity(design.effects().len());
    for effect in design.effects() {
        let mut present = vec![false; effect.n_levels()];
        for row in rows {
            present[effect.codes()[*row] as usize] = true;
        }
        let mut mapping = vec![u32::MAX; effect.n_levels()];
        let mut next = 0_u32;
        for (level, is_present) in present.into_iter().enumerate() {
            if is_present {
                mapping[level] = next;
                next += 1;
            }
        }
        if next == 0 {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                "fixed-effect subset contains no observed levels",
            ));
        }
        let codes = rows
            .iter()
            .map(|row| mapping[effect.codes()[*row] as usize])
            .collect();
        let slopes = effect
            .slopes()
            .iter()
            .map(|slope| select_rows(slope, rows))
            .collect();
        effects.push(EffectTerm::new(
            codes,
            next as usize,
            effect.has_intercept(),
            slopes,
        )?);
    }
    FeDesign::new(rows.len(), effects)
}

pub(super) fn select_rows<T: Copy>(values: &[T], rows: &[usize]) -> Vec<T> {
    rows.iter().map(|row| values[*row]).collect()
}

