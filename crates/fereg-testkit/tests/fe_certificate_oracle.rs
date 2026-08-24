use fereg_core::{
    absorb_lsmr, absorb_map, certify_residuals, DenseMatrix, EffectTerm, FeDesign, LsmrConfig,
    LsmrPreconditioner, MapConfig, SolveStopReason, ThreadConfig,
};
use fereg_testkit::explicit_dummy::fe_projection_certificate;

#[test]
fn nearly_collinear_channel_counterexample_is_rejected() {
    let sample_size = 20;
    let time = (0..sample_size)
        .map(|row| row as f64 - (sample_size - 1) as f64 / 2.0)
        .collect::<Vec<_>>();
    let nearly_constant = time
        .iter()
        .map(|value| 1.0 + 1e-9 * value)
        .collect::<Vec<_>>();
    let effect =
        EffectTerm::new(vec![0; sample_size], 1, true, vec![nearly_constant]).expect("effect");
    let design = FeDesign::new(sample_size, vec![effect]).expect("design");
    let residuals = DenseMatrix::from_columns(&[time]).expect("residuals");

    let certificate =
        certify_residuals(&design, &vec![1.0; sample_size], &residuals, 1e-8).expect("certificate");

    assert!(!certificate.passed);
    assert!(certificate.relative_infinity_norm > 0.99);
}

#[test]
fn certificate_is_invariant_to_equivalent_channel_bases() {
    let sample_size = 18;
    let time = (0..sample_size)
        .map(|row| row as f64 - 8.5)
        .collect::<Vec<_>>();
    let curve = time
        .iter()
        .map(|value| value.mul_add(*value, 0.25 * value))
        .collect::<Vec<_>>();
    let codes = vec![0; sample_size];
    let baseline = EffectTerm::new(codes.clone(), 1, true, vec![time.clone(), curve.clone()])
        .expect("baseline");
    let shifted_scaled = EffectTerm::new(
        codes.clone(),
        1,
        true,
        vec![
            time.iter().map(|value| 3.0 * value + 2.0).collect(),
            curve.iter().map(|value| -0.2 * value + 5.0).collect(),
        ],
    )
    .expect("shifted and scaled");
    let permuted = EffectTerm::new(codes.clone(), 1, true, vec![curve.clone(), time.clone()])
        .expect("permuted");
    let mixed = EffectTerm::new(
        codes,
        1,
        true,
        vec![
            time.iter()
                .zip(&curve)
                .map(|(left, right)| 2.0 + 3.0 * left - right)
                .collect(),
            time.iter()
                .zip(&curve)
                .map(|(left, right)| -1.0 + 0.5 * left + 2.0 * right)
                .collect(),
        ],
    )
    .expect("mixed");
    let weights = (0..sample_size)
        .map(|row| 0.5 + (row % 5) as f64)
        .collect::<Vec<_>>();
    let residuals = DenseMatrix::from_columns(&[time
        .iter()
        .enumerate()
        .map(|(row, value)| (0.7 * value).sin() + row as f64 / 17.0)
        .collect()])
    .expect("residuals");

    let values = [baseline, shifted_scaled, permuted, mixed]
        .into_iter()
        .map(|effect| {
            let design = FeDesign::new(sample_size, vec![effect]).expect("design");
            certify_residuals(&design, &weights, &residuals, 1e-12)
                .expect("certificate")
                .relative_infinity_norm
        })
        .collect::<Vec<_>>();

    for value in &values[1..] {
        assert!((value - values[0]).abs() < 2e-12, "{values:?}");
    }
}

