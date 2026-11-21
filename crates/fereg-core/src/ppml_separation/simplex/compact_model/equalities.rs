//! Implicit equalities of the projected cone.
//!
//! A distinct boundary row `m` whose exact negation `-m` is also a boundary
//! row bounds every feasible direction `t` on both sides: `m't <= 0` and
//! `-m't <= 0` give `m't = 0`. Such pairs are common when absorbed levels grow
//! with the data: zero rows that join two components of the positive-row graph
//! in both orientations. Phase I then runs on the null space of the equality
//! rows, whose dimension can be far below the cone's, and its results are
//! mapped back exactly in form: a direction is a combination of the reduced
//! basis, which is itself in the positive-row null space, and Farkas weights
//! are completed by adding mass to one row of each equality pair so that the
//! full-dimensional boundary total vanishes. The full-dimensional cone rows,
//! the raw design and the IR certificate check every result as before.
use super::projection::BoundaryCone;
use super::sparse_echelon::{self, Echelon};
use super::{fallible_filled, reserve, trace_refusal};
use crate::error::Result;

pub(super) struct Reduced {
    /// The cone rows in reduced coordinates, with the original groups.
    pub(super) cone: BoundaryCone,
    /// The reduced null-space basis in raw coordinates.
    pub(super) basis: Vec<Vec<f64>>,
    /// Words held by the reduction beyond the original cone.
    pub(super) cells: usize,
    pairs: Vec<(usize, usize)>,
    echelon: Echelon,
}

/// Exact opposite pairs among the distinct rows, each row in at most one.
fn opposite_pairs(rows: &[Vec<f64>], work: &mut usize) -> Result<Option<Vec<(usize, usize)>>> {
    let mut keys: Vec<(u64, usize)> = Vec::new();
    reserve(&mut keys, rows.len(), "unable to reserve cone row keys")?;
    for (i, row) in rows.iter().enumerate() {
        let Some(key) = super::projection::row_key(row, work) else {
            return Ok(None);
        };
        keys.push((key, i));
    }
    keys.sort_unstable();
    let dimension = rows.first().map_or(0, Vec::len);
    let mut negated = fallible_filled(dimension, 0., "unable to reserve negated cone row")?;
    let mut used = fallible_filled(rows.len(), false, "unable to reserve cone pair marks")?;
    let mut pairs = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if used[i] || row.iter().all(|&v| v == 0.) {
            continue;
        }
        for (value, &v) in negated.iter_mut().zip(row) {
            *value = if v == 0. { 0. } else { -v };
        }
        let Some(key) = super::projection::row_key(&negated, work) else {
            return Ok(None);
        };
        let start = keys.partition_point(|&(k, _)| k < key);
        let found = keys[start..]
            .iter()
            .take_while(|&&(k, _)| k == key)
            .map(|&(_, j)| j)
            .find(|&j| !used[j] && j != i && rows[j] == negated);
        if let Some(j) = found {
            let Some(left) = work.checked_sub(dimension) else {
                return Ok(None);
            };
            *work = left;
            used[i] = true;
            used[j] = true;
            pairs
                .try_reserve(1)
                .map_err(|_| crate::error::allocation("unable to reserve cone pairs"))?;
            pairs.push((i, j));
        }
    }
    Ok(Some(pairs))
}

