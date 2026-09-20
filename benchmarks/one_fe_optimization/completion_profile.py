"""Instrument independent completion jobs in an isolated source snapshot."""
from pathlib import Path


def instrument(root):
    path = Path(root) / 'crates/fereg-core/src/ols/prepared_fit.rs'
    text = path.read_text()

    def replace(before, after):
        nonlocal text
        assert text.count(before) == 1, before
        text = text.replace(before, after)

    def report(phase, timer):
        return ('if std::env::var_os("FEREG_PHASE_TIMING").is_some() { '
                f'eprintln!("FEREG_PHASE|completion_{phase}|seconds={{:.9}}", '
                f'{timer}.elapsed().as_secs_f64()); }}')

    replace('''                let projected = project();
''', '''                let profile_project = std::time::Instant::now();
                let projected = project();
                ''' + report('projection', 'profile_project') + '\n')
    replace('''                        let score = || certify_scores(prepared, projected.residualized().as_slice(), weights,
                            &reference_norms, score_norms, finite_score, &anticipated_statuses[..generated_columns],
                            within_outcome_norm, config.score_tolerance);''',
            '''                        let score = || {
                            let profile_score = std::time::Instant::now();
                            let result = certify_scores(prepared, projected.residualized().as_slice(), weights,
                                &reference_norms, score_norms, finite_score, &anticipated_statuses[..generated_columns],
                                within_outcome_norm, config.score_tolerance);
                            ''' + report('score', 'profile_score') + '''
                            result
                        };''')
    replace('''                            let (score, sum) = rayon::join(score, || super::prepared_unadjusted::independent_unit_residual_sum(projected.residualized().as_slice()));''',
            '''                            let (score, sum) = rayon::join(score, || {
                                let profile_rss = std::time::Instant::now();
                                let result = super::prepared_unadjusted::independent_unit_residual_sum(projected.residualized().as_slice());
                                ''' + report('rss', 'profile_rss') + '''
                                result
                            });''')
    replace('''            independent_raw_residuals = Some(build_independent_raw_residuals(
                prepared, frame.response(), &qr, reservation,
            )?);''',
            '''            let profile_raw = std::time::Instant::now();
            independent_raw_residuals = Some(build_independent_raw_residuals(
                prepared, frame.response(), &qr, reservation,
            )?);
            ''' + report('raw', 'profile_raw'))
    path.write_text(text)