#[test]
fn overflowing_raw_moment_intermediates_remain_nonfatal_diagnostics() {
    let codes = vec![0; 4];
    let extreme = EffectTerm::new(
        codes.clone(),
        1,
        false,
        vec![vec![1e308, 1e308, -1e308, -1e308]],
    )
    .expect("extreme basis");
    let scaled =
        EffectTerm::new(codes, 1, false, vec![vec![1.0, 1.0, -1.0, -1.0]]).expect("scaled basis");
    let residuals = DenseMatrix::from_columns(&[vec![1.0; 4]]).expect("residuals");

    let extreme = certify_residuals(
        &FeDesign::new(4, vec![extreme]).expect("extreme design"),
        &[1.0; 4],
        &residuals,
        1e-12,
    )
    .expect("extreme certificate");
    let scaled = certify_residuals(
        &FeDesign::new(4, vec![scaled]).expect("scaled design"),
        &[1.0; 4],
        &residuals,
        1e-12,
    )
    .expect("scaled certificate");

    assert!(extreme.passed);
    assert_eq!(extreme.passed, scaled.passed);
    assert!((extreme.relative_infinity_norm - scaled.relative_infinity_norm).abs() < 1e-14);
    assert_eq!(extreme.absolute_infinity_norm, 0.0);
    assert_eq!(scaled.absolute_infinity_norm, 0.0);
}

