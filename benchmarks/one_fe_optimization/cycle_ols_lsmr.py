"""Isolated exact-cycle route experiment; production routing remains unchanged."""
from pathlib import Path
import re


def instrument(source):
    path = source / 'crates/fereg-core/src/ols/prepared_fit.rs'
    text = path.read_text()
    field = "    single_slope: Option<crate::fe::SingleSlopeAbsorber<'a>>,"
    assert text.count(field) == 1
    text = text.replace(field, field + "\n    cycle_probe_cost: Option<(f64, crate::memory::SolverWorkspacePeak, bool)>,")
    text = re.sub(r'^(\s*)single_slope: (?!Option<)',
                  r'\1cycle_probe_cost: None,\n\1single_slope: ', text, flags=re.MULTILINE)
    anchor = "fn build_fit_fe_absorber_with_one_fe_reuse<'a, P: PreparedOlsFitView + ?Sized>("
    assert text.count(anchor) == 1
    text = text.replace(anchor, "fn build_fit_fe_absorber_without_cycle<'a, P: PreparedOlsFitView + ?Sized>(")
    wrapper = '''#[path = "cycle_ols_lsmr_experiment.rs"]
mod cycle_ols_lsmr_experiment;

fn build_fit_fe_absorber_with_one_fe_reuse<'a, P: PreparedOlsFitView + ?Sized>(
    prepared: &'a P, weights: BorrowedMapWeights<'a>, config: PreparedOlsFitConfig,
    reuse: bool, allow_single_slope: bool,
) -> Result<FitFeAbsorber<'a>> {
    let mut seconds = 0.0;
    let mut peak = crate::memory::SolverWorkspacePeak::ZERO;
    let width = prepared.fit_model().columns.len().checked_add(1)
        .ok_or_else(|| allocation("cycle route RHS width overflows"))?;
    let threads = config.execution.threads.resolved();
    if config.allow_final_fe_recovery && config.absorb.solver == FeSolverKind::Auto
        && config.execution.mode == PreparedOlsExecutionMode::Fast
        && config.absorb.certified_fallback && prepared.fit_supports_cmg_auto()
        && (8..=64).contains(&width) && (threads == 1 || threads >= 8)
        && config.absorb.map.max_iterations > crate::fe::OLS_SCHUR_MAX_ITERATIONS + crate::fe::OLS_SCHUR_PROBE_ITERATIONS
    {
        let started = std::time::Instant::now();
        let design = prepared.fit_borrowed_cmg_design()?;
        let (selected, bytes) = cycle_ols_lsmr_experiment::probe(&design, weights, config.absorb.solver_memory_budget)?;
        seconds = started.elapsed().as_secs_f64();
        peak = crate::memory::SolverWorkspacePeak::from_bytes(bytes)?;
        if selected {
            return Ok(FitFeAbsorber {
                single_slope: None, one_fe: None, session: None, lsmr: None,
                cycle_probe_cost: Some((seconds, peak, true)),
                auto_direct_solver: Some(FeSolverKind::Lsmr),
                auto_discarded_setup_seconds: 0.0,
                auto_discarded_workspace_peak: crate::memory::SolverWorkspacePeak::ZERO,
                auto_setup_warning: None,
                schur_config: None, schur_probe: None,
            });
        }
    }
    let mut absorber = build_fit_fe_absorber_without_cycle(prepared, weights, config, reuse, allow_single_slope)?;
    if seconds != 0.0 {
        absorber.cycle_probe_cost = Some((seconds, peak, false));
    }
    Ok(absorber)
}

'''
    anchor = "fn build_fit_fe_absorber_without_cycle<'a, P: PreparedOlsFitView + ?Sized>("
    text = text.replace(anchor, wrapper + anchor)
    anchor = "fn absorb_fit_matrix<'a, P: PreparedOlsFitView + ?Sized>("
    assert text.count(anchor) == 1
    text = text.replace(anchor, "fn absorb_fit_matrix_without_cycle<'a, P: PreparedOlsFitView + ?Sized>(")
    # Account for both accepted and rejected probes outside every early Schur
    # return, and outside the assignment that replaces a rejected Schur owner.
    wrapper = '''fn absorb_fit_matrix<'a, P: PreparedOlsFitView + ?Sized>(
    original_norms: Option<&[f64]>, prepared: &'a P, weights: BorrowedMapWeights<'a>,
    values: DenseMatrix, config: AbsorbConfig, cmg: &mut FitFeAbsorber<'a>,
    initial: bool, final_residual: bool,
) -> Result<CertifiedBorrowedMap> {
    let cost = cmg.cycle_probe_cost.take();
    let mut result = absorb_fit_matrix_without_cycle(original_norms, prepared, weights,
        values, config, cmg, initial, final_residual)?;
    let Some((seconds, peak, selected)) = cost else { return Ok(result); };
    let norms = result.take_residual_norms();
    let (values, mut report, coefficients) = result.into_full_parts();
    report.setup_seconds += seconds;
    report.solver_workspace_peak = report.solver_workspace_peak.max(peak);
    if selected {
        report.warnings.push("experimental exact-cycle guard selected automatic LSMR".to_owned());
    }
    let result = if let Some(coefficients) = coefficients {
        CertifiedBorrowedMap::from_certified_parts_with_coefficients(values, report, coefficients)?
    } else {
        CertifiedBorrowedMap::from_certified_parts(values, report)?
    };
    if norms.is_empty() { Ok(result) } else { result.with_residual_norms(norms) }
}

'''
    anchor = "fn absorb_fit_matrix_without_cycle<'a, P: PreparedOlsFitView + ?Sized>("
    path.write_text(text.replace(anchor, wrapper + anchor))
    path.with_name('cycle_ols_lsmr_experiment.rs').write_bytes(
        Path(__file__).with_suffix('.rs').read_bytes())
