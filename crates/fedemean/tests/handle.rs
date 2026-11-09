//! The FE preconditioner handle ([`Projector::fe_preconditioner`]):
//! against dense references, the assembled action is symmetric positive
//! definite on all of coefficient space for Jacobi, channel blocks (slopes,
//! offset slopes included) and every Schwarz rule; Jacobi is the inverse raw
//! level masses; exact Schwarz on two effects inverts `D'WD` on its range;
//! a block preconditioner with the handle as its FE block solves a joint
//! `[X D]` least-squares problem to the dense solution; the action scales
//! with the weights, is bitwise independent of the pool, leaves the
//! projector's projections unchanged, is admitted at its reported workspace,
//! and its build is cancellable.

use fedemean::{
    Choice, CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Decline, Design, Effect,
    Error, Fallback, FePreconditioner, Method, Outcome, Preconditioner, PreconditionerUsed,
    Projector, Reduction, ReductionUsed, Request, Settings, Weights,
};
#[cfg(feature = "schwarz")]
use fedemean::{SchwarzLocal, SchwarzSpec, Subdomains};

/// SplitMix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }
}

/// A design: codes per effect (every level observed), each effect's slopes
/// and whether it has an intercept.
struct Case {
    n: usize,
    levels: Vec<usize>,
    codes: Vec<Vec<u32>>,
    slopes: Vec<Vec<Vec<f64>>>,
    intercept: Vec<bool>,
}

impl Case {
    fn random(seed: u64, n: usize, levels: &[usize]) -> Self {
        let mut rng = Rng(seed);
        let codes = levels
            .iter()
            .map(|&count| {
                let mut codes: Vec<u32> = (0..n)
                    .map(|i| if i < count { i } else { rng.below(count) } as u32)
                    .collect();
                for i in (1..n).rev() {
                    codes.swap(i, rng.below(i + 1));
                }
                codes
            })
            .collect();
        Self {
            n,
            levels: levels.to_vec(),
            codes,
            slopes: vec![Vec::new(); levels.len()],
            intercept: vec![true; levels.len()],
        }
    }

    /// Effect `e` gains a slope `offset + u` (u uniform on [0, 2)).
    fn with_slope(mut self, e: usize, seed: u64, offset: f64) -> Self {
        let mut rng = Rng(seed);
        let slope = (0..self.n).map(|_| offset + 2.0 * rng.uniform()).collect();
        self.slopes[e].push(slope);
        self
    }

    fn design(&self) -> Design<'_> {
        let effects = (0..self.levels.len())
            .map(|e| {
                let effect = Effect::new(&self.codes[e], self.levels[e]);
                if self.slopes[e].is_empty() {
                    effect
                } else {
                    effect
                        .with_slopes(self.intercept[e], self.slopes[e].iter().map(Vec::as_slice))
                        .unwrap()
                }
            })
            .collect();
        Design::new(self.n, effects).unwrap()
    }

    fn channels(&self, e: usize) -> usize {
        usize::from(self.intercept[e]) + self.slopes[e].len()
    }

    /// Each effect's first coefficient in the canonical layout, with the
    /// count `L` last.
    fn offsets(&self) -> Vec<usize> {
        let mut offsets = vec![0];
        for e in 0..self.levels.len() {
            offsets.push(offsets[e] + self.levels[e] * self.channels(e));
        }
        offsets
    }

    /// Row `i` of `D` in the canonical layout (channel-major per effect).
    fn row(&self, i: usize) -> Vec<f64> {
        let offsets = self.offsets();
        let mut row = vec![0.0; offsets[self.levels.len()]];
        for e in 0..self.levels.len() {
            let l = self.codes[e][i] as usize;
            let mut channel = 0;
            if self.intercept[e] {
                row[offsets[e] + l] = 1.0;
                channel = 1;
            }
            for (s, slope) in self.slopes[e].iter().enumerate() {
                row[offsets[e] + (channel + s) * self.levels[e] + l] = slope[i];
            }
        }
        row
    }

    /// Dense `D'WD`.
    fn normal(&self, weights: &[f64]) -> Vec<Vec<f64>> {
        let p = self.offsets()[self.levels.len()];
        let mut normal = vec![vec![0.0; p]; p];
        for (i, &w) in weights.iter().enumerate() {
            let row = self.row(i);
            let nonzero: Vec<usize> = (0..p).filter(|&a| row[a] != 0.0).collect();
            for &a in &nonzero {
                for &b in &nonzero {
                    normal[a][b] += w * row[a] * row[b];
                }
            }
        }
        normal
    }

    fn weights(&self, seed: u64) -> Vec<f64> {
        let mut rng = Rng(seed);
        (0..self.n).map(|_| 0.3 + 2.5 * rng.uniform()).collect()
    }
}

