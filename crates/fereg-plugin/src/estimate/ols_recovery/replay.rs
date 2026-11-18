//! Independent observation-space OLS and covariance enclosures on the final
//! compact sample. Pivots, group means and sandwich influences are recomputed
//! from the supplied doubles, including the reported normalized intercept.
use super::interval::I;
use super::*;
use fereg_core::data::{PreparedWeights, WeightKind};

const K: usize = 16;
const GROUPS: usize = 64;
type Row = [I; K];
type Matrix = [Row; K];
const ZERO: Row = [I::ZERO; K];
const EMPTY: Matrix = [ZERO; K];
fn declined() -> FeregError {
    refusal("the independent original-unit regression and covariance cannot be certified")
}

#[derive(Clone, Copy)]
enum Column<'a> {
    Finite(usize),
    Constant,
    Dummy(&'a [u32], u32),
}

struct Design<'a> {
    data: &'a PreparedOlsData,
    cols: Vec<Column<'a>>,
    keep: Vec<Option<usize>>,
    constant: Option<usize>,
    origins: Row,
}

impl<'a> Design<'a> {
    fn new(data: &'a PreparedOlsData, fit: &CertifiedPreparedOlsFit) -> Result<Self> {
        let mut cols = reserved_empty(K, "bounded OLS replay columns")?;
        let p = fit.coefficients().len();
        if p > K {
            return Err(declined());
        }
        let mut keep = reserved_filled(p, None, "OLS reported slot replay")?;
        let mut constant = None;
        for c in fit.coefficients() {
            let slot = c.coefficient_slot.get() as usize - 1;
            match (c.status, c.generated_column_index) {
                (PreparedOlsFitStatus::Kept, Some(j)) => {
                    keep[slot] = Some(cols.len());
                    cols.push(Column::Finite(j));
                }
                (PreparedOlsFitStatus::NormalizedAbsorbedIntercept, _) => constant = Some(slot),
                _ => {}
            }
        }
        let topology = data.fe_topology();
        let finite_intercept=cols.iter().any(|col|matches!(col,Column::Finite(j) if matches!(data.model().columns[*j].kind, PreparedOlsColumnKind::Intercept)));
        if !finite_intercept
            && (constant.is_some() || data.structural_intercept() || data.has_absorbed_intercept())
        {
            if cols.len() >= K {
                return Err(declined());
            }
            cols.push(Column::Constant);
        }
        for (effect, recipe) in data
            .effect_specs()
            .iter()
            .zip(topology.remapped_effect_recipes())
        {
            if !effect.intercept || !effect.slopes.is_empty() || recipe.len() != 1 {
                return Err(declined());
            }
            let source = &topology.compacted_sources()[recipe[0]];
            if source.n_levels().saturating_sub(1) > K - cols.len() {
                return Err(declined());
            }
            for j in 1..source.n_levels() {
                cols.push(Column::Dummy(source.codes(), j as u32));
            }
        }
        if cols.is_empty() || data.sample().n_active().saturating_mul(cols.len().pow(2)) > 8 << 20 {
            return Err(declined());
        }
        let mut design = Self {
            data,
            cols,
            keep,
            constant,
            origins: ZERO,
        };
        if finite_intercept || design.cols.iter().any(|c| matches!(c, Column::Constant)) {
            design.choose_origins()?;
        }
        Ok(design)
    }
    /// Any point translation is exact in a model with a constant. A weighted
    /// location makes balanced finite and dummy columns exactly orthogonal
    /// when the input permits it, sharpening the independent interval solve.
    fn choose_origins(&mut self) -> Result<()> {
        let mut mass = I::ZERO;
        let mut sums = ZERO;
        for i in 0..self.data.sample().n_active() {
            if i % 1024 == 0 {
                fereg_core::interrupt::checkpoint()?;
            }
            let row = self.row(i)?;
            let w = self.weight(i);
            mass = mass + w;
            for (j, total) in sums[..self.cols.len()].iter_mut().enumerate() {
                *total = *total + w * row[j];
            }
        }
        for (j, &col) in self.cols.iter().enumerate() {
            let is_const = match col {
                Column::Constant => true,
                Column::Finite(k) => matches!(
                    self.data.model().columns[k].kind,
                    PreparedOlsColumnKind::Intercept
                ),
                Column::Dummy(..) => false,
            };
            if !is_const {
                let mean = sums[j] / mass;
                let center = mean.lo * 0.5 + mean.hi * 0.5;
                if center.is_finite() {
                    self.origins[j] = I::point(center);
                }
            }
        }
        Ok(())
    }
    fn row(&self, i: usize) -> Result<Row> {
        let mut row = ZERO;
        for (j, c) in self.cols.iter().enumerate() {
            row[j] = I::point(match c {
                Column::Finite(k) => self.data.column_value(*k, i)?,
                Column::Constant => 1.,
                Column::Dummy(codes, k) => f64::from(codes[i] == *k),
            }) - self.origins[j];
        }
        Ok(row)
    }
    fn weight(&self, i: usize) -> I {
        I::point(match &self.data.sample().weights {
            PreparedWeights::None => 1.,
            PreparedWeights::Provided { values, .. } => values[i],
        })
    }
    fn projection(&self, means: &Row, basis: Option<&Basis>, mean_anchor: I) -> Matrix {
        let mut l = EMPTY;
        for (s, c) in self.keep.iter().enumerate() {
            if let Some(j) = c {
                if matches!(self.cols[*j],Column::Finite(g) if matches!(self.data.model().columns[g].kind,PreparedOlsColumnKind::Intercept))
                {
                    for (cell, &origin) in l[s].iter_mut().zip(&self.origins) {
                        *cell = -origin;
                    }
                }
                l[s][*j] = I::ONE;
            }
        }
        if let Some(c) = self.constant {
            for (j, col) in self.cols.iter().enumerate() {
                l[c][j] = if matches!(col, Column::Finite(_)) {
                    -self.origins[j]
                } else {
                    means[j]
                };
            }
        }
        let Some(basis) = basis else { return l };
        let (source, target, a, scale) = match basis {
            Basis::Pair(r) => (
                Some(r.map.source),
                r.map.target,
                I::point(r.map.multiple),
                r.map.scale,
            ),
            Basis::Absorbed(r) => (self.constant, r.map.slot, mean_anchor, r.map.scale),
        };
        for j in 0..self.cols.len() {
            l[target][j] = l[target][j] * I::point(scale);
            if let Some(s) = source {
                l[s][j] = l[s][j] - a * l[target][j];
            }
        }
        l
    }
}

