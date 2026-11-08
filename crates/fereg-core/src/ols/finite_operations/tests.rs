//! Production factor retries against analytic observation-space witnesses.
use super::*;
use crate::data::DenseMatrix;
use crate::ols::centered::{CenteredRows, CenteredSummary};
use crate::ols::prepared_fit::SystemFitReservation;

fn raw_design(rows: usize, separation: Option<f64>) -> Result<DenseMatrix> {
    let mut raw = DenseMatrix::zeros(rows, 3)?;
    for row in 0..rows {
        let x = if row % 2 == 0 { 1.0 } else { -1.0 };
        let z = if row % 4 < 2 { 1.0 } else { -1.0 };
        let noise = if matches!(row % 4, 0 | 3) {
            0.25
        } else {
            -0.25
        };
        let second = separation.map_or(2.0 * z, |delta| x + delta * z);
        raw[(row, 0)] = 3.0 + 2.0 * x - 0.5 * second + noise;
        raw[(row, 1)] = x;
        raw[(row, 2)] = second;
    }
    Ok(raw)
}

fn retry(
    raw: &DenseMatrix,
    summary: &CenteredSummary,
    allow_cholesky: bool,
) -> Result<(Option<FactorResult>, FiniteWorkspace, usize)> {
    let ids = [0, 1];
    let source = FixedBlockMomentSource::Centered {
        values: CenteredRows::new(raw, summary)?,
        selected_columns: Some(&ids),
        column_scales: Some(&summary.raw_norms()[1..]),
    };
    let mut reservation = SystemFitReservation;
    let validated = validate_moment_source(source)?;
    let mut workspace = FiniteWorkspace::default();
    resize(&mut workspace.gram, 4, 0.0, &mut reservation)?;
    resize(&mut workspace.rhs, 2, 0.0, &mut reservation)?;
    resize(
        &mut workspace.moment_partials,
        validated.layout().partial_elements,
        0.0,
        &mut reservation,
    )?;
    fill_finite_wls_moments(
        validated,
        &mut workspace.moment_partials,
        &mut workspace.rhs,
        &mut workspace.gram,
    )?;
    // The real solver reuses the moment allocation. Its empty logical length
    // lets the refusal witness detect any entry into compression preparation.
    workspace.moment_partials.clear();
    let mut captures = 0;
    let factor = solve_factor(
        source,
        &ids,
        128.0 * f64::EPSILON * raw.nrows() as f64,
        RankRequest::OlsDiscover,
        allow_cholesky,
        FactorBanks {
            gram: &workspace.gram,
            rhs: &workspace.rhs,
            factor: &mut workspace.factor,
            auxiliary: &mut workspace.inverse_lower,
            solve: &mut workspace.solve,
            scores: &mut workspace.scores,
            row: &mut workspace.row,
            permutation: &mut workspace.permutation,
            coefficients: &mut workspace.coefficients,
            inverse: &mut workspace.bread,
            selected: &mut workspace.kept,
            compression: &mut workspace.moment_partials,
        },
        &mut reservation,
        |relations, rank, _| {
            captures += 1;
            assert_eq!(rank, 2);
            assert_eq!(relations.candidates, &[0, 1]);
            assert_eq!(relations.selected, &[0, 1]);
            assert!(relations.dropped_projection.is_empty());
            Ok(())
        },
    )?;
    Ok((factor, workspace, captures))
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual:e}, expected={expected:e}, tolerance={tolerance:e}"
    );
}

#[test]
fn centered_production_forced_qr_retry_has_analytic_solution_and_inverse() {
    let raw = raw_design(1024, None).unwrap();
    let summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let (ordinary, _, _) = retry(&raw, &summary, true).unwrap();
    assert!(!ordinary.unwrap().used_qr);
    let (retried, workspace, captures) = retry(&raw, &summary, false).unwrap();
    let retried = retried.expect("strong full-rank guard permits orthogonal retry");
    assert!(retried.used_qr);
    assert_eq!(retried.report.rank, 2);
    assert!(!retried.report.ambiguous);
    assert_eq!(captures, 1);
    assert!(!workspace.moment_partials.is_empty());
    assert_eq!(workspace.kept, vec![0, 1]);
    // x and z are orthogonal, centered sign columns, with squared norms
    // N and 4N. Noise is orthogonal to both and to the intercept.
    close(
        workspace.coefficients[0] / summary.raw_norms()[1],
        2.0,
        2e-12,
    );
    close(
        workspace.coefficients[1] / summary.raw_norms()[2],
        -0.5,
        2e-12,
    );
    for (actual, expected) in workspace.bread.iter().zip([1.0, 0.0, 0.0, 1.0]) {
        close(*actual, expected, 2e-12);
    }
    close(
        workspace.bread[0] / summary.raw_norms()[1].powi(2),
        1.0 / 1024.0,
        2e-15,
    );
    close(
        workspace.bread[3] / summary.raw_norms()[2].powi(2),
        1.0 / 4096.0,
        2e-15,
    );
}

#[test]
fn centered_rank_boundary_refuses_before_compression_even_on_forced_retry() {
    let rows = 1024;
    let threshold = 128.0 * f64::EPSILON * rows as f64;
    for delta in [0.0, threshold * 0.5, threshold, threshold * 2.0] {
        let raw = raw_design(rows, Some(delta)).unwrap();
        let summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
        for allow_cholesky in [true, false] {
            let (factor, workspace, captures) = retry(&raw, &summary, allow_cholesky).unwrap();
            assert!(
                factor.is_none(),
                "delta={delta:e}, allow_cholesky={allow_cholesky}"
            );
            assert!(
                workspace.moment_partials.is_empty(),
                "compression was prepared before guard refusal"
            );
            assert!(workspace.coefficients.is_empty());
            assert!(workspace.bread.is_empty());
            assert!(workspace.kept.is_empty());
            assert_eq!(captures, 0);
        }
    }
}

#[test]
fn centered_production_qr_retry_repeats_across_worker_counts() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let raw = raw_design(32_772, None).unwrap();
    let summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let mut baseline = None;
    for workers in [1, 2, 4] {
        let team = crate::parallel::ThreadTeam::new(
            crate::options::ThreadConfig::from_requested(workers, false).unwrap(),
        );
        let (factor, workspace, _) = team
            .install(|| retry(&raw, &summary, false))
            .unwrap()
            .unwrap();
        let factor = factor.unwrap();
        assert!(factor.used_qr);
        assert_eq!(factor.report.rank, 2);
        assert!(!factor.report.ambiguous);
        let bits: Vec<_> = workspace
            .coefficients
            .iter()
            .chain(&workspace.bread)
            .map(|value| value.to_bits())
            .collect();
        if let Some(expected) = &baseline {
            assert_eq!(&bits, expected);
        } else {
            baseline = Some(bits);
        }
    }
}