fn build(
    case: &Case,
    weights: &[f64],
    preconditioner: Preconditioner,
    limit: Option<u128>,
) -> Result<Result<FePreconditioner, Decline>, Error> {
    let mut projector = Projector::new(case.design(), Settings::default());
    projector.fe_preconditioner(
        Weights::Values(weights),
        preconditioner,
        limit,
        &Control::default(),
    )
}

fn built(case: &Case, weights: &[f64], preconditioner: Preconditioner) -> FePreconditioner {
    build(case, weights, preconditioner, None)
        .unwrap()
        .expect("handle")
}

/// The dense action `M^-1` (row-major), column by column.
fn assemble(handle: &FePreconditioner) -> Vec<f64> {
    let p = handle.coefficients();
    let mut m = vec![0.0; p * p];
    let mut e = vec![0.0; p];
    let mut z = vec![0.0; p];
    for j in 0..p {
        e[j] = 1.0;
        handle.apply(&e, &mut z).unwrap();
        e[j] = 0.0;
        for (i, &v) in z.iter().enumerate() {
            m[i * p + j] = v;
        }
    }
    m
}

/// Eigenvalues of a symmetric matrix by cyclic Jacobi rotations.
fn eigenvalues(mut a: Vec<f64>, n: usize) -> Vec<f64> {
    for _ in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|p| (0..n).filter(move |&q| q != p).map(move |q| (p, q)))
            .map(|(p, q)| a[p * n + q] * a[p * n + q])
            .sum();
        let total: f64 = a.iter().map(|x| x * x).sum();
        if off <= 1e-30 * total {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                if apq == 0.0 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t = if theta == 0.0 {
                    1.0
                } else {
                    theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt())
                };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (kp, kq) = (a[k * n + p], a[k * n + q]);
                    a[k * n + p] = c * kp - s * kq;
                    a[k * n + q] = s * kp + c * kq;
                }
                for k in 0..n {
                    let (pk, qk) = (a[p * n + k], a[q * n + k]);
                    a[p * n + k] = c * pk - s * qk;
                    a[q * n + k] = s * pk + c * qk;
                }
            }
        }
    }
    (0..n).map(|i| a[i * n + i]).collect()
}

/// Symmetric (to rounding) with every eigenvalue positive.
fn assert_spd(m: &[f64], p: usize, label: &str) {
    let scale = m.iter().fold(0.0_f64, |s, v| s.max(v.abs()));
    for i in 0..p {
        for j in 0..i {
            let gap = (m[i * p + j] - m[j * p + i]).abs();
            assert!(gap <= 1e-12 * scale, "{label} ({i}, {j}): {gap}");
        }
    }
    let lambda = eigenvalues(m.to_vec(), p);
    let (lo, hi) = lambda
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |(lo, hi), &l| {
            (lo.min(l), hi.max(l))
        });
    assert!(lo > 1e-9 * hi, "{label}: eigenvalues in [{lo}, {hi}]");
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

