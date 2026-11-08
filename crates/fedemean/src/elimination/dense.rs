//! The exact local solve of a small block Laplacian: one grounded Cholesky
//! factorization per connected component.
//!
//! On a connected component with `k` levels the Laplacian `L` has the
//! constants as its null space. Let `P` remove the component mean (the
//! Euclidean projection onto the range of the symmetric `L`) and `G` be the
//! inverse of `L` with one level grounded (its row and column removed),
//! padded with zeros. For `r` orthogonal to the constants, `y = G r` solves
//! every row of `L y = r` but the grounded one, which then holds as well
//! because the rows of `L` and the entries of `r` sum to zero; removing the
//! mean selects the minimum-norm solution. Hence `P G P = L^+`: the solve is
//! symmetric, positive semidefinite, kills the component constants and
//! inverts the block on its range. The grounded matrix of a connected
//! component is a symmetric positive definite M-matrix, factored without
//! pivoting, its diagonal the sum of its edge weights (no cancellation); the
//! grounded level is the one with the largest weighted degree. A pivot at
//! the rounding floor declines the dense solve (the block keeps its Jacobi
//! solve). Levels without edges are isolated and have a zero image.
//!
//! Memory: every allocation is fallible and kept across rebuilds.

use super::block::{BlockLaplacian, LocalSolve};
use super::structure::{refill, reserve};
use super::{Cancel, BATCH};
use crate::error::{Error, Result};

/// Position of a level without edges.
const ISOLATED: u32 = u32::MAX;

/// `L^+` of one block Laplacian, by grounded components.
#[derive(Default)]
pub(crate) struct DenseLocal {
    levels: usize,
    /// Component of each level (union-find parents during the build), then
    /// the factored component (`ISOLATED` for a level without edges).
    component: Vec<u32>,
    /// Position of each level within its component.
    local: Vec<u32>,
    /// Weighted degree of each level (build scratch).
    degree: Vec<f64>,
    /// Size, then factored index, of each union-find component (scratch).
    labels: Vec<u32>,
    /// The levels of each component of two or more levels in ascending
    /// order, the grounded level moved last.
    order: Vec<u32>,
    /// Component boundaries into `order`.
    starts: Vec<usize>,
    /// Packed row-major lower Cholesky factors of the grounded components,
    /// component `c` from `factor_starts[c]`.
    factor: Vec<f64>,
    factor_starts: Vec<usize>,
    /// One component's right-hand sides, up to [`BATCH`] wide.
    scratch: Vec<f64>,
}

fn root(parent: &mut [u32], mut v: u32) -> u32 {
    while parent[v as usize] != v {
        let up = parent[parent[v as usize] as usize];
        parent[v as usize] = up;
        v = up;
    }
    v
}

/// Index of `(row, col)`, `col <= row`, in a packed row-major lower triangle.
#[inline]
fn packed(row: usize, col: usize) -> usize {
    row * (row + 1) / 2 + col
}

