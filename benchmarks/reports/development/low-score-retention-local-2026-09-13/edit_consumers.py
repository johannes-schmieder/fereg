import os
from pathlib import Path
repo=Path(os.environ['REPOSITORY_ROOT'])
def append_calls(s,name,arg):
 token=name+'(';pos=0;edits=[]
 while True:
  start=s.find(token,pos)
  if start<0:break
  i=start+len(token);depth=1
  while depth:
   if s[i]=='(':depth+=1
   elif s[i]==')':depth-=1
   i+=1
  close=i-1;before=s[:close].rstrip();insert=('' if before.endswith(',') else ',')+' '+arg+','
  edits.append((close,insert));pos=i
 for pos,insert in reversed(edits):s=s[:pos]+insert+s[pos:]
 return s,len(edits)
p=repo/'crates/fereg-core/src/ols/prepared_robust.rs';s=p.read_text();s=s.replace('use super::prepared_inference_view::','use super::prepared_scratch::PreparedOlsScratch;\nuse super::prepared_inference_view::',1)
s=s.replace('    let fit = fit_prepared_ols_coefficients_view_map_for_inference(prepared, config.fit)?;','    let (fit, score_scratch) = fit_prepared_ols_coefficients_view_map_for_inference(prepared, config.fit)?;');assert s.count('let (fit, score_scratch)')==2
for name,expected in [('compose_internal',3),('compose_view_internal',2),('build_prepared_ols_score_design',2)]:
 s,n=append_calls(s,name,'None');assert n==expected,(name,n)
# Add the transport parameter only to the two private composers.
for name in ['compose_internal','compose_view_internal']:
 i=s.index('fn '+name+'<');j=s.index(') -> Result<',i);part=s[i:j];part+='    score_scratch: Option<PreparedOlsScratch>,\n';s=s[:i]+part+s[j:]
# Production calls thread the transport; test composers keep explicit None.
for start_name,end_name in [('pub fn fit_prepared_ols_robust_map','pub fn fit_prepared_categorical_ols_robust_map'),('pub fn fit_prepared_categorical_ols_robust_map','fn compose_internal<'),('fn compose_internal<','fn compose_view_internal<')]:
 i=s.index(start_name);j=s.index(end_name,i+len(start_name));part=s[i:j];assert part.count(' None,')==1;part=part.replace(' None,',' score_scratch,');s=s[:i]+part+s[j:]
i=s.index('fn compose_view_internal<');j=s.index('\nfn ',i+3);part=s[i:j];assert part.count(' None,')==1;part=part.replace(' None,',' score_scratch,');s=s[:i]+part+s[j:];p.write_text(s)
p=repo/'crates/fereg-core/src/ols/prepared_clustered.rs';s=p.read_text();s=s.replace('use super::prepared_inference_view::','use super::prepared_scratch::PreparedOlsScratch;\nuse super::prepared_inference_view::',1)
s=s.replace('    let fit =\n        fit_prepared_ols_coefficients_view_map_for_inference(prepared.clustered_ols(), config.fit)?;','    let (fit, score_scratch) =\n        fit_prepared_ols_coefficients_view_map_for_inference(prepared.clustered_ols(), config.fit)?;');assert s.count('let (fit, score_scratch)')==1
for name,expected in [('compose_clustered_view',2),('build_prepared_ols_score_design',1)]:s,n=append_calls(s,name,'None');assert n==expected,(name,n)
i=s.index('fn compose_clustered_view<');j=s.index(') -> Result<',i);s=s[:j]+'    score_scratch: Option<PreparedOlsScratch>,\n'+s[j:]
i=s.index('fn fit_clustered_view_with_reservation<');j=s.index('\n#[cfg(test)]',i);part=s[i:j];assert part.count(' None,')==1;part=part.replace(' None,',' score_scratch,');s=s[:i]+part+s[j:]
i=s.index('fn compose_clustered_view<');j=s.index('\nfn ',i+3);part=s[i:j];assert part.count(' None,')==1;part=part.replace(' None,',' score_scratch,');s=s[:i]+part+s[j:];p.write_text(s)
p=repo/'crates/fereg-core/src/ols/prepared_scores.rs';s=p.read_text()
i=s.index('pub(super) fn build_prepared_ols_score_design<');j=s.index(') -> Result<',i);s=s[:j]+'    retained_scratch: Option<PreparedOlsScratch>,\n'+s[j:]
s=s.replace('if let (Some(requested_solver), true) = (score_cmg_solver, cached.is_none()) {','if let (Some(requested_solver), true) = (score_cmg_solver, cached.is_none() && retained_scratch.is_none()) {',1)
a='''            fit.execution().poolsize,
            &mut cmg,
            reservation,
        )?,''';assert s.count(a)==1;s=s.replace(a,a.replace('            reservation,','            reservation,\n            retained_scratch,'))
i=s.index('fn build_scratch_score_storage<');j=s.index('\nfn fill_raw_score_block',i);part=s[i:j]
a=') -> Result<(PreparedOlsScoreStorage<\'a>, Vec<f64>, u64)> {';assert part.count(a)==1;part=part.replace(a,'    retained_scratch: Option<PreparedOlsScratch>,\n'+a)
a='    let mut columns = PreparedOlsScratch::create(nrows, rank)?;';assert part.count(a)==1;part=part.replace(a,'''    let reused = retained_scratch.is_some();
    let mut columns = match retained_scratch {
        Some(columns) => {
            if columns.rows() != nrows || columns.columns() != prepared.fit_model().columns.len() {
                return Err(invalid_argument("retained low-memory score scratch has a different model frame"));
            }
            columns
        }
        None => PreparedOlsScratch::create(nrows, rank)?,
    };''')
part=part.replace('        nrows,\n        0.0_f64,\n        "unable to reserve prepared robust scratch score column",','        if reused { 0 } else { nrows },\n        0.0_f64,\n        "unable to reserve prepared robust scratch score column",',1)
a='        let raw = DenseMatrix::from_row_major(nrows, block_rank, raw_values)?;';assert part.count(a)==1;part=part.replace(a,'''        if reused {
            continue;
        }
'''+a)
a='    let (rows, cache_values) = transpose_score_scratch(&mut columns, nrows, rank, reservation)?;';assert part.count(a)==1;part=part.replace(a,'''    let (rows, cache_values) = transpose_score_scratch(
        &mut columns, nrows, rank, reservation, reused.then_some(generated_columns),
    )?;''')
s=s[:i]+part+s[j:]
i=s.index('fn transpose_score_scratch<');j=s.index('\nstruct ScoreCertificateInputs',i);part=s[i:j];part=part.replace('    reservation: &mut R,\n','    reservation: &mut R,\n    selected_columns: Option<&[usize]>,\n',1)
part=part.replace('    let mut rows = PreparedOlsScratch::create(nrows, rank)?;','''    if selected_columns.is_some_and(|selected| selected.len() != rank) {
        return Err(invalid_argument("retained score selection differs from finite rank"));
    }
    let mut rows = PreparedOlsScratch::create(nrows, rank)?;''',1)
part=part.replace('            columns.read_column_range(finite, start_row, &mut column[..rows_in_tile])?;','            let stored = selected_columns.map_or(finite, |selected| selected[finite]);\n            columns.read_column_range(stored, start_row, &mut column[..rows_in_tile])?;',1)
s=s[:i]+part+s[j:];p.write_text(s)