/// The reduction, or `None` when there is no equality or a bound refuses it;
/// the unreduced search then runs unchanged.
pub(super) fn reduce(
    cone: &BoundaryCone,
    basis: &[Vec<f64>],
    tolerance: f64,
    stage_work: usize,
    admit: &mut dyn FnMut(usize) -> bool,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Reduced>> {
    let dimension = cone.rows.first().map_or(0, Vec::len);
    let mut work = stage_work;
    if dimension == 0 || basis.len() != dimension || cone.groups.len() != cone.rows.len() {
        return Ok(None);
    }
    let Some(pairs) = opposite_pairs(&cone.rows, &mut work)? else {
        trace_refusal("equality_pairs_work", cone.rows.len(), stage_work);
        return Ok(None);
    };
    if pairs.is_empty() {
        return Ok(None);
    }
    let ids: Vec<usize> = (0..pairs.len()).collect();
    let mut read = |p: usize, f: &mut dyn FnMut(usize, f64)| -> Result<()> {
        for (k, &v) in cone.rows[pairs[p].0].iter().enumerate() {
            if v != 0. {
                f(k, v);
            }
        }
        Ok(())
    };
    let eliminated = sparse_echelon::eliminate(
        &ids, dimension, &mut read, tolerance, work, admit, interrupt,
    )?;
    let echelon = match eliminated {
        Ok(echelon) => echelon,
        Err(refusal) => {
            trace_refusal(refusal.stage, refusal.needed, refusal.available);
            return Ok(None);
        }
    };
    let reduced = echelon.nullity();
    let width = basis.first().map_or(0, Vec::len);
    let rows = cone.rows.len();
    let cells = echelon
        .words
        .saturating_add(rows.saturating_mul(reduced.saturating_add(3)))
        .saturating_add(reduced.saturating_mul(width.saturating_add(dimension)))
        .saturating_add(cone.groups.len().saturating_mul(2))
        .saturating_add(cone.index.len().div_ceil(2));
    let cost = echelon
        .work
        .saturating_add(echelon.basis_work())
        .saturating_add(
            rows.saturating_mul(dimension)
                .saturating_mul(reduced.max(1)),
        )
        .saturating_add(
            width
                .saturating_mul(dimension)
                .saturating_mul(reduced.max(1)),
        );
    if cost > work {
        trace_refusal("equality_reduction_work", cost, work);
        return Ok(None);
    }
    if !admit(cells) {
        trace_refusal("equality_reduction_cells", cells, 0);
        return Ok(None);
    }
    let axes = echelon.basis(interrupt)?;
    let mut projected = Vec::new();
    reserve(&mut projected, rows, "unable to reserve reduced cone rows")?;
    for (i, row) in cone.rows.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let scale = row.iter().fold(1_f64, |s, v| s.max(v.abs()));
        let mut out = fallible_filled(reduced, 0., "unable to reserve reduced cone row")?;
        for (value, axis) in out.iter_mut().zip(&axes) {
            let dot: f64 = row.iter().zip(axis).map(|(&m, &z)| m * z).sum();
            *value = if dot.abs() <= tolerance * scale {
                0.
            } else {
                dot
            };
        }
        projected.push(out);
    }
    let mut lifted = Vec::new();
    reserve(&mut lifted, reduced, "unable to reserve reduced cone basis")?;
    for axis in &axes {
        crate::interrupt::checkpoint()?;
        let mut vector = fallible_filled(width, 0., "unable to reserve reduced basis vector")?;
        for (&z, direction) in axis.iter().zip(basis) {
            if z != 0. {
                for (value, &b) in vector.iter_mut().zip(direction) {
                    *value = z.mul_add(b, *value);
                }
            }
        }
        lifted.push(vector);
    }
    let mut groups = Vec::new();
    reserve(
        &mut groups,
        cone.groups.len(),
        "unable to reserve reduced cone groups",
    )?;
    groups.extend_from_slice(&cone.groups);
    let mut index = Vec::new();
    reserve(
        &mut index,
        cone.index.len(),
        "unable to reserve reduced cone index",
    )?;
    index.extend_from_slice(&cone.index);
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=cone_equalities|distinct={rows}|pairs={}|rank={}|dimension={dimension}|reduced={reduced}",
            pairs.len(),
            echelon.rank()
        );
    }
    Ok(Some(Reduced {
        cone: BoundaryCone {
            rows: projected,
            live_cells: rows.saturating_mul(reduced.saturating_add(3)),
            groups,
            index,
        },
        basis: lifted,
        cells,
        pairs,
        echelon,
    }))
}