fn preconditioners() -> Vec<Preconditioner> {
    #[allow(unused_mut)]
    let mut list = vec![Preconditioner::Jacobi, Preconditioner::Auto];
    #[cfg(feature = "schwarz")]
    for subdomains in [Subdomains::Star, Subdomains::AllPairs] {
        for local in [
            SchwarzLocal::Auto,
            SchwarzLocal::Exact,
            SchwarzLocal::Approximate,
        ] {
            list.push(Preconditioner::Schwarz(SchwarzSpec { subdomains, local }));
        }
    }
    list
}

/// Jacobi is the inverse raw level mass; every choice is symmetric positive
/// definite on all of coefficient space (although `D'WD` is singular).
#[test]
fn every_choice_is_symmetric_positive_definite_and_jacobi_is_the_inverse_mass() {
    let case = Case::random(11, 150, &[30, 12, 5, 3]);
    let weights = case.weights(12);
    let p = case.offsets()[4];
    let normal = case.normal(&weights);
    for preconditioner in preconditioners() {
        let handle = built(&case, &weights, preconditioner);
        assert_eq!(handle.coefficients(), p);
        let m = assemble(&handle);
        let label = format!("{preconditioner:?}");
        assert_spd(&m, p, &label);
        if preconditioner == Preconditioner::Jacobi {
            assert_eq!(handle.used(), PreconditionerUsed::Jacobi);
            for g in 0..p {
                let expected = 1.0 / normal[g][g];
                assert!((m[g * p + g] - expected).abs() <= 1e-14 * expected, "{g}");
                for h in (0..p).filter(|&h| h != g) {
                    assert_eq!(m[g * p + h], 0.0);
                }
            }
        }
        #[cfg(feature = "schwarz")]
        if preconditioner != Preconditioner::Jacobi {
            assert!(
                matches!(handle.used(), PreconditionerUsed::Schwarz { .. }),
                "{label}"
            );
        }
    }
}

/// Preconditioned CG on `D'WD x = D'W y` with the handle; the fitted
/// values `D x` and the iterations.
fn fe_pcg(case: &Case, weights: &[f64], handle: &FePreconditioner, y: &[f64]) -> (Vec<f64>, usize) {
    let p = handle.coefficients();
    let rows: Vec<Vec<f64>> = (0..case.n).map(|i| case.row(i)).collect();
    let normal = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; p];
        for (i, row) in rows.iter().enumerate() {
            let fit: f64 = row.iter().zip(v).map(|(a, b)| a * b).sum();
            for (o, a) in out.iter_mut().zip(row) {
                *o += weights[i] * fit * a;
            }
        }
        out
    };
    let mut r = vec![0.0; p];
    for (i, row) in rows.iter().enumerate() {
        for (o, a) in r.iter_mut().zip(row) {
            *o += weights[i] * y[i] * a;
        }
    }
    let b_norm = r.iter().map(|v| v * v).sum::<f64>().sqrt();
    let mut x = vec![0.0; p];
    let mut z = vec![0.0; p];
    handle.apply(&r, &mut z).unwrap();
    let mut d = z.clone();
    let mut rz: f64 = r.iter().zip(&z).map(|(a, b)| a * b).sum();
    let mut iterations = 0;
    while r.iter().map(|v| v * v).sum::<f64>().sqrt() > 1e-13 * b_norm && iterations < 3000 {
        let q = normal(&d);
        let alpha = rz / d.iter().zip(&q).map(|(a, b)| a * b).sum::<f64>();
        for j in 0..p {
            x[j] += alpha * d[j];
            r[j] -= alpha * q[j];
        }
        handle.apply(&r, &mut z).unwrap();
        let next: f64 = r.iter().zip(&z).map(|(a, b)| a * b).sum();
        for j in 0..p {
            d[j] = z[j] + next / rz * d[j];
        }
        rz = next;
        iterations += 1;
    }
    let fitted = rows
        .iter()
        .map(|row| row.iter().zip(&x).map(|(a, b)| a * b).sum())
        .collect();
    (fitted, iterations)
}

