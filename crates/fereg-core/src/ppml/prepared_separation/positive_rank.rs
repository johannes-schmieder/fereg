//! Exact sufficient proofs that every direction zero on the positive outcomes
//! also vanishes on the full prepared design. A failed test is inconclusive.
use super::*;
use crate::ppml::modular_rank::{insert, residue, PRIME};
use crate::ppml_separation::SeparationCertificateProof;

mod circulation;
mod coverage;
mod dual;
pub(super) mod finite;
pub(super) use dual::exact_dual;
mod packed;
pub(super) mod sparse_dual;
pub(super) use coverage::positive_twins;

pub(super) fn pure_pair(
    model: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
) -> bool {
    topology.effect_specs.len() == 2
        && topology
            .effect_specs
            .iter()
            .all(|e| e.intercept && e.slopes.is_empty())
        && model
            .model()
            .columns
            .iter()
            .all(|c| matches!(c.kind, crate::ols::PreparedOlsColumnKind::Intercept))
}

pub(super) fn detect(
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    topology_cells: usize,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<DetectionProposal>> {
    let limit = primal::ProofBudget::for_sparse(generation, SeparationMethod::Simplex, config)
        .words
        .saturating_sub(topology_cells);
    // Ordinary indicators and shared controls can use compact simplex, then
    // any requested IR projection, before exploring general dual weights.
    let search = if topology.effect_specs.iter().all(|e| e.slopes.is_empty()) {
        sparse_dual::Search::Marginal
    } else {
        sparse_dual::Search::Full
    };
    detect_for(
        SeparationMethod::Simplex,
        generation,
        topology,
        positive,
        (limit, search),
        config,
        interrupt,
    )
}

pub(super) fn detect_for(
    method: SeparationMethod,
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    (limit, search): (usize, sparse_dual::Search),
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<DetectionProposal>> {
    if let Some(proof) = rank_or_coverage(
        method, generation, topology, positive, limit, config, interrupt,
    )? {
        return Ok(Some(proof));
    }
    sparse_proof(
        method,
        generation,
        topology,
        positive,
        (limit, search),
        config,
        interrupt,
    )
}

fn sparse_proof(
    method: SeparationMethod,
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    (limit, search): (usize, sparse_dual::Search),
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<DetectionProposal>> {
    if sparse_dual::supports(
        search, generation, topology, positive, limit, config, interrupt,
    )? {
        return Ok(Some(no_separation(
            method,
            SeparationCertificateProof::ExactPositiveDual {
                boundary_rows: generation.outcome().len() - positive,
            },
        )));
    }
    Ok(None)
}

/// Explicit simplex can still explore raw-coordinate projections once cheap
/// directions and compact simplex have both failed. With IR requested, its
/// ordinary solver runs first; its unresolved terminal fallback calls detect_for.
pub(super) fn deferred_simplex(
    model: &PreparedPpmlModelGeneration,
    positive: usize,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<DetectionProposal>> {
    if config.methods.iterated_rectifier
        || model
            .fit_view()
            .effect_specs()
            .iter()
            .any(|e| !e.slopes.is_empty())
    {
        return Ok(None);
    }
    let method = SeparationMethod::Simplex;
    let topology = DetectorTopology::new(model.prepared())?;
    let metadata =
        topology.sources.len() + topology.effect_specs.len() + topology.effect_to_source.len();
    let limit = primal::ProofBudget::for_sparse(model, method, config)
        .words
        .saturating_sub(metadata);
    sparse_proof(
        method,
        model,
        &topology,
        positive,
        (limit, sparse_dual::Search::Full),
        config,
        interrupt,
    )
}

/// On a varying-slope IR generation, test global balance and exact small-cell
/// relations before repeated weighted projections. Reconstruct those same
/// direct proposals on terminal replay.
pub(super) fn direct_slope_support(
    model: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<PreparedNumericalProposal>> {
    if !model
        .fit_view()
        .effect_specs()
        .iter()
        .any(|s| !s.slopes.is_empty())
    {
        return Ok(None);
    }
    let method = SeparationMethod::IteratedRectifier;
    let topology = DetectorTopology::new(model.prepared())?;
    let metadata =
        topology.sources.len() + topology.effect_specs.len() + topology.effect_to_source.len();
    let limit = primal::ProofBudget::for_sparse(model, method, config)
        .words
        .saturating_sub(metadata);
    let positive = model.outcome().iter().filter(|&&y| y > 0.).count();
    let mut poll = |_| interrupt(PpmlInterruptPoint::SeparationIrIteration);
    if sparse_dual::supports(
        sparse_dual::Search::Direct,
        model,
        &topology,
        positive,
        limit,
        config,
        &mut poll,
    )? {
        let proof = SeparationCertificateProof::ExactPositiveDual {
            boundary_rows: model.outcome().len() - positive,
        };
        return bind_numerical_proposal(model, no_separation(method, proof)).map(Some);
    }
    Ok(None)
}

fn rank_or_coverage(
    method: SeparationMethod,
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    limit: usize,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<DetectionProposal>> {
    let work = primal::ProofBudget::new(generation, method, config)
        .work_cells
        .saturating_mul(8);
    let independent = |constant| RankMode::Independent {
        constant,
        iterations: config.simplex_max_iterations,
    };
    if let Some(proof) = anchored(
        generation,
        topology,
        positive,
        (limit, work),
        independent(&[]),
        interrupt,
    )? {
        return Ok(Some(no_separation(method, proof)));
    }
    let boundary = generation.outcome().len() - positive;
    if positive_twins(generation, topology, boundary, limit, work, None, interrupt)? {
        return Ok(Some(no_separation(
            method,
            SeparationCertificateProof::SimplexPositiveRowCoverage {
                boundary_rows: boundary,
            },
        )));
    }
    if boundary > 0
        && circulation::certified(generation, topology, limit, config.certificate_tolerance)?
    {
        return Ok(Some(no_separation(
            method,
            SeparationCertificateProof::ExactPositiveDual {
                boundary_rows: boundary,
            },
        )));
    }
    let n = generation.outcome().len();
    let columns = &generation.model().columns;
    if n == 0
        || columns.len() > limit
        || n.saturating_mul(columns.len()) > work
        || topology.effect_specs.is_empty()
        || topology
            .effect_specs
            .iter()
            .any(|e| !e.intercept || !e.slopes.is_empty())
    {
        return Ok(None);
    }
    interrupt(PpmlInterruptPoint::SeparationSimplex)?;
    let mut constant = fallible_filled(
        columns.len(),
        false,
        "unable to reserve constant positive-support column flags",
    )?;
    for (i, c) in columns.iter().enumerate() {
        if matches!(c.kind, crate::ols::PreparedOlsColumnKind::Intercept) {
            continue;
        }
        let first = generation.fit_view().column_value(i, 0)?;
        constant[i] = first.is_finite();
        for row in 1..n {
            crate::interrupt::row_checkpoint(row)?;
            if generation.fit_view().column_value(i, row)? != first {
                constant[i] = false;
                break;
            }
        }
    }
    if constant.contains(&true) {
        let proof = anchored(
            generation,
            topology,
            positive,
            (limit - constant.len(), work),
            independent(&constant),
            interrupt,
        )?;
        return Ok(proof.map(|proof| no_separation(method, proof)));
    }
    Ok(None)
}

/// Retry an unresolved numerical proposal on an aliased positive design.
pub(super) fn aliased(
    method: SeparationMethod,
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    limit: usize,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<DetectionProposal>> {
    let work = primal::ProofBudget::new(generation, method, config)
        .work_cells
        .saturating_mul(64)
        .min(128_000_000);
    Ok(anchored(
        generation,
        topology,
        positive,
        (limit, work),
        RankMode::Packed { ray: None },
        interrupt,
    )?
    .map(|proof| no_separation(method, proof)))
}

pub(super) fn packed_candidate(
    generation: &PreparedPpmlModelGeneration,
    budget: primal::ProofBudget,
) -> Result<Option<primal::Witness>> {
    let topology = DetectorTopology::new(generation.prepared())?;
    let metadata =
        topology.sources.len() + topology.effect_specs.len() + topology.effect_to_source.len();
    let mut values = Vec::new();
    let work = budget.work_cells.saturating_mul(64).min(128_000_000);
    anchored(
        generation,
        &topology,
        generation.outcome().iter().filter(|&&y| y > 0.).count(),
        (budget.words.saturating_sub(metadata), work),
        RankMode::Packed {
            ray: Some(&mut values),
        },
        &mut |_| crate::interrupt::checkpoint(),
    )?;
    if values.is_empty() {
        return Ok(None);
    }
    let units = fallible_filled(values.len(), 1.0, "unable to reserve anchored primal units")?;
    Ok(Some(primal::Witness::Coefficients {
        values,
        units,
        absorbed: true,
    }))
}

fn no_separation(method: SeparationMethod, proof: SeparationCertificateProof) -> DetectionProposal {
    let note = proof
        .positive_support_note()
        .expect("positive-support proof");
    let mut result = DetectionProposal::empty(method, MethodStatus::NoSeparation, note);
    result.certificate = Some(crate::ppml_separation::no_separation_certificate(proof));
    result
}

fn codes(generation: &PreparedPpmlModelGeneration, source: usize) -> Result<&[u32]> {
    crate::fe::ValidatedBorrowedInterceptSource::from_finalized_source(
        &generation.fit_view().fe_topology,
        source,
    )
    .map(|source| source.effect().codes())
    .ok_or_else(|| invalid_argument("invalid finalized topology source for positive-support proof"))
}

enum RankMode<'a> {
    Independent {
        constant: &'a [bool],
        iterations: usize,
    },
    Packed {
        ray: Option<&'a mut Vec<i64>>,
    },
}

/// Absorb the largest categorical intercept by subtracting a positive row in
/// each of its levels. Other effects require only L-1 dummies, as their common
/// constants belong to the anchor intercepts. A nonzero modular minor is an
/// exact rational rank lower bound at the known maximum possible width.
fn anchored(
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    (limit, work): (usize, usize),
    mode: RankMode<'_>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<SeparationCertificateProof>> {
    if topology
        .effect_specs
        .iter()
        .any(|e| !e.intercept || !e.slopes.is_empty())
    {
        return Ok(None);
    }
    let effect_levels =
        |effect: usize| topology.sources[topology.effect_to_source[effect]].n_levels();
    let Some(anchor_effect) = (0..topology.effect_specs.len()).max_by_key(|&e| effect_levels(e))
    else {
        return Ok(None);
    };
    let levels = topology.sources[topology.effect_to_source[anchor_effect]];
    let constant = match mode {
        RankMode::Independent { constant, .. } => constant,
        RankMode::Packed { .. } => &[],
    };
    let independent = matches!(mode, RankMode::Independent { .. });
    let view = generation.fit_view();
    let columns = &view.model().columns;
    let slopes = || {
        columns.iter().enumerate().filter_map(|(i, c)| {
            (!matches!(c.kind, crate::ols::PreparedOlsColumnKind::Intercept)
                && !constant.get(i).copied().unwrap_or(false))
            .then_some(i)
        })
    };
    let slope_count = slopes().count();
    let other_effects = || (0..topology.effect_specs.len()).filter(|&e| e != anchor_effect);
    let Some(width) = other_effects().try_fold(slope_count, |sum, e| {
        sum.checked_add(effect_levels(e).checked_sub(1)?)
    }) else {
        return Ok(None);
    };
    let constant_finite = constant.iter().filter(|&&flag| flag).count();
    if (width == 0 && constant_finite == 0 && topology.effect_specs.len() == 1)
        || (independent && positive < levels.n_levels().saturating_add(width))
    {
        return Ok(None);
    }
    // (start, borrowed codes) is three machine words per additional effect.
    let other_cells = topology
        .effect_specs
        .len()
        .saturating_sub(1)
        .saturating_mul(3);
    let Some(basis_cells) = width.checked_mul(width) else {
        return Ok(None);
    };
    let required = basis_cells
        .saturating_add(width.saturating_mul(2))
        .saturating_add(other_cells);
    if other_cells > limit || (independent && required > limit) {
        return Ok(None);
    }
    interrupt(PpmlInterruptPoint::SeparationSimplex)?;
    let mut others = Vec::new();
    others
        .try_reserve_exact(topology.effect_specs.len() - 1)
        .map_err(|_| allocation("unable to reserve positive-rank effect views"))?;
    let mut start = slope_count;
    for e in other_effects() {
        others.push((start, codes(generation, topology.effect_to_source[e])?));
        start += effect_levels(e) - 1;
    }
    let write_row = |index, target: &mut [u64]| -> Result<()> {
        for (dest, column) in target.iter_mut().zip(slopes()) {
            let value = view.column_value(column, index)?;
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "nonfinite positive-support design coordinate",
                ));
            }
            *dest = residue(value);
        }
        target[slope_count..].fill(0);
        for &(start, codes) in &others {
            let level = codes[index] as usize;
            if level > 0 {
                target[start + level - 1] = 1;
            }
        }
        Ok(())
    };
    let spanned = match mode {
        RankMode::Packed { ray } => packed::AnchoredDesign {
            model: generation,
            levels,
            others: &others,
            anchor_effect,
            width,
        }
        .spans(slopes(), ray, (limit - other_cells, work), interrupt)?,
        RankMode::Independent { iterations, .. } => positive_contrasts_span(
            generation.outcome(),
            levels,
            width,
            width <= limit - required,
            (iterations, work),
            write_row,
            interrupt,
        )?,
    };
    if !spanned {
        return Ok(None);
    }
    let explicit_constant = columns.len() != slope_count + constant_finite;
    Ok(Some(
        if topology.effect_specs.len() == 1 && constant_finite == 0 {
            SeparationCertificateProof::SimplexWithinLevelPositiveRank {
                slopes: width,
                levels: levels.n_levels(),
                explicit_constant,
            }
        } else {
            // The report bank reserves a record per possible generation. Keep
            // proof metadata as compact as the preexisting two-size payload.
            let [Ok(within_columns), Ok(anchor_levels), Ok(effects), Ok(constant_finite)] = [
                width,
                levels.n_levels(),
                topology.effect_specs.len(),
                constant_finite,
            ]
            .map(u32::try_from) else {
                return Ok(None);
            };
            SeparationCertificateProof::SimplexAnchoredPositiveRank {
                within_columns,
                anchor_levels,
                effects,
                constant_finite,
                explicit_constant,
            }
        },
    ))
}

/// Replay the admitted row order. Even after finding the final pivot the
/// remaining anchor levels must each contain a positive observation.
fn positive_contrasts_span(
    outcome: &[f64],
    levels: &LevelIndex,
    width: usize,
    cache_previous: bool,
    (iterations, work): (usize, usize),
    write_row: impl Fn(usize, &mut [u64]) -> Result<()>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<bool> {
    // The caller checked this square against overflow and the live cell cap.
    let basis_cells = width * width;
    let mut basis = fallible_filled(
        basis_cells,
        0u64,
        "unable to reserve positive-row rank basis",
    )?;
    let mut anchor = fallible_filled(width, 0u64, "unable to reserve positive-row anchor")?;
    let mut row = fallible_filled(width, 0u64, "unable to reserve positive-row rank work row")?;
    // Adjacent groups often repeat the same within contrast. Skipping an
    // identical modular row cannot change modular rank or consume a pivot
    // attempt. The cache is optional under a tighter workspace cap.
    let mut previous = if cache_previous {
        fallible_filled(
            width,
            0u64,
            "unable to reserve previous positive-row contrast",
        )?
    } else {
        Vec::new()
    };
    let mut rank = 0;
    let mut contrasts = iterations.min(work / basis_cells.max(1));
    let mut reads = work / width.max(1);
    for level in 0..levels.n_levels() {
        interrupt(PpmlInterruptPoint::SeparationSimplex)?;
        let mut positives = levels
            .checked_members(level)?
            .iter()
            .enumerate()
            .filter_map(
                |(position, &index)| match crate::interrupt::row_checkpoint(position) {
                    Err(error) => Some(Err(error)),
                    Ok(()) => (outcome[index] > 0.0).then_some(Ok(index)),
                },
            );
        let Some(first) = positives.next().transpose()? else {
            return Ok(false);
        };
        if rank == width {
            continue;
        }
        let mut anchor_ready = false;
        for index in positives {
            let index = index?;
            if !anchor_ready {
                write_row(first, &mut anchor)?;
                anchor_ready = true;
            }
            if reads == 0 {
                return Ok(false);
            }
            reads -= 1;
            write_row(index, &mut row)?;
            if row == anchor {
                continue;
            }
            for (entry, &origin) in row.iter_mut().zip(&anchor) {
                *entry = (*entry + PRIME - origin) % PRIME;
            }
            if row == previous {
                continue;
            }
            if contrasts == 0 {
                return Ok(false);
            }
            contrasts -= 1;
            if !previous.is_empty() {
                previous.copy_from_slice(&row);
            }
            rank += usize::from(insert(&mut basis, &mut row));
            if rank == width {
                break;
            }
        }
    }
    Ok(rank == width)
}
