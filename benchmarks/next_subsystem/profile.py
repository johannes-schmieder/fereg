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


    path = root / "crates/fereg-core/src/fe/map.rs"
    text = path.read_text()
    for function, label, values in (
        ("    pub(crate) fn absorb(", "absorb", "values"),
        ("    fn certify(", "certificate", "residuals"),
    ):
        start = text.index(function, text.index("impl ReusableMapAbsorber"))
        body = text.index(" {\n", start) + 3
        text = text[:body] + f'''        struct PpmlMapTimer(std::time::Instant, usize, usize);
        impl Drop for PpmlMapTimer {{
            fn drop(&mut self) {{
                if std::env::var_os("FEREG_PHASE_TIMING").is_some() {{
                    eprintln!("FEREG_PHASE|ppml_map|phase={label}|rows={{}}|rhs={{}}|seconds={{:.9}}", self.1, self.2, self.0.elapsed().as_secs_f64());
                }}
            }}
        }}
        let _map_timer = PpmlMapTimer(std::time::Instant::now(), {values}.nrows(), {values}.ncols());
''' + text[body:]
    path.write_text(text)
    instrument_reusable_certificate_parts(root)


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


def instrument_serial_sandwich(root):
    path = root / "crates/fereg-core/src/sandwich.rs"
    text = path.read_text()
    start = text.index("pub(crate) fn reduce_serial_influence_covariance<")
    end = text.index("/// Multiway clustered", start)
    body = text[start:end]
    points = [
        ("    for row in 0..nrows {", "    let mut development_parts = [0.0_f64; 3];\n    let mut development_lap = std::time::Instant::now();\n"),
        ("        for (left, influence_value) in influence.iter_mut().enumerate() {", "        development_parts[0] += development_lap.elapsed().as_secs_f64();\n        development_lap = std::time::Instant::now();\n"),
        ("        for left in 0..dimension {", "        development_parts[1] += development_lap.elapsed().as_secs_f64();\n        development_lap = std::time::Instant::now();\n"),
    ]
    for marker, before in points:
        assert body.count(marker) == 1, marker
        body = body.replace(marker, before + marker)
    marker = "    }\n    finalize_symmetric_triangle"
    assert body.count(marker) == 1
    body = body.replace(marker, '''        development_parts[2] += development_lap.elapsed().as_secs_f64();
        development_lap = std::time::Instant::now();
    }
    eprintln!("FEREG_PHASE|serial_sandwich|rows={nrows}|columns={dimension}|scores={:.9}|bread={:.9}|outer={:.9}", development_parts[0], development_parts[1], development_parts[2]);
    finalize_symmetric_triangle''')
    path.write_text(text[:start] + body + text[end:])


