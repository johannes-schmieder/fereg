//! Fixed row partitions; parallel scheduling cannot change a moment's additions.
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    threads::{blocked_sum, ThreadTeam},
};
use rayon::prelude::*;
pub(super) fn moments(
    design: &DenseMatrix,
    rhs: &DenseMatrix,
    team: &ThreadTeam,
) -> Result<(DenseMatrix, DenseMatrix)> {
    let p = design.ncols();
    let mut rows = reserved(p)?;
    rows.resize_with(p, || None);
    team.install(|| {
        rows.par_iter_mut().enumerate().for_each(|(left, result)| {
            *result = Some(moment_row(design, rhs, left));
        })
    });
    let mut gram = DenseMatrix::zeros(p, p)?;
    let mut cross = DenseMatrix::zeros(p, rhs.ncols())?;
    for (left, result) in rows.into_iter().enumerate() {
        let row = result.expect("visited moment row")?;
        for right in left..p {
            gram[(left, right)] = row[right - left];
            gram[(right, left)] = row[right - left];
        }
        for column in 0..rhs.ncols() {
            cross[(left, column)] = row[p - left + column];
        }
    }
    Ok((gram, cross))
}
fn moment_row(design: &DenseMatrix, rhs: &DenseMatrix, left: usize) -> Result<Vec<f64>> {
    let mut result = reserved(design.ncols() - left + rhs.ncols())?;
    for right in left..design.ncols() {
        result.push(blocked_sum(design.nrows(), |row| {
            design[(row, left)] * design[(row, right)]
        }));
    }
    for column in 0..rhs.ncols() {
        result.push(blocked_sum(design.nrows(), |row| {
            design[(row, left)] * rhs[(row, column)]
        }));
    }
    if result.iter().any(|v| !v.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "finite WLS moments are not representable",
        ));
    }
    Ok(result)
}
