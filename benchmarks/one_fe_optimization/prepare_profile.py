"""Preparation phase probes for an isolated, source-hashed native build."""


def instrument(root):
    path = root / "crates/fereg-core/src/ols/prepared.rs"
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
        change("fn finish_prepare_ols_data_with_membership(", "\n// A one-effect", [
            ("    let pruning_categorical_sources =", f"    let profile_start = {stamp};\n    let pruning_categorical_sources ="),
            ("    let prebuilt_index =", f"    let profile_factorized = {stamp};\n    let prebuilt_index ="),
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
