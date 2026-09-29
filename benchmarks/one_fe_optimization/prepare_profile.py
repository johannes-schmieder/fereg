"""Preparation phase probes for an isolated, source-hashed native build."""


def instrument(root):
    path = root / "legacy/fereg-core/src/ols/prepared.rs"
    text = path.read_text()

    def change(start, end, edits):
        nonlocal text
        first, last = text.index(start), text.index(end, text.index(start))
        body = text[first:last]
        for old, new in edits:
            assert body.count(old) == 1, old
            body = body.replace(old, new)
        text = text[:first] + body + text[last:]

    stamp = "std::time::Instant::now()"
    if "let early_source =" in text:
        extracted = "pub(crate) fn prepare_ols_numeric_sample_owned(" in text
        marker = "    Ok((model, sample, early_factor))" if extracted else "    finish_prepare_with_early_factor(request, cluster_sources, model, sample, true, early_factor)"
        change("pub(crate) fn prepare_ols_numeric_sample_owned(" if extracted else "pub(crate) fn prepare_ols_data_owned_with_cluster_sources(",
               "\npub(crate) fn prepare_ols_data_with_cluster_sources(", [
            ("    let model =", f"    let profile_start = {stamp};\n    let model ="),
            ("    let early_source =", f"    let profile_validated = {stamp};\n    let early_source ="),
            ("    let sample = if full {", f"    let profile_classified = {stamp};\n    let sample = if full {{"),
            (marker,
             "    if std::env::var_os(\"FEREG_PHASE_TIMING\").is_some() {\n"
             "        eprintln!(\"FEREG_PHASE|prepare_input|validate={:.9}|classify_and_factor={:.9}|sample_owner={:.9}\",\n"
             "            profile_validated.duration_since(profile_start).as_secs_f64(),\n"
             "            profile_classified.duration_since(profile_validated).as_secs_f64(),\n"
             "            profile_classified.elapsed().as_secs_f64());\n    }\n"
             + marker),
        ])
    else:
        change("pub(crate) fn prepare_ols_data_owned_with_cluster_sources(",
               "\npub(crate) fn prepare_ols_data_with_cluster_sources(", [
            ("    let model =", f"    let profile_start = {stamp};\n    let model ="),
            ("    let sample = if is_full_retained_unweighted_sample(&loaded) {",
             f"    let profile_validated = {stamp};\n"
             "    let full = is_full_retained_unweighted_sample(&loaded);\n"
             f"    let profile_classified = {stamp};\n    let sample = if full {{"),
            ("    finish_prepare_ols_data(request, cluster_sources, model, sample)",
             "    if std::env::var_os(\"FEREG_PHASE_TIMING\").is_some() {\n"
             "        eprintln!(\"FEREG_PHASE|prepare_input|validate={:.9}|complete_check={:.9}|sample_owner={:.9}\",\n"
             "            profile_validated.duration_since(profile_start).as_secs_f64(),\n"
             "            profile_classified.duration_since(profile_validated).as_secs_f64(),\n"
             "            profile_classified.elapsed().as_secs_f64());\n    }\n"
             "    finish_prepare_ols_data(request, cluster_sources, model, sample)"),
        ])
    if "fn finish_prepare_ols_data_with_membership(" in text:
        start = "fn finish_prepare_with_early_factor(" if "fn finish_prepare_with_early_factor(" in text else "fn finish_prepare_ols_data_with_membership("
        change(start, "\n// A one-effect", [
            ("    let pruning_categorical_sources =", f"    let profile_start = {stamp};\n    let pruning_categorical_sources ="),
            ("    let prebuilt_index = if reuse_membership", f"    let profile_factorized = {stamp};\n    let prebuilt_index = if reuse_membership"),
            ("    let (sample, categorical_sources, fixed_effects, num_singletons, cascade_events) =",
             f"    let profile_indexed = {stamp};\n    let (sample, categorical_sources, fixed_effects, num_singletons, cascade_events) ="),
            ("    resolve_categorical_level_codes", f"    let profile_selected = {stamp};\n    resolve_categorical_level_codes"),
            ("    Ok(PreparedOlsData {", "    if std::env::var_os(\"FEREG_PHASE_TIMING\").is_some() {\n"
             "        eprintln!(\"FEREG_PHASE|prepare_fe|factor={:.9}|early_index={:.9}|prune_compact={:.9}|topology={:.9}\",\n"
             "            profile_factorized.duration_since(profile_start).as_secs_f64(),\n"
             "            profile_indexed.duration_since(profile_factorized).as_secs_f64(),\n"
             "            profile_selected.duration_since(profile_indexed).as_secs_f64(),\n"
             "            profile_selected.elapsed().as_secs_f64());\n    }\n    Ok(PreparedOlsData {"),
        ])
    else:
        change("fn finish_prepare_ols_data(", "\n/// Materialize", [
        ("    let pruning_categorical_sources =", f"    let profile_start = {stamp};\n    let pruning_categorical_sources ="),
        ("    let singleton_result =", f"    let profile_factorized = {stamp};\n    let singleton_result ="),
        ("    let num_singletons =", f"    let profile_pruned = {stamp};\n    let num_singletons ="),
        ("    drop(singleton_result.active);", f"    drop(singleton_result.active);\n    let profile_compacted = {stamp};"),
        ("    Ok(PreparedOlsData {", "    if std::env::var_os(\"FEREG_PHASE_TIMING\").is_some() {\n"
         "        eprintln!(\"FEREG_PHASE|prepare_fe|factor={:.9}|prune={:.9}|compact={:.9}|topology={:.9}\",\n"
         "            profile_factorized.duration_since(profile_start).as_secs_f64(),\n"
         "            profile_pruned.duration_since(profile_factorized).as_secs_f64(),\n"
         "            profile_compacted.duration_since(profile_pruned).as_secs_f64(),\n"
         "            profile_compacted.elapsed().as_secs_f64());\n    }\n    Ok(PreparedOlsData {"),
        ])
    path.write_text(text)

    path = root / "legacy/fereg-core/src/ols/prepared_fit.rs"
    text = path.read_text()
    first = text.index("fn fill_raw_moment_cache_fused<")
    last = text.index("fn fill_raw_moment_cache_blocked<", first)
    body = text[first:last]
    for marker, insertion in [
        ("    let columns = raw.ncols();", "    let profile_moments_start = std::time::Instant::now();\n"),
        ("    partials.fill(0.0);", "    let profile_norms_done = std::time::Instant::now();\n"),
        ("    Ok(())\n}", '''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|moment_passes|norms={:.9}|means={:.9}",
            profile_norms_done.duration_since(profile_moments_start).as_secs_f64(),
            profile_norms_done.elapsed().as_secs_f64());
    }
'''),
    ]:
        assert body.count(marker) == 1, marker
        body = body.replace(marker, insertion + marker)
    path.write_text(text[:first] + body + text[last:])

    text = path.read_text()
    change("fn fit_with_controls_internal_mode_with_frame<", "\nfn ", [
        ("    let mut trace = PhaseTrace::new();", "    let mut trace = PhaseTrace::new();\n    let profile_setup_start = std::time::Instant::now();"),
        ("    let mut cmg = build_fit_fe_absorber_with_one_fe_reuse", "    let profile_frame_done = std::time::Instant::now();\n    let mut cmg = build_fit_fe_absorber_with_one_fe_reuse"),
        ("    let projection_phase = LinearSessionPhaseAllocationGuard::new(", "    let profile_absorber_done = std::time::Instant::now();\n    let projection_phase = LinearSessionPhaseAllocationGuard::new("),
        ("    let rhs_seconds = trace.lap();", '''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|rhs_setup|frame={:.9}|absorber={:.9}|materialize={:.9}",
            profile_frame_done.duration_since(profile_setup_start).as_secs_f64(),
            profile_absorber_done.duration_since(profile_frame_done).as_secs_f64(),
            profile_absorber_done.elapsed().as_secs_f64());
    }
    let rhs_seconds = trace.lap();'''),
    ])
    path.write_text(text)

    path = root / "legacy/fereg-core/src/ols/prepared_rhs.rs"
    text = path.read_text()
    change("pub(crate) fn build_prepared_ols_rhs_with_loaded_rows_and_reuse(",
           "\nfn build_prepared_ols_rhs_with_reservation", [
        ("        validate_prepared_rhs_inputs(prepared, response)?;", "        let profile_rhs_start = std::time::Instant::now();\n        validate_prepared_rhs_inputs(prepared, response)?;\n        let profile_rhs_validated = std::time::Instant::now();"),
        ("            if columns.matches_rhs(response, &rows) {", '''            if columns.matches_rhs(response, &rows) {
                if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
                    eprintln!("FEREG_PHASE|rhs_reuse|validation={:.9}|match={:.9}",
                        profile_rhs_validated.duration_since(profile_rhs_start).as_secs_f64(),
                        profile_rhs_validated.elapsed().as_secs_f64());
                }'''),
    ])
    change("fn validate_prepared_rhs_inputs(", "\nfn validate_row_map", [
        ("    let sample = &prepared.sample;", "    let profile_start = std::time::Instant::now();\n    let sample = &prepared.sample;"),
        ("    if response.iter().any", "    let profile_shape = std::time::Instant::now();\n    if response.iter().any"),
        ("    validate_row_map(prepared)?;", "    let profile_finite = std::time::Instant::now();\n    validate_row_map(prepared)?;"),
        ("    validate_weights(prepared)?;", "    let profile_rows = std::time::Instant::now();\n    validate_weights(prepared)?;"),
        ("    validate_categorical_sources(prepared)?;", "    let profile_weights = std::time::Instant::now();\n    validate_categorical_sources(prepared)?;"),
        ("    validate_coefficient_axis(prepared)?;", "    let profile_codes = std::time::Instant::now();\n    validate_coefficient_axis(prepared)?;"),
        ("    Ok(())\n}", '''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|rhs_validation|shape={:.9}|finite={:.9}|rows={:.9}|weights={:.9}|codes={:.9}|recipes={:.9}",
            profile_shape.duration_since(profile_start).as_secs_f64(),
            profile_finite.duration_since(profile_shape).as_secs_f64(),
            profile_rows.duration_since(profile_finite).as_secs_f64(),
            profile_weights.duration_since(profile_rows).as_secs_f64(),
            profile_codes.duration_since(profile_weights).as_secs_f64(),
            profile_codes.elapsed().as_secs_f64());
    }
    Ok(())
}'''),
    ])
    path.write_text(text)

    path = root / "crates/fereg-plugin/src/numeric_pipeline.rs"
    text = path.read_text()
    first = text.index("fn read_tiles(")
    last = text.index("\n#[cfg(test)]", first)
    body = text[first:last]
    for old, new in [
        ("            let mut index = 0;", "            let mut index = 0;\n            let mut profile_consume_seconds = 0.0;"),
        ("if slots.closed { return; }", '''if slots.closed {
                            if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
                                eprintln!("FEREG_PHASE|input_consumer|compute={:.9}", profile_consume_seconds);
                            }
                            return;
                        }'''),
        ("                consume(tile.start, tile.rows, &mut tile.values, tile.missing_boundary);",
         "                let profile_consume_start = std::time::Instant::now();\n"
         "                consume(tile.start, tile.rows, &mut tile.values, tile.missing_boundary);\n"
         "                profile_consume_seconds += profile_consume_start.elapsed().as_secs_f64();"),
        ("        for (sequence, start) in", "        let mut profile_wait_seconds = 0.0;\n        let mut profile_spi_seconds = 0.0;\n        for (sequence, start) in"),
        ("            let mut buffer = {", "            let profile_wait_start = std::time::Instant::now();\n            let mut buffer = {"),
        ("            spi.check_interrupted()?;", "            profile_wait_seconds += profile_wait_start.elapsed().as_secs_f64();\n            let profile_spi_start = std::time::Instant::now();\n            spi.check_interrupted()?;"),
        ("            if !boundary.is_finite()", "            profile_spi_seconds += profile_spi_start.elapsed().as_secs_f64();\n            if !boundary.is_finite()"),
        ("        Ok(())\n    })??;", '''        if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
            eprintln!("FEREG_PHASE|input_producer|wait={:.9}|spi={:.9}", profile_wait_seconds, profile_spi_seconds);
        }
        Ok(())
    })??;'''),
    ]:
        assert body.count(old) == 1, old
        body = body.replace(old, new)
    path.write_text(text[:first] + body + text[last:])

    # Distinguish the critical worker from work hidden by the admitted overlap.
    path = root / "crates/fereg-plugin/src/estimate.rs"
    text = path.read_text()
    if "fn prepare_loaded_ols_with_moments" in text:
        before = '''                || prepare(request, loaded),
                || loaded_rhs.map(|rows| LoadedOlsRows::from(rows).with_unit_moments(nrows, columns, bytes)),'''
        assert text.count(before) == 1
        text = text.replace(before, '''                || {
                    let started = Instant::now();
                    let result = prepare(request, loaded);
                    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
                        eprintln!("FEREG_PHASE|prepare_job|seconds={:.9}", started.elapsed().as_secs_f64());
                    }
                    result
                },
                || {
                    let started = Instant::now();
                    let result = loaded_rhs.map(|rows| LoadedOlsRows::from(rows).with_unit_moments(nrows, columns, bytes));
                    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
                        eprintln!("FEREG_PHASE|moment_job|seconds={:.9}", started.elapsed().as_secs_f64());
                    }
                    result
                },''')
        path.write_text(text)
        return
    before = '''                        || prepare_ols_data_owned(&action.request, loaded),
                        || loaded_rhs.map(|rows| LoadedOlsRows::from(rows).with_unit_moments(nrows, columns, bytes)),'''
    if "with_unit_moments" in text:
        assert text.count(before) == 1
        text = text.replace(before, '''                        || {
                            let started = Instant::now();
                            let result = prepare_ols_data_owned(&action.request, loaded);
                            if trace_enabled { eprintln!("FEREG_PHASE|prepare_job|seconds={:.9}", started.elapsed().as_secs_f64()); }
                            result
                        },
                        || {
                            let started = Instant::now();
                            let result = loaded_rhs.map(|rows| LoadedOlsRows::from(rows).with_unit_moments(nrows, columns, bytes));
                            if trace_enabled { eprintln!("FEREG_PHASE|moment_job|seconds={:.9}", started.elapsed().as_secs_f64()); }
                            result
                        },''')
        path.write_text(text)