/// With slopes (one plain, one without an intercept), Jacobi is the
/// levels' channel blocks and every choice stays symmetric positive
/// definite in the canonical raw layout. With an offset slope (taken in its
/// levels' centered basis internally) the raw-basis action
/// `T^-1 M_op^-1 T^-T` has the raw normal matrix's poor scaling, beyond a
/// dense eigenvalue check: it stays symmetric with positive forms, and
/// preconditions CG to the dense fit.
#[test]
fn designs_with_slopes_keep_a_symmetric_positive_definite_action() {
    let make = |offset: f64| {
        let mut case = Case::random(21, 300, &[20, 9, 4])
            .with_slope(0, 22, 0.0)
            .with_slope(1, 23, offset)
            .with_slope(2, 24, 0.5);
        case.intercept[2] = false;
        case
    };
    for offset in [1.0, 5000.0] {
        let case = make(offset);
        let weights = case.weights(25);
        let p = case.offsets()[3];
        let mut rng = Rng(26);
        let y: Vec<f64> = (0..case.n)
            .map(|i| case.slopes[1][0][i] * 1e-3 + case.codes[0][i] as f64 * 0.1 + rng.uniform())
            .collect();
        // The raw normal equations at offset 5000 are beyond the dense
        // elimination's accuracy: there the reference is fedemean's own
        // certified projection (centered basis, its own recurrence).
        let reference = if offset == 1.0 {
            dense_fit(&case, &[], &weights, &y)
        } else {
            let settings = Settings {
                choice: Choice {
                    reduction: Reduction::Full,
                    method: Method::Pcg,
                    ..Choice::default()
                },
                lanes: None,
            };
            let mut projector = Projector::new(case.design(), settings);
            let mut data = y.clone();
            let (outcome, _) = layout_project(&mut projector, &case, &weights, &mut data);
            assert!(matches!(outcome, Outcome::Projected(_)));
            y.iter().zip(&data).map(|(a, r)| a - r).collect()
        };
        for preconditioner in preconditioners() {
            #[cfg(feature = "schwarz")]
            if let Preconditioner::Schwarz(spec) = preconditioner {
                if spec.local == SchwarzLocal::Exact {
                    let error = build(&case, &weights, preconditioner, None).unwrap_err();
                    assert!(matches!(error, Error::Unsupported(_)));
                    continue;
                }
            }
            let handle = built(&case, &weights, preconditioner);
            let label = format!("offset {offset} {preconditioner:?}");
            if preconditioner == Preconditioner::Jacobi {
                assert!(matches!(
                    handle.used(),
                    PreconditionerUsed::ChannelBlocks { .. }
                ));
            }
            let m = assemble(&handle);
            if offset == 1.0 {
                assert_spd(&m, p, &label);
            } else {
                let scale = m.iter().fold(0.0_f64, |s, v| s.max(v.abs()));
                for i in 0..p {
                    for j in 0..i {
                        assert!(
                            (m[i * p + j] - m[j * p + i]).abs() <= 1e-12 * scale,
                            "{label}"
                        );
                    }
                }
                for _ in 0..8 {
                    let v: Vec<f64> = (0..p).map(|_| rng.uniform() - 0.5).collect();
                    let mut z = vec![0.0; p];
                    handle.apply(&v, &mut z).unwrap();
                    let form: f64 = v.iter().zip(&z).map(|(a, b)| a * b).sum();
                    assert!(form > 0.0, "{label}: {form}");
                }
            }
            let (fitted, iterations) = fe_pcg(&case, &weights, &handle, &y);
            assert!(iterations < 3000, "{label}");
            let bound = 1e-7 * reference.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
            for (a, b) in fitted.iter().zip(&reference) {
                assert!((a - b).abs() <= bound, "{label}: {a} vs {b}");
            }
        }
    }
}

