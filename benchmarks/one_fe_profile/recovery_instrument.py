"""Isolated probes retaining the reason and cost of every recovery fallback."""
import re


def instrument(source):
    path = source / 'legacy/fereg-core/src/ols/prepared_categorical_data.rs'
    text = path.read_text()
    start = text.index('    fn recover_absorbed_effects_fast_with_weights_inner(')
    end = text.index('\n    /// Compute absorbed rank', start)
    body = text[start:end]
    pattern = r'match (?:crate::fe::)?(recover_finalized_\w+)\('
    matches = list(re.finditer(pattern, body))
    assert len(matches) == 6, [m.group(1) for m in matches]
    # Work backwards so the recorded boundaries continue to name the original
    # disjoint match arms. Rejections stay rejections; only diagnostics change.
    for index in range(len(matches) - 1, -1, -1):
        match = matches[index]
        boundary = matches[index + 1].start() if index + 1 < len(matches) else len(body)
        section = body[match.start():boundary]
        marker = 'Err(_) => {}'
        assert section.count(marker) == 1, match.group(1)
        route = match.group(1).replace('recover_finalized_', '').replace('_certified', '')
        section = section.replace(marker, 'Err(error) => { eprintln!("FEREG_PHASE|recovery_rejection|route=' + route
            + '|seconds={:.9}|error={:?}", recovery_probe_start.elapsed().as_secs_f64(), error); }')
        section = ('let recovery_probe_start = std::time::Instant::now();\n'
            + '        eprintln!("FEREG_PHASE|recovery_attempt|route=' + route + '");\n        ' + section)
        body = body[:match.start()] + section + body[boundary:]
    marker = '        recover_finalized_categorical_effects_map_certified('
    assert body.count(marker) == 1
    body = body.replace(marker, '        eprintln!("FEREG_PHASE|recovery_attempt|route=map_fallback");\n' + marker)
    path.write_text(text[:start] + body + text[end:])