fn dot(x: &Row, y: &Row, n: usize) -> I {
    x[..n]
        .iter()
        .zip(&y[..n])
        .fold(I::ZERO, |sum, (x, y)| sum + *x * *y)
}

struct Equations {
    gram: Matrix,
    inverse: Matrix,
    b: Row,
    means: Row,
    mean_anchor: I,
}
impl Equations {
    fn solve(design: &Design<'_>, basis: Option<&Basis>) -> Result<Self> {
        let mut eq = Self {
            gram: EMPTY,
            inverse: EMPTY,
            b: ZERO,
            means: ZERO,
            mean_anchor: I::ZERO,
        };
        let k = design.cols.len();
        let mut total = I::ZERO;
        for (i, &y) in design.data.sample().outcome.iter().enumerate() {
            if i % 1024 == 0 {
                fereg_core::interrupt::checkpoint()?;
            }
            let x = design.row(i)?;
            let weight = design.weight(i);
            total = total + weight;
            if let Some(Basis::Absorbed(r)) = basis {
                eq.mean_anchor = eq.mean_anchor + weight * I::point(r.row_adjustment(i, 1., false));
            }
            for j in 0..k {
                let a = weight * x[j];
                eq.means[j] = eq.means[j] + a;
                eq.b[j] = eq.b[j] + a * I::point(y);
                for (l, &xl) in x[..=j].iter().enumerate() {
                    eq.gram[j][l] = eq.gram[j][l] + a * xl;
                }
            }
        }
        eq.mean_anchor = eq.mean_anchor / total;
        for j in 0..k {
            eq.means[j] = eq.means[j] / total;
            eq.inverse[j][j] = I::ONE;
            for l in 0..j {
                eq.gram[l][j] = eq.gram[j][l];
            }
        }
        // SPD on an independently full-rank basis; intervals containing zero
        // give an inconclusive solve. Keep the RHS separate from H^-1*y so
        // an exactly divisible fitted coefficient retains its singleton bound.
        for j in 0..k {
            let pivot = eq.gram[j][j];
            if pivot.lo <= 0. || !pivot.hi.is_finite() {
                return Err(declined());
            }
            for l in 0..k {
                eq.inverse[j][l] = eq.inverse[j][l] / pivot;
                if l > j {
                    eq.gram[j][l] = eq.gram[j][l] / pivot;
                }
            }
            eq.b[j] = eq.b[j] / pivot;
            for i in 0..k {
                if i == j {
                    continue;
                }
                let mult = eq.gram[i][j];
                eq.b[i] = eq.b[i] - mult * eq.b[j];
                for l in 0..k {
                    eq.inverse[i][l] = eq.inverse[i][l] - mult * eq.inverse[j][l];
                    if l > j {
                        eq.gram[i][l] = eq.gram[i][l] - mult * eq.gram[j][l];
                    }
                }
            }
        }
        Ok(eq)
    }
}

