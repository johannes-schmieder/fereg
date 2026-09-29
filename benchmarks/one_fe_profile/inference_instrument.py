"""Isolated-build phase probes for clustered OLS inference."""


def instrument(source):
    path = source / 'legacy/fereg-core/src/ols/prepared_clustered.rs'
    text = path.read_text()
    start = text.index('fn compose_clustered_view<')
    end = text.index('\nfn ', start + 1)
    body = text[start:end]
    points = [
        ('dof', '    let ols = prepared.clustered_ols();'),
        ('weights', '    let (weights, weight_view, weight_error) ='),
        ('statistics', '    let base_dof ='),
        ('scores', '    let mut scores = build_prepared_ols_score_design('),
        ('covariance', '    let mut raw = build_raw_covariance('),
        ('covariance_certificate', '    let independent_error = certify_with_group_influences('),
        ('psd', '    let (values, psd) = apply_psd_policy('),
        ('wald', '    let wald = build_wald('),
        ('final_checks', '    let expected_correction = cluster_correction('),
        ('end', '    Ok(PreparedOlsClusteredComponents {'),
    ]
    for index, (_, marker) in enumerate(points):
        assert body.count(marker) == 1, marker
        body = body.replace(marker, f'    let inference_stamp_{index} = std::time::Instant::now();\n' + marker)
    fields = '|'.join(name + '={:.9}' for name, _ in points[:-1])
    values = ', '.join(f'inference_stamp_{i + 1}.duration_since(inference_stamp_{i}).as_secs_f64()'
                       for i in range(len(points) - 1))
    body = body.replace(points[-1][1], f'    eprintln!("FEREG_PHASE|clustered_inference|{fields}", {values});\n' + points[-1][1])
    path.write_text(text[:start] + body + text[end:])
