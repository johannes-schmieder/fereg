import os
from pathlib import Path
r=Path(__file__).resolve().parent;repo=Path(os.environ['REPOSITORY_ROOT']);p=repo/'crates/fereg-core/src/ols/prepared_fit_view.rs';s=p.read_text()
a=s.index('            match &model.columns[column].kind {');b=s.index('            for (target, value) in destination.iter_mut().zip(values.iter()) {',a);match=s[a:b];helper=match.replace('            match &model.columns[column].kind {','    match kind {',1)
import textwrap
helper=textwrap.dedent(match).replace('match &model.columns[column].kind {','match kind {',1)
s=s[:a]+'            fill_generated_column_tile(prepared, &model.columns[column].kind, start, values)?;\n'+s[b:]
s=s.replace('        let end = start + destination.len();\n','',1)
insert=s.index('/// Accumulate generated finite predictors')
fill='''fn fill_generated_column_tile<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    kind: &crate::ols::PreparedOlsColumnKind,
    start: usize,
    values: &mut [f64],
) -> Result<()> {
    use crate::ols::PreparedOlsColumnKind;
    let end = start + values.len();
    let invalid = || FeregError::new(ErrorCode::InvalidArgument,
        "generated predictor columns are not aligned and finite");
'''+textwrap.indent(helper,'    ')+'''    Ok(())
}

/// Reuse generated recipes while retaining the prepared layout's original
/// absent-indicator arithmetic and generated-column accumulation order.
fn subtract_raw_columns<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    coefficients: &[Option<f64>],
    residuals: &mut [f64],
    skip_absent_indicators: bool,
) -> Result<()> {
    use crate::ols::PreparedOlsColumnKind;
    let model = prepared.fit_model();
    let rows = prepared.fit_sample().n_active();
    if residuals.len() != rows || coefficients.len() != model.columns.len()
        || coefficients.iter().flatten().any(|c| !c.is_finite()) {
        return Err(FeregError::new(ErrorCode::InvalidArgument,
            "generated raw residual columns are not aligned and finite"));
    }
    if rows < 4096 || model.columns.len() < 8 || !model.columns.iter().any(|c|
        matches!(c.kind, PreparedOlsColumnKind::Categorical { .. }
            | PreparedOlsColumnKind::CategoricalInteraction { .. })) {
        for (column, coefficient) in coefficients.iter().enumerate() {
            if let Some(coefficient) = coefficient {
                prepared.fit_subtract_raw_column(column, *coefficient, residuals)?;
            }
        }
        return Ok(());
    }
    const TILE_ROWS: usize = 128;
    let fill = |(tile, destination): (usize, &mut [f64])| -> Result<()> {
        let mut tile_values = [0.0_f64; TILE_ROWS];
        let values = &mut tile_values[..destination.len()];
        for (column, coefficient) in coefficients.iter().enumerate() {
            let Some(coefficient) = coefficient else { continue; };
            let kind = &model.columns[column].kind;
            fill_generated_column_tile(prepared, kind, tile * TILE_ROWS, values)?;
            let skip = skip_absent_indicators && matches!(kind,
                PreparedOlsColumnKind::Categorical { .. }
                | PreparedOlsColumnKind::CategoricalInteraction { .. });
            if skip {
                for (target, value) in destination.iter_mut().zip(values.iter()) {
                    if *value != 0.0 { *target = (-*coefficient).mul_add(*value, *target); }
                }
            } else {
                for (target, value) in destination.iter_mut().zip(values.iter()) {
                    *target = (-*coefficient).mul_add(*value, *target);
                }
            }
        }
        if destination.iter().any(|v| !v.is_finite()) {
            return Err(FeregError::new(ErrorCode::InvalidData,
                "generated raw residual is not finite"));
        }
        Ok(())
    };
    if rayon::current_thread_index().is_some() && rayon::current_num_threads() > 1 {
        residuals.par_chunks_mut(TILE_ROWS).enumerate().try_for_each(fill)?;
    } else {
        residuals.chunks_mut(TILE_ROWS).enumerate().try_for_each(fill)?;
    }
    Ok(())
}

'''
s=s[:insert]+fill+s[insert:]
needle='    /// Subtract one fitted raw-scale column from an observation vector.'
s=s.replace(needle,'''    /// Build the independent raw residual in the original generated-column
    /// order. Complex preparation retains FMA on absent indicators.
    fn fit_subtract_raw_columns(&self, coefficients: &[Option<f64>], residuals: &mut [f64]) -> Result<()> {
        subtract_raw_columns(self, coefficients, residuals, false)
    }

'''+needle,1)
needle='impl PreparedOlsFitView for PreparedOlsData {\n'
s=s.replace(needle,needle+'''    fn fit_subtract_raw_columns(&self, coefficients: &[Option<f64>], residuals: &mut [f64]) -> Result<()> {
        // Ordinary preparation historically skips absent indicator FMAs.
        subtract_raw_columns(self, coefficients, residuals, true)
    }

''',1);p.write_text(s)
p=repo/'crates/fereg-core/src/ols/prepared_fit.rs';s=p.read_text();a=s.index('fn build_independent_raw_residuals<');b=s.index('// Shared upper bound',a);part=s[a:b]
old='''        for (column, coefficient) in qr.generated_coefficients.iter().copied().enumerate() {
            if let Some(coefficient) = coefficient {
                prepared.fit_subtract_raw_column(column, coefficient, &mut residuals)?;
            }
        }''';assert part.count(old)==1
