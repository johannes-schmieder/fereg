use super::*;
use crate::fe::EffectTerm;

#[test]
fn joint_slope_directions_match_analytic_weighted_oracle_and_revoke_failed_frames() {
    let mut f = Vec::new();
    let mut g = Vec::new();
    let mut slope = Vec::new();
    let mut raw = Vec::new();
    let mut cells = Vec::new();
    for component in 0..2 {
        for a in 0..3 {
            for b in 0..4 {
                for r in 0..5 {
                    let t = r as f64 - 2.;
                    f.push((component * 3 + a) as u32);
                    g.push((component * 4 + b) as u32);
                    slope.push(t);
                    raw.extend_from_slice(&[t * t * t * 1e5, 1., t * t]);
                    cells.push((a, b, r));
                }
            }
        }
    }
    let n = f.len();
    let controls = DenseMatrix::from_row_major(n, 3, raw).unwrap();
    let design = FeDesign::new(
        n,
        vec![
            EffectTerm::new(
                f,
                6,
                true,
                vec![slope.clone(), slope.iter().map(|v| 2. * v).collect()],
            )
            .unwrap(),
            EffectTerm::new(g, 8, true, vec![slope.clone()]).unwrap(),
        ],
    )
    .unwrap();
    for threads in [1, 2, 4] {
        for coupling in [0, 1, 2] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut owner = ReusableMapAbsorber::new(design.clone(), 4).unwrap();
            owner.enable_certificate_geometry().unwrap();
            owner.enable_coefficient_solver(threads).unwrap();
            owner.enable_certificate_workspace().unwrap();
            if coupling != 0 {
                owner.enable_coefficient_coupling().unwrap();
                if coupling == 2 {
                    owner
                        .coefficient_solver
                        .as_mut()
                        .unwrap()
                        .force_compact_coupling_for_test();
                }
            }
            owner.enable_joint_slopes().unwrap();
            let capacity = owner.capacities();
            let charged = ReusableMapAbsorber::joint_slope_bytes(&design, 3).unwrap();
            let joint = owner.joint_slopes().unwrap();
            assert!(charged >= 8 * joint.elements() as u128 + std::mem::size_of_val(joint) as u128);
            let config = MapConfig {
                tolerance: 1e-12,
                ..MapConfig::default()
            };
            for frame in 0..4 {
                let weights: Vec<_> = cells
                    .iter()
                    .map(|&(a, b, r)| {
                        (1. + a as f64 + 2. * b as f64)
                            * (1. + frame as f64 * 0.25 + r as f64 * r as f64 * 0.1)
                    })
                    .collect();
                let residual: Vec<_> = cells
                    .iter()
                    .zip(&weights)
                    .map(|(&(_, _, r), &w)| [1., -4., 6., -4., 1.][r] * 0.01 / w)
                    .collect();
                let y: Vec<_> = cells
                    .iter()
                    .enumerate()
                    .map(|(i, &(a, b, _))| {
                        0.7 * controls[(i, 2)] - 3e-6 * controls[(i, 0)] + a as f64 * 0.2
                            - b as f64 * 0.1
                            + (a + b) as f64 * slope[i] * 0.05
                            + residual[i]
                    })
                    .collect();
                let generation = MapGeometryGeneration {
                    sample: 1,
                    topology: 1,
                    weights: frame + 1,
                };
                pool.install(|| {
                    owner.solve_joint_slopes(&weights, &controls, &[2, 0], &y, config, generation)
                })
                .unwrap();
                let out = owner.joint_slopes().unwrap();
                assert!(out.report.as_ref().unwrap().certified());
                assert!((out.beta()[0] - 0.7).abs() < 1e-10);
                assert!((out.beta()[1] + 3e-6).abs() < 1e-13);
                for (&a, &b) in out.residuals.as_slice().iter().zip(&residual) {
                    assert!((a - b).abs() < 1e-10, "{a} != {b}");
                }
                assert_eq!(capacity, owner.capacities());
                let changed: Vec<_> = y.iter().map(|v| 2. * v).collect();
                pool.install(|| {
                    owner.solve_joint_slopes(
                        &weights,
                        &controls,
                        &[2, 0],
                        &changed,
                        config,
                        generation,
                    )
                })
                .unwrap();
                assert!((owner.joint_slopes().unwrap().beta()[0] - 1.4).abs() < 1e-10);
                let zero = vec![0.; y.len()];
                pool.install(|| {
                    owner.solve_joint_slopes(
                        &weights,
                        &controls,
                        &[2, 0],
                        &zero,
                        config,
                        generation,
                    )
                })
                .unwrap();
                assert!(owner
                    .joint_slopes()
                    .unwrap()
                    .beta()
                    .iter()
                    .all(|&v| v == 0.));
                assert!(owner
                    .joint_slopes()
                    .unwrap()
                    .residuals
                    .as_slice()
                    .iter()
                    .all(|&v| v == 0.));
                let short = MapConfig {
                    max_iterations: 1,
                    ..config
                };
                let short_error = pool
                    .install(|| {
                        owner.solve_joint_slopes(
                            &weights,
                            &controls,
                            &[2, 0],
                            &y,
                            short,
                            generation,
                        )
                    })
                    .unwrap_err();
                assert_eq!(short_error.code, ErrorCode::NonConvergence);
                assert!(owner.joint_slopes().unwrap().report.is_none());

                let error = pool
                    .install(|| {
                        owner.solve_joint_slopes(
                            &weights,
                            &controls,
                            &[2, 2],
                            &y,
                            config,
                            generation,
                        )
                    })
                    .unwrap_err();
                assert_eq!(error.code, ErrorCode::NonConvergence);
                assert!(owner.joint_slopes().unwrap().report.is_none());
                assert!(owner
                    .certificate_geometry
                    .as_ref()
                    .unwrap()
                    .generation
                    .is_none());
                pool.install(|| {
                    owner.solve_joint_slopes(&weights, &controls, &[2, 0], &y, config, generation)
                })
                .unwrap();
                let mut bad = weights.clone();
                bad[0] = f64::NAN;
                assert!(owner
                    .solve_joint_slopes(&bad, &controls, &[2, 0], &y, config, generation)
                    .is_err());
                assert!(owner.joint_slopes().unwrap().report.is_none());
                assert!(owner
                    .certificate_geometry
                    .as_ref()
                    .unwrap()
                    .generation
                    .is_none());
            }
        }
    }
    assert!(ReusableMapAbsorber::joint_slope_bytes(&design, usize::MAX).is_err());
}

#[test]
fn streamed_joint_slope_directions_match_analytic_weighted_oracle_and_revoke_failed_frames() {
    check_streamed_joint_slope_directions::<false>();
}

#[test]
fn column_oriented_joint_slope_directions_match_analytic_weighted_oracle_and_revoke_failed_frames()
{
    check_streamed_joint_slope_directions::<true>();
}

