use super::*;
use crate::ols::canonical_products::root;

// These are the retained A3 incidence geometries, independent of their RHS.
// The comparator is the production serial fallback, not a second test kernel.
#[test]
fn retained_a3_assembly_is_bitwise_equal_for_every_row_owner() {
    for (p, q, degree) in [
        (2048, 128, 4),
        (512, 128, 16),
        (128, 128, 64),
        (2048, 32, 4),
        (2048, 256, 4),
        (2048, 2048, 4),
    ] {
        let (mut left, mut right, mut weights) = (Vec::new(), Vec::new(), Vec::new());
        for a in 0..p {
            for e in 0..degree {
                for _ in 0..2 {
                    left.push(a as u32);
                    right.push(((a + 17 * e) % q) as u32);
                    weights.push((2 + (a + 3 * e) % 7) as f64 / 8.);
                }
            }
        }
        compare(&left, &right, &weights, p, q);
    }
}

#[test]
fn interleaved_components_unequal_weights_and_singletons_keep_assembly_bits() {
    // Three interleaved components have unequal sizes; the last two levels
    // are singleton components. Lane boundaries fall both inside and between
    // packed blocks, unlike level-code order.
    let groups = [
        vec![0, 3, 6, 9],
        vec![1, 4],
        vec![2, 5, 7],
        vec![8],
        vec![10],
    ];
    for scale in [1e-100, 1., 1e100] {
        let (mut left, mut right, mut weights) = (Vec::new(), Vec::new(), Vec::new());
        for a in 0..35 {
            for (e, &b) in groups[a % groups.len()].iter().rev().enumerate() {
                for repeat in 0..3 {
                    left.push(a as u32);
                    right.push(b);
                    weights.push(scale * (1.1 + ((a + 3 * e + repeat) % 11) as f64));
                }
            }
        }
        compare(&left, &right, &weights, 35, 11);
    }
    compare(&[0, 0, 1], &[0, 0, 0], &[0.3, 1.7, 3.1], 2, 1);
}

#[test]
fn parallel_pair_underflow_declines_and_releases_optional_scratch() {
    let members = LevelIndex::from_codes(&[0, 0, 0], 1).unwrap();
    let components = [ComponentFactor {
        len: 2,
        ..ComponentFactor::default()
    }; 3];
    let geometry = Geometry {
        members: &members,
        codes: &[0, 1, 2],
        w: &[1e-320, 1e-320, 1e308],
        mass_a: &[1e308],
        parent: &[0, 0, 0],
        local: &[0, 1, 2],
        components: &components,
    };
    let serial_error =
        serial(geometry, &mut [0.; 9], &mut [0.; 3], &mut [0; 3], false).unwrap_err();
    let mut budget = Budget::new(4096);
    let parallel_error = try_parallel(
        geometry,
        &mut [0.; 9],
        &mut [0.; 3],
        &mut [0; 3],
        3,
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(parallel_error.code, serial_error.code);
    assert_eq!(budget.remaining, budget.limit);
}

fn compare(left: &[u32], right: &[u32], weights: &[f64], p: usize, q: usize) {
    let members = LevelIndex::from_codes(left, p).unwrap();
    let mut parent: Vec<_> = (0..q).collect();
    let mut mass_a = vec![0.; p];
    for (g, mass) in mass_a.iter_mut().enumerate() {
        let rows = members.members(g);
        for &i in rows {
            let r = root(&mut parent, right[rows[0]] as usize);
            let s = root(&mut parent, right[i] as usize);
            parent[r.max(s)] = r.min(s);
            *mass += weights[i];
        }
    }
    let mut components = vec![ComponentFactor::default(); q];
    let mut local = vec![0; q];
    for h in 0..q {
        let r = root(&mut parent, h);
        parent[h] = r;
        local[h] = components[r].len;
        components[r].len += 1;
    }
    let (mut cells, mut rows) = (0, 0);
    for (r, block) in components.iter_mut().enumerate() {
        if parent[r] == r {
            let size = block.len;
            block.len -= 1;
            block.matrix_offset = cells;
            block.level_start = rows;
            cells += size * size;
            rows += size;
        }
    }
    let geometry = Geometry {
        members: &members,
        codes: right,
        w: weights,
        mass_a: &mass_a,
        parent: &parent,
        local: &local,
        components: &components,
    };
    let mut expected = vec![0.; cells];
    serial(
        geometry,
        &mut expected,
        &mut vec![0.; q],
        &mut vec![0; q],
        false,
    )
    .unwrap();
    for workers in [1, 2, 4, 16] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                for extra_lanes in [0, 1, workers] {
                    let allowance =
                        (extra_lanes * q * 16 + (extra_lanes + 1) * size_of::<Lane<'_>>()) as u128;
                    let mut budget = Budget::new(allowance);
                    let mut actual = vec![0.; cells];
                    let (mut totals, mut touched) = (vec![0.; q], vec![0; q]);
                    let parallel = try_parallel(
                        geometry,
                        &mut actual,
                        &mut totals,
                        &mut touched,
                        workers,
                        &mut budget,
                    )
                    .unwrap();
                    assert_eq!(parallel, workers > 1 && q > 1 && extra_lanes > 0);
                    assert_eq!(budget.remaining, allowance);
                    assert!(budget.peak <= allowance);
                    if !parallel {
                        assert!(actual.iter().all(|&value| value == 0.));
                        serial(geometry, &mut actual, &mut totals, &mut touched, false).unwrap();
                    }
                    assert!(totals.iter().all(|&value| value == 0.));
                    for (cell, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "p={p} q={q} workers={workers} extra_lanes={extra_lanes} cell={cell}"
                        );
                    }
                }
            });
    }
}
