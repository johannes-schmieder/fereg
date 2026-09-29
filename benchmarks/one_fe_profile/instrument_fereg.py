"""Add diagnostic laps to an isolated source copy, never to the installed plugin."""
import argparse
from pathlib import Path
from instrument import replace


def instrument_ado(root):
    path = root / "stata/_fereg_estimate_ols.ado"
    text = path.read_text()
    end = text.index('\nprogram define _fereg_eols_validate')
    body = text[:end]
    body = replace(body, '    version 18.0', '    version 18.0\n    timer clear 81\n    timer on 81')
    body = replace(body, '    plugin call __fereg_estimate_plugin',
                   '    timer off 81\n    timer clear 82\n    timer on 82\n    plugin call __fereg_estimate_plugin')
    body = replace(body, '\n    if `"`inject\'"\' == "breakpostplugin"',
                   '\n    timer off 82\n    timer clear 83\n    timer on 83\n    if `"`inject\'"\' == "breakpostplugin"')
    body = replace(body, "    if `coefficient_count' > 0 {\n        // Whole-matrix",
                   "    timer off 83\n    timer clear 84\n    timer on 84\n    if `coefficient_count' > 0 {\n        // Whole-matrix")
    body = replace(body, '    if `"`inject\'"\' == "postereturn"',
                   '    timer off 84\n    timer clear 85\n    timer on 85\n    if `"`inject\'"\' == "postereturn"')
    index = body.rindex('\nend')
    body = body[:index] + '''
    timer off 85
    forvalues profile_timer = 81/85 {
        quietly timer list `profile_timer'
        noisily display "FEREG_ADO_PHASE|`profile_timer'|seconds=" %12.6f r(t`profile_timer')
    }
''' + body[index:]
    path.write_text(body + text[end:])


def scoped(path, start, end, transform):
    text = path.read_text()
    a, b = text.index(start), text.index(end, text.index(start))
    path.write_text(text[:a] + transform(text[a:b]) + text[b:])


def instrument(root):
    path = root / "crates/fereg-plugin/src/estimate.rs"
    def input_laps(body):
        body = replace(body, '    let n = spi.nobs()?;',
                       '    let profile_start = Instant::now();\n    let n = spi.nobs()?;')
        body = replace(body, '    let all_sources_numeric =',
                       '    let profile_allocated = Instant::now();\n    let all_sources_numeric =')
        body = replace(body, '    spi.check_interrupted()?;\n    let mut numeric_sources',
                       '    let profile_loaded = Instant::now();\n    spi.check_interrupted()?;\n    let mut numeric_sources')
        body = replace(body, '    Ok(LoadedOlsInput {', '''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|input_detail|allocation={:.9}|spi_read={:.9}|finalize={:.9}",
            profile_allocated.duration_since(profile_start).as_secs_f64(),
            profile_loaded.duration_since(profile_allocated).as_secs_f64(),
            profile_loaded.elapsed().as_secs_f64());
    }
    Ok(LoadedOlsInput {''')
        return body
    scoped(path, 'pub(crate) fn load_ols_input_with_memory(', '\nfn ', input_laps)

    path = root / "legacy/fereg-core/src/ols/prepared.rs"
    def preparation(body):
        body = replace(body, '    let model = request.validate_and_compile_with_cluster_sources',
                       '    let profile_start = std::time::Instant::now();\n    let model = request.validate_and_compile_with_cluster_sources')
        body = replace(body, '    let sample = if',
                       '    let profile_validated = std::time::Instant::now();\n    let sample = if')
        body = replace(body, '    finish_prepare_ols_data(request, cluster_sources, model, sample)', '''    let profile_sampled = std::time::Instant::now();
    let result = finish_prepare_ols_data(request, cluster_sources, model, sample);
    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|prepare_detail|validate={:.9}|sample={:.9}|factor_singletons_finalize={:.9}",
            profile_validated.duration_since(profile_start).as_secs_f64(),
            profile_sampled.duration_since(profile_validated).as_secs_f64(),
            profile_sampled.elapsed().as_secs_f64());
    }
    result''')
        return body
    scoped(path, 'pub(crate) fn prepare_ols_data_owned_with_cluster_sources(',
           '\npub(crate) fn prepare_ols_data_with_cluster_sources(', preparation)

    path = root / "legacy/fereg-core/src/ols/prepared_fit.rs"
    def finite(body):
        body = replace(body, '    let nrows = within.nrows();',
                       '    let profile_start = Instant::now();\n    let nrows = within.nrows();')
        body = replace(body, '    let (standardized_rhs, standardized_gram) =\n        build_standardized_moments',
                       '    let profile_standardized = Instant::now();\n    let (standardized_rhs, standardized_gram) =\n        build_standardized_moments')
        body = replace(body, '    if let Some(fit) = try_fit_scaled_pivoted_cholesky(',
                       '    let profile_moments = Instant::now();\n    if let Some(fit) = try_fit_scaled_pivoted_cholesky(')
        body = replace(body, '        return Ok(fit);', '''        if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
            eprintln!("FEREG_PHASE|finite_detail|setup_standardize={:.9}|moments={:.9}|cholesky_residual_certificates={:.9}|qr_fallback=0",
                profile_standardized.duration_since(profile_start).as_secs_f64(),
                profile_moments.duration_since(profile_standardized).as_secs_f64(),
                profile_moments.elapsed().as_secs_f64());
        }
        return Ok(fit);''')
        body = replace(body, '    fill_trailing_norms(within, &candidate_columns, 0, &mut trailing_norms)?;', '''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|finite_detail|qr_fallback=1");
    }
    fill_trailing_norms(within, &candidate_columns, 0, &mut trailing_norms)?;''')
        return body
    scoped(path, 'fn fit_scaled_pivoted_qr<', '\nfn fit_scaled_pivoted_qr_low<', finite)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("--ado-only", action="store_true")
    args = parser.parse_args()
    (instrument_ado if args.ado_only else instrument)(args.source)
