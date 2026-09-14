import os
from pathlib import Path
p=Path(os.path.join(os.environ['REPOSITORY_ROOT'], 'crates/fereg-core/src/ols/prepared_fit.rs'));s=p.read_text()
def replace(a,b,n=1):
 global s
 assert s.count(a)==n,(a[:90],s.count(a),n);s=s.replace(a,b)
a='''    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    fit_prepared_ols_coefficients_view_map_for_inference_with_cache_limit(
        prepared,
        config,
        MAX_FAST_SCORE_CACHE_BYTES.min(32 * 1024 * 1024),
    )
}'''
b='''    config: PreparedOlsFitConfig,
) -> Result<(CertifiedPreparedOlsFit, Option<PreparedOlsScratch>)> {
    let mut scratch = None;
    let fit = fit_for_inference_with_scratch(
        prepared,
        config,
        MAX_FAST_SCORE_CACHE_BYTES.min(32 * 1024 * 1024),
        Some(&mut scratch),
    )?;
    Ok((fit, scratch))
}''';replace(a,b)
a='''    max_fast_score_cache_bytes: usize,
) -> Result<CertifiedPreparedOlsFit> {
    let team = ThreadTeam::new(config.execution.threads);'''
b='''    max_fast_score_cache_bytes: usize,
) -> Result<CertifiedPreparedOlsFit> {
    fit_for_inference_with_scratch(prepared, config, max_fast_score_cache_bytes, None)
}

fn fit_for_inference_with_scratch<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    max_fast_score_cache_bytes: usize,
    retained_scratch: Option<&mut Option<PreparedOlsScratch>>,
) -> Result<CertifiedPreparedOlsFit> {
    let team = ThreadTeam::new(config.execution.threads);''';replace(a,b)
# The ordinary coefficient-only low entry has no inference scratch consumer.
a='''            PreparedOlsExecutionMode::Low => fit_with_controls_low_internal(
                prepared,
                config,
                &mut reservation,
                FitPerturbations::default(),
            ),''';assert s.count(a)==2
first=s.index(a);s=s[:first]+s[first:].replace(a,a.replace('                FitPerturbations::default(),','                FitPerturbations::default(),\n                None,'),1)
replace(a,a.replace('                FitPerturbations::default(),','                FitPerturbations::default(),\n                retained_scratch,'))
a='''                phase_probe,
                phase_observation,
            ),''';replace(a,'''                phase_probe,
                phase_observation,
                None,
            ),''')
i=s.index('fn fit_with_controls_low_internal<');j=s.index('fn fit_with_controls_low_internal_with_frame<',i);part=s[i:j]
part=part.replace('    perturbations: FitPerturbations,\n','    perturbations: FitPerturbations,\n    retained_scratch: Option<&mut Option<PreparedOlsScratch>>,\n',1)
part=part.replace('        None,\n        None,\n','        None,\n        None,\n        retained_scratch,\n',1);s=s[:i]+part+s[j:]
i=s.index('fn fit_with_controls_low_internal_with_frame<');j=s.index('\nfn ',i+3);part=s[i:j]
part=part.replace('    mut phase_observation: Option<&mut LinearSessionPhaseAllocationObservation>,\n','    mut phase_observation: Option<&mut LinearSessionPhaseAllocationObservation>,\n    retained_scratch: Option<&mut Option<PreparedOlsScratch>>,\n',1)
a='    let mut scratch = PreparedOlsScratch::create(rows, generated_columns)?;';assert part.count(a)==1
part=part.replace(a,a+'''
    // Keep original within columns outside the numerical fit result. The copy
    // borrows this file's I/O bank while QR still needs the working columns.
    // Optional file creation failure leaves the established uncached route.
    let mut score_scratch = if retained_scratch.is_some() && generated_columns > 0 {
        PreparedOlsScratch::create_unbuffered(rows, generated_columns).ok()
    } else {
        None
    };
    let scratch_bytes = scratch.bytes().checked_mul(if score_scratch.is_some() { 2 } else { 1 })
        .ok_or_else(|| allocation("retained low-memory scratch capacity overflows"))?;
''')
a='            scratch.write_column(generated, &column_buffer)?;';assert part.count(a)==1
part=part.replace(a,a+'''
            if score_scratch.as_mut().is_some_and(|original| {
                original.write_column_with_borrowed_buffer(generated, &column_buffer, &mut scratch).is_err()
            }) {
                // The working QR column and its buffer survive an optional
                // copy failure. Dropping the partial copy removes its file.
                score_scratch = None;
            }
''')
part=part.replace('    let scratch_bytes = scratch.bytes();\n','',1)
a='    Ok(CertifiedPreparedOlsFit {';assert part.count(a)==1
part=part.replace(a,'''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|low_score_retention|retained={}|scratch_peak_bytes={scratch_bytes}", usize::from(score_scratch.is_some()));
    }
    if let (Some(destination), Some(mut original)) = (retained_scratch, score_scratch) {
        original.take_io_buffer_from(&mut scratch)?;
        *destination = Some(original);
    }
'''+a)
s=s[:i]+part+s[j:];p.write_text(s)
