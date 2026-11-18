use super::*;
use std::collections::BTreeMap;

fn certificate(
    keys: &[u64],
    stride: usize,
    witness: &[Coordinate],
    y: &[f64],
    clusters: &[&[u32]],
) -> bool {
    let mut groups = [0; 5];
    for (j, c) in clusters.iter().enumerate() {
        groups[j] = *c.iter().max().unwrap() as usize + 1;
    }
    saturated_cells(keys, stride, witness, y, clusters, &groups, &mut || Ok(())).unwrap()
}

// Four observations, two fitted cells, one free score contrast in each.
// Form every subset's group score outer products literally. Integer arithmetic
// is exact and does not use the pair-sharing identity or rank certificate.
fn oracle(clusters: &[&[u32]]) -> [[i64; 2]; 2] {
    let mut total = [[0; 2]; 2];
    for mask in 1_usize..1 << clusters.len() {
        let mut scores = BTreeMap::new();
        for row in 0..4 {
            let tuple: Vec<_> = clusters
                .iter()
                .enumerate()
                .filter(|(d, _)| mask & (1 << d) != 0)
                .map(|(_, codes)| codes[row])
                .collect();
            scores.entry(tuple).or_insert([0_i64; 2])[row / 2] += if row % 2 == 0 { 1 } else { -1 };
        }
        let sign = if mask.count_ones() % 2 == 1 { 1 } else { -1 };
        for v in scores.values() {
            for i in 0..2 {
                for j in 0..2 {
                    total[i][j] += sign * v[i] * v[j];
                }
            }
        }
    }
    total
}

#[test]
fn all_small_cluster_partitions_match_independent_signed_score_meats() {
    let mut partitions = Vec::new();
    for b in 0_u32..=1 {
        for c in 0..=b + 1 {
            for d in 0..=b.max(c) + 1 {
                if b + c + d != 0 {
                    partitions.push([0, b, c, d]);
                }
            }
        }
    }
    assert_eq!(partitions.len(), 14);
    let keys = [1, 1, 2, 2];
    let witness = [Coordinate::Indicator(0), Coordinate::Indicator(1)];
    let check = |dims: &[&[u32]]| {
        assert_eq!(
            certificate(&keys, 1, &witness, &[1., 2., 3., 4.], dims),
            oracle(dims) == [[0; 2]; 2],
            "{dims:?}"
        );
    };
    for (a, p) in partitions.iter().enumerate() {
        check(&[p]);
        for (b, q) in partitions[..=a].iter().enumerate() {
            check(&[p, q]);
            for r in &partitions[..=b] {
                check(&[p, q, r]);
            }
        }
    }
    // Extend coverage to the fourth/fifth CGM signs and repeated dimensions.
    for (i, p) in partitions.iter().enumerate() {
        for (j, q) in partitions.iter().enumerate() {
            let r = &partitions[(i + j + 1) % partitions.len()];
            let s = &partitions[(i + 2 * j + 2) % partitions.len()];
            let t = &partitions[(2 * i + j + 3) % partitions.len()];
            check(&[p, q, r, s]);
            check(&[p, q, r, s, t]);
        }
    }
    let a = [0, 0, 1, 2];
    let b = [0, 1, 2, 2];
    check(&[&a, &b, &a, &b, &a]);
}

#[test]
fn raw_bits_define_cells_and_only_fitted_directions_can_witness_rank() {
    let basis = [Coordinate::Indicator(0), Coordinate::Numeric(1)];
    let keys = |x: [f64; 4], omitted: [f64; 4]| {
        let mut keys = vec![0; 12];
        for (i, row) in keys.chunks_exact_mut(3).enumerate() {
            basis[0].set(row, 1.);
            basis[1].set(row, x[i]);
            Coordinate::Numeric(2).set(row, omitted[i]);
        }
        keys
    };
    let coarse = [0, 0, 1, 1];
    let run = |x, extra, y: &[f64]| certificate(&keys(x, extra), 3, &basis, y, &[&coarse]);
    let x = [0., -0., 0.125, 0.125];
    assert!(run(x, [0., -0., 0.5, 0.5], &[0., 1., 2., 0.]));
    assert!(!run(x, [0., f64::from_bits(1), 0.5, 0.5], &[1.; 4]));
    assert!(!run(x, [0.; 4], &[1., 1., 0., 0.]));
    let p = super::super::modular_rank::PRIME as f64;
    // The raw columns [1,x] are independent over R but singular mod P.
    // A modular collision cannot merge these rows or certify zero.
    assert!(!run([1., 1., 1. + p, 1. + p], [0.; 4], &[1.; 4]));
}
