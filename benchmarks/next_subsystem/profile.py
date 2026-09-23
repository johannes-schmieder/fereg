"""Diagnostic-only native phase laps in an isolated source copy."""


def instrument(root):
    path = root / "crates/fereg-core/src/fe/borrowed_map.rs"
    text = path.read_text()
    start = text.index("fn run_borrowed_lsmr_with_prepared<'a>(")
    end = text.index("\nfn resolve_borrowed_lsmr_preconditioner", start)
    body = text[start:end]
    changes = [
        ("    let retained = prepared.as_ref().map_or(0, |p| p.retained_bytes);",
         "    let profile_solve_done = Instant::now();\n    let retained = prepared.as_ref().map_or(0, |p| p.retained_bytes);"),
        ("    drop(std::mem::take(&mut solved.demeaned));",
         "    let profile_copy_done = Instant::now();\n    drop(std::mem::take(&mut solved.demeaned));"),
        ("    let certification_seconds = certification_started.elapsed().as_secs_f64();",
         '''    let certification_seconds = certification_started.elapsed().as_secs_f64();
    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|lsmr_transport|rows={}|rhs={}|input={owned_setup_seconds:.9}|upstream={:.9}|upstream_setup={:.9}|output={:.9}|norms_workspace={:.9}|certificate={certification_seconds:.9}",
            values.nrows(), values.ncols(), solved.time_total, solved.time_setup,
            profile_copy_done.duration_since(profile_solve_done).as_secs_f64(),
            certification_started.duration_since(profile_copy_done).as_secs_f64());
    }'''),
    ]
    for before, after in changes:
        assert body.count(before) == 1, before
        body = body.replace(before, after)
    path.write_text(text[:start] + body + text[end:])


def instrument_ppml(root):
    path = root / "crates/fereg-core/src/ols/linear_session.rs"
    text = path.read_text()
    text += '''
struct DevelopmentSessionLap(&'static str, usize, std::time::Instant);
impl Drop for DevelopmentSessionLap {
    fn drop(&mut self) {
        if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
            eprintln!("FEREG_PHASE|linear_session|name={}|rows={}|seconds={:.9}",
                self.0, self.1, self.2.elapsed().as_secs_f64());
        }
    }
}
'''
    for name in ("strict_candidate", "prepare_retained_solve", "solve_full_retained_rank",
                 "build_full_retained_design", "weighted_dense_column_norms",
                 "weighted_dense_column_means", "refresh_retained_norms",
                 "reproject_retained_within_in_place", "reproject_retained_with_response_in_place"):
        needle = f"fn {name}("
        assert text.count(needle) == 1, name
        start = text.index("{", text.index(needle)) + 1
        text = text[:start] + f'\n        let _development_lap = DevelopmentSessionLap("{name}", self.response.len(), Instant::now());' + text[start:]
    start = text.index("fn solve_full_retained_rank(")
    end = text.index("fn full_retained_generated_columns(", start)
    body = text[start:end]
    body = body.replace("        let rows = self.response.len();", "        let mut profile_rank_lap = Instant::now();\n        let rows = self.response.len();", 1)
    for before, label in [("        let residual_absorbed =", "rank_before_residual_projection"),
                          ("        let draft = RetainedFiniteFitDraft", "rank_residual_projection"),
                          ("        Ok(RetainedRankSolveDraft", "rank_assembly")]:
        assert body.count(before) == 1
        body = body.replace(before, f'''        if std::env::var_os("FEREG_PHASE_TIMING").is_some() {{
            eprintln!("FEREG_PHASE|linear_session|name={label}|rows={{rows}}|seconds={{:.9}}", profile_rank_lap.elapsed().as_secs_f64());
        }}
        profile_rank_lap = Instant::now();
''' + before)
    text = text[:start] + body + text[end:]
    path.write_text(text)

    path = root / "crates/fereg-core/src/ols/prepared_fit.rs"
    text = path.read_text()
    start = text.index("pub(super) fn assemble_retained_weighted_linear_fit")
    end = text.index("/// Canonical finite-plus-FE projection", start)
    body = text[start:end]
    body = body.replace("    let weights = frame.weights();", "    let mut development_lap = std::time::Instant::now();\n    let weights = frame.weights();", 1)
    for before, label in [("    let bread = finalize_reported_bread_and_intercept(", "raw_residual"),
                          ("    let initial_absorbed_outcome_sum_of_squares", "bread"),
                          ("    let coefficients = build_coefficient_slots", "score_certificate")]:
        assert body.count(before) == 1
        body = body.replace(before, f'''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {{
        eprintln!("FEREG_PHASE|retained_assembly|name={label}|rows={{}}|seconds={{:.9}}", prepared.fit_sample().n_active(), development_lap.elapsed().as_secs_f64());
    }}
    development_lap = std::time::Instant::now();
''' + before)
    text = text[:start] + body + text[end:]
    start = text.index("fn finalize_reported_bread_and_intercept<")
    end = text.index("fn reported_inverse_relative_infinity_norm(", start)
    body = text[start:end]
    before = "        let mut means = reserved_filled("
    assert body.count(before) == 1
    body = body.replace(before, "        let mut development_lap = std::time::Instant::now();\n" + before, 1)
    for before, label in [("        // Fast mode already owns", "raw_means"),
                          ("        let mut reported_intercept = raw_residual_mean;", "residual_mean"),
                          ("        let mut finite_mean_fit =", "outcome_mean")]:
        assert body.count(before) == 1
        body = body.replace(before, f'''        if std::env::var_os("FEREG_PHASE_TIMING").is_some() {{
            eprintln!("FEREG_PHASE|retained_bread|name={label}|rows={{}}|seconds={{:.9}}", prepared.fit_sample().n_active(), development_lap.elapsed().as_secs_f64());
        }}
        development_lap = std::time::Instant::now();
''' + before)
    path.write_text(text[:start] + body + text[end:])


