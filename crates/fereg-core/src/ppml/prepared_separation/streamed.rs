//! Streamed compact simplex at measured size and its IR handoffs.
//!
//! The fixed-allowance compact lane packs every raw incidence inside a
//! one-million-cell allowance. A generation it leaves unresolved can retry on a
//! streamed view: rows are regenerated from the factor codes, every buffer is
//! requested at its measured size from the fit's live plan, and linear stages
//! receive work in proportion to the measured raw pass. Its rays and Farkas
//! vectors pass the same raw-design and IR certificates as any other proposal.
use super::*;

/// Streamed rows store one finite-column mask per observation and borrow the
/// generation's factor codes, so the design occupies O(N) words instead of
/// O(incidences). The fit's live plan admits the design and every later
/// buffer at its measured size. Linear passes over raw rows receive work in
/// proportion to the measured pass cost at full model width; the dense
/// algebra and tableau keep explicit bounds expressed in the same measure.
///
/// Only a generation every fixed-allowance simplex lane left unresolved
/// reaches this lane, so each result those lanes certify is unchanged. An
/// explicit numeric cell limit keeps the fixed bounded policy. A Farkas vector
/// the exact terminal cannot lift becomes a hint for a requested IR step.
pub(super) fn simplex_fallback(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    (compact_hint, dual_hint): (&mut Option<Vec<i64>>, &mut Option<Vec<f64>>),
) -> Result<Option<PreparedNumericalProposal>> {
    if compact_hint.is_some() || !config.adaptive_sparse {
        return Ok(None);
    }
    let streamed = match prepared_streamed_compact(
        generation,
        config,
        interrupt,
        SeparationMethod::Simplex,
    )? {
        Some(CompactModel::NumericalDual { dual }) => {
            if config.methods.iterated_rectifier {
                *dual_hint = Some(dual);
            }
            None
        }
        other => other,
    };
    bind_compact_result(generation, streamed, config, compact_hint)
}

/// A stalled requested IR recurrence can switch once to the streamed exact
/// lane when no simplex phase ran it on this generation. A ray is replayed
/// as an ordinary IR proposal; a Farkas vector must pass the complete
/// unit-metric certificate. Anything else leaves the recurrence in control.
// Keep the certified proposal inline, as for the unit-metric resolution:
// boxing would add an unadmitted heap allocation to the detector envelope.
#[allow(clippy::large_enum_variant)]
pub(super) enum ExactSwitch {
    Direction(PreparedNumericalProposal),
    Dual(Vec<f64>),
}

pub(super) fn streamed_switch(
    generation: &PreparedPpmlModelGeneration,
    (ppml, linear): (PpmlConfig, PreparedOlsFitConfig),
    config: SeparationConfig,
    admission: DetectorAdmission<'_>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<ExactSwitch>> {
    if config.methods.simplex || !config.adaptive_sparse {
        return Ok(None);
    }
    let method = SeparationMethod::IteratedRectifier;
    let switched = match prepared_streamed_compact(generation, config, interrupt, method)? {
        Some(CompactModel::Direction {
            coefficients,
            iterations,
        }) if reconcile::admits_compact_replay(
            generation,
            method,
            config,
            coefficients.capacity(),
        ) =>
        {
            reconcile::compact_ray(generation, coefficients, iterations.max(1), method, config)?
                .map(ExactSwitch::Direction)
        }
        Some(CompactModel::NumericalDual { dual }) => {
            stable::certify_hint(generation, &dual, ppml, config, linear, admission)?
                .map(|_| ExactSwitch::Dual(dual))
        }
        _ => None,
    };
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=ir_exact_switch|generation={}|outcome={}",
            generation.generation(),
            match &switched {
                Some(ExactSwitch::Direction(_)) => "direction",
                Some(ExactSwitch::Dual(_)) => "dual",
                None => "unresolved",
            }
        ));
    }
    Ok(switched)
}

