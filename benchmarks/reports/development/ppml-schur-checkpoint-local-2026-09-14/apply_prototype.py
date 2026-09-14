import os
from pathlib import Path
p=Path(os.path.join(os.environ['REPOSITORY_ROOT'], 'crates/fereg-core/src/fe/borrowed_map.rs'))
s=p.read_text()
# Preserve OLS's exact three-column arithmetic while admitting a larger retained stride.
a=s.index('    let left = &design.effects()[graph.left_effect];',s.index('pub(crate) fn schur_ols_probe('))
b=s.index('\npub(crate) fn absorb_ols_schur_certified(',a)
body=s[a:b]
body=body.replace('.par_chunks_mut(3)', '.par_chunks_mut(stride)').replace('a * 3 + rhs','a * stride + rhs').replace('level * 3 + rhs','level * stride + rhs').replace('graph.apply_schur(design, weights, 3, 3)?','graph.apply_schur(design, weights, 3, stride)?')
body=body.replace('            *total = 0.0;', '            sum[..3].fill(0.0);\n            *total = 0.0;').replace('            for value in sum {','            for value in &mut sum[..3] {')
s=s[:a]+'''    schur_probe_reusing(design, weights, values, &mut graph, 3)
}

fn schur_probe_reusing(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    graph: &mut TwoFePcgWorkspace,
    stride: usize,
) -> Result<f64> {
    if design.effects().len() != 2 || design.nobs() != values.nrows()
        || values.ncols() < 3 || stride < 3
        || graph.left_rhs.len() != graph.left_weights.len().saturating_mul(stride)
        || graph.residual.len() != graph.right_weights.len().saturating_mul(stride)
    {
        return Err(invalid_argument("Schur probe workspace dimensions are inconsistent"));
    }
'''+body+s[b:]
# Test-only injections do not enlarge the production retained session.
s=s.replace('    checkpoint_lsmr_retry_error: Option<ErrorCode>,','''    checkpoint_lsmr_retry_error: Option<ErrorCode>,
    #[cfg(test)]
    checkpoint_schur_iteration_cap: Option<usize>,
    #[cfg(test)]
    checkpoint_schur_certificate_failure: bool,''',1)
s=s.replace('            checkpoint_lsmr_retry_error: None,','''            checkpoint_lsmr_retry_error: None,
            #[cfg(test)]
            checkpoint_schur_iteration_cap: None,
            #[cfg(test)]
            checkpoint_schur_certificate_failure: false,''',1)
needle='''            #[cfg(not(test))]
            let force_rejection = false;'''
s=s.replace(needle,'''            let (schur, schur_cost) = self.try_checkpoint_schur(weights, &values, config)?;
            if let Some(mut certified) = schur {
                certified.report.setup_seconds += setup_seconds;
                certified.report.certification_seconds += initial_certification_seconds;
                return Ok(certified);
            }
'''+needle,1)
s=s.replace('''                    return CertifiedBorrowedMap::from_certified_parts(candidate, report);
                }
                report.stop_reason = SolveStopReason::Uncertified;''','''                    schur_cost.add_to(&mut report);
                    return CertifiedBorrowedMap::from_certified_parts(candidate, report);
                }
                report.stop_reason = SolveStopReason::Uncertified;''',1)
s=s.replace('''            // The LSMR candidate never aliases `values`.''','''            let mut report = report;
            schur_cost.add_to(&mut report);

            // The LSMR candidate never aliases `values`.''',1)
