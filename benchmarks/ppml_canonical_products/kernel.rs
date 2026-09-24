//! Development-only two-partition product experiment. No production routing.
//!
//! Eliminate the larger partition by group centering, factor a grounded,
//! diagonally scaled Schur matrix, and form H - U'U for every RHS together.
//! Original-input reconstruction supplies a separate projection witness.
use rayon::prelude::*;
use crate::fe::LevelIndex;

pub struct Geometry<'a> {
    left: &'a [u32],
    right: &'a [u32],
    index: &'a LevelIndex,
    weights: Vec<f64>,
    left_mass: Vec<f64>,
    right_mass: Vec<f64>,
    free: Vec<usize>,
    roots: Vec<f64>,
    factor: Vec<f64>,
    dimension: usize,
    pub setup_bytes: usize,
}

pub struct Products {
    pub gram: Vec<f64>,
    pub scales: Vec<f64>,
    left: Vec<f64>,
    right: Vec<f64>,
    pub certificate: f64,
    pub cancellation_bound: f64,
    pub usable: bool,
    pub bytes: usize,
    pub phases: [f64; 4],
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

impl<'a> Geometry<'a> {
    pub fn new(
        left: &'a [u32], right: &'a [u32], index: &'a LevelIndex,
        levels: [usize; 2], raw_weights: &[f64],
    ) -> Option<Self> {
        let [p, q] = levels;
        if p == 0 || q == 0 || left.len() != right.len() || left.len() != raw_weights.len() {
            return None;
        }
        let scale = raw_weights.iter().copied().fold(0., f64::max);
        if !scale.is_finite() || scale <= 0. { return None; }
        let weights: Vec<_> = raw_weights.iter().map(|w| w / scale).collect();
        if weights.iter().any(|w| !w.is_finite() || *w <= 0.) { return None; }
        let mut left_mass = vec![0.; p];
        let mut right_mass = vec![0.; q];
        for i in 0..weights.len() {
            left_mass[left[i] as usize] += weights[i];
            right_mass[right[i] as usize] += weights[i];
        }
        if left_mass.iter().chain(&right_mass).any(|w| !w.is_finite() || *w <= 0.) {
            return None;
        }
        let mut matrix = vec![0.; q.checked_mul(q)?];
        let mut parent: Vec<_> = (0..q).collect();
        let mut totals = vec![0.; q];
        let mut touched = Vec::with_capacity(q);
        for a in 0..p {
            for &i in index.checked_members(a).ok()? {
                let b = right[i] as usize;
                if totals[b] == 0. { touched.push(b); }
                totals[b] += weights[i];
            }
            touched.sort_unstable();
            let first = *touched.first()?;
            for (position, &b) in touched.iter().enumerate() {
                let rb = root(&mut parent, b);
                let rf = root(&mut parent, first);
                parent[rb] = rf;
                for &c in &touched[position + 1..] {
                    let hi = totals[b].max(totals[c]);
                    let lo = totals[b].min(totals[c]);
                    let value = (hi / left_mass[a]) * lo;
                    if value <= 0. || !value.is_finite() { return None; }
                    matrix[b*q+c] -= value;
                    matrix[c*q+b] -= value;
                    matrix[b*q+b] += value;
                    matrix[c*q+c] += value;
                }
            }
            for &b in &touched { totals[b] = 0.; }
            touched.clear();
        }
        let mut anchors = vec![usize::MAX; q];
        for b in 0..q {
            let r = root(&mut parent, b);
            let old = anchors[r];
            if old == usize::MAX || right_mass[b] > right_mass[old] { anchors[r] = b; }
        }
        let mut free = vec![usize::MAX; q];
        let mut dimension = 0;
        let mut roots = vec![0.; q];
        for b in 0..q {
            roots[b] = matrix[b*q+b].sqrt();
            if anchors[root(&mut parent, b)] != b {
                if !roots[b].is_finite() || roots[b] <= 0. { return None; }
                free[b] = dimension;
                dimension += 1;
            }
        }
        let mut factor = vec![0.; dimension.checked_mul(dimension)?];
        for b in 0..q {
            if free[b] == usize::MAX { continue; }
            for c in 0..q {
                if free[c] != usize::MAX {
                    factor[free[b]*dimension+free[c]] =
                        (matrix[b*q+c] / roots[b]) / roots[c];
                }
            }
        }
        // This is a speculative solve, never a rank decision. Weak geometry
        // refuses this representation and leaves the canonical route intact.
        let guard = 128. * f64::EPSILON * dimension.max(1) as f64;
        for j in 0..dimension {
            let mut diagonal = factor[j*dimension+j];
            for k in 0..j { diagonal -= factor[j*dimension+k].powi(2); }
            if !diagonal.is_finite() || diagonal <= guard { return None; }
            factor[j*dimension+j] = diagonal.sqrt();
            for i in j+1..dimension {
                let mut value = factor[i*dimension+j];
                for k in 0..j { value -= factor[i*dimension+k]*factor[j*dimension+k]; }
                factor[i*dimension+j] = value/factor[j*dimension+j];
            }
        }
        let setup_bytes = 8*(weights.len()+p+2*q+q*q+dimension*dimension)
            + std::mem::size_of::<usize>()*5*q;
        Some(Self { left, right, index, weights, left_mass, right_mass, free,
            roots, factor, dimension, setup_bytes })
    }

