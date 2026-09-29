"""Diagnostic-only Schur pilot histories; no admission or arithmetic changes."""


def instrument(source):
    path = source / 'legacy/fereg-core/src/fe/borrowed_map.rs'
    text = path.read_text()
    for name, end_marker in [
        ('schur_probe_reusing', '\nfn schur_probe_narrow_continuation('),
        ('schur_probe_narrow_continuation', '\npub(crate) fn absorb_ols_schur_certified('),
    ]:
        start = text.index('fn ' + name + '(')
        end = text.index(end_marker, start)
        body = text[start:end]
        marker = 'for _ in 0..OLS_SCHUR_PROBE_ITERATIONS {'
        assert body.count(marker) == 1
        body = body.replace(marker, 'for probe_iteration in 0..OLS_SCHUR_PROBE_ITERATIONS {')
        marker = '        square = next;'
        assert body.count(marker) == 1
        body = body.replace(marker, marker + '''
        eprintln!("FEREG_PHASE|schur_pilot_step|route=ROUTE|n={}|rhs={}|step={}|initial={:?}|square={:?}",
            values.nrows(), values.ncols(), probe_iteration + 1, initial, square);
'''.replace('ROUTE', name))
        text = text[:start] + body + text[end:]
    path.write_text(text)