def instrument_owned_fe(root):
    for file, function, values in (
        ("projection.rs", "pub fn project_effect_in_place(", "values"),
        ("certificate.rs", "pub(crate) fn certify_residuals_with_reference_norms(", "residuals"),
    ):
        path = root / "crates/fereg-core/src/fe" / file
        text = path.read_text()
        if file == "certificate.rs" and "fn certify_with_bases(" in text:
            function = "fn certify_with_bases("
        start = text.index(function)
        body = text.index(" {\n", start) + 3
        label = file.removesuffix(".rs")
        text = text[:body] + f'''    struct OwnedFeTimer(std::time::Instant, usize, usize);
    impl Drop for OwnedFeTimer {{
        fn drop(&mut self) {{
            if std::env::var_os("FEREG_PHASE_TIMING").is_some() {{
                eprintln!("FEREG_PHASE|owned_fe|phase={label}|rows={{}}|rhs={{}}|seconds={{:.9}}",
                    self.1, self.2, self.0.elapsed().as_secs_f64());
            }}
        }}
    }}
    let _profile_timer = OwnedFeTimer(std::time::Instant::now(), {values}.nrows(), {values}.ncols());
''' + text[body:]
        path.write_text(text)

    path = root / "crates/fereg-core/src/fe/certificate.rs"
    text = path.read_text()
    def replace(before, after):
        nonlocal text
        expected = 2 if ("fn certify_parallel_groups(" in text and before == "    let mut absolute_infinity_norm = 0.0_f64;") else 1
        assert text.count(before) == expected, before
        text = text.replace(before, after, 1)
    replace("    let mut absolute_infinity_norm = 0.0_f64;",
            "    let mut development_parts = [0.0_f64; 4];\n    let mut absolute_infinity_norm = 0.0_f64;")
    if "fn certify_with_bases(" in text:
        replace("            let fresh_basis;",
                "            let mut development_lap = std::time::Instant::now();\n            let fresh_basis;")
        for index, marker in enumerate((
            ("            for rhs in 0..residuals.ncols() {\n                let relative = relative_projection("
             if "fn relative_projection(" in text else
             "            for rhs in 0..residuals.ncols() {\n                let denominator = rhs_scales[rhs];"),
            "            absolute_infinity_norm = absolute_infinity_norm.max(raw_moment_maximum(",
        )):
            replace(marker, f"            development_parts[{index}] += development_lap.elapsed().as_secs_f64();\n            development_lap = std::time::Instant::now();\n" + marker)
        marker = "    let relative_two_norm = if relative_count == 0 {"
        replace("        }\n    }\n\n" + marker,
            "            development_parts[2] += development_lap.elapsed().as_secs_f64();\n        }\n    }\n"
            '    eprintln!("FEREG_PHASE|owned_certificate|rows={}|rhs={}|geometry={:.9}|project={:.9}|raw_moments={:.9}", residuals.nrows(), residuals.ncols(), development_parts[0], development_parts[1], development_parts[2]);\n\n' + marker)
        replace("    pub(super) fn cache(&mut self) -> Result<()> {",
            "    pub(super) fn cache(&mut self) -> Result<()> {\n        let geometry_started = std::time::Instant::now();")
        replace("        self.bases = Some(bases);",
            '        eprintln!("FEREG_PHASE|owned_fe|phase=certificate_geometry|rows={}|rhs=0|seconds={:.9}", self.design.nobs(), geometry_started.elapsed().as_secs_f64());\n        self.bases = Some(bases);')
    else:
        replace("            let mut normalized_columns = fallible_empty(",
                "            let mut development_lap = std::time::Instant::now();\n            let mut normalized_columns = fallible_empty(")
        for index, marker in enumerate((
            "            let orthonormal_basis = pivoted_reorthogonalized_qr(normalized_columns)?;",
            "            for rhs in 0..residuals.ncols() {\n                let denominator = rhs_scales[rhs];",
            ("            absolute_infinity_norm = absolute_infinity_norm.max(raw_moment_maximum("
             if "raw_moment_maximum(" in text else
             "            for channel in 0..channels {\n                for rhs in 0..residuals.ncols() {"),
            "    let relative_two_norm = if relative_count == 0 {",
        )):
            if index == 3:
                # The final group accumulator belongs inside both loops.
                replace("        }\n    }\n\n" + marker,
                    "            development_parts[3] += development_lap.elapsed().as_secs_f64();\n        }\n    }\n"
                    '    eprintln!("FEREG_PHASE|owned_certificate|rows={}|rhs={}|normalize={:.9}|qr={:.9}|project={:.9}|raw_moments={:.9}", residuals.nrows(), residuals.ncols(), development_parts[0], development_parts[1], development_parts[2], development_parts[3]);\n\n' + marker)
            else:
                replace(marker, f"            development_parts[{index}] += development_lap.elapsed().as_secs_f64();\n            development_lap = std::time::Instant::now();\n" + marker)
    if "fn certify_parallel_groups(" in text:
        start = text.index("fn certify_parallel_groups(")
        end = text.index("/// One absorption call", start)
        region = text[start:end]
        entry = "    let rhs_count = residuals.ncols();"
        assert region.count(entry) == 1
        region = region.replace(entry, "    let parallel_started = std::time::Instant::now();\n" + entry)
        entry = "    Ok(FeCertificate {"
        assert region.count(entry) == 1
        region = region.replace(entry, '    eprintln!("FEREG_PHASE|owned_fe|phase=parallel_groups|rows={}|rhs={}|threads={}|seconds={:.9}", design.nobs(), residuals.ncols(), rayon::current_num_threads(), parallel_started.elapsed().as_secs_f64());\n' + entry)
        text = text[:start] + region + text[end:]
    path.write_text(text)

    path = root / "crates/fereg-core/src/fe/projection.rs"
    text, tests = path.read_text().split("\n#[cfg(test)]", 1)
    if "fn group_coefficients(" in text:
        # Each successful projection is timed once. The parallel timer starts
        # before shared validation; workers never emit or sum wall-clock laps.
        timer_start = text.index("    struct OwnedFeTimer(")
        timer_end = text.index("    validate_projection(", timer_start)
        timer = text[timer_start:timer_end]
        timer = timer.replace("OwnedFeTimer(std::time::Instant::now(),", "OwnedFeTimer(profile_parallel_started,")
        marker = "        if let Some(parallel) = self.parallel.as_mut() {"
        assert text.count(marker) == 1
        text = text.replace(marker, marker + "\n            let profile_parallel_started = std::time::Instant::now();")
        marker = "                return project_parallel(effect, weights, values, parallel);"
        assert text.count(marker) == 1
        text = text.replace(marker, timer + marker)
        start = text.index("fn project_parallel(")
        end = len(text)
        region = text[start:end]
        marker = "    let group_width = channels * rhs;"
        assert region.count(marker) == 1
        region = region.replace(marker, "    let profile_group_started = std::time::Instant::now();\n" + marker)
        marker = "    values\n        .as_mut_slice()"
        assert region.count(marker) == 1
        region = region.replace(marker, "    let profile_groups = profile_group_started.elapsed().as_secs_f64();\n    let profile_subtract_started = std::time::Instant::now();\n" + marker)
        marker = "    Ok(())"
        assert region.count(marker) == 1
        region = region.replace(marker, '    eprintln!("FEREG_PHASE|owned_projection_parallel|rows={}|rhs={}|threads={}|groups={:.9}|subtract={:.9}", values.nrows(), values.ncols(), rayon::current_num_threads(), profile_groups, profile_subtract_started.elapsed().as_secs_f64());\n' + marker)
        text = text[:start] + region
        path.write_text(text + '\n#[cfg(test)]' + tests)
        return
    def replace_projection(before, after):
        nonlocal text
        assert text.count(before) == 1, before
        text = text.replace(before, after)
    replace_projection("    for level in 0..effect.n_levels() {", '''    let development_validation = _profile_timer.0.elapsed().as_secs_f64();
    let mut development_parts = [0.0_f64; 3];
    for level in 0..effect.n_levels() {
        let mut development_lap = std::time::Instant::now();''')
    replace_projection("        solve_linear_system(", '''        development_parts[0] += development_lap.elapsed().as_secs_f64();
        development_lap = std::time::Instant::now();
        solve_linear_system(''')
    subtraction = ("        // Batch independent RHSs while keeping each fitted value's channel FMA"
        if "// Batch independent RHSs" in text else
        "        for observation in effect.level_index().members(level) {\n            for rhs in 0..rhs_count {")
    replace_projection(subtraction, '''        development_parts[1] += development_lap.elapsed().as_secs_f64();
        development_lap = std::time::Instant::now();
''' + subtraction)
    replace_projection("    }\n    Ok(())\n}\n", '''        development_parts[2] += development_lap.elapsed().as_secs_f64();
    }
    eprintln!("FEREG_PHASE|owned_projection|rows={}|rhs={}|validation={:.9}|moments={:.9}|solve={:.9}|subtract={:.9}", values.nrows(), values.ncols(), development_validation, development_parts[0], development_parts[1], development_parts[2]);
    Ok(())
}
''')
    path.write_text(text + '\n#[cfg(test)]' + tests)


