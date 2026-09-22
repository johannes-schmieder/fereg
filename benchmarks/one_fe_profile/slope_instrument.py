"""Diagnostic-only timers inserted into an isolated source copy before build."""


def instrument(source):
    probes = [
        ("fe/certificate.rs", "pub(crate) fn certify_residuals_with_reference_norms(", "certificate", "residuals"),
        ("fe/projection.rs", "pub fn project_effect_in_place(", "projection", "values"),
        ("fe/auto.rs", "fn absorb_auto(", "auto", "values"),
        ("fe/map.rs", "pub fn absorb_map(", "map", "values"),
    ]
    for relative, marker, label, values in probes:
        path = source / "crates/fereg-core/src" / relative
        text = path.read_text()
        assert text.count(marker) == 1, relative
        position = text.index("{", text.index(marker)) + 1
        timer = '''
    struct SlopeTimer(std::time::Instant, usize, usize);
    impl Drop for SlopeTimer { fn drop(&mut self) {
        eprintln!("SLOPE_PROFILE|LABEL|n={}|rhs={}|seconds={:.9}", self.1, self.2, self.0.elapsed().as_secs_f64());
    } }
    let _slope_timer = SlopeTimer(std::time::Instant::now(), VALUES.nrows(), VALUES.ncols());
'''.replace("LABEL", label).replace("VALUES", values)
        path.write_text(text[:position] + timer + text[position:])
    path = source / "crates/fereg-core/src/ols/prepared_fit_view.rs"
    text = path.read_text()
    start = "        let design = self.fit_owned_fe_design()?;"
    end = "        let mut report = absorb_fixed_effects(&design, weights, &mut residualized, config)?;"
    assert text.count(start) == text.count(end) == 1
    text = text.replace(start, "        let owned_started = std::time::Instant::now();\n" + start)
    text = text.replace(end, '        eprintln!("SLOPE_PROFILE|owned_design|n={}|rhs={}|seconds={:.9}", residualized.nrows(), residualized.ncols(), owned_started.elapsed().as_secs_f64());\n' + end)
    path.write_text(text)
    path = source / "crates/fereg-core/src/fe/single_slope.rs"
    if path.exists():
        text = path.read_text()
        if "check_input: bool" in text:
            marker = "        let started = Instant::now();"
            first = text.index("    pub(crate) fn absorb(")
            position = text.index(marker, first) + len(marker)
            timer = '\n        struct CachedTimer(Instant, usize);\n        impl Drop for CachedTimer { fn drop(&mut self) {\n            eprintln!("SLOPE_PROFILE|cached_total|rhs={}|seconds={:.9}", self.1, self.0.elapsed().as_secs_f64());\n        } }\n        let _cached_timer = CachedTimer(Instant::now(), original.ncols());\n'
            text = text[:position] + timer + text[position:]
            if "let serial_norms" in text:
                markers = [
                    ('        // In row-ordered groups', '        let profile_projection = Instant::now();\n'),
                    ('        let certificate_started = Instant::now();', '        let profile_norms = Instant::now();\n'),
                    ('        let certificate = self.check_matrix(&result, &scales, tolerance, &mut checks, right_scales)?;', '        let profile_checks = Instant::now();\n'),
                ]
                for marker, insertion in markers:
                    assert text.count(marker) == 1, marker
                    text = text.replace(marker, insertion + marker)
                marker = '        let certification_seconds = certificate_started.elapsed().as_secs_f64();'
                assert text.count(marker) == 1
                text = text.replace(marker, '''        eprintln!("SLOPE_PROFILE|cached_fused_passes|rhs={}|prepare={:.9}|projection_reconstruction={:.9}|remaining_norms={:.9}|checks={:.9}",
                    rhs, profile_projection.duration_since(started).as_secs_f64(),
                    profile_norms.duration_since(profile_projection).as_secs_f64(),
                    profile_checks.duration_since(profile_norms).as_secs_f64(), profile_checks.elapsed().as_secs_f64());
''' + marker)
            else:
                markers = [
                    ('        coefficients\n            .par_chunks_mut(channels * rhs)', '        let profile_projection = Instant::now();\n'),
                    ('        result\n            .as_mut_slice()', '        let profile_reconstruction = Instant::now();\n'),
                    ('        let certificate_started = Instant::now();', '        let profile_norms = Instant::now();\n'),
                    ('        let certificate = self.check_matrix(&result, &scales, tolerance, &mut checks)?;', '        let profile_checks = Instant::now();\n'),
                ]
                for marker, insertion in markers:
                    assert text.count(marker) == 1, marker
                    text = text.replace(marker, insertion + marker)
                marker = '        let certification_seconds = certificate_started.elapsed().as_secs_f64();'
                assert text.count(marker) == 1
                text = text.replace(marker, '''        eprintln!("SLOPE_PROFILE|cached_passes|rhs={}|prepare={:.9}|projection={:.9}|reconstruction={:.9}|norms={:.9}|checks={:.9}",
                    rhs, profile_projection.duration_since(started).as_secs_f64(),
                    profile_reconstruction.duration_since(profile_projection).as_secs_f64(),
                    profile_norms.duration_since(profile_reconstruction).as_secs_f64(),
                    profile_checks.duration_since(profile_norms).as_secs_f64(), profile_checks.elapsed().as_secs_f64());
    ''' + marker)
        else:
            probes = [
                ("        let mut coefficients = filled(", "        let profile_reference = Instant::now();\n"),
                ("        result\n            .as_mut_slice()", "        let profile_projection = Instant::now();\n"),
                ("        let certificate_started = Instant::now();", "        let profile_reconstruction = Instant::now();\n"),
                ("        checks\n            .par_iter_mut()", "        let profile_norms = Instant::now();\n"),
                ("        let mut certificate = FeCertificate", "        let profile_checks = Instant::now();\n"),
            ]
            for marker, insertion in probes:
                assert text.count(marker) == 1, marker
                text = text.replace(marker, insertion + marker)
            marker = "        let certification_seconds = certificate_started.elapsed().as_secs_f64();"
            assert text.count(marker) == 1
            text = text.replace(marker, '''        eprintln!("SLOPE_PROFILE|cached|rhs={}|reference={:.9}|projection={:.9}|reconstruction={:.9}|norms={:.9}|checks={:.9}|reduce={:.9}",
                rhs, profile_reference.duration_since(started).as_secs_f64(),
                profile_projection.duration_since(profile_reference).as_secs_f64(),
                profile_reconstruction.duration_since(profile_projection).as_secs_f64(),
                profile_norms.duration_since(profile_reconstruction).as_secs_f64(),
                profile_checks.duration_since(profile_norms).as_secs_f64(), profile_checks.elapsed().as_secs_f64());
    ''' + marker)
        path.write_text(text)

    path = source / "crates/fereg-core/src/fe/topology_slope_map.rs"
    text = path.read_text()
    marker = "pub(crate) fn recover_finalized_single_slope_direct_certified("
    if marker in text:
        first = text.index(marker)
        last = text.index("/// Recover normalized categorical effects", first)
        section = text[first:last]
        cached_geometry = 'geometry.as_deref()' in section
        group_marker = ('    if let Some(geometry) = geometry.as_mut() {' if cached_geometry
                        else "    group_coefficients\n        .par_chunks_mut(")
        finish_name = ('finish_channel_major_recovery_with_geometry' if cached_geometry
                       else 'finish_channel_major_recovery')
        markers = [
            ("    validate_recovery_inputs(", "let recovery_started = Instant::now();"),
            (group_marker, "let recovery_groups = Instant::now();"),
            ("    for channel in 0..channels {", "let recovery_scatter = Instant::now();"),
            ("    " + finish_name + "(", "let recovery_finish = Instant::now();"),
        ]
        for needle, code in markers:
            assert section.count(needle) == 1, needle
            section = section.replace(needle, "    " + code + "\n" + needle)
        section = section.replace("    " + finish_name + "(", "    let recovered = " + finish_name + "(")
        needle = ("        CategoricalRecoveryMethod::Direct,\n        materialize_absorbed_sum,\n"
                  + ("        geometry.as_deref(),\n" if cached_geometry else "") + "    )")
        assert section.count(needle) == 1, needle
        section = section.replace(needle, needle + ";\n    eprintln!(\"SLOPE_PROFILE|direct_recovery|setup={:.9}|groups={:.9}|transpose={:.9}|finish={:.9}\", recovery_groups.duration_since(recovery_started).as_secs_f64(), recovery_scatter.duration_since(recovery_groups).as_secs_f64(), recovery_finish.duration_since(recovery_scatter).as_secs_f64(), recovery_finish.elapsed().as_secs_f64());\n    recovered")
        text = text[:first] + section + text[last:]
        path.write_text(text)

    path = source / "crates/fereg-core/src/ols/prepared_fit.rs"
    text = path.read_text()
    first = text.index("fn build_fast_score_cache<")
    last = text.index("fn build_strided_fast_score_cache<", first)
    section = text[first:last]
    markers = [
        ("    let nrows = prepared.fit_sample().n_active();", "let score_cache_started = std::time::Instant::now();"),
        ("    values\n        .par_chunks_mut(value_chunk_elements)", "let score_cache_rows = std::time::Instant::now();"),
        ("    let mut denominator = 0.0_f64;", "let score_cache_reduce = std::time::Instant::now();"),
    ]
    for needle, code in markers:
        assert section.count(needle) == 1, needle
        section = section.replace(needle, "    " + code + "\n" + needle)
    needle = "    let within = DenseMatrix::from_row_major(nrows, generated_columns.len(), values)?;"
    assert section.count(needle) == 1
    section = section.replace(needle, '''    eprintln!("SLOPE_PROFILE|packed_score_cache|n={}|k={}|allocation={:.9}|rows={:.9}|merge={:.9}",
        nrows, generated_columns.len(), score_cache_rows.duration_since(score_cache_started).as_secs_f64(),
        score_cache_reduce.duration_since(score_cache_rows).as_secs_f64(), score_cache_reduce.elapsed().as_secs_f64());
''' + needle)
    text = text[:first] + section + text[last:]
    path.write_text(text)
