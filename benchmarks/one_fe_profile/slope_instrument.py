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
