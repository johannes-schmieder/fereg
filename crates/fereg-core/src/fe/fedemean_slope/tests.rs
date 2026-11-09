use super::*;
use crate::fe::EffectTerm;

/// `worker + firm##c.t` with a constant slope on firm 0 and a zero slope on
/// firm 1 (singular channel blocks), plus a slope-only `year#c.t`.
fn design() -> (FeDesign, Vec<f64>, DenseMatrix) {
    let n = 240;
    let worker: Vec<u32> = (0..n).map(|r| (r / 6) as u32).collect();
    let firm: Vec<u32> = (0..n).map(|r| ((r / 6 + (r % 6) / 2) % 9) as u32).collect();
    let year: Vec<u32> = (0..n).map(|r| (r % 6) as u32).collect();
    let t: Vec<f64> = (0..n)
        .map(|r| match firm[r] {
            0 => 2.0,
            1 => 0.0,
            _ => 1.0 + (r % 6) as f64 + ((r * 7 % 5) as f64 - 2.0) / 8.0,
        })
        .collect();
    let u: Vec<f64> = (0..n).map(|r| ((r * 11 % 13) as f64 - 6.0) / 3.0).collect();
    let y: Vec<f64> = (0..n)
        .map(|r| {
            f64::from(worker[r] % 4) + 0.3 * f64::from(firm[r]) * t[r] - 0.2 * u[r]
                + ((r * 31 % 17) as f64 - 8.0) / 20.0
        })
        .collect();
    let x: Vec<f64> = (0..n)
        .map(|r| (r as f64 * 0.21).sin() + t[r] / 4.0)
        .collect();
    let design = FeDesign::new(
        n,
        vec![
            EffectTerm::new(worker, 40, true, vec![]).unwrap(),
            EffectTerm::new(firm, 9, true, vec![t]).unwrap(),
            EffectTerm::new(year, 6, false, vec![u]).unwrap(),
        ],
    )
    .unwrap();
    let weights = (0..n).map(|r| 1.0 + (r % 4) as f64 / 2.0).collect();
    (design, weights, DenseMatrix::from_columns(&[y, x]).unwrap())
}

fn fitted(design: &FeDesign, layout: &Layout, coefficients: &[f64], rhs: usize) -> Vec<f64> {
    (0..design.nobs())
        .map(|row| {
            let column = &coefficients[rhs * layout.per_rhs..(rhs + 1) * layout.per_rhs];
            design
                .effects()
                .iter()
                .enumerate()
                .map(|(e, effect)| {
                    let level = effect.codes()[row] as usize;
                    (0..effect.channels())
                        .map(|c| {
                            column[layout.offsets[e] + c * effect.n_levels() + level]
                                * effect.basis_value(c, row)
                        })
                        .sum::<f64>()
                })
                .sum()
        })
        .collect()
}

#[test]
fn the_view_borrows_codes_and_slopes_and_the_layout_is_fereg_s() {
    let (design, _, _) = design();
    let view = design_view(&design).unwrap();
    for (effect, entry) in design.effects().iter().zip(view.effects()) {
        assert!(std::ptr::eq(effect.codes(), entry.codes()));
        assert_eq!(entry.channels(), effect.channels());
        for (slope, borrowed) in effect.slopes().iter().zip(entry.slopes()) {
            assert!(std::ptr::eq(slope.as_slice(), *borrowed));
        }
    }
    let layout = Layout::new(&design).unwrap();
    let batch = layout.batch(layout.zeros(2).unwrap(), 2).unwrap();
    assert_eq!(batch.effect_offsets()[..3], layout.offsets[..]);
    assert_eq!(batch.coefficients_per_rhs(), layout.per_rhs);
    assert_eq!(batch.effect_channels(), &[1, 2, 1]);
}

#[test]
fn certified_projection_returns_its_coefficients_and_repeats_at_any_worker_count() {
    let (design, weights, original) = design();
    let layout = Layout::new(&design).unwrap();
    let mut reference = None;
    for threads in [1, 3, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for attempt in [
                    Attempt::auto(1e-10, 2000, None),
                    Attempt::lsmr(
                        LsmrPreconditioner::Auto,
                        FeSolverKind::Lsmr,
                        1e-10,
                        2000,
                        None,
                    ),
                    Attempt::lsmr(
                        LsmrPreconditioner::Diagonal,
                        FeSolverKind::Lsmr,
                        1e-10,
                        2000,
                        None,
                    ),
                    Attempt::lsmr(
                        LsmrPreconditioner::Schwarz,
                        FeSolverKind::Lsmr,
                        1e-10,
                        2000,
                        None,
                    ),
                ] {
                    let mut values = original.clone();
                    let mut coefficients = layout.zeros(2).unwrap();
                    let certified = absorb_certified(
                        &design,
                        (&weights, false),
                        &mut values,
                        &attempt,
                        Some((&layout, &mut coefficients)),
                    )
                    .unwrap();
                    assert!(certified.declined.is_none());
                    assert!(certified.report.certified());
                    for rhs in 0..2 {
                        let fit = fitted(&design, &layout, &coefficients, rhs);
                        for row in 0..design.nobs() {
                            let removed = original[(row, rhs)] - values[(row, rhs)];
                            assert!((removed - fit[row]).abs() < 1e-11, "{row}");
                        }
                    }
                    // Zero slopes on a level keep exactly zero coefficients.
                    let firm = &design.effects()[1];
                    for level in 0..firm.n_levels() {
                        if level == 1 {
                            let slot = layout.offsets[1] + firm.n_levels() + level;
                            assert_eq!(coefficients[slot], 0.0);
                        }
                    }
                    if attempt.choice.method == fedemean::Method::Pcg {
                        let result = (values, coefficients);
                        match &reference {
                            None => reference = Some(result),
                            Some(previous) => assert_eq!(&result, previous, "{threads}"),
                        }
                    }
                }
            });
    }
}

#[test]
fn declines_report_uncertified_or_refuse_memory() {
    let (design, weights, original) = design();
    let mut values = original.clone();
    let declined = absorb_certified(
        &design,
        (&weights, false),
        &mut values,
        &Attempt::lsmr(LsmrPreconditioner::Off, FeSolverKind::Lsmr, 1e-12, 2, None),
        None,
    )
    .unwrap();
    assert!(declined.declined.is_some());
    assert!(!declined.report.certified());
    assert_eq!(declined.report.used_solver, FeSolverKind::Lsmr);
    let mut values = original.clone();
    let refused = absorb_certified(
        &design,
        (&weights, false),
        &mut values,
        &Attempt::lsmr(
            LsmrPreconditioner::Schwarz,
            FeSolverKind::Lsmr,
            1e-10,
            2000,
            Some(1),
        ),
        None,
    );
    assert_eq!(
        refused.err().map(|e| e.code),
        Some(crate::error::ErrorCode::Allocation)
    );
}