fn check_streamed_joint_slope_directions<const COLUMNS: bool>() {
    use super::super::slope_coefficients::joint::JointFiniteDesign;
    struct Layout<'a, const COLUMNS: bool>(&'a DenseMatrix);
    impl<const COLUMNS: bool> JointFiniteDesign for Layout<'_, COLUMNS> {
        fn prefers_column_scans(&self) -> bool {
            COLUMNS
        }
        fn nrows(&self) -> usize {
            self.0.nrows()
        }
        fn ncols(&self) -> usize {
            self.0.ncols()
        }
        fn visit_column_range(
            &self,
            column: usize,
            range: std::ops::Range<usize>,
            visit: impl FnMut(usize, f64) -> Result<()>,
        ) -> Result<()> {
            self.0.visit_column_range(column, range, visit)
        }
        fn visit_rows(
            &self,
            scratch: &mut [f64],
            visit: impl FnMut(usize, &[f64]) -> Result<()>,
        ) -> Result<()> {
            self.0.visit_rows(scratch, visit)
        }
    }
    let mut f = Vec::new();
    let mut g = Vec::new();
    let mut slope = Vec::new();
    let mut raw = Vec::new();
    let mut cells = Vec::new();
    for component in 0..2 {
        for a in 0..3 {
            for b in 0..4 {
                for r in 0..5 {
                    let t = r as f64 - 2.;
                    f.push((component * 3 + a) as u32);
                    g.push((component * 4 + b) as u32);
                    slope.push(t);
                    raw.extend_from_slice(&[t * t * t * 1e5, 1., t * t]);
                    cells.push((a, b, r));
                }
            }
        }
    }
    let n = f.len();
    let controls = DenseMatrix::from_row_major(n, 3, raw).unwrap();
    let streamed = Layout::<COLUMNS>(&controls);
    let design = FeDesign::new(
        n,
        vec![
            EffectTerm::new(
                f,
                6,
                true,
                vec![slope.clone(), slope.iter().map(|v| 2. * v).collect()],
            )
            .unwrap(),
            EffectTerm::new(g, 8, true, vec![slope.clone()]).unwrap(),
        ],
    )
    .unwrap();
    for threads in [1, 2, 4] {
        for coupling in [0, 1, 2] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut owner =
                ReusableMapAbsorber::new(design.clone(), if coupling == 2 { 4 } else { 1 })
                    .unwrap();
            owner.enable_certificate_geometry().unwrap();
            owner.enable_coefficient_solver(threads).unwrap();
            owner.enable_certificate_workspace().unwrap();
            if coupling != 0 {
                owner.enable_coefficient_coupling().unwrap();
                if coupling == 2 {
                    owner
                        .coefficient_solver
                        .as_mut()
                        .unwrap()
                        .force_compact_coupling_for_test();
                }
            }
            owner.enable_joint_slopes_for_columns(3).unwrap();
            let capacity = owner.capacities();
            let charged = ReusableMapAbsorber::joint_slope_bytes(&design, 3).unwrap();
            let joint = owner.joint_slopes().unwrap();
            assert!(charged >= 8 * joint.elements() as u128 + std::mem::size_of_val(joint) as u128);
            let config = MapConfig {
                tolerance: 1e-12,
                ..MapConfig::default()
            };
            for frame in 0..4 {
                let weights: Vec<_> = cells
                    .iter()
                    .map(|&(a, b, r)| {
                        (1. + a as f64 + 2. * b as f64)
                            * (1. + frame as f64 * 0.25 + r as f64 * r as f64 * 0.1)
                    })
                    .collect();
                let residual: Vec<_> = cells
                    .iter()
                    .zip(&weights)
                    .map(|(&(_, _, r), &w)| [1., -4., 6., -4., 1.][r] * 0.01 / w)
                    .collect();
                let y: Vec<_> = cells
                    .iter()
                    .enumerate()
                    .map(|(i, &(a, b, _))| {
                        0.7 * controls[(i, 2)] - 3e-6 * controls[(i, 0)] + a as f64 * 0.2
                            - b as f64 * 0.1
                            + (a + b) as f64 * slope[i] * 0.05
                            + residual[i]
                    })
                    .collect();
                let generation = MapGeometryGeneration {
                    sample: 1,
                    topology: 1,
                    weights: frame + 1,
                };
                pool.install(|| {
                    owner.solve_joint_slopes_streamed(
                        &weights,
                        &streamed,
                        &[2, 0],
                        &y,
                        config,
                        generation,
                    )
                })
                .unwrap();
                let out = owner.joint_slopes().unwrap();
                assert!(out.report.as_ref().unwrap().certified());
                for (&column, &norm) in [2, 0].iter().zip(out.input_norms().unwrap()) {
                    let squared: f64 = weights
                        .iter()
                        .enumerate()
                        .map(|(row, &weight)| weight * controls[(row, column)].powi(2))
                        .sum();
                    assert!((norm / squared.sqrt() - 1.).abs() < 2e-14);
                }
                assert!((out.beta()[0] - 0.7).abs() < 1e-10);
                assert!((out.beta()[1] + 3e-6).abs() < 1e-13);
                for (&a, &b) in out.residuals.as_slice().iter().zip(&residual) {
                    assert!((a - b).abs() < 1e-10, "{a} != {b}");
                }
                assert_eq!(capacity, owner.capacities());
                let changed: Vec<_> = y.iter().map(|v| 2. * v).collect();
                pool.install(|| {
                    owner.solve_joint_slopes_streamed(
                        &weights,
                        &streamed,
                        &[2, 0],
                        &changed,
                        config,
                        generation,
                    )
                })
                .unwrap();
                assert!((owner.joint_slopes().unwrap().beta()[0] - 1.4).abs() < 1e-10);
                let zero = vec![0.; y.len()];
                pool.install(|| {
                    owner.solve_joint_slopes_streamed(
                        &weights,
                        &streamed,
                        &[2, 0],
                        &zero,
                        config,
                        generation,
                    )
                })
                .unwrap();
                assert!(owner
                    .joint_slopes()
                    .unwrap()
                    .beta()
                    .iter()
                    .all(|&v| v == 0.));
                assert!(owner
                    .joint_slopes()
                    .unwrap()
                    .residuals
                    .as_slice()
                    .iter()
                    .all(|&v| v == 0.));
                let short = MapConfig {
                    max_iterations: 1,
                    ..config
                };
                let short_error = pool
                    .install(|| {
                        owner.solve_joint_slopes_streamed(
                            &weights,
                            &streamed,
                            &[2, 0],
                            &y,
                            short,
                            generation,
                        )
                    })
                    .unwrap_err();
                assert_eq!(short_error.code, ErrorCode::NonConvergence);
                assert!(owner.joint_slopes().unwrap().report.is_none());
                assert!(owner.joint_slopes().unwrap().input_norms().is_none());

                let error = pool
                    .install(|| {
                        owner.solve_joint_slopes_streamed(
                            &weights,
                            &streamed,
                            &[2, 2],
                            &y,
                            config,
                            generation,
                        )
                    })
                    .unwrap_err();
                assert_eq!(error.code, ErrorCode::NonConvergence);
                assert!(owner.joint_slopes().unwrap().report.is_none());
                assert!(owner.joint_slopes().unwrap().input_norms().is_none());
                assert!(owner
                    .certificate_geometry
                    .as_ref()
                    .unwrap()
                    .generation
                    .is_none());
                pool.install(|| {
                    owner.solve_joint_slopes_streamed(
                        &weights,
                        &streamed,
                        &[2, 0],
                        &y,
                        config,
                        generation,
                    )
                })
                .unwrap();
                let mut bad = weights.clone();
                bad[0] = f64::NAN;
                assert!(owner
                    .solve_joint_slopes_streamed(&bad, &streamed, &[2, 0], &y, config, generation)
                    .is_err());
                assert!(owner.joint_slopes().unwrap().report.is_none());
                assert!(owner.joint_slopes().unwrap().input_norms().is_none());
                assert!(owner
                    .certificate_geometry
                    .as_ref()
                    .unwrap()
                    .generation
                    .is_none());
            }
        }
    }
    assert!(ReusableMapAbsorber::joint_slope_bytes(&design, usize::MAX).is_err());
}

