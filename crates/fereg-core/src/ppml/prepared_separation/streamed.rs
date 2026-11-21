//! Streamed compact simplex at measured size and its IR handoffs.
//!
//! The fixed-allowance compact lane packs every raw incidence inside a
//! one-million-cell allowance. A generation it leaves unresolved can retry on a
//! streamed view: rows are regenerated from the generation (factor codes for a
//! binary intercept design, the raw reader for any other design), every buffer
//! is requested at its measured size from the fit's live plan, and linear
//! stages receive work in proportion to the measured raw pass. Rays and Farkas
//! vectors pass the same raw-design and IR certificates as any other proposal.
use super::*;

/// One streamed exact attempt on a generation.
// Keep the certified proposal inline, as for the unit-metric resolution:
// boxing would add an unadmitted heap allocation to the detector envelope.
#[allow(clippy::large_enum_variant)]
enum Exact {
    /// A binary compact proof or proposal (exact ray, exact terminal, or a
    /// numerical Farkas vector).
    Binary(CompactModel),
    /// An exactly replayed ray from the valued lane, already bound.
    Ray(PreparedNumericalProposal),
    /// A physical observation dual proposal for the IR certificate.
    Dual(Vec<f64>),
    /// Raw-coordinate ray coefficients without an exact integer lift.
    FloatRay(Vec<f64>, usize),
}

/// What the simplex phase hands to the requested IR step of the same
/// generation. Neither is a result until IR's own certificate accepts it.
pub(super) enum IrHint {
    /// A physical Farkas dual for the complete unit-metric certificate.
    Dual(Vec<f64>),
    /// A linear-predictor direction for the IR direction certificate.
    Direction {
        direction: Vec<f64>,
        iterations: usize,
    },
}

/// Only a generation every fixed-allowance simplex lane left unresolved
/// reaches this lane, so each result those lanes certify is unchanged. An
/// explicit numeric cell limit keeps the fixed bounded policy. A Farkas vector
/// the exact terminal cannot lift becomes a hint for a requested IR step.
pub(super) fn simplex_fallback(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    intercept_only: bool,
    (compact_hint, dual_hint, tried): SimplexHints<'_>,
) -> Result<Option<PreparedNumericalProposal>> {
    if compact_hint.is_some() {
        return Ok(None);
    }
    if !config.adaptive_sparse {
        trace_decline(generation, "explicit_cell_limit");
        return Ok(None);
    }
    *tried = true;
    let method = SeparationMethod::Simplex;
    match exact_lane(generation, config, interrupt, method, intercept_only)? {
        Some(Exact::Dual(dual)) => {
            if config.methods.iterated_rectifier {
                *dual_hint = Some(IrHint::Dual(dual));
            }
            Ok(None)
        }
        Some(Exact::FloatRay(coefficients, iterations)) => {
            if config.methods.iterated_rectifier {
                *dual_hint = Some(IrHint::Direction {
                    direction: ray_direction(generation, &coefficients)?,
                    iterations,
                });
            }
            Ok(None)
        }
        Some(Exact::Binary(model)) => {
            bind_compact_result(generation, Some(model), config, compact_hint)
        }
        Some(Exact::Ray(proposal)) => Ok(Some(proposal)),
        None => Ok(None),
    }
}

/// A stalled requested IR recurrence switches once to the streamed exact
/// lane when no simplex phase ran it on this generation. A ray is replayed
/// as an ordinary IR proposal; a Farkas vector must pass the complete
/// unit-metric certificate. Anything else leaves the stall in force.
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
    let reason = if admission.exact_tried {
        Some("tried_in_simplex")
    } else if !config.adaptive_sparse {
        Some("explicit_cell_limit")
    } else {
        None
    };
    if let Some(reason) = reason {
        trace_switch(generation, reason);
        return Ok(None);
    }
    let method = SeparationMethod::IteratedRectifier;
    let intercept_only = generation
        .fit_view()
        .effect_specs()
        .iter()
        .all(|spec| spec.slopes.is_empty());
    let switched = match exact_lane(generation, config, interrupt, method, intercept_only)? {
        Some(Exact::Binary(CompactModel::Direction {
            coefficients,
            iterations,
        })) if reconcile::admits_compact_replay(
            generation,
            method,
            config,
            coefficients.capacity(),
        ) =>
        {
            reconcile::compact_ray(generation, coefficients, iterations.max(1), method, config)?
                .map(ExactSwitch::Direction)
        }
        Some(Exact::Ray(proposal)) => Some(ExactSwitch::Direction(proposal)),
        Some(Exact::Dual(dual)) => {
            stable::certify_hint(generation, &dual, ppml, config, linear, admission)?
                .map(|_| ExactSwitch::Dual(dual))
        }
        Some(Exact::FloatRay(coefficients, iterations)) => {
            let mut direction = ray_direction(generation, &coefficients)?;
            stable::direction_proposal(
                generation,
                config,
                iterations,
                &mut direction,
                "streamed compact ray without an exact integer lift",
            )?
            .map(ExactSwitch::Direction)
        }
        _ => None,
    };
    trace_switch(
        generation,
        match &switched {
            Some(ExactSwitch::Direction(_)) => "direction",
            Some(ExactSwitch::Dual(_)) => "dual",
            None => "unresolved",
        },
    );
    Ok(switched)
}