fn prepared_streamed_compact(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    method: SeparationMethod,
) -> Result<Option<CompactModel>> {
    let rows = generation.outcome().len();
    let raw = primal::RawRows::new(generation, true)?;
    let Some(blocks) = raw.intercept_blocks()? else {
        return Ok(None);
    };
    let columns = raw.width;
    let finite = generation.model().columns.len();
    let absorbed = columns - finite;
    let metadata = blocks.len().saturating_mul(16);
    // IR's own sparse phase admits the switch; it starts from the same
    // one-million-cell compact allowance as the public simplex lane.
    let mut admission = primal::SparseProofAdmission::new_with_initial(
        generation,
        method,
        config,
        config.simplex_max_tableau_cells,
    );
    let design_cells = rows
        .saturating_add(blocks.len().saturating_mul(2))
        .saturating_add(metadata);
    if !admission.admit(design_cells) {
        trace_streamed_compact(rows, columns, "design_memory", None, 0);
        return Ok(None);
    }
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(blocks.len())
        .map_err(|_| allocation("unable to reserve streamed compact effects"))?;
    for &(codes, base, levels) in &blocks {
        let (Ok(offset), Ok(levels)) = (u32::try_from(finite + base), u32::try_from(levels)) else {
            return Ok(None);
        };
        effects.push(StreamedEffect {
            codes,
            offset,
            levels,
        });
    }
    let row_terms = finite.saturating_add(blocks.len());
    drop(blocks);
    let mut poll = || interrupt(PpmlInterruptPoint::SeparationSimplexPivot);
    let mut read = compact_raw_reader(&raw, finite, absorbed);
    let Some(design) =
        BinaryDesign::streamed(rows, (columns, finite), effects, &mut read, &mut poll)?
    else {
        drop(read);
        trace_streamed_compact(rows, columns, "nonbinary", None, 0);
        return Ok(None);
    };
    drop(read);
    drop(raw);
    let work = CompactWork::scaled(rows, row_terms, columns);
    let base = design.cells().saturating_add(metadata);
    let limit = admission.words().saturating_sub(metadata);
    let mut pivots = 0;
    let outcome = generation.outcome();
    let compact = compact_model_scaled(
        outcome,
        &design,
        config,
        (limit, work, method == SeparationMethod::IteratedRectifier),
        &mut pivots,
        &mut poll,
        &mut |required| {
            let total = base.checked_add(required)?;
            admission.admit(total).then(|| admission.words() - base)
        },
    )?;
    let branch = match &compact {
        Some(CompactModel::Direction { .. }) => "direction",
        Some(CompactModel::Terminal(_)) => "terminal",
        Some(CompactModel::NumericalDual { .. }) => "numerical_dual",
        None => "unresolved",
    };
    trace_streamed_compact(rows, columns, branch, Some(work), pivots);
    Ok(compact)
}

fn trace_streamed_compact(
    rows: usize,
    columns: usize,
    outcome: &str,
    work: Option<CompactWork>,
    pivots: usize,
) {
    if crate::diagnostics::solver_trace_enabled() {
        let (stage, visits) = work.map_or((0, 0), |w| (w.stage, w.cell_visits));
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=compact_streamed|rows={rows}|columns={columns}|outcome={outcome}|stage_work={stage}|cell_visits={visits}|pivots={pivots}"
        );
    }
}

/// The raw reader emits absorbed coordinates first; packed and streamed
/// binary designs use finite-first coordinates.
pub(super) fn compact_raw_reader<'r>(
    raw: &'r primal::RawRows<'_>,
    finite: usize,
    absorbed: usize,
) -> impl FnMut(usize, &mut Vec<u32>) -> Result<bool> + 'r {
    move |i, row| {
        let mut absorbed_ones = 0;
        let mut binary = true;
        raw.terms(i, |coordinate, value| {
            if value == 1. {
                if coordinate < absorbed {
                    row.push((finite + coordinate) as u32);
                    absorbed_ones += 1;
                } else {
                    row.push((coordinate - absorbed) as u32);
                }
            } else if value != 0. {
                binary = false;
            }
        })?;
        row.rotate_left(absorbed_ones);
        Ok(binary)
    }
}

/// Bind a compact proof, or hand integer ray coefficients to a requested IR
/// step when the replay output does not fit the simplex allowance.
pub(super) fn bind_compact_result(
    generation: &PreparedPpmlModelGeneration,
    compact: Option<CompactModel>,
    config: SeparationConfig,
    compact_hint: &mut Option<Vec<i64>>,
) -> Result<Option<PreparedNumericalProposal>> {
    let proposal = match compact {
        Some(CompactModel::Terminal(raw)) => Some(bind_numerical_proposal(generation, raw)?),
        Some(CompactModel::NumericalDual { .. }) => None,
        Some(CompactModel::Direction {
            coefficients,
            iterations,
        }) => {
            let required =
                primal::RawRows::sparse_replay_words(generation, coefficients.capacity());
            if !reconcile::admits_compact_replay(
                generation,
                SeparationMethod::Simplex,
                config,
                coefficients.capacity(),
            ) {
                if config.methods.iterated_rectifier
                    && required
                        <= primal::ProofBudget::for_sparse(
                            generation,
                            SeparationMethod::IteratedRectifier,
                            config,
                        )
                        .words
                {
                    *compact_hint = Some(coefficients);
                }
                None
            } else {
                reconcile::compact_ray(
                    generation,
                    coefficients,
                    iterations,
                    SeparationMethod::Simplex,
                    config,
                )?
            }
        }
        None => None,
    };
    Ok(proposal)
}