part=part.replace(old,'        prepared.fit_subtract_raw_columns(&qr.generated_coefficients, &mut residuals)?;');p.write_text(s[:a]+part+s[b:])
p=repo/'crates/fereg-core/src/ols/prepared_rhs.rs';s=p.read_text();a=s.index('    fn generated_predictor_tiles_preserve_raw_level_oracle_and_fma_order()');b=s.index('    #[test]',a);part=s[a:b].replace('for prepared in views {','for (layout, prepared) in views.into_iter().enumerate() {',1)
needle='                for workers in [1, 4, 16] {'
new='''                let residual_coefficients: Vec<_> = (0..width).map(|column|
                    if column % 7 == 0 { None } else { Some(if column % 3 == 0 { -0.0 } else { (column as f64 - 17.0) / 31.0 }) }
                ).collect();
                let mut raw_wanted = vec![-0.0_f64; n];
                for (column, coefficient) in residual_coefficients.iter().enumerate() {
                    if let Some(coefficient) = coefficient {
                        let indicator = matches!(prepared.fit_model().columns[column].kind,
                            PreparedOlsColumnKind::Categorical { .. } | PreparedOlsColumnKind::CategoricalInteraction { .. });
                        for row in 0..n {
                            let value = expected[row * (width + 1) + column + 1];
                            if !(layout == 0 && indicator && value == 0.0) {
                                raw_wanted[row] = (-*coefficient).mul_add(value, raw_wanted[row]);
                            }
                        }
                    }
                }
                let mut scalar_wanted = vec![-0.0_f64; n];
                for (column, coefficient) in residual_coefficients.iter().enumerate() {
                    if let Some(coefficient) = coefficient {
                        prepared.fit_subtract_raw_column(column, *coefficient, &mut scalar_wanted).unwrap();
                    }
                }
                assert!(scalar_wanted.iter().zip(&raw_wanted).all(|(a,b)|a.to_bits()==b.to_bits()));
'''+needle
assert part.count(needle)==1;part=part.replace(needle,new,1)
needle='                    let mut actual = vec![-0.0_f64; n];'
part=part.replace(needle,'''                    let mut raw_actual = vec![-0.0_f64; n];
                    team.install(|| prepared.fit_subtract_raw_columns(&residual_coefficients, &mut raw_actual)).unwrap().unwrap();
                    assert!(raw_actual.iter().zip(&raw_wanted).all(|(a,b)|a.to_bits()==b.to_bits()));
'''+needle,1)
s=s[:a]+part+s[b:];p.write_text(s)
print('GENERATED_OLS_RESIDUAL_PROTOTYPE_APPLIED')