#[test]
fn common_geometry_without_coefficient_solver_preserves_frozen_active_basis() {
    let rows = 32_771;
    for skewed in [false, true] {
        let design = FeDesign::new(
            rows,
            vec![EffectTerm::new(
                (0..rows)
                    .map(|r| {
                        if skewed && r < 30_000 {
                            0
                        } else {
                            (2 * (r % 17)) as u32
                        }
                    })
                    .collect(),
                39,
                true,
                vec![
                    (0..rows)
                        .map(|r| ((r * 37 % 173) as f64 - 86.0) / 91.0)
                        .collect(),
                    (0..rows)
                        .map(|r| 2.0 * ((r * 37 % 173) as f64 - 86.0) / 91.0)
                        .collect(),
                    vec![0.; rows],
                ],
            )
            .unwrap()],
        )
        .unwrap();
        for threads in [1, 2, 4, 7] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut owner = ReusableMapAbsorber::new(design.clone(), 3).unwrap();
            owner.enable_certificate_geometry().unwrap();
            assert!(owner.coefficient_solver.is_none());
            let capacity = owner.capacities();
            for scale in [1., 1e-80, 1e80, 1.] {
                let weights: Vec<_> = (0..rows)
                    .map(|r| scale * (0.2 + (r * 13 % 17) as f64))
                    .collect();
                let mut expected = Vec::new();
                let mut expected_ranks = Vec::new();
                let mut columns = vec![0.; owner.certificate_columns.len()];
                let mut basis = vec![0.; owner.certificate_basis.len()];
                // Frozen former padded-stride, serial traversal.
                for effect in design.effects() {
                    for level in 0..effect.n_levels() {
                        let members = effect.level_index().members(level);
                        let rank = prepare_reusable_group_basis(
                            effect,
                            members,
                            &weights,
                            owner.max_level_members,
                            &mut columns,
                            &mut basis,
                        )
                        .unwrap();
                        expected_ranks.push(rank);
                        let mut active = Vec::new();
                        for channel in 0..rank {
                            let start = channel * owner.max_level_members;
                            active.extend_from_slice(&basis[start..start + members.len()]);
                        }
                        expected.push(active);
                    }
                }
                pool.install(|| owner.prepare_coefficient_geometry(&weights))
                    .unwrap();
                assert_eq!(owner.capacities(), capacity);
                let actual = owner.certificate_geometry.as_ref().unwrap();
                assert_eq!(actual.ranks, expected_ranks);
                assert!(actual.generation.is_none());
                let (mut offset, mut group) = (0, 0);
                for effect in design.effects() {
                    for level in 0..effect.n_levels() {
                        let members = effect.level_index().members(level).len();
                        let end = offset + members * actual.ranks[group];
                        assert_eq!(
                            actual.basis[offset..end]
                                .iter()
                                .map(|v| v.to_bits())
                                .collect::<Vec<_>>(),
                            expected[group]
                                .iter()
                                .map(|v| v.to_bits())
                                .collect::<Vec<_>>()
                        );
                        offset += members * effect.channels();
                        group += 1;
                    }
                }
            }
        }
    }
}

