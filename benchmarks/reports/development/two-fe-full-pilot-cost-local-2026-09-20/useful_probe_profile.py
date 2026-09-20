"""Isolated diagnostic: price full-width pilot work; never reuse its state."""
from pathlib import Path


def instrument(source):
 p=Path(source)/'crates/fereg-core/src/fe/borrowed_map.rs';text=p.read_text()
 start=text.index('fn schur_probe_reusing(');end=text.index('\npub(crate) fn absorb_ols_schur_certified(',start)
 original=text[start:end]
 prototype=original.replace('fn schur_probe_reusing(', 'fn schur_probe_fullwidth_cost(')
 prototype=prototype.replace('    // A previous failed full-width attempt', '    let rhs_count = values.ncols();\n    let selected = [0, 1, rhs_count / 2];\n    let mut discarded = 0_u8;\n    graph.solution.fill(0.0);\n    // A previous failed full-width attempt',1)
 prototype=prototype.replace('let columns = [0, 1, values.ncols() / 2];', 'let all_columns = [0, 1, 2, 3, 4, 5, 6, 7];\n    let columns = &all_columns[..rhs_count];')
 prototype=prototype.replace('[..3]', '[..rhs_count]').replace('0..3', '0..rhs_count').replace('[0.0; 3]', '[0.0; 8]').replace('weights, 3, stride', 'weights, rhs_count, stride')
 old='''            if !projection[rhs].is_finite() || projection[rhs] <= 0.0 {
                return Ok(f64::INFINITY);
            }'''
 new='''            if !projection[rhs].is_finite() || projection[rhs] <= 0.0 {
                if selected.contains(&rhs) { return Ok(f64::INFINITY); }
                discarded |= 1 << rhs;
                continue;
            }'''
 assert prototype.count(old)==1;prototype=prototype.replace(old,new)
 old='''                graph.residual[i] = (-step[rhs]).mul_add(graph.operator[i], graph.residual[i]);'''
 new='''                graph.solution[i] = step[rhs].mul_add(graph.direction[i], graph.solution[i]);
                graph.residual[i] = (-step[rhs]).mul_add(graph.operator[i], graph.residual[i]);'''
 assert prototype.count(old)==1;prototype=prototype.replace(old,new)
 old='''    Ok((0..rhs_count)
        .map(|rhs| {'''
 new='''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|full_pilot_cost|width={rhs_count}|discarded_nonselected_mask={discarded}");
    }
    Ok(selected.into_iter()
        .map(|rhs| {'''
 assert prototype.count(old)==1;prototype=prototype.replace(old,new)
 # Only the fresh OLS probe uses the experimental helper; the shared/PPML
 # three-column function remains byte-for-byte unchanged.
 needle='''    let mut graph = TwoFePcgWorkspace::new(design, 3, &mut reservation)?;
    let peak = SolverWorkspacePeak::from_bytes(graph.capacity_elements() as u128 * 8)?;
    let contraction = schur_probe_reusing(design, weights, values, &mut graph, 3)?;'''
 replacement='''    let width = values.ncols();
    let full = (3..=8).contains(&width);
    let stride = if full { width } else { 3 };
    let mut graph = TwoFePcgWorkspace::new(design, stride, &mut reservation)?;
    let peak = SolverWorkspacePeak::from_bytes(graph.capacity_elements() as u128 * 8)?;
    let contraction = if full {
        schur_probe_fullwidth_cost(design, weights, values, &mut graph, stride)?
    } else { schur_probe_reusing(design, weights, values, &mut graph, stride)? };'''
 assert text.count(needle)==1;text=text.replace(needle,replacement)
 marker='\npub(crate) fn absorb_ols_schur_certified('
 assert text.count(marker)==1;text=text.replace(marker,'\n'+prototype+marker)
 p.write_text(text)