/// Exact Schwarz on two effects is one subdomain per component, solved
/// exactly: on the range of `D'WD` it is its inverse, `N M^-1 N x = N x`.
#[cfg(feature = "schwarz")]
#[test]
fn exact_schwarz_on_two_effects_inverts_the_normal_matrix_on_its_range() {
    let case = Case::random(31, 400, &[60, 25]);
    let weights = case.weights(32);
    let p = case.offsets()[2];
    let normal = case.normal(&weights);
    let exact = Preconditioner::Schwarz(SchwarzSpec {
        subdomains: Subdomains::Star,
        local: SchwarzLocal::Exact,
    });
    let handle = built(&case, &weights, exact);
    let mut rng = Rng(33);
    let times = |x: &[f64]| -> Vec<f64> {
        (0..p)
            .map(|a| (0..p).map(|b| normal[a][b] * x[b]).sum())
            .collect()
    };
    for _ in 0..4 {
        let x: Vec<f64> = (0..p).map(|_| rng.uniform() - 0.5).collect();
        let v = times(&x);
        let mut z = vec![0.0; p];
        handle.apply(&v, &mut z).unwrap();
        let back = times(&z);
        let scale = v.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
        for (a, b) in back.iter().zip(&v) {
            assert!((a - b).abs() <= 1e-10 * scale, "{a} vs {b}");
        }
    }
}

/// Weights scaled by a power of two scale the action by its inverse,
/// bitwise.
#[test]
fn the_action_is_at_the_callers_weight_scale() {
    let case = Case::random(41, 300, &[40, 15, 6]);
    let weights = case.weights(42);
    let scaled: Vec<f64> = weights.iter().map(|w| w * 1024.0).collect();
    let mut rng = Rng(43);
    let p = case.offsets()[3];
    let r: Vec<f64> = (0..p).map(|_| rng.uniform() - 0.5).collect();
    for preconditioner in preconditioners() {
        let (mut z, mut y) = (vec![0.0; p], vec![0.0; p]);
        built(&case, &weights, preconditioner)
            .apply(&r, &mut z)
            .unwrap();
        built(&case, &scaled, preconditioner)
            .apply(&r, &mut y)
            .unwrap();
        let expected: Vec<f64> = z.iter().map(|v| v / 1024.0).collect();
        assert_eq!(bits(&y), bits(&expected), "{preconditioner:?}");
    }
}

/// An independent dense weighted least-squares fit of `y` on `[X D]`
/// (Gaussian elimination with complete pivoting on the normal equations,
/// dropping null pivots): the fitted values.
fn dense_fit(case: &Case, x: &[Vec<f64>], weights: &[f64], y: &[f64]) -> Vec<f64> {
    let k = x.len();
    let p = case.offsets()[case.levels.len()];
    let width = k + p;
    let row = |i: usize| {
        let mut values: Vec<f64> = x.iter().map(|c| c[i]).collect();
        values.extend(case.row(i));
        values
    };
    let mut normal = vec![vec![0.0; width + 1]; width];
    for i in 0..case.n {
        let values = row(i);
        for a in 0..width {
            for b in 0..width {
                normal[a][b] += weights[i] * values[a] * values[b];
            }
            normal[a][width] += weights[i] * values[a] * y[i];
        }
    }
    let scale = (0..width).map(|a| normal[a][a].abs()).fold(0.0, f64::max);
    let mut order: Vec<usize> = (0..width).collect();
    let mut rank = 0;
    for pivot in 0..width {
        let (mut best, mut value) = (pivot, 0.0);
        for c in pivot..width {
            let v = normal[order[c]][order[c]].abs();
            if v > value {
                (best, value) = (c, v);
            }
        }
        if value <= 1e-11 * scale {
            break;
        }
        order.swap(pivot, best);
        let q = order[pivot];
        for &r in &order[pivot + 1..] {
            let factor = normal[r][q] / normal[q][q];
            for c in 0..=width {
                normal[r][c] -= factor * normal[q][c];
            }
        }
        rank += 1;
    }
    let mut theta = vec![0.0; width];
    for pivot in (0..rank).rev() {
        let q = order[pivot];
        let mut value = normal[q][width];
        for &c in &order[pivot + 1..rank] {
            value -= normal[q][c] * theta[c];
        }
        theta[q] = value / normal[q][q];
    }
    (0..case.n)
        .map(|i| row(i).iter().zip(&theta).map(|(a, t)| a * t).sum())
        .collect()
}