impl Reduced {
    /// Complete reduced Farkas weights on every boundary row: the residual
    /// boundary total lies in the span of the equality rows, and each
    /// independent equality takes its coefficient as extra mass on the pair
    /// row of matching sign. Every weight stays at least its reduced value.
    pub(super) fn complete(&self, cone: &BoundaryCone, mut weights: Vec<f64>) -> Result<Vec<f64>> {
        let dimension = cone.rows.first().map_or(0, Vec::len);
        let mut mass = fallible_filled(cone.rows.len(), 0., "unable to reserve cone group mass")?;
        for (&group, &w) in cone.index.iter().zip(&weights) {
            mass[group as usize] += w;
        }
        let mut total = fallible_filled(dimension, 0., "unable to reserve cone boundary total")?;
        let mut correction =
            fallible_filled(dimension, 0., "unable to reserve cone total correction")?;
        for (row, &m) in cone.rows.iter().zip(&mass) {
            for ((t, c), &v) in total.iter_mut().zip(&mut correction).zip(row) {
                crate::linalg::neumaier_add(m * v, t, c);
            }
        }
        for (t, c) in total.iter_mut().zip(&correction) {
            *t = -(*t + c);
        }
        let ids: Vec<usize> = (0..self.pairs.len()).collect();
        let pivots = self.echelon.pivot_rows(&ids)?;
        let coefficients = self.echelon.combine(&total)?;
        for (&p, &c) in pivots.iter().zip(&coefficients) {
            if !c.is_finite() {
                return Ok(weights);
            }
            let (row, negated) = self.pairs[p];
            let (group, extra) = if c >= 0. { (row, c) } else { (negated, -c) };
            weights[cone.groups[group].0] += extra;
        }
        Ok(weights)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cone(rows: Vec<Vec<f64>>) -> BoundaryCone {
        let n = rows.len();
        BoundaryCone {
            rows,
            live_cells: 0,
            groups: (0..n).map(|i| (i, 1)).collect(),
            index: (0..n as u32).collect(),
        }
    }

    fn identity(d: usize) -> Vec<Vec<f64>> {
        (0..d)
            .map(|k| (0..d).map(|j| f64::from(u8::from(j == k))).collect())
            .collect()
    }

    fn reduced(cone: &BoundaryCone) -> Reduced {
        let basis = identity(cone.rows[0].len());
        reduce(cone, &basis, 1e-12, usize::MAX, &mut |_| true, &mut || {
            Ok(())
        })
        .unwrap()
        .expect("opposite rows give an equality reduction")
    }

    #[test]
    fn opposite_rows_pin_their_span_and_the_lifted_direction_keeps_them_at_zero() {
        let rows = vec![
            vec![1., -1., 0.],
            vec![-1., 1., 0.],
            vec![0., 1., -1.],
            vec![0., -1., 1.],
            vec![-1., -1., -1.],
        ];
        let cone = cone(rows.clone());
        let reduction = reduced(&cone);
        assert_eq!(reduction.basis.len(), 1);
        let axis = &reduction.basis[0];
        for row in &rows[..4] {
            let score: f64 = row.iter().zip(axis).map(|(m, z)| m * z).sum();
            assert!(score.abs() < 1e-14);
        }
        // The unpaired row is strictly one-signed on the reduced axis.
        assert!(reduction.cone.rows[4][0].abs() > 1.);
        for row in &reduction.cone.rows[..4] {
            assert_eq!(row[0], 0.);
        }
    }

    #[test]
    fn completed_weights_cancel_the_full_dimensional_boundary_total() {
        let rows = vec![
            vec![1., -1., 0.],
            vec![-1., 1., 0.],
            vec![2., 0., 0.],
            vec![-2., 0., 0.],
            vec![0., 0., 3.],
            vec![0., 0., -1.],
        ];
        let cone = cone(rows.clone());
        let reduction = reduced(&cone);
        assert_eq!(reduction.basis.len(), 1);
        // Reduced Farkas weights balance only the free axis.
        let weights = vec![2., 1., 1., 4., 1., 3.];
        let completed = reduction.complete(&cone, weights.clone()).unwrap();
        for (c, w) in completed.iter().zip(&weights) {
            assert!(c >= w);
        }
        for k in 0..3 {
            let total: f64 = rows.iter().zip(&completed).map(|(row, w)| w * row[k]).sum();
            assert!(total.abs() < 1e-12, "axis {k}: {total}");
        }
    }

    #[test]
    fn no_pair_leaves_the_search_unreduced() {
        let cone = cone(vec![vec![1., 0.], vec![0., -1.], vec![-1., -1.]]);
        let basis = identity(2);
        let reduction = reduce(&cone, &basis, 1e-12, usize::MAX, &mut |_| true, &mut || {
            Ok(())
        })
        .unwrap();
        assert!(reduction.is_none());
    }
}
