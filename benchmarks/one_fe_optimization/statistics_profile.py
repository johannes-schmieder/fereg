"""Instrument independent weighted statistics in an isolated source snapshot."""
from pathlib import Path


def instrument(root):
    path = Path(root) / 'crates/fereg-core/src/ols/prepared_unadjusted.rs'
    text = path.read_text()

    def replace(before, after):
        nonlocal text
        assert text.count(before) == 1, before
        text = text.replace(before, after)

    def report(phase, timer):
        return ('if std::env::var_os("FEREG_PHASE_TIMING").is_some() { '
                f'eprintln!("FEREG_PHASE|statistics_{phase}|seconds={{:.9}}", '
                f'{timer}.elapsed().as_secs_f64()); }}')

    overlapped = 'struct IndependentOutcome' in text
    call = ('build_reference_weight_report_with_outcome(prepared, &fit, tolerance, false, &mut outcome_tss)?;'
            if overlapped else 'build_reference_weight_report_view(prepared, &fit, tolerance, false)?;')
    replace("    validate_fit_and_absorbed_dof_view(prepared, &fit, absorbed_dof)?;\n"
            "    let (weights, weight_view, weight_sum_relative_error) =\n        " + call,
            "    validate_fit_and_absorbed_dof_view(prepared, &fit, absorbed_dof)?;\n    "
            + report('fit_dof_validation', 'phase_started')
            + "\n    let profile_weight_report = Instant::now();\n"
              "    let (weights, weight_view, weight_sum_relative_error) =\n        " + call
            + "\n    " + report('weight_report', 'profile_weight_report'))
    if overlapped:
        replace("    let nobs_f64 = exact_integer_as_f64(nobs as u128)?;\n"
                "    let (kind, effective, raw_weight_sum",
                "    let profile_weight_domain = Instant::now();\n"
                "    let nobs_f64 = exact_integer_as_f64(nobs as u128)?;\n"
                "    let (kind, effective, raw_weight_sum")
    else:
        replace("    let bread_scale = fit.bread().weight_scale();\n    let nobs_f64",
                "    let profile_weight_domain = Instant::now();\n"
                "    let bread_scale = fit.bread().weight_scale();\n    let nobs_f64")
    raw_call = ('let (raw_sum, normalized_sum) = positive_weight_sums(values, divisor)?;'
                if overlapped else 'let raw_sum = positive_sum_scaled(values, divisor)?;')
    replace("                " + raw_call + "\n                match kind",
            report('weight_domain_scale', 'profile_weight_domain')
            + "\n                let profile_raw_sum = Instant::now();\n                " + raw_call
            + "\n                " + report('weight_raw_sum', 'profile_raw_sum')
            + "\n                match kind")
    reference_call = ('let reference_sum = reference_weight_sum(view, facts.physical);'
                      if overlapped else 'let computed_sum = reference_weight_sum(view, nobs)?;')
    replace("    " + reference_call,
            "    let profile_reference_sum = Instant::now();\n    " + reference_call
            + "\n    " + report('weight_reference_sum', 'profile_reference_sum'))
    if overlapped:
        replace("    let facts = scan_reference_weight_facts(prepared, true)?;",
                "    let profile_source_scan = Instant::now();\n"
                "    let facts = scan_reference_weight_facts(prepared, true)?;")
        replace("    Ok(IndependentOutcome { source, source_weights, has_intercept, facts, reference_sum, tss })",
                "    " + report('source_overlap', 'profile_source_scan')
                + "\n    Ok(IndependentOutcome { source, source_weights, has_intercept, facts, reference_sum, tss })")
        replace("        (tss, match rss { Some(rss) => rss, None => weighted_sum_of_squares(fit.residuals(), weight_view, 0.0)? })",
                "        let profile_rss = Instant::now();\n"
                "        let rss = match rss { Some(rss) => rss, None => weighted_sum_of_squares(fit.residuals(), weight_view, 0.0)? };\n        "
                + report('rss', 'profile_rss') + "\n        (tss, rss)")
    replace('''    let rss = || weighted_sum_of_squares(residuals, weight_view, 0.0);''',
        '''    let rss = || {
        let profile_rss = Instant::now();
        let result = weighted_sum_of_squares(residuals, weight_view, 0.0);
        ''' + report('rss', 'profile_rss') + '''
        result
    };''')
    replace('''    let mean = weighted_mean(outcome, weights)?;
    weighted_sum_of_squares(outcome, weights, if has_intercept { mean } else { 0.0 })''',
        '''    let profile_mean = Instant::now();
    let mean = weighted_mean(outcome, weights)?;
    ''' + report('outcome_mean', 'profile_mean') + '''
    let profile_outcome_sum = Instant::now();
    let result = weighted_sum_of_squares(outcome, weights, if has_intercept { mean } else { 0.0 });
    ''' + report('outcome_squares', 'profile_outcome_sum') + '''
    result''')
    path.write_text(text)
