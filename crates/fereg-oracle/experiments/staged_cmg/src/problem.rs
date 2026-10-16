use cmg::{Components, Laplacian};

#[derive(Clone)]
pub struct Problem {
    pub n: usize,
    pub offsets: Vec<usize>,
    pub firms: Vec<usize>,
    pub weights: Vec<f64>,
    pub totals: Vec<f64>,
    pub diagonal: Vec<f64>,
    pub labels: Vec<usize>,
    pub sizes: Vec<usize>,
}

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / ((1u64 << 53) as f64)
    }
}

impl Problem {
    pub fn new(n: usize, workers: Vec<Vec<(usize, f64)>>) -> Self {
        let mut p = Self {
            n,
            offsets: vec![0],
            firms: vec![],
            weights: vec![],
            totals: vec![],
            diagonal: vec![0.; n],
            labels: vec![],
            sizes: vec![],
        };
        let mut links = Vec::new();
        for worker in workers {
            let mut cells = std::collections::BTreeMap::new();
            for (f, w) in worker {
                assert!(f < n && w.is_finite() && w > 0.);
                *cells.entry(f).or_insert(0.) += w;
            }
            let cells: Vec<_> = cells.into_iter().collect();
            if cells.is_empty() {
                continue;
            }
            let total: f64 = cells.iter().map(|c| c.1).sum();
            p.totals.push(total);
            let mut suffix = vec![0.; cells.len() + 1];
            for i in (0..cells.len()).rev() {
                suffix[i] = suffix[i + 1] + cells[i].1;
            }
            let mut prefix = 0.;
            for (i, &(f, w)) in cells.iter().enumerate() {
                p.firms.push(f);
                p.weights.push(w);
                p.diagonal[f] += w * ((prefix + suffix[i + 1]) / total);
                prefix += w;
                if i > 0 {
                    links.push((cells[0].0, f, 1.));
                }
            }
            p.offsets.push(p.firms.len());
        }
        let connectivity = Laplacian::from_edges(n, links).unwrap();
        let components = Components::from_laplacian(&connectivity);
        p.labels = components.labels().to_vec();
        p.sizes = components.sizes().to_vec();
        p
    }
    pub fn project(&self, x: &mut [f64]) {
        project(&self.labels, &self.sizes, x);
    }
    pub fn apply(&self, x: &[f64], y: &mut [f64]) {
        y.fill(0.);
        for (w, ends) in self.offsets.windows(2).enumerate() {
            let anchor = x[self.firms[ends[0]]];
            let mean: f64 = (ends[0]..ends[1])
                .map(|j| self.weights[j] * (x[self.firms[j]] - anchor))
                .sum::<f64>()
                / self.totals[w];
            for j in ends[0]..ends[1] {
                y[self.firms[j]] += self.weights[j] * ((x[self.firms[j]] - anchor) - mean);
            }
        }
    }
    pub fn graph(&self) -> Laplacian {
        let mut edges = Vec::with_capacity(self.edge_bound());
        for (w, ends) in self.offsets.windows(2).enumerate() {
            for i in ends[0]..ends[1] {
                for j in i + 1..ends[1] {
                    edges.push((
                        self.firms[i],
                        self.firms[j],
                        self.weights[i] * (self.weights[j] / self.totals[w]),
                    ));
                }
            }
        }
        Laplacian::from_edges(self.n, edges).unwrap()
    }
    pub fn edge_bound(&self) -> usize {
        self.offsets
            .windows(2)
            .map(|e| {
                let d = e[1] - e[0];
                d * (d - 1) / 2
            })
            .sum()
    }
    pub fn rhs(&self, count: usize) -> Vec<Vec<f64>> {
        let mut rng = Rng(1729);
        (0..count)
            .map(|k| {
                let mut b: Vec<_> = (0..self.n)
                    .map(|i| {
                        if k == 0 {
                            if i < self.n / 2 {
                                1.
                            } else {
                                -1.
                            }
                        } else {
                            2. * rng.unit() - 1.
                        }
                    })
                    .collect();
                self.project(&mut b);
                b
            })
            .collect()
    }
}

pub fn project(labels: &[usize], sizes: &[usize], x: &mut [f64]) {
    let mut sums = vec![0.; sizes.len()];
    for (&c, &v) in labels.iter().zip(x.iter()) {
        sums[c] += v;
    }
    for (s, &n) in sums.iter_mut().zip(sizes) {
        *s /= n as f64;
    }
    for (v, &c) in x.iter_mut().zip(labels) {
        *v -= sums[c];
    }
}
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
pub fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

pub fn generate(case: &str, n: usize) -> Problem {
    if case == "mosaic" {
        assert!(n >= 128 && n % 2 == 0);
        let mut workers = vec![];
        for (offset, name) in ["expander", "grid"].into_iter().enumerate() {
            let part = generate(name, n / 2);
            for e in part.offsets.windows(2) {
                workers.push(
                    (e[0]..e[1])
                        .map(|j| (2 * part.firms[j] + offset, part.weights[j]))
                        .collect(),
                );
            }
        }
        return Problem::new(n, workers);
    }

    assert!(n >= 64);
    let mut rng = Rng(0x97531);
    let mut workers = vec![];
    let mut edge = |a: usize, b: usize, w: f64| {
        if a != b {
            workers.push(vec![(a, 2. * w), (b, 2. * w)]);
        }
    };
    match case {
        "expander" | "hubs" => {
            for i in 0..n {
                edge(i, (i + 1) % n, 1.);
            }
            for _ in 0..3 * n {
                edge(rng.next() as usize % n, rng.next() as usize % n, 1.);
            }
        }
        "fringe" => {
            let core = n / 4;
            for i in 0..n - 1 {
                edge(i, i + 1, 1.);
            }
            for _ in 0..4 * core {
                edge(rng.next() as usize % core, rng.next() as usize % core, 1.);
            }
        }
        "clusters" => {
            for base in (0..n).step_by(64) {
                let size = 64.min(n - base);
                for i in 0..size {
                    edge(base + i, base + (i + 1) % size, 1.);
                }
                for _ in 0..3 * size {
                    edge(
                        base + rng.next() as usize % size,
                        base + rng.next() as usize % size,
                        1.,
                    );
                }
                if base > 0 {
                    edge(base - 1, base, 0.001);
                }
            }
        }
        "grid" => {
            let side = (n as f64).sqrt() as usize;
            assert_eq!(side * side, n);
            for i in 0..n {
                if i % side + 1 < side {
                    edge(i, i + 1, 10_f64.powf(6. * rng.unit() - 3.));
                }
                if i + side < n {
                    edge(i, i + side, 10_f64.powf(6. * rng.unit() - 3.));
                }
            }
        }
        _ => panic!("unknown case {case}"),
    }
    if case == "hubs" {
        for _ in 0..n / 8 {
            workers.push(
                (0..32)
                    .map(|_| (rng.next() as usize % n, 0.25 + rng.unit()))
                    .collect(),
            );
        }
    }
    Problem::new(n, workers)
}