def instrument_low_qr(root):
    path = root / "crates/fereg-core/src/ols/prepared_fit.rs"
    text = path.read_text()

    def phases(text, start_marker, end_marker, entry, family, rows, columns, points):
        start = text.index(start_marker)
        end = text.index(end_marker, start)
        body = text[start:end]
        assert body.count(entry) == 1, entry
        body = body.replace(entry, "    let mut development_low_lap = std::time::Instant::now();\n" + entry)
        for before, label in points:
            assert body.count(before) == 1, before
            body = body.replace(before, f'''    development_low_profile_lap("{family}", "{label}", {rows}, {columns}, &mut development_low_lap);
''' + before)
        return text[:start] + body + text[end:]

    text = phases(text, "fn fit_with_controls_low_internal_with_frame<",
        "fn initial_within_sum_of_squares_low<", "    frame.validate_for(prepared)?;",
        "low_fit", "rows", "generated_columns", [
            ("    let raw_outcome =", "setup"),
            ("    for start in (0..generated_columns).step_by(block_width)", "outcome_and_scratch"),
            ("    let normalized_initial_within_sum_of_squares =", "regressors_and_scratch"),
            ("    let mut raw_residuals =", "finite_qr"),
            ("    let bread = finalize_reported_bread_and_intercept(", "raw_residual"),
            ("    let initial_absorbed_outcome_sum_of_squares =", "reported_bread"),
            ("    let score_certificate = certify_scores(", "final_residual_projection"),
            ("    let coefficients = build_coefficient_slots", "score_certificate"),
            ("    Ok(CertifiedPreparedOlsFit {", "assembly"),
        ])
    low_start = text.index("fn fit_scaled_pivoted_qr_low<")
    low_end = text.index("fn reconstruct_within_residuals<", low_start)
    guarded = "    if let Some(fit) = try_fit_scaled_pivoted_cholesky(" in text[low_start:low_end]
    moments_end = ("    if let Some(fit) = try_fit_scaled_pivoted_cholesky(" if guarded else
        "    fill_scratch_trailing_norms(\n        scratch,\n        &candidate_columns,\n        0,")
    text = phases(text, "fn fit_scaled_pivoted_qr_low<", "fn reconstruct_within_residuals<",
        "    let nrows =", "low_qr", "nrows", "generated_columns", [
            ("    for (row, value) in within_outcome.iter_mut().enumerate()", "setup"),
            ("    let mut standardized_rhs =", "standardization"),
            (moments_end, "moments"),
            ("    let mut rank = 0_usize;", "initial_norms"),
            ("    fill_scratch_trailing_norms(\n        scratch,\n        &candidate_columns,\n        rank,", "reflectors"),
            ("    let compact_columns =", "rank_certificate"),
            ("    let condition_proxy =", "compact_solve"),
            ("    let bread = build_finite_bread_draft(", "finite_score"),
            ("    Ok(QrFit {", "finite_bread"),
        ])
    if guarded:
        start = text.index("fn fit_scaled_pivoted_qr_low<")
        end = text.index("fn reconstruct_within_residuals<", start)
        body = text[start:end]
        assert body.count("        return Ok(fit);") == 1
        body = body.replace("        return Ok(fit);", '''        development_low_profile_lap("low_qr", "guarded_cholesky", nrows, generated_columns, &mut development_low_lap);
        return Ok(fit);''')
        text = text[:start] + body + text[end:]
    text += '''
fn development_low_profile_lap(family: &str, phase: &str, rows: usize, columns: usize, lap: &mut std::time::Instant) {
    let now = std::time::Instant::now();
    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|{family}|phase={phase}|rows={rows}|columns={columns}|seconds={:.9}", now.duration_since(*lap).as_secs_f64());
    }
    *lap = std::time::Instant::now();
}
'''
    path.write_text(text)