pub(super) fn enclose(
    data: &PreparedOlsData,
    view: &CertifiedOlsInference<'_>,
    basis: Option<&Basis>,
    coefficients: &[f64],
) -> Result<Covariance> {
    let design = Design::new(data, view.fit())?;
    let k = design.cols.len();
    let p = coefficients.len();
    let eq = Equations::solve(&design, basis)?;
    let l = design.projection(&eq.means, basis, eq.mean_anchor);
    let mut influence = EMPTY;
    for s in 0..p {
        if !dot(&l[s], &eq.b, k).close(coefficients[s], 1e-7, 1e-10) {
            return Err(refusal(&format!(
                "coefficient {}: posted {:e}, independent {:?}",
                s,
                coefficients[s],
                dot(&l[s], &eq.b, k)
            )));
        }
        for j in 0..k {
            for h in 0..k {
                influence[s][j] = influence[s][j] + l[s][h] * eq.inverse[h][j];
            }
        }
    }
    let cluster = match view {
        CertifiedOlsInference::Clustered(c) => {
            if c.cluster_counts().len() != 1 || c.cluster_counts()[0] > GROUPS {
                return Err(declined());
            }
            Some(data.cluster_codes(0)?)
        }
        _ => None,
    };
    let mut scores = [ZERO; GROUPS];
    let mut cov = EMPTY;
    let mut rss = I::ZERO;
    let iid = view.kind() == OlsVceKind::Iid;
    let frequency = matches!(
        data.sample().weights,
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            ..
        }
    );
    for (i, &y) in data.sample().outcome.iter().enumerate() {
        if i % 1024 == 0 {
            fereg_core::interrupt::checkpoint()?;
        }
        let x = design.row(i)?;
        let residual = I::point(y) - dot(&x, &eq.b, k);
        if !residual.close(view.fit().residuals()[i], 1e-7, 1e-10) {
            return Err(refusal(&format!(
                "residual row {}: posted {:e}, independent {:?}",
                i,
                view.fit().residuals()[i],
                residual
            )));
        }
        let w = design.weight(i);
        rss = rss + w * residual * residual;
        if iid {
            continue;
        }
        let mut score = ZERO;
        for j in 0..p {
            score[j] = dot(&influence[j], &x, k) * residual;
        }
        if let Some(codes) = cluster {
            let group = codes[i] as usize;
            for (entry, &value) in scores[group][..p].iter_mut().zip(&score[..p]) {
                *entry = *entry + w * value;
            }
        } else {
            let weight = if frequency { w } else { w * w };
            outer_add(&mut cov, &score, weight, p);
        }
    }
    let correction = match view {
        CertifiedOlsInference::Iid(c) => {
            for i in 0..p {
                for (j, form) in l[..=i].iter().enumerate() {
                    cov[i][j] = dot(&influence[i], form, k);
                }
            }
            rss / I::point(c.dof().residual_dof as f64)
        }
        CertifiedOlsInference::Robust(c) => {
            I::point(c.weights().effective_observations as f64)
                / I::point(c.dof().residual_dof as f64)
        }
        CertifiedOlsInference::Clustered(c) => {
            for score in &scores[..c.cluster_counts()[0]] {
                outer_add(&mut cov, score, I::ONE, p);
            }
            let n = I::point(c.weights().effective_observations as f64);
            let g = I::point(c.dof().minimum_cluster_count as f64);
            (n - I::ONE) / I::point(c.dof().correction_denominator_dof as f64) * (g / (g - I::ONE))
        }
    };
    for (i, row) in cov[..p].iter_mut().enumerate() {
        for cell in &mut row[..=i] {
            *cell = *cell * correction;
        }
    }
    Ok(Covariance { values: cov, p })
}

/// A bounded covariance on a specified reporting basis, with symmetric
/// entries enclosed independently of the production QR and score assembly.
pub(super) struct Covariance {
    values: Matrix,
    p: usize,
}
impl Covariance {
    pub(super) fn check(&self, values: &[f64]) -> Result<()> {
        for (i, row) in self.values[..self.p].iter().enumerate() {
            for (j, &exact) in row[..=i].iter().enumerate() {
                let v = values[i * self.p + j];
                if !exact.close(v, 1e-6, 1e-10)
                    || i == j && !exact.sqrt().close(v.sqrt(), 1e-6, 1e-10)
                {
                    return Err(refusal(&format!(
                        "covariance ({}, {}) is outside its independently verified accuracy",
                        i + 1,
                        j + 1
                    )));
                }
            }
        }
        Ok(())
    }
    pub(super) fn refine(&self, values: &mut [f64]) -> Result<()> {
        for (i, row) in self.values[..self.p].iter().enumerate() {
            for (j, &interval) in row[..=i].iter().enumerate() {
                let v = values[i * self.p + j];
                if v < interval.lo || v > interval.hi {
                    let value = if interval.lo == interval.hi {
                        interval.lo
                    } else {
                        interval.lo * 0.5 + interval.hi * 0.5
                    };
                    values[i * self.p + j] = value;
                    values[j * self.p + i] = value;
                }
            }
        }
        self.check(values)
    }
}
fn outer_add(matrix: &mut Matrix, score: &Row, w: I, n: usize) {
    for i in 0..n {
        let a = w * score[i];
        for (cell, &s) in matrix[i][..=i].iter_mut().zip(&score[..=i]) {
            *cell = *cell + a * s;
        }
    }
}
