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

    replace('''    validate_fit_and_absorbed_dof_view(prepared, &fit, absorbed_dof)?;
    let (weights, weight_view, weight_sum_relative_error) =
        build_reference_weight_report_view(prepared, &fit, tolerance, false)?;''',
        '''    validate_fit_and_absorbed_dof_view(prepared, &fit, absorbed_dof)?;
    ''' + report('fit_dof_validation', 'phase_started') + '''
    let profile_weight_report = Instant::now();
    let (weights, weight_view, weight_sum_relative_error) =
        build_reference_weight_report_view(prepared, &fit, tolerance, false)?;
    ''' + report('weight_report', 'profile_weight_report'))
    replace('''    let bread_scale = fit.bread().weight_scale();
    let nobs_f64''',
        '''    let profile_weight_domain = Instant::now();
    let bread_scale = fit.bread().weight_scale();
    let nobs_f64''')
    replace('''                let raw_sum = positive_sum_scaled(values, divisor)?;
                match kind''',
        report('weight_domain_scale', 'profile_weight_domain') + '''
                let profile_raw_sum = Instant::now();
                let raw_sum = positive_sum_scaled(values, divisor)?;
                ''' + report('weight_raw_sum', 'profile_raw_sum') + '''
                match kind''')
    replace('''    let computed_sum = reference_weight_sum(view, nobs)?;''',
        '''    let profile_reference_sum = Instant::now();
    let computed_sum = reference_weight_sum(view, nobs)?;
    ''' + report('weight_reference_sum', 'profile_reference_sum'))
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
