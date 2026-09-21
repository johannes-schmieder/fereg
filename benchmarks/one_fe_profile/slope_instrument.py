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
