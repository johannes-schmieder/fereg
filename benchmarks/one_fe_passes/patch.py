"""Prototype numeric RHS initialization and concurrent source-moment scans."""
import argparse
from pathlib import Path


def replace(text, old, new):
    assert text.count(old) == 1, old[:100]
    return text.replace(old, new)


def patch(root, direct, overlap):
    rhs = root / "legacy/fereg-core/src/ols/prepared_rhs.rs"
    text = rhs.read_text()
    if direct:
        first = text.index('        // The caller already reserved the complete matrix fallibly.')
        last = text.index('\n        Ok(())', first)
        text = text[:first] + '''        // Initialize the reserved matrix directly from source columns. Rayon
        // owns initialization; no uninitialized slice enters the safe core.
        (0..elements)
            .into_par_iter()
            .map(|index| {
                let row = index / width;
                let column = index % width;
                if column == 0 {
                    response[row]
                } else {
                    self.columns[column - 1].map_or(1.0, |source| source[row])
                }
            })
            .collect_into_vec(data);''' + text[last:]
    if overlap:
        text = replace(text, '    pub(crate) fn fill_reserved_rhs(', '''    pub(crate) fn rhs_value(&self, response: &[f64], row: usize, column: usize) -> f64 {
        if column == 0 {
            response[row]
        } else {
            self.columns[column - 1].map_or(1.0, |source| source[row])
        }
    }

    pub(crate) fn fill_reserved_rhs(''')
    rhs.write_text(text)
    if not overlap:
        return
    path = root / "legacy/fereg-core/src/ols/prepared_fit.rs"
    text = path.read_text()
    text = replace(text, 'trait FitReservation {', 'trait FitReservation: Send {')
    first = text.index('fn fit_with_controls_internal_mode_with_frame<')
    before, body = text[:first], text[first:]
    body = replace(body, '''    let raw = prepared.fit_build_rhs_with_response(frame.response())?;
    let rhs_seconds = trace.lap();
    let generated_columns = prepared.fit_model().columns.len();''', '''    let generated_columns = prepared.fit_model().columns.len();
    let numeric = if !retain_fast_score_cache
        && weights.values.is_none()
        && config.absorb.solver == FeSolverKind::Auto
        && prepared.fit_has_absorbed_intercept()
        && prepared.fit_supports_cmg_auto()
        && generated_columns < FUSED_MOMENT_MAX_COLUMNS
        && prepared.fit_borrowed_cmg_design()?.effects().len() == 1
    {
        super::prepared_rhs::NumericOlsColumns::bind(prepared.fit_model(), prepared.fit_sample())?
    } else {
        None
    };
    let (raw, mut source_moments) = if let Some(columns) = numeric {
        let rows = prepared.fit_sample().n_active();
        let response = frame.response();
        if response.len() != rows {
            return Err(invalid_argument("numeric source moments response is not aligned"));
        }
        let (rhs, moments) = rayon::join(
            || prepared.fit_build_rhs_with_response(response),
            || build_numeric_raw_moment_cache(&columns, response, generated_columns + 1, weights, reservation),
        );
        // Keep the established RHS validation error ahead of moment errors.
        (rhs?, Some(moments?))
    } else {
        (prepared.fit_build_rhs_with_response(frame.response())?, None)
    };
    let rhs_seconds = trace.lap();''')
    body = replace(body, '        let cache = build_raw_moment_cache(&raw, weights, reservation)?;', '''        let cache = if let Some(cache) = source_moments.take() {
            cache
        } else {
            build_raw_moment_cache(&raw, weights, reservation)?
        };''')
    text = before + body
    first = text.index('#[allow(clippy::too_many_arguments)]\nfn fill_raw_moment_cache_fused')
    last = text.index('\n/// Keep the wide columnwise', first)
    body = text[first:last]
    body = replace(body, 'fn fill_raw_moment_cache_fused<R: FitReservation>(\n    raw: &DenseMatrix,', '''fn fill_raw_moment_cache_by_value<R: FitReservation>(
    rows: usize,
    columns: usize,
    get_value: impl Fn(usize, usize) -> f64 + Sync,''')
    body = replace(body, '    let columns = raw.ncols();\n', '')
    body = body.replace('raw.nrows()', 'rows').replace('raw\n        .nrows()', 'rows')
    body = body.replace('                let values = &raw.as_slice()[row * columns..(row + 1) * columns];\n', '')
    body = body.replace('values[column]', 'get_value(row, column)')
    assert 'raw.' not in body and 'raw\n' not in body
    wrapper = '''#[allow(clippy::too_many_arguments)]
fn fill_raw_moment_cache_fused<R: FitReservation>(
    raw: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    weight_sum: f64,
    scales: &mut [f64],
    means: &mut [f64],
    norms: &mut [f64],
    reservation: &mut R,
) -> Result<()> {
    fill_raw_moment_cache_by_value(raw.nrows(), raw.ncols(), |row, column| raw[(row, column)],
        weights, weight_sum, scales, means, norms, reservation)
}

fn build_numeric_raw_moment_cache<R: FitReservation>(
    source: &super::prepared_rhs::NumericOlsColumns<'_>,
    response: &[f64],
    columns: usize,
    weights: NormalizedWeights<'_>,
    reservation: &mut R,
) -> Result<RawMomentCache> {
    let mut scales = reserved_filled(reservation, columns, 0.0, "unable to reserve source-moment scales")?;
    let mut means = reserved_filled(reservation, columns, 0.0, "unable to reserve source-moment means")?;
    let mut norms = reserved_filled(reservation, columns, 0.0, "unable to reserve source-moment norms")?;
    let weight_sum = response.len() as f64;
    fill_raw_moment_cache_by_value(response.len(), columns,
        |row, column| source.rhs_value(response, row, column), weights, weight_sum,
        &mut scales, &mut means, &mut norms, reservation)?;
    Ok(RawMomentCache { means, scales, norms, weight_sum })
}

'''
    text = text[:first] + wrapper + body + text[last:]
    path.write_text(text)


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('source', type=Path)
    p.add_argument('--direct', action='store_true')
    p.add_argument('--overlap', action='store_true')
    a = p.parse_args()
    patch(a.source, a.direct, a.overlap)