/// The handle as the FE block of a block-diagonal preconditioner of the
/// joint normal equations of `W^1/2 [X D]` (the controls' block their exact
/// Gram inverse): preconditioned CG reaches the dense joint fit.
#[test]
fn a_block_preconditioner_with_the_handle_solves_a_joint_fit() {
    let case = Case::random(51, 500, &[70, 30, 6]);
    let weights = case.weights(52);
    let mut rng = Rng(53);
    let codes = &case.codes;
    let x: Vec<Vec<f64>> = (0..2)
        .map(|c| {
            (0..case.n)
                .map(|i| (codes[0][i] as f64 * 0.01 + c as f64) * rng.uniform() + rng.uniform())
                .collect()
        })
        .collect();
    let y: Vec<f64> = (0..case.n)
        .map(|i| x[0][i] - 0.5 * x[1][i] + codes[1][i] as f64 * 0.1 + rng.uniform())
        .collect();
    let reference = dense_fit(&case, &x, &weights, &y);
    let p = case.offsets()[3];
    let k = x.len();
    let rows: Vec<Vec<f64>> = (0..case.n)
        .map(|i| {
            let mut r: Vec<f64> = x.iter().map(|c| c[i]).collect();
            r.extend(case.row(i));
            r
        })
        .collect();
    // y = A' W A x and the controls' Gram inverse.
    let normal = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; k + p];
        for (i, row) in rows.iter().enumerate() {
            let fit: f64 = row.iter().zip(v).map(|(a, b)| a * b).sum();
            for (o, a) in out.iter_mut().zip(row) {
                *o += weights[i] * fit * a;
            }
        }
        out
    };
    let mut gram = [[0.0; 2]; 2];
    for (i, &w) in weights.iter().enumerate() {
        for a in 0..2 {
            for b in 0..2 {
                gram[a][b] += w * x[a][i] * x[b][i];
            }
        }
    }
    let det = gram[0][0] * gram[1][1] - gram[0][1] * gram[1][0];
    let inverse = [
        [gram[1][1] / det, -gram[0][1] / det],
        [-gram[1][0] / det, gram[0][0] / det],
    ];
    for preconditioner in preconditioners() {
        let handle = built(&case, &weights, preconditioner);
        let apply = |r: &[f64], z: &mut [f64]| {
            z[0] = inverse[0][0] * r[0] + inverse[0][1] * r[1];
            z[1] = inverse[1][0] * r[0] + inverse[1][1] * r[1];
            handle.apply(&r[k..], &mut z[k..]).unwrap();
        };
        let mut b = vec![0.0; k + p];
        for (i, row) in rows.iter().enumerate() {
            for (o, a) in b.iter_mut().zip(row) {
                *o += weights[i] * y[i] * a;
            }
        }
        let b_norm = b.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut theta = vec![0.0; k + p];
        let mut r = b.clone();
        let mut z = vec![0.0; k + p];
        apply(&r, &mut z);
        let mut d = z.clone();
        let mut rz: f64 = r.iter().zip(&z).map(|(a, b)| a * b).sum();
        let mut iterations = 0;
        while r.iter().map(|v| v * v).sum::<f64>().sqrt() > 1e-13 * b_norm && iterations < 2000 {
            let q = normal(&d);
            let alpha = rz / d.iter().zip(&q).map(|(a, b)| a * b).sum::<f64>();
            for j in 0..k + p {
                theta[j] += alpha * d[j];
                r[j] -= alpha * q[j];
            }
            apply(&r, &mut z);
            let next: f64 = r.iter().zip(&z).map(|(a, b)| a * b).sum();
            for j in 0..k + p {
                d[j] = z[j] + next / rz * d[j];
            }
            rz = next;
            iterations += 1;
        }
        assert!(iterations < 2000, "{preconditioner:?}");
        let scale = reference.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        for (i, row) in rows.iter().enumerate() {
            let fit: f64 = row.iter().zip(&theta).map(|(a, b)| a * b).sum();
            assert!(
                (fit - reference[i]).abs() <= 1e-8 * scale,
                "{preconditioner:?} row {i}: {fit} vs {}",
                reference[i]
            );
        }
    }
}