/// The direction `-X b` of raw coefficients `b` on every row, compensated:
/// positive on the rows a separating ray drops.
fn ray_direction(
    generation: &PreparedPpmlModelGeneration,
    coefficients: &[f64],
) -> Result<Vec<f64>> {
    let raw = primal::RawRows::new(generation, true)?;
    let rows = generation.outcome().len();
    let mut direction = fallible_filled(rows, 0., "unable to reserve streamed ray direction")?;
    if coefficients.len() != raw.width {
        return Ok(direction);
    }
    for (i, value) in direction.iter_mut().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let (mut score, mut correction) = (0., 0.);
        raw.terms(i, |j, v| {
            crate::linalg::neumaier_add(v * coefficients[j], &mut score, &mut correction)
        })?;
        *value = -(score + correction);
    }
    Ok(direction)
}

fn trace_switch(generation: &PreparedPpmlModelGeneration, outcome: &str) {
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=ir_exact_switch|generation={}|outcome={outcome}",
            generation.generation(),
        ));
    }
}

fn trace_decline(generation: &PreparedPpmlModelGeneration, reason: &str) {
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=compact_streamed_decline|generation={}|reason={reason}",
            generation.generation()
        );
    }
}

/// The binary lane when the design is a binary intercept design; otherwise,
/// or when it declines, the valued lane on the raw reader.
fn exact_lane(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    method: SeparationMethod,
    intercept_only: bool,
) -> Result<Option<Exact>> {
    if intercept_only {
        match prepared_streamed_compact(generation, config, interrupt, method)? {
            Ok(model) => {
                return Ok(model.map(|model| match model {
                    CompactModel::NumericalDual { dual } => Exact::Dual(dual),
                    CompactModel::NumericalDirection {
                        coefficients,
                        iterations,
                    } => Exact::FloatRay(coefficients, iterations),
                    model => Exact::Binary(model),
                }))
            }
            Err(reason) => trace_decline(generation, reason),
        }
    } else {
        trace_decline(generation, "absorbed_slopes");
    }
    valued_lane(generation, config, interrupt, method)
}

/// The projected-cone search on the raw reader: any finite values, any number
/// of finite columns and absorbed coordinates, intercepts and slopes. Rows
/// are not stored; every buffer is requested at its measured size.
fn valued_lane(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    method: SeparationMethod,
) -> Result<Option<Exact>> {
    let rows = generation.outcome().len();
    let raw = primal::RawRows::new(generation, true)?;
    let width = raw.width;
    let row_terms = raw.sparse_row_visits();
    let read = |i: usize, f: &mut dyn FnMut(usize, f64)| raw.terms(i, &mut *f);
    let design = ValuedDesign {
        width,
        row_terms,
        read: &read,
    };
    let mut admission = primal::SparseProofAdmission::new_with_initial(
        generation,
        method,
        config,
        config.simplex_max_tableau_cells,
    );
    let work = CompactWork::scaled(rows, row_terms, width);
    let mut search = ValuedSearch {
        outcome: generation.outcome(),
        design: &design,
        config,
        work,
        available: admission.words(),
        held: 0,
    };
    let mut pivots = 0;
    let mut poll = || interrupt(PpmlInterruptPoint::SeparationSimplexPivot);
    let mut verify = |coefficients: &[i64]| primal::exact_ray(generation, coefficients);
    let model = search.run(
        &mut pivots,
        &mut poll,
        &mut |needed| admission.admit(needed).then(|| admission.words()),
        &mut verify,
    )?;
    let result = match model {
        Some(ValuedModel::Direction {
            coefficients,
            iterations,
        }) => reconcile::sparse_ray(generation, coefficients, iterations.max(1), method, config)?
            .map(Exact::Ray),
        Some(ValuedModel::NumericalDual { dual }) => Some(Exact::Dual(dual)),
        Some(ValuedModel::NumericalDirection {
            coefficients,
            iterations,
        }) => Some(Exact::FloatRay(coefficients, iterations)),
        None => None,
    };
    if crate::diagnostics::solver_trace_enabled() {
        let outcome = match &result {
            Some(Exact::Ray(_)) => "direction",
            Some(Exact::Dual(_)) => "numerical_dual",
            Some(Exact::FloatRay(..)) => "numerical_direction",
            _ => "unresolved",
        };
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=valued_streamed|rows={rows}|columns={width}|row_terms={row_terms}|outcome={outcome}|stage_work={}|pivots={pivots}",
            work.stage
        );
    }
    Ok(result)
}

/// The binary lane: `Err` names why the design is not a binary intercept
/// design the factor-code view can regenerate.
fn prepared_streamed_compact(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    method: SeparationMethod,
) -> Result<std::result::Result<Option<CompactModel>, &'static str>> {
    let rows = generation.outcome().len();
    let raw = primal::RawRows::new(generation, true)?;
    let Some(blocks) = raw.intercept_blocks()? else {
        return Ok(Err("absorbed_slopes"));
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
        return Ok(Ok(None));
    }
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(blocks.len())
        .map_err(|_| allocation("unable to reserve streamed compact effects"))?;
    for &(codes, base, levels) in &blocks {
        let (Ok(offset), Ok(levels)) = (u32::try_from(finite + base), u32::try_from(levels)) else {
            return Ok(Err("index_over_u32"));
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
    let design =
        match BinaryDesign::streamed(rows, (columns, finite), effects, &mut read, &mut poll)? {
            Ok(design) => design,
            Err(reason) => return Ok(Err(reason)),
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
        Some(CompactModel::NumericalDirection { .. }) => "numerical_direction",
        None => "unresolved",
    };
    trace_streamed_compact(rows, columns, branch, Some(work), pivots);
    Ok(Ok(compact))
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
        Some(CompactModel::NumericalDual { .. } | CompactModel::NumericalDirection { .. }) => None,
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