def instrument_reusable_certificate_parts(root):
    """Separate current-weight geometry, projection and raw moments diagnostically."""
    path = root / 'crates/fereg-core/src/fe/map.rs'
    text = path.read_text()
    start = text.index('    fn certify(', text.index('impl ReusableMapAbsorber'))
    end = text.index('\nfn reusable_map_report(', start)
    body = text[start:end]
    def replace(before, after):
        nonlocal body
        assert body.count(before) == 1, before
        body = body.replace(before, after)
    replace('        reusable_weighted_norms(\n',
            '        let mut development_parts = [0.0_f64; 5];\n        let development_norm_start = std::time::Instant::now();\n        reusable_weighted_norms(\n')
    replace('        for effect in self.design.effects() {',
            '        development_parts[0] = development_norm_start.elapsed().as_secs_f64();\n        for effect in self.design.effects() {')
    replace('                let members = effect.level_index().members(level);',
            '                let mut development_lap = std::time::Instant::now();\n                let members = effect.level_index().members(level);')
    cached = 'let basis_count = if *geometry_valid {' in body
    indent = '                    ' if cached else '                '
    replace('\n' + indent + 'let mut basis_count = 0_usize;',
            '\n' + indent + 'development_parts[1] += development_lap.elapsed().as_secs_f64();\n' + indent + 'development_lap = std::time::Instant::now();\n' + indent + 'let mut basis_count = 0_usize;')
    project = '                for column in 0..rhs {\n                    let denominator = self.rhs_scales[column];'
    if cached:
        marker = '                    if let Some(geometry) = self.certificate_geometry.as_mut() {'
        replace(marker, '                    development_parts[2] += development_lap.elapsed().as_secs_f64();\n' + marker)
        replace(project, '                development_lap = std::time::Instant::now();\n' + project)
    else:
        replace(project, '                development_parts[2] += development_lap.elapsed().as_secs_f64();\n                development_lap = std::time::Instant::now();\n' + project)
    raw = '                for channel in 0..channels {\n                    for column in 0..rhs {'
    replace(raw, '                development_parts[3] += development_lap.elapsed().as_secs_f64();\n                development_lap = std::time::Instant::now();\n' + raw)
    if cached:
        marker = '                geometry_offset += channels * member_count;'
        replace(marker, '                development_parts[4] += development_lap.elapsed().as_secs_f64();\n' + marker)
    else:
        replace('            }\n        }\n        let relative_two',
                '                development_parts[4] += development_lap.elapsed().as_secs_f64();\n            }\n        }\n        let relative_two')
    replace('        Ok(FeCertificate {',
            '        eprintln!("FEREG_PHASE|reusable_certificate|rows={}|rhs={}|norms={:.9}|normalize={:.9}|qr={:.9}|project={:.9}|raw_moments={:.9}", residuals.nrows(), rhs, development_parts[0], development_parts[1], development_parts[2], development_parts[3], development_parts[4]);\n        Ok(FeCertificate {')
    path.write_text(text[:start] + body + text[end:])