/// Builds and applies repeat bitwise at every pool width (a design large
/// enough for the parallel passes and approximate local solves).
#[test]
fn the_handle_is_bitwise_independent_of_the_pool() {
    let case = Case::random(61, 24_000, &[6000, 2500, 12, 7]);
    let weights = case.weights(62);
    let mut rng = Rng(63);
    let p = case.offsets()[4];
    let r: Vec<f64> = (0..p).map(|_| rng.uniform() - 0.5).collect();
    for preconditioner in preconditioners() {
        let run = |threads: usize| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                let handle = built(&case, &weights, preconditioner);
                let mut z = vec![0.0; p];
                handle.apply(&r, &mut z).unwrap();
                (bits(&z), handle.used(), handle.workspace_bytes())
            })
        };
        let reference = run(4);
        for threads in [1, 2, 3] {
            assert_eq!(run(threads), reference, "{preconditioner:?} {threads}");
        }
    }
}

fn layout_project(
    projector: &mut Projector<'_>,
    case: &Case,
    weights: &[f64],
    data: &mut [f64],
) -> (Outcome, Vec<f64>) {
    let offsets = case.offsets();
    let channels: Vec<usize> = (0..case.levels.len()).map(|e| case.channels(e)).collect();
    let per = offsets[case.levels.len()];
    let mut coefficients = vec![0.0; per];
    let layout = CoefficientLayout::new(per, &offsets[..case.levels.len()], &case.levels)
        .with_channels(&channels);
    let columns = ColumnsMut::new(data, case.n, 1).unwrap();
    let outcome = projector
        .project(
            Weights::Values(weights),
            columns,
            Some(CoefficientsMut::new(&mut coefficients, layout)),
            &Request::new(1e-10, 1000),
            &Control::default(),
        )
        .unwrap();
    (outcome, coefficients)
}

/// Building handles (other weights included) leaves a projector's
/// projections bitwise unchanged, on the full formulation and on the
/// two-effect slope elimination, which still eliminates when a handle
/// built the structure first.
#[test]
fn handles_leave_the_projections_unchanged() {
    let full = Case::random(71, 2000, &[300, 80, 7]);
    let sloped = Case::random(72, 2000, &[300, 80]).with_slope(1, 73, 1.0);
    for (case, choice) in [
        (
            &full,
            Choice {
                reduction: Reduction::Full,
                method: Method::Pcg,
                ..Choice::default()
            },
        ),
        (&sloped, Choice::default()),
    ] {
        let weights = case.weights(74);
        let other = case.weights(75);
        let settings = Settings {
            choice: Choice {
                fallback: Fallback::Escalate,
                ..choice
            },
            lanes: None,
        };
        let mut rng = Rng(76);
        let original: Vec<f64> = (0..case.n).map(|_| rng.uniform()).collect();
        let mut plain = Projector::new(case.design(), settings);
        let mut data = original.clone();
        let (outcome, coefficients) = layout_project(&mut plain, case, &weights, &mut data);
        let Outcome::Projected(report) = outcome else {
            panic!("plain projection declined");
        };
        let mut shared = Projector::new(case.design(), settings);
        for w in [&other, &weights] {
            shared
                .fe_preconditioner(
                    Weights::Values(w),
                    Preconditioner::Auto,
                    None,
                    &Control::default(),
                )
                .unwrap()
                .expect("handle");
        }
        let mut again = original.clone();
        let (outcome, shared_coefficients) =
            layout_project(&mut shared, case, &weights, &mut again);
        let Outcome::Projected(shared_report) = outcome else {
            panic!("shared projection declined");
        };
        assert_eq!(bits(&data), bits(&again));
        assert_eq!(bits(&coefficients), bits(&shared_coefficients));
        assert_eq!(report.route, shared_report.route);
        if case.slopes[1].is_empty() {
            assert_eq!(report.route.reduction, ReductionUsed::Full);
        } else {
            assert!(matches!(
                shared_report.route.reduction,
                ReductionUsed::Eliminated { .. }
            ));
        }
    }
}

