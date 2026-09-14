import os
from pathlib import Path
repo=Path(os.environ['REPOSITORY_ROOT']);p=repo/'crates/fereg-core/src/ols/prepared_rhs.rs';s=p.read_text();s=s.replace('const MAX_BOUND_NUMERIC_COLUMNS: usize = 64;','pub(crate) const MAX_BOUND_NUMERIC_COLUMNS: usize = 64;',1)
needle='    pub(crate) fn subtract_fitted(\n';assert s.count(needle)==1
method='''    /// Add selected raw columns in the caller's coefficient order. Keeping
    /// the original order preserves each row's FMA result after rank selection.
    pub(crate) fn add_selected_columns(
        &self,
        selected: &[(usize, f64)],
        fitted: &mut [f64],
    ) -> Result<()> {
        if fitted.len() != self.rows
            || selected.iter().any(|&(column, value)| column >= self.width || !value.is_finite())
        {
            return Err(invalid_argument("numeric predictor columns are not aligned and finite"));
        }
        fitted.par_chunks_mut(NUMERIC_TILE_ROWS).enumerate().try_for_each(
            |(tile, output)| -> Result<()> {
                let start = tile * NUMERIC_TILE_ROWS;
                let rows = output.len();
                for &(column, coefficient) in selected {
                    if let Some(source) = self.columns[column] {
                        for (target, &value) in output.iter_mut().zip(&source[start..start + rows]) {
                            *target = coefficient.mul_add(value, *target);
                        }
                    } else {
                        for target in output.iter_mut() {
                            *target = coefficient.mul_add(1.0, *target);
                        }
                    }
                }
                if output.iter().any(|value| !value.is_finite()) {
                    return Err(invalid_data("numeric finite predictor is not finite"));
                }
                Ok(())
            },
        )
    }

''';s=s.replace(needle,method+needle,1)
needle='mod tests {\n';assert s.count(needle)==1
tests='''
    #[test]
    fn selected_predictor_tiles_preserve_scalar_fma_order_and_reject_invalid_inputs() {
        use crate::options::ThreadConfig;
        use crate::parallel::ThreadTeam;
        for width in [8_usize, 64] {
            let n = 4099;
            let sources = (0..width - 1).map(|column| {
                (0..n).map(|row| ((row * 17 + column * 3) as f64).sin()
                    * if column % 2 == 0 { 1e100 } else { 1e-100 }).collect::<Vec<_>>()
            }).collect::<Vec<_>>();
            let mut bindings = [None; MAX_BOUND_NUMERIC_COLUMNS];
            for (target, source) in bindings.iter_mut().zip(&sources) { *target = Some(source.as_slice()); }
            let columns = NumericOlsColumns { columns: bindings, width, rows: n };
            // Deliberately noncanonical order and a repeated identity catch
            // regrouping or sorting that would change per-row accumulation.
            let selected: [(usize, f64); 5] = [(width - 1, -0.0), (3, 1e100), (0, 1e-100), (3, -1e100), (1, 0.7)];
            let mut expected = vec![0.0; n];
            for &(column, coefficient) in &selected {
                for row in 0..n {
                    let value = columns.columns[column].map_or(1.0, |source| source[row]);
                    expected[row] = coefficient.mul_add(value, expected[row]);
                }
            }
            for workers in [1, 4, 16] {
                let mut actual = vec![0.0; n];
                ThreadTeam::new(ThreadConfig::from_requested(workers, true).unwrap()).install(|| {
                    columns.add_selected_columns(&selected, &mut actual)
                }).unwrap().unwrap();
                assert!(actual.iter().zip(&expected).all(|(a,b)| a.to_bits()==b.to_bits()));
            }
            let mut unchanged = vec![7.0; n];
            for invalid in [[(width, 1.0)], [(0, f64::NAN)], [(0, f64::INFINITY)]] {
                assert!(columns.add_selected_columns(&invalid, &mut unchanged).is_err());
                assert!(unchanged.iter().all(|x| *x == 7.0));
            }
            assert!(columns.add_selected_columns(&[], &mut unchanged[..n-1]).is_err());
            columns.add_selected_columns(&[], &mut unchanged).unwrap();
            assert!(unchanged.iter().all(|x| *x == 7.0));
            assert!(columns.add_selected_columns(&[(0, f64::MAX)], &mut unchanged).is_err());
        }
    }
''';s=s.replace(needle,needle+tests,1);p.write_text(s)
p=repo/'crates/fereg-core/src/ols/linear_session.rs';s=p.read_text();start=s.index('    fn rebuild_predictors(\n');end=s.index('\n    #[cfg(test)]\n    fn root_solver_weight',start);part=s[start:end]
needle='        for coefficient in fit.coefficients() {\n';assert part.count(needle)==1
part=part.replace(needle,'''        let numeric = super::prepared_rhs::NumericOlsColumns::bind(
            self.prepared.fit_model(), self.prepared.fit_sample(),
        )?;
        let mut selected = [(0_usize, 0.0_f64); super::prepared_rhs::MAX_BOUND_NUMERIC_COLUMNS];
        let mut selected_count = 0;
        for coefficient in fit.coefficients() {
''',1)
old='''            for row in 0..rows {
                self.finite_predictor[row] = value.mul_add(
                    self.prepared.fit_column_value(generated, row)?,
                    self.finite_predictor[row],
                );
            }
        }
        let mut certificate'''
new='''            if numeric.is_some() {
                let target = selected.get_mut(selected_count).ok_or_else(|| {
                    invalid_argument("numeric predictor selection exceeds its bound")
                })?;
                *target = (generated, value);
                selected_count += 1;
            } else {
                for row in 0..rows {
                    self.finite_predictor[row] = value.mul_add(
                        self.prepared.fit_column_value(generated, row)?,
                        self.finite_predictor[row],
                    );
                }
            }
        }
        if let Some(numeric) = numeric {
            numeric.add_selected_columns(&selected[..selected_count], &mut self.finite_predictor)?;
        }
        let mut certificate''';assert old in part;part=part.replace(old,new,1);s=s[:start]+part+s[end:];p.write_text(s)
