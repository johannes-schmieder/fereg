"""Source-bound phase probes for isolated breadth builds, never installed code."""


def instrument(root):
    def phases(relative, start, end, label, points):
        path = root / relative
        text = path.read_text()
        first = text.index(start)
        last = text.index(end, first)
        body = text[first:last]
        for index, (_, marker) in enumerate(points):
            assert body.count(marker) == 1, (relative, marker)
            body = body.replace(marker, f"let breadth_stamp_{index} = std::time::Instant::now();\n    " + marker)
        fields = "|".join(f"{name}={{:.9}}" for name, _ in points[:-1])
        values = ", ".join(
            f"breadth_stamp_{i+1}.duration_since(breadth_stamp_{i}).as_secs_f64()"
            for i in range(len(points)-1)
        )
        report = (f'if std::env::var_os("FEREG_PHASE_TIMING").is_some() {{ '
                  f'eprintln!("FEREG_PHASE|{label}|{fields}", {values}); }}\n    ')
        body = body.replace(points[-1][1], report + points[-1][1])
        path.write_text(text[:first] + body + text[last:])

    phases("crates/fereg-core/src/ols/prepared_categorical_data.rs",
           "fn finish_prepare_categorical_ols_data(", "\n#[derive(Debug)]", "categorical_prepare", [
        ("pruning", "let bound_effects ="),
        ("topology", "let fe_topology =\n        finalize_categorical_topology"),
        ("sample_sources", "let PreparedOlsData {"),
        ("rebase", "let structural_intercept ="),
        ("validation", "prepared.validate()?;"),
        ("end", "Ok(prepared)"),
    ])
    phases("crates/fereg-core/src/ols/prepared_categorical_data.rs",
           "fn recover_absorbed_effects_fast_with_weights_inner(",
           "\n    /// Compute absorbed rank", "recovery_setup", [
        ("validation", "let validation_bytes ="),
        ("planning", "let effects ="),
        ("end", "let fits ="),
    ])
    phases("crates/fereg-core/src/fe/topology_slope_map.rs",
           "pub(crate) fn recover_finalized_categorical_effects_sd_lsmr_certified(",
           "\n/// Recover intercept-only effects with", "recovery_direct", [
        ("validation_design", "validate_recovery_inputs("),
        ("solve", "let recovered ="),
        ("normalization", "let validated_weights ="),
        ("end", "finish_channel_major_recovery("),
    ])
    phases("crates/fereg-plugin/src/estimate.rs", "fn precommit_and_store(",
           "\nfn build_runtime_shadow(", "publication", [
        ("planning", "let expected_mode ="),
        ("validation", "validate_shadow_against_outputs("),
        ("end", "store_shadow("),
    ])
    path = root / "crates/fereg-plugin/src/estimate.rs"
    text = path.read_text()
    old = "store_shadow(action, shadow, derived, &runtime, &memory_report, spi)\n}"
    assert text.count(old) == 1
    text = text.replace(old, '''let breadth_store_start = Instant::now();
    let result = store_shadow(action, shadow, derived, &runtime, &memory_report, spi);
    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|publication_store|seconds={:.9}", breadth_store_start.elapsed().as_secs_f64());
    }
    result
}''')
    path.write_text(text)