/// A build is admitted at exactly its reported workspace; one byte below,
/// an explicit choice declines and the automatic one keeps Jacobi. A
/// smaller limit than the structure and weight state declines Jacobi too.
#[test]
fn builds_are_admitted_at_their_reported_workspace() {
    let case = Case::random(81, 4000, &[900, 120, 7]);
    let weights = case.weights(82);
    for preconditioner in preconditioners() {
        let handle = built(&case, &weights, preconditioner);
        let reported = handle.workspace_bytes();
        assert!(handle.retained_bytes() <= reported, "{preconditioner:?}");
        let at = build(&case, &weights, preconditioner, Some(reported))
            .unwrap()
            .expect("admitted at its workspace");
        assert_eq!(at.used(), handle.used());
        let below = build(&case, &weights, preconditioner, Some(reported - 1)).unwrap();
        match preconditioner {
            Preconditioner::Auto if matches!(handle.used(), PreconditionerUsed::Schwarz { .. }) => {
                assert_eq!(below.expect("Jacobi").used(), PreconditionerUsed::Jacobi);
            }
            _ => assert_eq!(
                below.unwrap_err(),
                Decline::MemoryLimit,
                "{preconditioner:?}"
            ),
        }
    }
    let tiny = build(&case, &weights, Preconditioner::Jacobi, Some(1024)).unwrap();
    assert_eq!(tiny.unwrap_err(), Decline::MemoryLimit);
}

/// Cancellation stops a build; later builds and projections run.
#[test]
fn cancellation_stops_a_build() {
    let case = Case::random(91, 6000, &[1500, 400, 9]);
    let weights = case.weights(92);
    let always = || true;
    for preconditioner in preconditioners() {
        let mut projector = Projector::new(case.design(), Settings::default());
        let error = projector
            .fe_preconditioner(
                Weights::Values(&weights),
                preconditioner,
                None,
                &Control::with_cancel(&always),
            )
            .unwrap_err();
        assert_eq!(error, Error::Cancelled, "{preconditioner:?}");
        projector
            .fe_preconditioner(
                Weights::Values(&weights),
                preconditioner,
                None,
                &Control::with_cancel(&|| false),
            )
            .unwrap()
            .expect("a later build");
    }
}

/// Unsupported choices, invalid weights and mismatched vectors are errors.
#[test]
fn invalid_requests_are_errors() {
    let case = Case::random(101, 200, &[20, 6]);
    let weights = case.weights(102);
    for preconditioner in [
        Preconditioner::Identity,
        Preconditioner::BlockJacobi(fedemean::BlockSpec::default()),
    ] {
        let error = build(&case, &weights, preconditioner, None).unwrap_err();
        assert!(matches!(error, Error::Unsupported(_)), "{preconditioner:?}");
    }
    let mut bad = weights.clone();
    bad[3] = 0.0;
    let error = build(&case, &bad, Preconditioner::Jacobi, None).unwrap_err();
    assert!(matches!(error, Error::InvalidData(_)));
    let handle = built(&case, &weights, Preconditioner::Auto);
    let mut out = vec![0.0; 25];
    assert!(matches!(
        handle.apply(&[0.0; 26], &mut out),
        Err(Error::InvalidInput(_))
    ));
}