    pub fn products(&self, input: &[f64], width: usize, tolerance: f64) -> Option<Products> {
        self.products_with_policy(input, width, tolerance, false)
    }

    pub fn products_direct(&self, input: &[f64], width: usize, tolerance: f64) -> Option<Products> {
        self.products_with_policy(input, width, tolerance, true)
    }

    fn products_with_policy(&self, input: &[f64], width: usize, tolerance: f64, direct: bool) -> Option<Products> {
        let n = self.left.len();
        let p = self.left_mass.len();
        let q = self.right_mass.len();
        let d = self.dimension;
        if width == 0 || input.len() != n.checked_mul(width)? || input.iter().any(|x| !x.is_finite()) {
            return None;
        }
        let started = std::time::Instant::now();
        let mut scales = vec![0.; width];
        let mut left = vec![0.; p*width]; // column major
        left.par_chunks_mut(p).zip(scales.par_iter_mut()).enumerate().for_each(|(j, (means, norm))| {
            let mut scale = 0_f64;
            let mut square = 1.;
            for i in 0..n {
                let absolute = (input[i*width+j]*self.weights[i].sqrt()).abs();
                if absolute > scale {
                    square = 1. + square*(scale/absolute).powi(2);
                    scale = absolute;
                } else if absolute > 0. { square += (absolute/scale).powi(2); }
            }
            *norm = scale*square.sqrt();
            let divisor = if *norm == 0. { 1. } else { *norm };
            for a in 0..p {
                let mut sum = 0.;
                let mut correction = 0.;
                for &i in self.index.checked_members(a).unwrap() {
                    let value = (self.weights[i]/self.left_mass[a])*(input[i*width+j]/divisor);
                    let next = value - correction;
                    let updated = sum + next;
                    correction = (updated-sum)-next;
                    sum = updated;
                }
                means[a] = sum;
            }
        });
        if scales.iter().any(|x| !x.is_finite()) { return None; }
        for value in &mut scales { if *value == 0. { *value = 1.; } }
        let reference_seconds = started.elapsed().as_secs_f64();
        let started = std::time::Instant::now();
        let workers = rayon::current_num_threads().min(n);
        let stride = width*width+q*width+2*width;
        let mut partials = vec![0.; workers*stride];
        let rows_per_worker = n.div_ceil(workers);
        partials.par_chunks_mut(stride).enumerate().for_each(|(worker, partial)| {
            let (gram, rest) = partial.split_at_mut(width*width);
            let (cross, row) = rest.split_at_mut(q*width);
            for i in worker*rows_per_worker..((worker+1)*rows_per_worker).min(n) {
                let a = self.left[i] as usize;
                let b = self.right[i] as usize;
                let w = self.weights[i];
                for j in 0..width {
                    row[j] = input[i*width+j]/scales[j]-left[j*p+a];
                    cross[b*width+j] = w.mul_add(row[j], cross[b*width+j]);
                }
                if !direct {
                    for j in 0..width {
                        let value = w*row[j];
                        for k in j..width { gram[j*width+k] = value.mul_add(row[k], gram[j*width+k]); }
                    }
                }
            }
        });
        let mut gram = vec![0.; width*width];
        let mut cross = vec![0.; q*width];
        for slot in 0..width*width+q*width {
            let mut sum = 0.;
            let mut correction = 0.;
            for partial in partials.chunks(stride) {
                let term = partial[slot]-correction;
                let next = sum+term;
                correction = (next-sum)-term;
                sum = next;
            }
            if slot < width*width { gram[slot] = sum; } else { cross[slot-width*width] = sum; }
        }
        let moment_seconds = started.elapsed().as_secs_f64();
        let started = std::time::Instant::now();
        let mut solved = vec![0.; d*width]; // column major, forward solutions
        if d > 0 {
            solved.par_chunks_mut(d).enumerate().for_each(|(j, values)| {
                for b in 0..q {
                    if self.free[b] != usize::MAX { values[self.free[b]] = cross[b*width+j]/self.roots[b]; }
                }
                for i in 0..d {
                    for k in 0..i { values[i] -= self.factor[i*d+k]*values[k]; }
                    values[i] /= self.factor[i*d+i];
                }
            });
        }
        // A conservative subtraction diagnostic. The independent projection
        // witness below is still required; this is not a certified error bound
        // for the entire factorization, nor authority for canonical rank.
        let gamma = 64.*f64::EPSILON*(n+p+q+d+width).max(1) as f64;
        let mut bound = 0_f64;
        let mut usable = true;
        if !direct { for j in 0..width {
            for k in j..width {
                let mut correction = 0.;
                let mut absolute = 0.;
                for i in 0..d {
                    let term = solved[j*d+i]*solved[k*d+i];
                    correction += term;
                    absolute += term.abs();
                }
                let original = gram[j*width+k];
                let value = original-correction;
                let error = gamma*(original.abs()+absolute);
                if j == k {
                    bound = bound.max(error/value.abs().max(f64::MIN_POSITIVE));
                    if value <= error || !value.is_finite() { usable = false; }
                }
                gram[j*width+k] = value;
                gram[k*width+j] = value;
            }
        }}
        let mut right = vec![0.; q*width]; // column major
        right.par_chunks_mut(q).enumerate().for_each(|(j, values)| {
            let mut solution = solved[j*d..(j+1)*d].to_vec();
            for i in (0..d).rev() {
                for k in i+1..d { solution[i] -= self.factor[k*d+i]*solution[k]; }
                solution[i] /= self.factor[i*d+i];
            }
            for b in 0..q {
                if self.free[b] != usize::MAX { values[b] = solution[self.free[b]]/self.roots[b]; }
            }
        });
        // Replace first-partition-only means with complete FE coefficients.
        left.par_chunks_mut(p).enumerate().for_each(|(j, values)| {
            for a in 0..p {
                let mut sum = 0.;
                for &i in self.index.checked_members(a).unwrap() {
                    sum += (self.weights[i]/self.left_mass[a])*right[j*q+self.right[i] as usize];
                }
                values[a] -= sum;
            }
        });
        let solve_seconds = started.elapsed().as_secs_f64();
        let started = std::time::Instant::now();
        let certificate = if direct {
            // Each partition owns complete eliminated groups: left scores need
            // no p-by-width private bank. Gram and right scores share the row
            // reconstruction; a bounded q-by-width bank is reduced afterward.
            partials.fill(0.);
            let groups_per_worker = p.div_ceil(workers);
            partials.par_chunks_mut(stride).enumerate().for_each(|(worker, partial)| {
                let (gram, rest) = partial.split_at_mut(width*width);
                let (right_scores, rest) = rest.split_at_mut(q*width);
                let (row, left_maximum) = rest.split_at_mut(width);
                let mut left_scores = vec![0.; width];
                for a in worker*groups_per_worker..((worker+1)*groups_per_worker).min(p) {
                    left_scores.fill(0.);
                    for &i in self.index.checked_members(a).unwrap() {
                        let b = self.right[i] as usize;
                        let w = self.weights[i];
                        for j in 0..width {
                            row[j] = input[i*width+j]/scales[j]-left[j*p+a]-right[j*q+b];
                            let score = w*row[j];
                            left_scores[j] += score;
                            right_scores[b*width+j] += score;
                            if !row[j].is_finite() { left_maximum[j] = f64::INFINITY; }
                        }
                        for j in 0..width {
                            let value = w*row[j];
                            for k in j..width { gram[j*width+k] = value.mul_add(row[k], gram[j*width+k]); }
                        }
                    }
                    for j in 0..width {
                        let value = left_scores[j].abs()/self.left_mass[a].sqrt();
                        left_maximum[j] = left_maximum[j].max(value);
                        if !value.is_finite() { left_maximum[j] = f64::INFINITY; }
                    }
                }
            });
            let mut maximum = 0_f64;
            for slot in 0..width*width+q*width {
                let mut sum = 0.; let mut correction = 0.;
                for partial in partials.chunks(stride) {
                    let term = partial[slot]-correction; let next = sum+term;
                    correction = (next-sum)-term; sum = next;
                }
                if slot < width*width { gram[slot] = sum; }
                else {
                    let group = (slot-width*width)/width;
                    let score = sum.abs()/self.right_mass[group].sqrt();
                    maximum = maximum.max(score);
                    if !score.is_finite() { maximum = f64::INFINITY; }
                }
            }
            for j in 0..width { for k in 0..j { gram[j*width+k] = gram[k*width+j]; } }
            for partial in partials.chunks(stride) {
                for &value in &partial[stride-width..] { maximum = maximum.max(value); }
            }
            usable &= gram.iter().all(|x| x.is_finite());
            maximum
        } else {
          let mut checks = vec![0.; width];
          checks.par_iter_mut().enumerate().for_each(|(j, check)| {
            let mut sums = vec![0.; p+q];
            let mut norm = 0.;
            for i in 0..n {
                let a = self.left[i] as usize;
                let b = self.right[i] as usize;
                let r = input[i*width+j]/scales[j]-left[j*p+a]-right[j*q+b];
                let wr = self.weights[i]*r;
                sums[a] += wr;
                sums[p+b] += wr;
                norm = wr.mul_add(r, norm);
            }
            let denominator = norm.sqrt().max(1.);
            let mut maximum = 0_f64;
            for (sum, mass) in sums.iter().zip(self.left_mass.iter().chain(&self.right_mass)) {
                maximum = maximum.max((sum.abs()/mass.sqrt())/denominator);
            }
            if !norm.is_finite() || sums.iter().any(|x| !x.is_finite()) { maximum = f64::INFINITY; }
            *check = maximum;
          });
          checks.into_iter().fold(0_f64, f64::max)
        };
        usable &= certificate <= tolerance;
        let certificate_seconds = started.elapsed().as_secs_f64();
        let bytes = 8*(self.weights.len()+p+2*q+d*d + p*width+q*width
            + workers*stride+width*width+q*width+d*width+width*3
            + workers*(p+q+d+width)) + std::mem::size_of::<usize>()*q;
        Some(Products { gram, scales, left, right, certificate,
            cancellation_bound: bound, usable, bytes,
            phases: [reference_seconds, moment_seconds, solve_seconds, certificate_seconds] })
    }

    pub fn residuals(&self, input: &[f64], width: usize, result: &Products) -> Vec<f64> {
        let p = self.left_mass.len();
        let q = self.right_mass.len();
        input.par_chunks(width).enumerate().flat_map_iter(|(i, row)| {
            (0..width).map(move |j| (row[j]/result.scales[j]
                - result.left[j*p+self.left[i] as usize]
                - result.right[j*q+self.right[i] as usize])*result.scales[j])
        }).collect()
    }
}