#[test]
fn duplicate_and_constant_channels_match_the_reduced_basis() {
    let time = vec![-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
    let codes = vec![0; time.len()];
    let reduced = EffectTerm::new(codes.clone(), 1, true, vec![time.clone()]).expect("reduced");
    let redundant = EffectTerm::new(
        codes,
        1,
        true,
        vec![vec![1.0; time.len()], time.clone(), time.clone()],
    )
    .expect("redundant");
    let residuals =
        DenseMatrix::from_columns(&[vec![2.0, -1.0, 4.0, 0.5, 3.0, -2.0, 1.0]]).expect("residuals");
    let weights = vec![1.0, 2.0, 3.0, 1.5, 0.75, 4.0, 2.5];
    let reduced = certify_residuals(
        &FeDesign::new(time.len(), vec![reduced]).expect("design"),
        &weights,
        &residuals,
        1e-12,
    )
    .expect("certificate");
    let redundant = certify_residuals(
        &FeDesign::new(time.len(), vec![redundant]).expect("design"),
        &weights,
        &residuals,
        1e-12,
    )
    .expect("certificate");

    assert!((reduced.relative_infinity_norm - redundant.relative_infinity_norm).abs() < 1e-14);
}

#[test]
fn randomized_multiple_rhs_matches_independent_householder_oracle() {
    let mut generator = Generator::new(0x5f1e57c3);
    for _ in 0..32 {
        let sample_size = 19 + generator.usize(29);
        let first_codes = (0..sample_size)
            .map(|_| generator.usize(5) as u32)
            .collect::<Vec<_>>();
        let second_codes = (0..sample_size)
            .map(|_| generator.usize(4) as u32)
            .collect::<Vec<_>>();
        let first_slope = (0..sample_size)
            .map(|_| generator.f64(-2.0, 2.0))
            .collect::<Vec<_>>();
        let second_slope = (0..sample_size)
            .map(|row| first_slope[row].mul_add(0.4, generator.f64(-1.0, 1.0)))
            .collect::<Vec<_>>();
        let design = FeDesign::new(
            sample_size,
            vec![
                EffectTerm::new(first_codes, 5, true, vec![first_slope]).expect("first effect"),
                EffectTerm::new(second_codes, 4, false, vec![second_slope]).expect("second effect"),
            ],
        )
        .expect("design");
        let weights = (0..sample_size)
            .map(|_| generator.f64(0.2, 4.0))
            .collect::<Vec<_>>();
        let columns = (0..3)
            .map(|_| {
                (0..sample_size)
                    .map(|_| generator.f64(-3.0, 3.0))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let residuals = DenseMatrix::from_columns(&columns).expect("residuals");

        let production =
            certify_residuals(&design, &weights, &residuals, 1e-12).expect("production");
        let oracle =
            fe_projection_certificate(&design, &weights, &residuals).expect("explicit oracle");

        assert!(
            (production.relative_infinity_norm - oracle.relative_infinity_norm).abs() < 5e-12,
            "production={production:?}, oracle={oracle:?}"
        );
        assert!(
            (production.relative_two_norm - oracle.relative_two_norm).abs() < 5e-12,
            "production={production:?}, oracle={oracle:?}"
        );
    }
}

#[test]
fn scaling_zero_nonfinite_and_exact_rank_edges_fail_closed() {
    let effect =
        EffectTerm::new(vec![0, 0, 0], 1, true, vec![vec![7.0; 3], vec![0.0; 3]]).expect("effect");
    let design = FeDesign::new(3, vec![effect]).expect("design");
    let residuals = DenseMatrix::from_columns(&[vec![1.0, -2.0, 4.0]]).expect("residuals");
    let first =
        certify_residuals(&design, &[0.5, 2.0, 3.0], &residuals, 1e-12).expect("certificate");
    let second = certify_residuals(&design, &[50.0, 200.0, 300.0], &residuals, 1e-12)
        .expect("scaled certificate");
    assert!((first.relative_infinity_norm - second.relative_infinity_norm).abs() < 1e-14);

    let zero = DenseMatrix::from_columns(&[vec![0.0; 3]]).expect("zero RHS");
    let zero_certificate =
        certify_residuals(&design, &[1.0; 3], &zero, 1e-12).expect("zero certificate");
    assert!(zero_certificate.passed);
    assert_eq!(zero_certificate.relative_infinity_norm, 0.0);

    let one_row_effect =
        EffectTerm::new(vec![0], 1, true, vec![vec![2.0], vec![-9.0]]).expect("one row effect");
    let one_row_design = FeDesign::new(1, vec![one_row_effect]).expect("one row design");
    let one_row_residual = DenseMatrix::from_columns(&[vec![3.0]]).expect("one row residual");
    let exact_rank = certify_residuals(&one_row_design, &[2.0], &one_row_residual, 1e-12)
        .expect("exact-rank certificate");
    assert!((exact_rank.relative_infinity_norm - 1.0).abs() < 1e-14);
    assert!(!exact_rank.passed);

    let nonfinite = DenseMatrix::from_columns(&[vec![1.0, f64::NAN, 2.0]]).expect("nonfinite");
    assert!(certify_residuals(&design, &[1.0; 3], &nonfinite, 1e-12).is_err());
    assert!(certify_residuals(&design, &[1.0, f64::INFINITY, 1.0], &residuals, 1e-12).is_err());
}

#[test]
fn solver_reports_cannot_claim_certified_when_final_projection_fails() {
    let first = EffectTerm::new(vec![0, 0, 0, 1, 1, 1], 2, true, vec![]).expect("first");
    let second = EffectTerm::new(vec![0, 0, 1, 0, 1, 1], 2, true, vec![]).expect("second");
    let design = FeDesign::new(6, vec![first, second]).expect("design");
    let weights = vec![1.0, 2.0, 0.7, 3.0, 1.5, 0.5];
    let original =
        DenseMatrix::from_columns(&[vec![1.0, -3.0, 2.0, 5.0, -4.0, 0.5]]).expect("values");

    let mut map_values = original.clone();
    let map = absorb_map(
        &design,
        &weights,
        &mut map_values,
        MapConfig {
            tolerance: 1e-16,
            max_iterations: 1,
            check_every: 1,
            symmetric: false,
        },
    )
    .expect("MAP");
    assert!(!map.certificate.passed);
    assert_ne!(map.stop_reason, SolveStopReason::Certified);

    let mut lsmr_values = original;
    let lsmr = absorb_lsmr(
        &design,
        &weights,
        &mut lsmr_values,
        LsmrConfig {
            tolerance: 1e-16,
            max_iterations: 1,
            local_size: None,
            preconditioner: LsmrPreconditioner::Off,
            threads: ThreadConfig::from_requested(1, true).expect("thread config"),
        },
    )
    .expect("LSMR");
    assert!(!lsmr.certificate.passed);
    assert_ne!(lsmr.stop_reason, SolveStopReason::Certified);
}

struct Generator(u64);

impl Generator {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn usize(&mut self, upper: usize) -> usize {
        (self.next() as usize) % upper
    }

    fn f64(&mut self, lower: f64, upper: f64) -> f64 {
        let unit = (self.next() >> 11) as f64 / (1_u64 << 53) as f64;
        (upper - lower).mul_add(unit, lower)
    }
}
