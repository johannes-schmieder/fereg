"""Captured-source diagnostic of automatic LSMR without a Schur/CMG attempt.

This is an experimental build transform, not a production routing policy. The
existing automatic LSMR tolerance, memory admission, fallback and certification
remain active. Public fast two-intercept OLS alone enters the diagnostic.
"""


def instrument(source):
    path = source / 'crates/fereg-core/src/ols/prepared_fit.rs'
    text = path.read_text()
    anchor = '''    if allow_single_slope && config.absorb.solver == FeSolverKind::Auto'''
    assert text.count(anchor) == 1, 'Inspect the OLS owner before applying the route diagnostic.'
    insertion = '''    // Captured-source diagnostic: keep the automatic LSMR numerical
    // contract while removing the preceding Schur/CMG attempt.
    if config.allow_final_fe_recovery
        && config.absorb.solver == FeSolverKind::Auto
        && config.execution.mode == PreparedOlsExecutionMode::Fast
        && config.absorb.certified_fallback
        && prepared.fit_supports_cmg_auto()
    {
        let design = prepared.fit_borrowed_cmg_design()?;
        if design.effects().len() == 2 {
            return Ok(FitFeAbsorber {
                single_slope: None, one_fe: None, session: None, lsmr: None,
                auto_direct_solver: Some(FeSolverKind::Lsmr),
                auto_discarded_setup_seconds: 0.0,
                auto_discarded_workspace_peak: crate::memory::SolverWorkspacePeak::ZERO,
                auto_setup_warning: None, schur_config: None, schur_probe: None,
            });
        }
    }
'''
    path.write_text(text.replace(anchor, insertion + anchor))