#[test]
fn parallel_slope_geometry_preserves_active_basis_and_scratch_refusal() {
    // Empty declared levels stress disjoint zero-length slices. Duplicate
    // and zero slopes stress inactive padding, which has no rank authority.
    let rows = 192;
    for oversized in [false, true] {
        let z: Vec<_> = (0..rows).map(|i| ((i / 8) as f64).sin()).collect();
        let design = FeDesign::new(
            rows,
            vec![
                EffectTerm::new(
                    (0..rows)
                        .map(|i| if oversized { 0 } else { (2 * (i % 8)) as u32 })
                        .collect(),
                    19,
                    true,
                    vec![
                        z.clone(),
                        z.iter().map(|v| v * 2.).collect(),
                        vec![0.; rows],
                    ],
                )
                .unwrap(),
                EffectTerm::new(
                    (0..rows).map(|i| (i % 11) as u32).collect(),
                    13,
                    false,
                    vec![(0..rows).map(|i| 1e120 * ((i * 7) as f64).cos()).collect()],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let mut serial = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
        serial.enable_certificate_geometry().unwrap();
        for threads in [1, 2, 4, 7] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut candidate = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
            candidate.enable_certificate_geometry().unwrap();
            candidate.enable_coefficient_solver(threads).unwrap();
            let capacities = candidate.capacities();
            for (generation, scale) in [1., 1e-80, 1e80, 1.].into_iter().enumerate() {
                let weights: Vec<_> = (0..rows)
                    .map(|i| scale * (0.2 + ((i * (generation + 3)) % 17) as f64))
                    .collect();
                serial.prepare_coefficient_geometry(&weights).unwrap();
                let before = candidate
                    .certificate_geometry
                    .as_ref()
                    .unwrap()
                    .basis
                    .clone();
                let prepared = pool
                    .install(|| {
                        candidate
                            .coefficient_solver
                            .as_mut()
                            .unwrap()
                            .prepare_geometry(
                                &design,
                                &weights,
                                candidate.certificate_geometry.as_mut().unwrap(),
                            )
                    })
                    .unwrap();
                assert_eq!(prepared, threads > 1 && !oversized);
                if !prepared {
                    assert_eq!(
                        candidate.certificate_geometry.as_ref().unwrap().basis,
                        before
                    );
                    pool.install(|| candidate.prepare_coefficient_geometry(&weights))
                        .unwrap();
                }
                let expected = serial.certificate_geometry.as_ref().unwrap();
                let actual = candidate.certificate_geometry.as_ref().unwrap();
                assert_eq!(actual.ranks, expected.ranks);
                let mut group = 0;
                let mut offset = 0;
                for effect in design.effects() {
                    for level in 0..effect.n_levels() {
                        let members = effect.level_index().members(level).len();
                        let end = offset + members * expected.ranks[group];
                        assert_eq!(&actual.basis[offset..end], &expected.basis[offset..end]);
                        offset += members * effect.channels();
                        group += 1;
                    }
                }
                assert_eq!(candidate.capacities(), capacities);
            }
        }
    }
}

#[test]
fn coefficient_residual_gate_checks_truth_refusal_and_weight_authority() {
    let n = 6 * 4 * 3;
    let ci: Vec<_> = (0..n).map(|r| (r / 12) as u32).collect();
    let cj: Vec<_> = (0..n).map(|r| ((r / 3) % 4) as u32).collect();
    let t: Vec<_> = (0..n).map(|r| (r % 3) as f64 - 1.).collect();
    let design = FeDesign::new(
        n,
        vec![
            EffectTerm::new(
                ci.clone(),
                6,
                true,
                vec![t.clone(), t.iter().map(|v| 2. * v).collect(), vec![0.; n]],
            )
            .unwrap(),
            EffectTerm::new(
                cj.clone(),
                4,
                true,
                vec![ci.iter().map(|&i| i as f64).collect()],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let config = MapConfig {
        tolerance: 1e-12,
        ..MapConfig::default()
    };
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut owner = ReusableMapAbsorber::new(design.clone(), 2).unwrap();
        owner.enable_certificate_geometry().unwrap();
        owner.enable_coefficient_solver(threads).unwrap();
        owner.enable_coefficient_coupling().unwrap();
        owner.enable_certificate_workspace().unwrap();
        let capacity = owner.capacities();
        for (frame, scale) in [1., 1e-60, 1e60].into_iter().enumerate() {
            let generation = MapGeometryGeneration {
                sample: 1,
                topology: 1,
                weights: frame as u64 + 1,
            };
            let jw: Vec<_> = (0..4).map(|j| (j + frame + 1) as f64).collect();
            let mean = (0..4).map(|j| jw[j] * j as f64).sum::<f64>() / jw.iter().sum::<f64>();
            let weights: Vec<_> = (0..n)
                .map(|r| scale * (1. + ci[r] as f64) * jw[cj[r] as usize])
                .collect();
            // Weighted complete-product contrasts are orthogonal to every
            // declared channel, including the locally redundant slopes.
            let truth: Vec<_> = (0..n)
                .map(|r| (ci[r] as f64 + 1.).sin() * (cj[r] as f64 - mean) * t[r])
                .collect();
            let raw: Vec<_> = (0..n)
                .map(|r| truth[r] + (ci[r] as f64).cos() * t[r] + (cj[r] as f64).sin())
                .collect();
            let matrix = |x: &[f64]| DenseMatrix::from_row_major(n, 1, x.to_vec()).unwrap();
            let preparations = owner.certificate_geometry.as_ref().unwrap().preparations;
            let first = pool
                .install(|| {
                    owner.absorb_coefficient_residual(&weights, matrix(&raw), config, generation)
                })
                .unwrap();
            assert!(first.report().iterations > 0);
            assert_eq!(
                owner.certificate_geometry.as_ref().unwrap().preparations,
                preparations + 1
            );
            for (&a, &b) in first.residualized().as_slice().iter().zip(&truth) {
                assert!((a - b).abs() < 1e-10, "{a} != {b}");
            }
            for rhs_scale in [1., -3., 1e-60, 1e60] {
                let residual: Vec<_> = truth.iter().map(|v| v * rhs_scale).collect();
                let accepted = pool
                    .install(|| {
                        owner.absorb_coefficient_residual(
                            &weights,
                            matrix(&residual),
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                assert!(accepted.report().certified());
                assert_eq!(accepted.report().iterations, 0);
                assert_eq!(accepted.residualized().as_slice(), residual.as_slice());
            }
            // Identical geometry does not authorize an untested RHS. A
            // material FE component must trigger the original correction.
            let corrected = pool
                .install(|| {
                    owner.absorb_coefficient_residual(&weights, matrix(&raw), config, generation)
                })
                .unwrap();
            assert!(corrected.report().iterations > 0);
            for (&a, &b) in corrected.residualized().as_slice().iter().zip(&truth) {
                assert!((a - b).abs() < 1e-10);
            }
            let zero = vec![0.; n];
            let accepted = pool
                .install(|| {
                    owner.absorb_coefficient_residual(&weights, matrix(&zero), config, generation)
                })
                .unwrap();
            assert_eq!(accepted.report().iterations, 0);
            assert_eq!(accepted.residualized().as_slice(), zero.as_slice());
            for failure in 0..3 {
                let mut invalid = truth.clone();
                if failure == 0 {
                    invalid[0] = f64::NAN;
                }
                let values = if failure == 1 {
                    DenseMatrix::zeros(n, 2).unwrap()
                } else {
                    matrix(&invalid)
                };
                let cfg = if failure == 2 {
                    MapConfig {
                        max_iterations: 0,
                        ..config
                    }
                } else {
                    config
                };
                assert!(
                    pool.install(
                        || owner.absorb_coefficient_residual(&weights, values, cfg, generation)
                    )
                    .is_err()
                );
                assert!(owner
                    .certificate_geometry
                    .as_ref()
                    .unwrap()
                    .generation
                    .is_none());
                let before = owner.certificate_geometry.as_ref().unwrap().preparations;
                let recovered = pool
                    .install(|| {
                        owner.absorb_coefficient_residual(
                            &weights,
                            matrix(&raw),
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                assert!(recovered.report().certified());
                assert_eq!(
                    owner.certificate_geometry.as_ref().unwrap().preparations,
                    before + 1
                );
            }
            assert_eq!(owner.capacities(), capacity);
        }
    }
}

#[test]
fn coefficient_norm_handoff_tracks_current_columns_weights_and_failures() {
    let n = 6 * 4 * 3;
    let codes_i: Vec<_> = (0..n).map(|row| (row / 12) as u32).collect();
    let codes_j: Vec<_> = (0..n).map(|row| ((row / 3) % 4) as u32).collect();
    let t: Vec<_> = (0..n).map(|row| (row % 3) as f64 - 1.).collect();
    let design = FeDesign::new(
        n,
        vec![
            EffectTerm::new(
                codes_i.clone(),
                6,
                true,
                vec![t.clone(), t.iter().map(|v| 2. * v).collect(), vec![0.; n]],
            )
            .unwrap(),
            EffectTerm::new(
                codes_j.clone(),
                4,
                true,
                vec![codes_i.iter().map(|&i| i as f64).collect()],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let config = MapConfig {
        tolerance: 1e-12,
        ..MapConfig::default()
    };
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut owner = ReusableMapAbsorber::new(design.clone(), 4).unwrap();
        owner.enable_certificate_geometry().unwrap();
        owner.enable_coefficient_solver(threads).unwrap();
        owner.enable_coefficient_coupling().unwrap();
        let capacities = owner.capacities();
        for (frame, scale) in [1., 1e-120, 1e120].into_iter().enumerate() {
            let generation = MapGeometryGeneration {
                sample: 1,
                topology: 1,
                weights: frame as u64 + 1,
            };
            let jw: Vec<_> = (0..4).map(|j| 0.4 + j as f64 + frame as f64).collect();
            let mean_j = (0..4).map(|j| jw[j] * j as f64).sum::<f64>() / jw.iter().sum::<f64>();
            let weights: Vec<_> = (0..n)
                .map(|row| scale * (1. + codes_i[row] as f64) * jw[codes_j[row] as usize])
                .collect();
            let mut last_reference = 0.;
            for rhs_frame in 0..2 {
                let mut raw = DenseMatrix::zeros(n, 4).unwrap();
                let mut expected = DenseMatrix::zeros(n, 4).unwrap();
                let value_scale = [1., 1e-60, 1e60][frame] * (rhs_frame + 1) as f64;
                for row in 0..n {
                    let i = codes_i[row] as f64;
                    let j = codes_j[row] as f64;
                    let remainder = (i + 1.).sin() * (j - mean_j) * t[row];
                    let fitted = i.sin() + i.cos() * t[row] + j.cos() + j.sin() * i;
                    for col in 0..3 {
                        let c = (col + 1) as f64;
                        raw[(row, col)] = value_scale * (c * fitted + remainder / c);
                        expected[(row, col)] = value_scale * remainder / c;
                    }
                }
                let selected = [2, 0, 3, 2];
                let input_norm: Vec<_> = selected
                    .iter()
                    .map(|&col| {
                        (0..n)
                            .map(|row| weights[row] * raw[(row, col)] * raw[(row, col)])
                            .sum::<f64>()
                            .sqrt()
                    })
                    .collect();
                let mut references = [f64::NAN; 4];
                let mut residuals = [f64::NAN; 4];
                let actual = pool
                    .install(|| {
                        owner.absorb_coefficient_with_selected_norms(
                            &weights,
                            raw.clone(),
                            config,
                            generation,
                            selected.into_iter(),
                            Some(&mut references),
                            &mut residuals,
                        )
                    })
                    .unwrap();
                assert!(actual.report().certified());
                for (a, e) in actual
                    .residualized()
                    .as_slice()
                    .iter()
                    .zip(expected.as_slice())
                {
                    assert!((a - e).abs() <= 1e-9 * value_scale);
                }
                for (out, &col) in selected.iter().enumerate() {
                    // Direct sums use an independent arithmetic path, within
                    // ranges that cannot overflow or underflow in this oracle.
                    let norm = (0..n)
                        .map(|row| weights[row] * actual.residualized()[(row, col)].powi(2))
                        .sum::<f64>()
                        .sqrt();
                    assert!((references[out] - input_norm[out]).abs() <= 2e-14 * input_norm[out]);
                    assert!((residuals[out] - norm).abs() <= 2e-14 * norm);
                }
                if rhs_frame == 1 {
                    assert!(references[1] > last_reference);
                }
                last_reference = references[1];
                let preparations = owner.certificate_geometry.as_ref().unwrap().preparations;
                for failure in 0..3 {
                    references.fill(-1.);
                    residuals.fill(-2.);
                    let mut bad_weights = weights.clone();
                    if failure == 1 {
                        bad_weights[0] = f64::NAN;
                    }
                    let positions = if failure == 0 { [2, 0, 4, 2] } else { selected };
                    let budget = if failure == 2 {
                        MapConfig {
                            max_iterations: 0,
                            ..config
                        }
                    } else {
                        config
                    };
                    assert!(pool
                        .install(|| owner.absorb_coefficient_with_selected_norms(
                            &bad_weights,
                            raw.clone(),
                            budget,
                            generation,
                            positions.into_iter(),
                            Some(&mut references),
                            &mut residuals,
                        ))
                        .is_err());
                    assert_eq!(references, [-1.; 4]);
                    assert_eq!(residuals, [-2.; 4]);
                    assert!(owner
                        .certificate_geometry
                        .as_ref()
                        .unwrap()
                        .generation
                        .is_none());
                }
                pool.install(|| {
                    owner.absorb_coefficient_with_selected_norms(
                        &weights,
                        raw,
                        config,
                        generation,
                        selected.into_iter(),
                        None,
                        &mut residuals,
                    )
                })
                .unwrap();
                assert_eq!(
                    owner.certificate_geometry.as_ref().unwrap().preparations,
                    preparations + 1
                );
                assert_eq!(owner.capacities(), capacities);
                if frame == 2 && rhs_frame == 1 {
                    let values = DenseMatrix::zeros(n, 4).unwrap();
                    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let positions = selected
                            .into_iter()
                            .map(|_| panic!("injected selection unwind"));
                        owner.absorb_coefficient_with_selected_norms(
                            &weights,
                            values,
                            config,
                            generation,
                            positions,
                            Some(&mut references),
                            &mut residuals,
                        )
                    }));
                    assert!(failure.is_err());
                    assert!(owner
                        .certificate_geometry
                        .as_ref()
                        .unwrap()
                        .generation
                        .is_none());
                }
            }
        }
    }
}

#[test]
fn slope_coefficients_preserve_analytic_projection_across_weights_rank_and_threads() {
    // On this complete product, sum_j w_j*g(j)=0 and sum_t w_t*h(t)=0.
    // Therefore f(i)*g(j)*h(t) is orthogonal to i[t], j[i] and t effects,
    // even with duplicate/zero channels, disconnected copies and no i intercept.
    for third in [false, true] {
        for intercept in [false, true] {
            let rows = 2 * 8 * 6 * 2;
            let mut g1 = Vec::new();
            let mut g2 = Vec::new();
            let mut g3 = Vec::new();
            let mut z1 = Vec::new();
            let mut z2 = Vec::new();
            for component in 0..2 {
                for i in 0..8 {
                    for j in 0..6 {
                        for t in 0..2 {
                            g1.push((component * 8 + i) as u32);
                            g2.push((component * 6 + j) as u32);
                            g3.push((component * 2 + t) as u32);
                            z1.push(if t == 0 { -1. } else { 1. });
                            z2.push(i as f64);
                        }
                    }
                }
            }
            let mut effects = vec![
                EffectTerm::new(
                    g1,
                    16,
                    intercept,
                    vec![
                        z1.clone(),
                        z1.iter().map(|z| 2. * z).collect(),
                        vec![0.; rows],
                    ],
                )
                .unwrap(),
                EffectTerm::new(g2, 12, true, vec![z2]).unwrap(),
            ];
            if third {
                effects.push(EffectTerm::new(g3, 4, true, vec![]).unwrap());
            }
            let design = FeDesign::new(rows, effects).unwrap();
            for coupling_kind in 0..3 {
                let prepared_coupling = coupling_kind != 0;
                let mut reference: Option<DenseMatrix> = None;
                for threads in [1, 4, 8] {
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap();
                    let mut solver = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
                    solver.enable_certificate_geometry().unwrap();
                    solver.enable_coefficient_solver(threads).unwrap();
                    if prepared_coupling {
                        solver.enable_coefficient_coupling().unwrap();
                        if coupling_kind == 2 {
                            solver
                                .coefficient_solver
                                .as_mut()
                                .unwrap()
                                .force_compact_coupling_for_test();
                        }
                    }
                    let capacity = solver.capacities();
                    assert!(ReusableMapAbsorber::coefficient_solver_bytes(
                        &design,
                        usize::MAX,
                        threads
                    )
                    .is_err());
                    for (generation, rhs) in [7, 1, 3, 7].into_iter().enumerate() {
                        let mut values = DenseMatrix::zeros(rows, rhs).unwrap();
                        let mut expected = DenseMatrix::zeros(rows, rhs).unwrap();
                        let mut weights = Vec::new();
                        let jw: Vec<_> = (0..6)
                            .map(|j| 0.4 + j as f64 * 0.3 + generation as f64)
                            .collect();
                        let mean = jw
                            .iter()
                            .enumerate()
                            .map(|(j, w)| j as f64 * w)
                            .sum::<f64>()
                            / jw.iter().sum::<f64>();
                        let scale = [1., 1., 1e-60, 1e60][generation];
                        let mut row = 0;
                        for component in 0..2 {
                            for i in 0..8 {
                                // Keep the legacy matrix indices and numerical traversal order explicit.
                                #[allow(clippy::needless_range_loop)]
                                for j in 0..6 {
                                    for t in 0..2 {
                                        let sign = if t == 0 { -1. } else { 1. };
                                        let tw = if t == 0 { 0.5 } else { 2. };
                                        weights.push(
                                            scale
                                                * (0.2 + i as f64 + generation as f64)
                                                * jw[j]
                                                * tw,
                                        );
                                        let remainder =
                                            (i as f64 + 1.).sin() * (j as f64 - mean) * sign / tw;
                                        let fitted = if intercept {
                                            (i * i + component) as f64
                                        } else {
                                            0.
                                        } + (i as f64).sin() * sign
                                            + (j as f64).cos()
                                            + j as f64 * i as f64 / 8.
                                            + if third { t as f64 * 0.31 } else { 0. };
                                        for col in 0..rhs {
                                            let c = (col + 1) as f64;
                                            expected[(row, col)] = remainder / c;
                                            values[(row, col)] = fitted * c + remainder / c;
                                        }
                                        row += 1;
                                    }
                                }
                            }
                        }
                        let norms = weighted_column_norms(&values, &weights).unwrap();
                        let result = pool
                            .install(|| {
                                solver.absorb(
                                    &weights,
                                    values,
                                    MapConfig {
                                        tolerance: 1e-12,
                                        ..MapConfig::default()
                                    },
                                    false,
                                )
                            })
                            .unwrap();
                        for (&a, &e) in result
                            .residualized()
                            .as_slice()
                            .iter()
                            .zip(expected.as_slice())
                        {
                            assert!(
                                (a - e).abs() < 2e-8,
                                "generation={generation}, threads={threads}: {a} != {e}"
                            );
                        }
                        assert!(
                            certify_residuals_with_reference_norms(
                                &design,
                                &weights,
                                result.residualized(),
                                &norms,
                                1e-12
                            )
                            .unwrap()
                            .passed
                        );
                        assert_eq!(capacity, solver.capacities());
                        if prepared_coupling {
                            solver
                                .coefficient_solver
                                .as_ref()
                                .unwrap()
                                .check_coupling_products(
                                    solver.design(),
                                    solver.certificate_geometry.as_ref().unwrap(),
                                );
                        }
                        if generation == 3 {
                            if let Some(prior) = &reference {
                                assert_eq!(prior, result.residualized());
                            } else {
                                reference = Some(result.residualized().clone());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn slope_coefficients_fail_closed_and_recover_after_a_short_solve() {
    let n = 180;
    let design = FeDesign::new(
        n,
        vec![
            EffectTerm::new(
                (0..n).map(|i| (i % 15) as u32).collect(),
                15,
                true,
                vec![(0..n).map(|i| (i as f64 * 0.37).sin()).collect()],
            )
            .unwrap(),
            EffectTerm::new(
                (0..n).map(|i| ((i / 9) % 10) as u32).collect(),
                10,
                true,
                vec![(0..n).map(|i| (i as f64 * 0.19).cos()).collect()],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let values =
        DenseMatrix::from_row_major(n, 2, (0..n * 2).map(|i| (i as f64 * 0.23).sin()).collect())
            .unwrap();
    let weights: Vec<_> = (0..n).map(|i| 0.4 + (i % 7) as f64).collect();
    let mut owner = ReusableMapAbsorber::new(design, 2).unwrap();
    assert!(owner.enable_coefficient_solver(2).is_err());
    owner.enable_certificate_geometry().unwrap();
    owner.enable_coefficient_solver(2).unwrap();
    owner.enable_coefficient_coupling().unwrap();
    let capacity = owner.capacities();
    assert!(owner
        .absorb(
            &weights,
            values.clone(),
            MapConfig {
                max_iterations: 1,
                tolerance: 1e-12,
                ..MapConfig::default()
            },
            false
        )
        .is_err());
    let mut invalid = values.clone();
    invalid[(0, 0)] = f64::NAN;
    assert!(owner
        .absorb(&weights, invalid, MapConfig::default(), false)
        .is_err());
    let result = owner
        .absorb(
            &weights,
            values,
            MapConfig {
                tolerance: 1e-12,
                ..MapConfig::default()
            },
            false,
        )
        .unwrap();
    assert!(result.report().certified());
    assert_eq!(capacity, owner.capacities());
}

#[test]
#[ignore = "bounded frozen-frame development diagnostic; requires FEREG_SLOPE_FRAME"]
fn slope_coefficient_frozen_frame() {
    let root = std::path::PathBuf::from(std::env::var("FEREG_SLOPE_FRAME").unwrap());
    let meta = std::fs::read_to_string(root.join("frame.json")).unwrap();
    let scalar = |key: &str| -> usize {
        meta.split(&format!("\"{key}\":"))
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let list = |key: &str| -> Vec<usize> {
        meta.split(&format!("\"{key}\":["))
            .nth(1)
            .unwrap()
            .split(']')
            .next()
            .unwrap()
            .split(',')
            .map(|v| v.trim().parse().unwrap())
            .collect()
    };
    let n = scalar("rows");
    let rhs = scalar("rhs");
    let levels = list("levels");
    let channels = list("channels");
    let data: Vec<Vec<f64>> = std::fs::read_to_string(root.join("frame.csv"))
        .unwrap()
        .lines()
        .map(|line| line.split(',').map(|x| x.parse().unwrap()).collect())
        .collect();
    let weights: Vec<_> = data.iter().map(|r| r[0]).collect();
    let values = DenseMatrix::from_row_major(
        n,
        rhs,
        data.iter()
            .flat_map(|r| r[1..1 + rhs].iter().copied())
            .collect(),
    )
    .unwrap();
    let mut effects = Vec::new();
    let mut at = 1 + 2 * rhs;
    for (&levels, &channels) in levels.iter().zip(&channels) {
        let codes = data.iter().map(|r| r[at] as u32).collect();
        let slopes = (1..channels)
            .map(|c| data.iter().map(|r| r[at + c]).collect())
            .collect();
        effects.push(EffectTerm::new(codes, levels, true, slopes).unwrap());
        at += channels;
    }
    let design = FeDesign::new(n, effects).unwrap();
    let baseline = std::env::var_os("FEREG_SLOPE_BASELINE").is_some();
    let tolerance = std::env::var("FEREG_SLOPE_TOL")
        .unwrap_or_else(|_| "1e-12".into())
        .parse()
        .unwrap();
    let threads: usize = std::env::var("FEREG_SLOPE_THREADS")
        .unwrap_or_else(|_| "4".into())
        .parse()
        .unwrap();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    let started = Instant::now();
    let mut absorber = ReusableMapAbsorber::new(design, rhs).unwrap();
    absorber.enable_certificate_geometry().unwrap();
    if baseline {
        if let Some(plan) =
            ParallelProjectionPlan::for_design_and_threads(absorber.design(), rhs, threads).unwrap()
        {
            absorber.enable_parallel_projection(plan).unwrap();
        }
    } else {
        absorber.enable_coefficient_solver(threads).unwrap();
        absorber.enable_certificate_workspace().unwrap();
        if std::env::var_os("FEREG_SLOPE_COUPLING").is_some() {
            absorber.enable_coefficient_coupling().unwrap();
        }
    }
    let construction = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let result = pool
        .install(|| {
            absorber.absorb(
                &weights,
                values,
                MapConfig {
                    tolerance,
                    ..MapConfig::default()
                },
                false,
            )
        })
        .unwrap();
    let elapsed = started.elapsed().as_secs_f64();
    println!("SLOPE_FRAME|baseline={baseline}|rows={n}|rhs={rhs}|threads={threads}|construction={construction}|seconds={elapsed}|iterations={}|certificate={}|workspace_bytes={}",result.report().iterations,result.report().certificate.relative_infinity_norm,ReusableMapAbsorber::coefficient_solver_bytes(absorber.design(),rhs,threads).unwrap());
    if let Ok(output) = std::env::var("FEREG_SLOPE_OUTPUT") {
        use std::io::Write;
        let mut out = std::io::BufWriter::new(std::fs::File::create(output).unwrap());
        for row in 0..n {
            for col in 0..rhs {
                write!(
                    out,
                    "{}{:.17e}",
                    if col == 0 { "" } else { "," },
                    result.residualized()[(row, col)]
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }
    }
}

#[test]
fn slope_geometry_reuses_only_explicit_generations_and_invalidates_before_errors() {
    let rows = 120;
    let z: Vec<_> = (0..rows).map(|i| (i % 7) as f64 - 3.0).collect();
    let design = FeDesign::new(
        rows,
        vec![
            EffectTerm::new(
                (0..rows).map(|i| (i % 5) as u32).collect(),
                5,
                true,
                vec![z.clone(), z.iter().map(|v| 2.0 * v).collect()],
            )
            .unwrap(),
            EffectTerm::new(
                (0..rows).map(|i| ((i / 5) % 4) as u32).collect(),
                4,
                true,
                vec![],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    for coefficients in [false, true] {
        let mut cached = ReusableMapAbsorber::new(design.clone(), 3).unwrap();
        cached.enable_certificate_geometry().unwrap();
        if coefficients {
            cached.enable_coefficient_solver(1).unwrap();
        }
        cached.enable_certificate_workspace().unwrap();
        if coefficients {
            cached.enable_coefficient_coupling().unwrap();
        }
        let mut fresh = ReusableMapAbsorber::new(design.clone(), 3).unwrap();
        fresh.enable_certificate_geometry().unwrap();
        if coefficients {
            fresh.enable_coefficient_solver(1).unwrap();
        }
        if coefficients {
            fresh.enable_coefficient_coupling().unwrap();
        }
        let capacities = cached.capacities();
        let mut generation = MapGeometryGeneration {
            sample: 1,
            topology: 2,
            weights: 3,
        };
        for step in 0..6 {
            if step == 2 {
                generation.weights += 1;
            }
            if step == 4 {
                generation.topology += 1;
            }
            let weights: Vec<_> = (0..rows)
                .map(|i| 0.2 + ((i * generation.weights as usize) % 17) as f64)
                .collect();
            let rhs = if step % 2 == 0 { 3 } else { 1 };
            let mut values = DenseMatrix::zeros(rows, rhs).unwrap();
            for row in 0..rows {
                for col in 0..rhs {
                    values[(row, col)] = ((row * 19 + col * 11 + step) as f64).sin();
                }
            }
            let expected = fresh
                .absorb(&weights, values.clone(), MapConfig::default(), false)
                .unwrap();
            let actual = cached
                .absorb_in_generation(
                    &weights,
                    values.clone(),
                    MapConfig::default(),
                    false,
                    Some(generation),
                )
                .unwrap();
            assert_eq!(actual.residualized(), expected.residualized());
            assert_eq!(actual.report().certificate, expected.report().certificate);
            assert_eq!(cached.capacities(), capacities);
            let geometry = cached.certificate_geometry.as_ref().unwrap();
            assert_eq!(geometry.generation, Some(generation));
            assert_eq!(geometry.preparations, step / 2 + 1);
            assert_eq!(
                cached
                    .coefficient_solver
                    .as_ref()
                    .and_then(|s| s.coupling_preparations()),
                coefficients.then_some(step / 2 + 1)
            );
            if step == 1 {
                let wrong_sample = MapGeometryGeneration {
                    sample: generation.sample + 1,
                    ..generation
                };
                cached.rebind_identical_weight_generation(generation, wrong_sample);
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().generation,
                    Some(generation)
                );
                let next = MapGeometryGeneration {
                    weights: generation.weights + 100,
                    ..generation
                };
                cached.rebind_identical_weight_generation(generation, next);
                let repeated = cached
                    .absorb_in_generation(
                        &weights,
                        values.clone(),
                        MapConfig::default(),
                        false,
                        Some(next),
                    )
                    .unwrap();
                assert_eq!(repeated.residualized(), expected.residualized());
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().preparations,
                    1
                );
                assert_eq!(
                    cached
                        .coefficient_solver
                        .as_ref()
                        .and_then(|s| s.coupling_preparations()),
                    coefficients.then_some(1)
                );
                generation = next;
            }
            if step == 5 {
                values[(0, 0)] = f64::NAN;
                assert!(cached
                    .absorb_in_generation(
                        &weights,
                        values,
                        MapConfig::default(),
                        false,
                        Some(generation)
                    )
                    .is_err());
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().generation,
                    None
                );
                cached.rebind_identical_weight_generation(
                    generation,
                    MapGeometryGeneration {
                        weights: generation.weights + 1,
                        ..generation
                    },
                );
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().generation,
                    None
                );
                let zeros = DenseMatrix::zeros(rows, 1).unwrap();
                cached
                    .absorb_in_generation(
                        &weights,
                        zeros.clone(),
                        MapConfig::default(),
                        false,
                        Some(generation),
                    )
                    .unwrap();
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().preparations,
                    4
                );
                assert_eq!(
                    cached
                        .coefficient_solver
                        .as_ref()
                        .and_then(|s| s.coupling_preparations()),
                    coefficients.then_some(4)
                );
                cached
                    .absorb(&weights, zeros, MapConfig::default(), false)
                    .unwrap();
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().generation,
                    None
                );
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().preparations,
                    5
                );
                assert_eq!(
                    cached
                        .coefficient_solver
                        .as_ref()
                        .and_then(|s| s.coupling_preparations()),
                    coefficients.then_some(5)
                );
            }
        }
    }
}

#[test]
fn reusable_certificate_tiles_match_serial_bits_across_threads_widths_and_scales() {
    let rows = 120;
    let z: Vec<_> = (0..rows).map(|i| (i % 7) as f64 - 3.0).collect();
    let design = FeDesign::new(
        rows,
        vec![
            EffectTerm::new(
                (0..rows).map(|i| (i % 5) as u32).collect(),
                6,
                true,
                vec![
                    z.clone(),
                    z.iter().map(|v| 2.0 * v).collect(),
                    vec![0.0; rows],
                ],
            )
            .unwrap(),
            EffectTerm::new(
                (0..rows).map(|i| ((i / 5) % 4) as u32).collect(),
                4,
                false,
                vec![(0..rows).map(|i| (i % 11) as f64 - 5.0).collect()],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut serial = ReusableMapAbsorber::new(design.clone(), 33).unwrap();
        serial.enable_certificate_geometry().unwrap();
        let mut tiled = ReusableMapAbsorber::new(design.clone(), 33).unwrap();
        tiled.enable_certificate_geometry().unwrap();
        assert!(tiled.coefficient_solver.is_none());
        let before = tiled.capacities().accelerator_state;
        tiled.enable_certificate_workspace().unwrap();
        assert_eq!(
            (tiled.capacities().accelerator_state - before) as u128 * 8,
            ReusableMapAbsorber::certificate_workspace_bytes(&design, 33).unwrap(),
        );
        let capacities = tiled.capacities();
        for rhs in [1, 7, 16, 17, 33] {
            for factor in [1e-60, 1.0, 1e60] {
                let weights: Vec<_> = (0..rows)
                    .map(|i| factor * (0.1 + (i % 13) as f64))
                    .collect();
                let mut residuals = DenseMatrix::zeros(rows, rhs).unwrap();
                for row in 0..rows {
                    for column in 1..rhs {
                        residuals[(row, column)] = ((row * 19 + column * 11) as f64).sin()
                            * if column % 2 == 0 { 1e-60 } else { 1e60 };
                    }
                }
                serial.prepare_coefficient_geometry(&weights).unwrap();
                tiled.prepare_coefficient_geometry(&weights).unwrap();
                let expected = serial
                    .certify(&residuals, &weights, rhs, 1e-12, &mut true)
                    .unwrap();
                let actual = pool
                    .install(|| tiled.certify(&residuals, &weights, rhs, 1e-12, &mut true))
                    .unwrap();
                assert_eq!(
                    actual, expected,
                    "threads={threads}, rhs={rhs}, factor={factor}"
                );
                assert_eq!(tiled.capacities(), capacities);
            }
        }
    }
}

#[test]
fn reusable_certificate_geometry_is_exact_and_rebuilt_after_every_call() {
    let rows = 120;
    let z: Vec<_> = (0..rows).map(|i| (i % 7) as f64 - 3.0).collect();
    let design = FeDesign::new(
        rows,
        vec![
            EffectTerm::new(
                (0..rows).map(|i| (i % 5) as u32).collect(),
                6,
                true,
                vec![
                    z.clone(),
                    z.iter().map(|v| 2.0 * v).collect(),
                    vec![0.0; rows],
                ],
            )
            .unwrap(),
            EffectTerm::new(
                (0..rows).map(|i| ((i / 5) % 4) as u32).collect(),
                4,
                false,
                vec![(0..rows).map(|i| (i % 11) as f64 - 5.0).collect()],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let mut plain = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
    let mut cached = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
    cached.enable_certificate_geometry().unwrap();
    let capacities = cached.capacities();
    assert_eq!(capacities.certificate_geometry, rows * 5);
    assert_eq!(capacities.certificate_ranks, 10);
    assert_eq!(
        ReusableMapAbsorber::certificate_geometry_bytes(&design).unwrap(),
        (rows * 5 * 8
            + 10 * std::mem::size_of::<usize>()
            + std::mem::size_of::<Option<MapGeometryGeneration>>()) as u128
    );
    for (generation, rhs) in [7, 1, 3, 7].into_iter().enumerate() {
        let weights: Vec<_> = (0..rows)
            .map(|i| 0.1 + ((i * (generation + 1)) % 13) as f64)
            .collect();
        let mut values = DenseMatrix::zeros(rows, rhs).unwrap();
        for row in 0..rows {
            for col in 0..rhs {
                values[(row, col)] = ((row * 19 + col * 11 + generation) % 97) as f64;
            }
        }
        for limit in [1, 16000] {
            let config = MapConfig {
                max_iterations: limit,
                ..MapConfig::default()
            };
            let expected = plain.absorb(&weights, values.clone(), config, false);
            let actual = cached.absorb(&weights, values.clone(), config, false);
            match (actual, expected) {
                (Ok(actual), Ok(expected)) => {
                    assert_eq!(actual.residualized(), expected.residualized());
                    assert_eq!(actual.report().certificate, expected.report().certificate);
                    assert_eq!(actual.report().iterations, expected.report().iterations);
                    let norms = weighted_column_norms(&values, &weights).unwrap();
                    assert!(
                        certify_residuals_with_reference_norms(
                            &design,
                            &weights,
                            actual.residualized(),
                            &norms,
                            1e-8
                        )
                        .unwrap()
                        .passed
                    );
                }
                (Err(actual), Err(expected)) => assert_eq!(actual.code, expected.code),
                _ => panic!("cached and uncached outcomes differ"),
            }
            assert_eq!(cached.capacities(), capacities);
        }
    }
    // Initial zero-residual success must rebuild even when the old basis is
    // finite but has the wrong numerical rank under the new weights.
    let design = FeDesign::new(
        3,
        vec![EffectTerm::new(
            vec![0; 3],
            2,
            true,
            vec![vec![1e150, 2e150, 3e150], vec![0.0; 3]],
        )
        .unwrap()],
    )
    .unwrap();
    let mut cached = ReusableMapAbsorber::new(design, 1).unwrap();
    cached.enable_certificate_geometry().unwrap();
    for (weights, rank) in [
        (vec![1.0; 3], 2),
        (vec![1.0, 1e-60, 1e-60], 1),
        (vec![1.0; 3], 2),
    ] {
        cached
            .absorb(
                &weights,
                DenseMatrix::zeros(3, 1).unwrap(),
                MapConfig::default(),
                false,
            )
            .unwrap();
        assert_eq!(
            cached.certificate_geometry.as_ref().unwrap().ranks,
            vec![rank, 0]
        );
        assert!(cached
            .absorb(
                &[f64::NAN; 3],
                DenseMatrix::zeros(3, 1).unwrap(),
                MapConfig::default(),
                false
            )
            .is_err());
    }
}

#[test]
fn reusable_parallel_projection_refreshes_weights_rhs_and_capacity() {
    let rows = 16_384;
    let design = FeDesign::new(
        rows,
        vec![
            EffectTerm::new(
                (0..rows).map(|i| (i % 64) as u32).collect(),
                64,
                true,
                vec![(0..rows).map(|i| ((i * 13) % 79) as f64 / 79.0).collect()],
            )
            .unwrap(),
            EffectTerm::new(
                (0..rows).map(|i| ((i / 64) % 32) as u32).collect(),
                32,
                true,
                vec![(0..rows).map(|i| ((i * 17) % 83) as f64 / 83.0).collect()],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let mut serial = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
    let mut parallel = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
    let plan = ParallelProjectionPlan::for_design_and_threads(&design, 7, 4)
        .unwrap()
        .unwrap();
    parallel.enable_parallel_projection(plan).unwrap();
    parallel.enable_certificate_geometry().unwrap();
    parallel.enable_certificate_workspace().unwrap();
    assert!(parallel.coefficient_solver.is_none());
    let before = parallel.capacities();
    assert_eq!(before.projection_workers, 4);
    assert_eq!(
        before.projection_rhs,
        serial.capacities().projection_rhs + 64 * 2 * 7
    );
    assert_eq!(
        before.projection_gram,
        serial.capacities().projection_gram + 4 * 2 * 2
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    for (generation, rhs) in [7, 1, 3, 7].into_iter().enumerate() {
        let weights: Vec<_> = (0..rows)
            .map(|i| 0.1 + ((i * (generation + 1)) % 11) as f64)
            .collect();
        let mut values = DenseMatrix::zeros(rows, rhs).unwrap();
        for row in 0..rows {
            for col in 0..rhs {
                values[(row, col)] = ((row * 19 + col * 11 + generation) % 97) as f64;
            }
        }
        let expected = serial
            .absorb(&weights, values.clone(), MapConfig::default(), false)
            .unwrap();
        let actual = pool
            .install(|| parallel.absorb(&weights, values, MapConfig::default(), false))
            .unwrap();
        assert_eq!(actual.residualized(), expected.residualized());
        assert_eq!(actual.report().certificate, expected.report().certificate);
        assert_eq!(actual.report().iterations, expected.report().iterations);
        assert_eq!(parallel.capacities(), before);
    }
    let mut bad_weights = vec![1.0; rows];
    bad_weights[rows - 1] = f64::NAN;
    assert_eq!(
        parallel
            .absorb(
                &bad_weights,
                DenseMatrix::zeros(rows, 1).unwrap(),
                MapConfig::default(),
                false
            )
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );
    assert_eq!(parallel.capacities(), before);
}

#[test]
fn map_preserves_scheduled_certificates_and_iteration_boundaries() {
    let rows = 60;
    let left = EffectTerm::new(
        (0..rows).map(|i| (i % 5) as u32).collect(),
        6,
        true,
        vec![(0..rows).map(|i| (i % 7) as f64 - 3.0).collect()],
    )
    .unwrap();
    let right = EffectTerm::new(
        (0..rows).map(|i| ((i / 5) % 4) as u32).collect(),
        4,
        true,
        vec![(0..rows).map(|i| (i % 11) as f64 - 5.0).collect()],
    )
    .unwrap();
    let design = FeDesign::new(rows, vec![left, right]).unwrap();
    for rhs in [0, 1, 7] {
        for weighted in [false, true] {
            let weights: Vec<_> = (0..rows)
                .map(|i| if weighted { 0.5 + (i % 7) as f64 } else { 1.0 })
                .collect();
            for max_iterations in [1, 3, 8, 100] {
                for check_every in [1, 4, 99] {
                    for symmetric in [false, true] {
                        let mut actual = DenseMatrix::zeros(rows, rhs).unwrap();
                        for row in 0..rows {
                            for col in 0..rhs {
                                actual[(row, col)] = ((row * 13 + col * 5) % 37) as f64 - 18.0;
                            }
                        }
                        let mut expected = actual.clone();
                        let config = MapConfig {
                            max_iterations,
                            check_every,
                            symmetric,
                            ..MapConfig::default()
                        };
                        let a = absorb_map(&design, &weights, &mut actual, config).unwrap();
                        let b =
                            baseline_absorb_map(&design, &weights, &mut expected, config).unwrap();
                        assert_eq!(a.iterations, b.iterations);
                        assert_eq!(a.stop_reason, b.stop_reason);
                        assert_eq!(a.certificate, b.certificate);
                        for (x, y) in actual.as_slice().iter().zip(expected.as_slice()) {
                            assert_eq!(x.to_bits(), y.to_bits());
                        }
                    }
                }
            }
        }
    }
    for check_every in [1, 4, 99] {
        let mut zeros = DenseMatrix::zeros(rows, 1).unwrap();
        let report = absorb_map(
            &design,
            &vec![1.0; rows],
            &mut zeros,
            MapConfig {
                max_iterations: 8,
                check_every,
                ..MapConfig::default()
            },
        )
        .unwrap();
        assert_eq!(report.iterations, check_every.min(8));
        assert!(report.certified());
    }
}

#[test]
fn map_rejects_weighted_basis_overflow_before_mutation() {
    let effect = EffectTerm::new(vec![0; 4], 1, true, vec![vec![f64::MAX / 2.0; 4]]).unwrap();
    let design = FeDesign::new(4, vec![effect]).unwrap();
    let original = DenseMatrix::from_columns(&[vec![0.0, 1.0, -1.0, 0.0]]).unwrap();
    let mut actual = original.clone();
    let mut expected = original.clone();
    let a = absorb_map(&design, &[16.0; 4], &mut actual, MapConfig::default()).unwrap_err();
    let b =
        baseline_absorb_map(&design, &[16.0; 4], &mut expected, MapConfig::default()).unwrap_err();
    assert_eq!(a, b);
    assert_eq!(a.code, ErrorCode::InvalidData);
    assert_eq!(actual, original);
    assert_eq!(expected, original);
}

fn baseline_absorb_map(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: MapConfig,
) -> Result<SolveReport> {
    validate_config(config)?;
    let started = Instant::now();
    if design.nobs() != values.nrows() || weights.len() != values.nrows() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "MAP inputs have inconsistent observation counts",
        ));
    }
    let reference_norms = weighted_column_norms(values, weights)?;
    if design.effects().is_empty() {
        let certificate = certify_residuals_with_reference_norms(
            design,
            weights,
            values,
            &reference_norms,
            config.tolerance,
        )?;
        return Ok(SolveReport {
            checkpoint_retries: 0,
            solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
            requested_solver: FeSolverKind::Map,
            used_solver: FeSolverKind::Map,
            iterations: 0,
            stop_reason: SolveStopReason::NoFixedEffects,
            certificate,
            normal_equation_residual: 0.0,
            setup_seconds: 0.0,
            solve_seconds: started.elapsed().as_secs_f64(),
            certification_seconds: 0.0,
            fallback_used: false,
            requested_preconditioner: None,
            used_preconditioner: None,
            pilot_iterations: 0,
            pilot_contraction: 0.0,
            warnings: Vec::new(),
        });
    }
    let mut workspace = ProjectionWorkspace::new(design.max_channels(), values.ncols())?;
    let mut final_certificate = certify_residuals_with_reference_norms(
        design,
        weights,
        values,
        &reference_norms,
        config.tolerance,
    )?;

    for iteration in 1..=config.max_iterations {
        for effect in design.effects() {
            project_effect_in_place(effect, weights, values, &mut workspace)?;
        }
        if config.symmetric && design.effects().len() > 1 {
            for effect in design.effects().iter().rev().skip(1) {
                project_effect_in_place(effect, weights, values, &mut workspace)?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            final_certificate = certify_residuals_with_reference_norms(
                design,
                weights,
                values,
                &reference_norms,
                config.tolerance,
            )?;
            if final_certificate.passed {
                return Ok(SolveReport {
                    checkpoint_retries: 0,
                    solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
                    requested_solver: FeSolverKind::Map,
                    used_solver: FeSolverKind::Map,
                    iterations: iteration,
                    stop_reason: SolveStopReason::Certified,
                    normal_equation_residual: final_certificate.relative_two_norm,
                    certificate: final_certificate,
                    setup_seconds: 0.0,
                    solve_seconds: started.elapsed().as_secs_f64(),
                    certification_seconds: 0.0,
                    fallback_used: false,
                    requested_preconditioner: None,
                    used_preconditioner: None,
                    pilot_iterations: 0,
                    pilot_contraction: 0.0,
                    warnings: Vec::new(),
                });
            }
        }
    }

    Ok(SolveReport {
        checkpoint_retries: 0,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
        requested_solver: FeSolverKind::Map,
        used_solver: FeSolverKind::Map,
        iterations: config.max_iterations,
        stop_reason: SolveStopReason::MaximumIterations,
        normal_equation_residual: final_certificate.relative_two_norm,
        certificate: final_certificate,
        setup_seconds: 0.0,
        solve_seconds: started.elapsed().as_secs_f64(),
        certification_seconds: 0.0,
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
    })
}

#[test]
fn absorbs_two_way_intercepts_and_certifies() {
    let worker = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("worker");
    let firm = EffectTerm::new(vec![0, 1, 0, 1, 0, 1], 2, true, vec![]).expect("firm");
    let design = FeDesign::new(6, vec![worker, firm]).expect("design");
    let weights = vec![1.0; 6];
    let mut values =
        DenseMatrix::from_columns(&[vec![1.0, 2.0, 2.0, 3.0, 4.0, 5.0]]).expect("values");
    let report = absorb_map(&design, &weights, &mut values, MapConfig::default()).expect("absorb");
    assert_eq!(report.stop_reason, SolveStopReason::Certified);
    assert!(report.certificate.passed);
    assert!(values.as_slice().iter().all(|value| value.abs() < 1e-8));
}
