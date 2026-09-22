"""Diagnostic-only LSMR transport laps in an isolated source copy."""


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