impl DenseLocal {
    /// Factor `lap` in place, keeping allocations; `false` when a pivot
    /// reaches the rounding floor.
    pub(crate) fn build(&mut self, lap: &BlockLaplacian, cancel: &Cancel<'_>) -> Result<bool> {
        let levels = lap.levels;
        if levels >= ISOLATED as usize || lap.offsets.len() != lap.totals.len() + 1 {
            return Err(Error::InvalidInput(
                "block Laplacian layout is inconsistent",
            ));
        }
        self.levels = levels;
        self.components(lap, cancel)?;
        self.order_components()?;
        // Packed grounded factors: `k - 1` rows per component of `k` levels.
        let count = self.starts.len() - 1;
        self.factor_starts.clear();
        reserve(&mut self.factor_starts, count + 1)?;
        let mut total = 0_usize;
        let mut widest = 0;
        for c in 0..count {
            self.factor_starts.push(total);
            let k = self.starts[c + 1] - self.starts[c];
            widest = widest.max(k);
            total = (k * (k - 1) / 2)
                .checked_add(total)
                .ok_or(Error::OutOfMemory("dense block factor is too large"))?;
        }
        self.factor_starts.push(total);
        refill(&mut self.factor, total, 0.0)?;
        refill(&mut self.scratch, widest * BATCH, 0.0)?;
        self.form(lap, cancel)?;
        for c in 0..count {
            cancel.check()?;
            let n = self.starts[c + 1] - self.starts[c] - 1;
            let base = self.factor_starts[c];
            if !cholesky(&mut self.factor[base..base + n * (n + 1) / 2], n) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Union-find components (labels in ascending order of their smallest
    /// level) and weighted degrees `a_j (W_s - a_j) / W_s`, summed without
    /// cancellation from the other levels' weights.
    fn components(&mut self, lap: &BlockLaplacian, cancel: &Cancel<'_>) -> Result<()> {
        let levels = self.levels;
        self.component.clear();
        reserve(&mut self.component, levels)?;
        self.component.extend(0..levels as u32);
        refill(&mut self.degree, levels, 0.0)?;
        let parent = &mut self.component[..];
        for s in 0..lap.totals.len() {
            if s % 4096 == 0 {
                cancel.check()?;
            }
            let cells = lap.segment(s);
            let total = lap.totals[s];
            let mut before = 0.0;
            for &(j, a) in cells {
                self.degree[j as usize] += a * before / total;
                before += a;
            }
            let mut after = 0.0;
            for &(j, a) in cells.iter().rev() {
                self.degree[j as usize] += a * after / total;
                after += a;
            }
            if let Some((&(first, _), rest)) = cells.split_first() {
                for &(j, _) in rest {
                    let (a, b) = (root(parent, first), root(parent, j));
                    parent[a.max(b) as usize] = a.min(b);
                }
            }
        }
        // Unions and path halving only point a level at a smaller one, so in
        // ascending order a level's parent already holds its label.
        let mut count = 0_u32;
        for v in 0..levels {
            let p = parent[v] as usize;
            parent[v] = if p == v {
                count += 1;
                count - 1
            } else {
                parent[p]
            };
        }
        refill(&mut self.labels, count as usize, 0)?;
        for &label in parent.iter() {
            self.labels[label as usize] += 1;
        }
        Ok(())
    }

    /// Factored components (two or more levels) in label order, their
    /// levels in ascending order with the largest degree (first of equals)
    /// grounded last, and each level's position.
    fn order_components(&mut self) -> Result<()> {
        self.starts.clear();
        reserve(&mut self.starts, self.labels.len() + 1)?;
        let mut cursor = 0_usize;
        let mut factored = 0_u32;
        for size in &mut self.labels {
            if *size >= 2 {
                self.starts.push(cursor);
                cursor += *size as usize;
                *size = factored;
                factored += 1;
            } else {
                *size = ISOLATED;
            }
        }
        self.starts.push(cursor);
        refill(&mut self.order, cursor, 0)?;
        refill(&mut self.local, self.levels, ISOLATED)?;
        // `local` counts the levels placed per component while filling.
        for v in 0..self.levels {
            let c = self.labels[self.component[v] as usize];
            self.component[v] = c;
            if c != ISOLATED {
                let fill = &mut self.local[c as usize];
                let placed = if *fill == ISOLATED { 0 } else { *fill as usize };
                self.order[self.starts[c as usize] + placed] = v as u32;
                *fill = placed as u32 + 1;
            }
        }
        for c in 0..factored as usize {
            let members = &mut self.order[self.starts[c]..self.starts[c + 1]];
            let mut grounded = 0;
            for (i, &v) in members.iter().enumerate() {
                if self.degree[v as usize] > self.degree[members[grounded] as usize] {
                    grounded = i;
                }
            }
            members[grounded..].rotate_left(1);
        }
        self.local.fill(ISOLATED);
        for bounds in self.starts.windows(2) {
            for (i, &v) in self.order[bounds[0]..bounds[1]].iter().enumerate() {
                self.local[v as usize] = i as u32;
            }
        }
        Ok(())
    }

    /// The grounded Laplacians: every segment adds the edges
    /// `a_sj a_sk / W_s` between its levels.
    fn form(&mut self, lap: &BlockLaplacian, cancel: &Cancel<'_>) -> Result<()> {
        for s in 0..lap.totals.len() {
            if s % 4096 == 0 {
                cancel.check()?;
            }
            let cells = lap.segment(s);
            let Some(&(first, _)) = cells.first() else {
                continue;
            };
            let c = self.component[first as usize] as usize;
            let grounded = (self.starts[c + 1] - self.starts[c] - 1) as u32;
            let matrix = &mut self.factor[self.factor_starts[c]..self.factor_starts[c + 1]];
            let total = lap.totals[s];
            for (p, &(jp, ap)) in cells.iter().enumerate() {
                let lp = self.local[jp as usize];
                for &(jq, aq) in &cells[p + 1..] {
                    let lq = self.local[jq as usize];
                    let edge = ap * aq / total;
                    if lp != grounded {
                        matrix[packed(lp as usize, lp as usize)] += edge;
                    }
                    if lq != grounded {
                        matrix[packed(lq as usize, lq as usize)] += edge;
                        if lp != grounded {
                            let (row, col) = (lp.max(lq) as usize, lp.min(lq) as usize);
                            matrix[packed(row, col)] -= edge;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// In-place packed row-major Cholesky factorization of an `n x n` matrix;
/// `false` when a pivot reaches the rounding floor of its diagonal.
fn cholesky(a: &mut [f64], n: usize) -> bool {
    let floor = (4 * n).max(16) as f64 * f64::EPSILON;
    for i in 0..n {
        let (done, row) = a.split_at_mut(packed(i, 0));
        let row = &mut row[..=i];
        for j in 0..i {
            let base = packed(j, 0);
            let earlier = &done[base..=base + j];
            let mut s = row[j];
            for (x, y) in row[..j].iter().zip(&earlier[..j]) {
                s -= x * y;
            }
            row[j] = s / earlier[j];
        }
        let diagonal = row[i];
        let mut d = diagonal;
        for x in &row[..i] {
            d -= x * x;
        }
        if !(d.is_finite() && d > floor * diagonal) {
            return false;
        }
        row[i] = d.sqrt();
    }
    true
}

/// Mean removal over the `k` rows of `y`, column by column.
fn center(y: &mut [f64], k: usize, width: usize) {
    for c in 0..width {
        let mut sum = 0.0;
        for i in 0..k {
            sum += y[i * width + c];
        }
        let mean = sum / k as f64;
        for i in 0..k {
            y[i * width + c] -= mean;
        }
    }
}

/// `y = (L L')^-1 y` for the first `n` rows with the packed factor `l`.
fn solve(l: &[f64], y: &mut [f64], n: usize, width: usize) {
    for i in 0..n {
        let row = &l[packed(i, 0)..=packed(i, i)];
        let (done, rest) = y.split_at_mut(i * width);
        let yi = &mut rest[..width];
        for (t, &lit) in row[..i].iter().enumerate() {
            for (v, &d) in yi.iter_mut().zip(&done[t * width..(t + 1) * width]) {
                *v -= lit * d;
            }
        }
        for v in yi.iter_mut() {
            *v /= row[i];
        }
    }
    for i in (0..n).rev() {
        let row = &l[packed(i, 0)..=packed(i, i)];
        let (head, rest) = y.split_at_mut(i * width);
        let yi = &mut rest[..width];
        for v in yi.iter_mut() {
            *v /= row[i];
        }
        for (t, &lit) in row[..i].iter().enumerate() {
            for (v, &x) in head[t * width..(t + 1) * width].iter_mut().zip(yi.iter()) {
                *v -= lit * x;
            }
        }
    }
}

impl LocalSolve for DenseLocal {
    fn apply(
        &mut self,
        r: &[f64],
        z: &mut [f64],
        width: usize,
        _: Option<&rayon::ThreadPool>,
    ) -> Result<()> {
        let n = self.levels.checked_mul(width);
        if width == 0 || width > BATCH || n != Some(r.len()) || n != Some(z.len()) {
            return Err(Error::InvalidInput(
                "local solve dimensions are inconsistent",
            ));
        }
        for (c, bounds) in self.starts.windows(2).enumerate() {
            let members = &self.order[bounds[0]..bounds[1]];
            let k = members.len();
            let y = &mut self.scratch[..k * width];
            for (row, &v) in y.chunks_exact_mut(width).zip(members) {
                let v = v as usize;
                row.copy_from_slice(&r[v * width..(v + 1) * width]);
            }
            center(y, k, width);
            let l = &self.factor[self.factor_starts[c]..self.factor_starts[c + 1]];
            solve(l, y, k - 1, width);
            y[(k - 1) * width..].fill(0.0);
            center(y, k, width);
            for (row, &v) in y.chunks_exact(width).zip(members) {
                let v = v as usize;
                z[v * width..(v + 1) * width].copy_from_slice(row);
            }
        }
        for (row, &c) in z.chunks_exact_mut(width).zip(&self.component) {
            if c == ISOLATED {
                row.fill(0.0);
            }
        }
        Ok(())
    }

    fn bytes(&self) -> u128 {
        ((self.component.capacity()
            + self.local.capacity()
            + self.labels.capacity()
            + self.order.capacity())
            * 4
            + (self.degree.capacity()
                + self.starts.capacity()
                + self.factor.capacity()
                + self.factor_starts.capacity()
                + self.scratch.capacity())
                * 8) as u128
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two components ({0, 2, 5, 7} and {1, 3, 4}) and two isolated levels
    /// (6 and 8); segment totals are the sums of their entries.
    fn laplacian() -> BlockLaplacian {
        let segments: [&[(u32, f64)]; 5] = [
            &[(0, 1.0), (2, 0.5), (5, 2.0)],
            &[(5, 0.3), (7, 1.2)],
            &[(1, 1.0), (3, 1.0), (4, 0.25)],
            &[(3, 0.7), (4, 0.1)],
            &[(7, 0.9), (0, 0.4)],
        ];
        let mut lap = BlockLaplacian {
            levels: 9,
            offsets: vec![0],
            ..BlockLaplacian::default()
        };
        for cells in segments {
            lap.entries.extend_from_slice(cells);
            lap.offsets.push(lap.entries.len());
            lap.totals.push(cells.iter().map(|e| e.1).sum());
        }
        lap
    }

    /// `S_bb = Σ_s (diag(a_s) - a_s a_s' / W_s)`, dense.
    fn dense(lap: &BlockLaplacian) -> Vec<Vec<f64>> {
        let k = lap.levels;
        let mut l = vec![vec![0.0; k]; k];
        for s in 0..lap.totals.len() {
            for &(i, a) in lap.segment(s) {
                l[i as usize][i as usize] += a;
                for &(j, b) in lap.segment(s) {
                    l[i as usize][j as usize] -= a * b / lap.totals[s];
                }
            }
        }
        l
    }

    /// Gauss-Jordan inverse with partial pivoting.
    fn inverse(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let k = a.len();
        let mut m: Vec<Vec<f64>> = a
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mut row = row.clone();
                row.extend((0..k).map(|j| f64::from(u8::from(i == j))));
                row
            })
            .collect();
        for col in 0..k {
            let pivot = (col..k)
                .max_by(|&x, &y| m[x][col].abs().total_cmp(&m[y][col].abs()))
                .unwrap();
            m.swap(col, pivot);
            let d = m[col][col];
            for v in &mut m[col] {
                *v /= d;
            }
            for row in 0..k {
                if row != col {
                    let f = m[row][col];
                    let pivot_row = m[col].clone();
                    for (v, p) in m[row].iter_mut().zip(&pivot_row) {
                        *v -= f * p;
                    }
                }
            }
        }
        m.into_iter().map(|row| row[k..].to_vec()).collect()
    }

    fn multiply(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let k = a.len();
        (0..k)
            .map(|i| {
                (0..k)
                    .map(|j| (0..k).map(|t| a[i][t] * b[t][j]).sum())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn dense_local_solve_is_the_block_pseudo_inverse() {
        let lap = laplacian();
        let k = lap.levels;
        let mut solve = DenseLocal::default();
        assert!(solve.build(&lap, &Cancel(None)).unwrap());
        // The applied matrix, one unit vector at a time.
        let mut m = vec![vec![0.0; k]; k];
        for j in 0..k {
            let mut r = vec![0.0; k];
            r[j] = 1.0;
            let mut z = vec![f64::NAN; k];
            solve.apply(&r, &mut z, 1, None).unwrap();
            for i in 0..k {
                m[i][j] = z[i];
            }
        }
        // Reference: L^+ = (L + Π)^-1 - Π with Π the projection onto the
        // component constants (isolated levels are one-level components).
        let components: [&[usize]; 4] = [&[0, 2, 5, 7], &[1, 3, 4], &[6], &[8]];
        let mut projection = vec![vec![0.0; k]; k];
        for members in components {
            for &i in members {
                for &j in members {
                    projection[i][j] = 1.0 / members.len() as f64;
                }
            }
        }
        let l = dense(&lap);
        let shifted: Vec<Vec<f64>> = (0..k)
            .map(|i| (0..k).map(|j| l[i][j] + projection[i][j]).collect())
            .collect();
        let shifted_inverse = inverse(&shifted);
        for i in 0..k {
            for j in 0..k {
                let reference = shifted_inverse[i][j] - projection[i][j];
                assert!((m[i][j] - reference).abs() <= 1e-12, "({i}, {j})");
                assert!((m[i][j] - m[j][i]).abs() <= 1e-13, "symmetry ({i}, {j})");
            }
        }
        // Component constants are killed; the block is inverted on its
        // range (L M L = L), and the quadratic form is nonnegative.
        for members in components {
            for row in &m {
                let image: f64 = members.iter().map(|&j| row[j]).sum();
                assert!(image.abs() <= 1e-13);
            }
        }
        let lml = multiply(&multiply(&l, &m), &l);
        for i in 0..k {
            for j in 0..k {
                assert!((lml[i][j] - l[i][j]).abs() <= 1e-12);
            }
        }
        let mut seed = 7_u64;
        for _ in 0..20 {
            let x: Vec<f64> = (0..k)
                .map(|_| {
                    seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                    (seed >> 11) as f64 / (1_u64 << 53) as f64 - 0.5
                })
                .collect();
            let form: f64 = (0..k)
                .map(|i| (0..k).map(|j| x[i] * m[i][j] * x[j]).sum::<f64>())
                .sum();
            assert!(form >= -1e-14);
        }
        // Interleaved columns: column `c` of level `j` at `j * width + c`.
        let width = 3;
        let r: Vec<f64> = (0..k * width).map(|i| (i as f64 * 0.37).sin()).collect();
        let mut z = vec![f64::NAN; k * width];
        solve.apply(&r, &mut z, width, None).unwrap();
        for c in 0..width {
            for i in 0..k {
                let expected: f64 = (0..k).map(|j| m[i][j] * r[j * width + c]).sum();
                assert!((z[i * width + c] - expected).abs() <= 1e-12);
            }
        }
    }

    #[test]
    fn dense_rebuild_keeps_allocations() {
        let lap = laplacian();
        let mut solve = DenseLocal::default();
        assert!(solve.build(&lap, &Cancel(None)).unwrap());
        let bytes = solve.bytes();
        let factor = solve.factor.as_ptr();
        assert!(solve.build(&lap, &Cancel(None)).unwrap());
        assert_eq!(solve.bytes(), bytes);
        assert_eq!(solve.factor.as_ptr(), factor);
    }
}