idx=s.index('    fn absorb_two_fe_pcg_after_pilot(')
s=s[:idx]+'''    fn try_checkpoint_schur(
        &mut self,
        weights: ValidatedWeights<'_>,
        values: &DenseMatrix,
        config: MapConfig,
    ) -> Result<(Option<CertifiedBorrowedMap>, CheckpointSchurCost)> {
        let mut cost = CheckpointSchurCost::default();
        if !self.checkpoint_lsmr_enabled || self.two_fe_pcg.is_none()
            || self.design.nobs() < PARALLEL_PROJECTION_MIN_ROWS
            || !(16..=64).contains(&values.ncols())
            || config.max_iterations <= OLS_SCHUR_PROBE_ITERATIONS
        {
            return Ok((None, cost));
        }
        let started = Instant::now();
        cost.contraction = schur_probe_reusing(
            &self.design, weights, values, self.two_fe_pcg.as_mut().unwrap(), self.max_rhs,
        )?;
        cost.probe_iterations = OLS_SCHUR_PROBE_ITERATIONS;
        cost.setup_seconds = started.elapsed().as_secs_f64();
        if cost.contraction > OLS_SCHUR_MAX_CONTRACTION {
            cost.trace(values, "probe_rejected");
            return Ok((None, cost));
        }

        // Only checkpoint-LSMR callers enter here. Their preflight charges an
        // input plus the borrowed adapter's output/transport banks, while this
        // retained graph is already in the canonical session's selected plan.
        // One candidate replaces those serial LSMR banks, never overlaps them.
        let level_counts = [self.design.effects()[0].n_levels(), self.design.effects()[1].n_levels()];
        let transient = estimate_fe_solver_memory(
            values.nrows(), values.ncols(), &level_counts, rayon::current_num_threads(), Some(4),
        )?.lsmr_schwarz_bytes;
        let copy_started = Instant::now();
        let candidate = checkpoint_schur_copy(values, transient)?;
        cost.setup_seconds += copy_started.elapsed().as_secs_f64();
        let cap = OLS_SCHUR_MAX_ITERATIONS.min(config.max_iterations - cost.probe_iterations);
        #[cfg(test)]
        let cap = self.checkpoint_schur_iteration_cap.take().unwrap_or(cap).min(cap);
        let solve_started = Instant::now();
        for scalars in [&mut self.cg_residual_square, &mut self.cg_initial_residual_square,
            &mut self.cg_next_residual_square, &mut self.cg_direction_projection, &mut self.cg_ratio]
        {
            scalars.fill(0.0);
        }
        let attempt = absorb_two_fe_pcg_reusing(
            &self.design, weights, candidate, config, 0, cost.contraction, 0.0,
            solve_started, self.max_rhs, self.two_fe_pcg.as_mut().unwrap(),
            &mut self.workspace, &mut self.cg_residual_square,
            &mut self.cg_initial_residual_square, &mut self.cg_next_residual_square,
            &mut self.cg_direction_projection, &mut self.cg_ratio, Some(cap),
        )?;
        cost.solve_seconds = solve_started.elapsed().as_secs_f64();
        match attempt {
            SchurAttempt::Certified(mut result) => {
                #[cfg(test)]
                if std::mem::take(&mut self.checkpoint_schur_certificate_failure) {
                    cost.iterations = result.report.iterations;
                    cost.rejected = true;
                    cost.trace(values, "certificate_rejected");
                    return Ok((None, cost));
                }
                result.report.requested_solver = FeSolverKind::Lsmr;
                result.report.setup_seconds += cost.setup_seconds;
                result.report.iterations += cost.probe_iterations;
                result.report.pilot_iterations = cost.probe_iterations;
                cost.iterations = result.report.iterations - cost.probe_iterations;
                cost.trace(values, "certified");
                Ok((Some(result), cost))
            }
            SchurAttempt::Continue { iterations, candidate, .. } => {
                cost.iterations = iterations;
                cost.rejected = true;
                cost.trace(values, if candidate { "certificate_rejected" } else { "capped_or_breakdown" });
                // `values` is borrowed and untouched. The finite attempt is
                // dropped before any LSMR setup or tighter retry can allocate.
                Ok((None, cost))
            }
        }
    }

'''+s[idx:]
idx=s.index('// Values remain authoritative input until reconstruction.')
s=s[:idx]+'''#[derive(Default)]
struct CheckpointSchurCost {
    probe_iterations: usize,
    iterations: usize,
    contraction: f64,
    setup_seconds: f64,
    solve_seconds: f64,
    rejected: bool,
}

impl CheckpointSchurCost {
    fn add_to(&self, report: &mut SolveReport) {
        if self.probe_iterations == 0 { return; }
        report.iterations = report.iterations.saturating_add(self.probe_iterations + self.iterations);
        report.setup_seconds += self.setup_seconds;
        report.solve_seconds += self.solve_seconds;
        report.pilot_iterations = self.probe_iterations;
        report.pilot_contraction = self.contraction;
        report.checkpoint_retries += usize::from(self.rejected);
        if self.rejected {
            report.warnings.push(format!(
                "bounded current-weight Schur checkpoint rejected after {} iterations; LSMR used untouched authoritative inputs",
                self.iterations,
            ));
        }
    }

    fn trace(&self, values: &DenseMatrix, outcome: &str) {
        if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
            emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=checkpoint_schur_attempt|outcome={outcome}|rows={}|rhs={}|probe_budget={}|contraction={:.17e}|iterations={}|setup_seconds={:.17e}|solve_seconds={:.17e}",
                values.nrows(), values.ncols(), self.probe_iterations, self.contraction,
                self.iterations, self.setup_seconds, self.solve_seconds,
            ));
        }
    }
}

fn checkpoint_schur_copy(values: &DenseMatrix, admitted_transient_bytes: u128) -> Result<DenseMatrix> {
    let payload = (values.as_slice().len() as u128).checked_mul(8)
        .ok_or_else(|| allocation("checkpoint Schur candidate size overflow"))?;
    if payload > admitted_transient_bytes {
        return Err(allocation("checkpoint Schur candidate exceeds admitted transient workspace"));
    }
    fallible_matrix_copy(values, "unable to reserve checkpoint Schur candidate")
}

'''+s[idx:]
p.write_text(s)
