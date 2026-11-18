//! Provisional full-model starts for offsets and a failed frozen-rank path.
//!
//! Project log(y) (the weighted mean for zero outcomes) minus the original
//! offset onto [X,D,1]. Add the original offset back, including its entire
//! out-of-span component. This is only a starting state: the temporary
//! session, its rank, and its bread are discarded before IRLS is installed.
use super::*;

type Seed = (Vec<f64>, Vec<f64>);
pub(super) const RESTART_SEED_REFUSED: &str = "PPML alternate projected start did not certify";

pub(super) fn propose<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    plan: &mut PpmlMemoryPlan,
) -> Result<Option<Seed>> {
    if offset.iter().all(|value| *value == 0.0) {
        return Ok(None);
    }
    admitted_projection(prepared, outcome, weights, offset, config, plan)
}

pub(super) fn restart<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    plan: &mut PpmlMemoryPlan,
) -> Result<Seed> {
    admitted_projection(prepared, outcome, weights, offset, config, plan)?
        .ok_or_else(|| invalid_data(RESTART_SEED_REFUSED))
}

fn admitted_projection<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    mut config: PreparedPpmlIrlsConfig,
    plan: &mut PpmlMemoryPlan,
) -> Result<Option<Seed>> {
    // A complete temporary linear session uses the selected Fast/Low route
    // and its already admitted linear/vector workspace. It runs before the
    // nine-vector IRLS workspace exists. Only its second topology overlaps
    // the topology waiting for the final session; charge that conservatively
    // for the whole fit, before constructing it. A tight budget simply keeps
    // the established seed policy.
    let topology_bytes = plan.selected_inventory().fe_solver_topology_bytes;
    let Some(bytes) = plan.solver_workspace_bytes.checked_add(topology_bytes) else {
        return Ok(None);
    };
    let admitted = match plan.with_solver_workspace(bytes) {
        Ok(admitted) => admitted,
        Err(error) if error.code == ErrorCode::Allocation => return Ok(None),
        Err(error) => return Err(error),
    };
    let Some(budget) = seed_solver_budget(config, *plan, topology_bytes) else {
        return Ok(None);
    };
    // The execution-overlap budget is distinct from the fit peak. Dynamic
    // FE workspace must not reuse headroom consumed by the second topology.
    // Rebind only this temporary config; final IRLS keeps its original one.
    config.ppml.absorb.solver_memory_budget = Some(budget);
    config.linear.absorb.solver_memory_budget = Some(budget);
    // Keep the reservation even if a numerical proposal declines: its peak
    // was reached. No optional proposal is allowed to swallow Break.
    *plan = admitted;
    match project(prepared, outcome, weights, offset, config) {
        Ok(seed) => Ok(seed),
        Err(error)
            if matches!(
                error.code,
                ErrorCode::NonConvergence | ErrorCode::InvalidData | ErrorCode::Allocation
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn seed_solver_budget(
    config: PreparedPpmlIrlsConfig,
    plan: PpmlMemoryPlan,
    topology_bytes: u128,
) -> Option<FeSolverMemoryBudget> {
    let original = config
        .ppml
        .absorb
        .solver_memory_budget
        .unwrap_or(FeSolverMemoryBudget {
            execution_bytes: plan.solver_workspace_peak(),
            usable_bytes: plan.usable_bytes.unwrap_or(u128::MAX),
        });
    let budget = FeSolverMemoryBudget {
        execution_bytes: original.execution_bytes.checked_add(topology_bytes)?,
        usable_bytes: original
            .usable_bytes
            .min(plan.usable_bytes.unwrap_or(u128::MAX)),
    };
    budget.fits(0).then_some(budget)
}

fn project<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
) -> Result<Option<Seed>> {
    crate::interrupt::checkpoint()?;
    let zero_proxy = log_weighted_outcome_sum(outcome, weights)?;
    // The weighted mean is a positive, scale-equivariant zero-row proxy;
    // zero observations in the likelihood itself remain exactly zero.
    let weight_max = weights.iter().copied().fold(0.0_f64, f64::max);
    let mut weight_sum = CompensatedSum::default();
    for &weight in weights {
        weight_sum.add(weight / weight_max);
    }
    let zero_proxy = zero_proxy - weight_max.ln() - weight_sum.total().ln();
    let mut response = fallible_filled(
        outcome.len(),
        0.0,
        "unable to reserve offset start response",
    )?;
    for row in 0..outcome.len() {
        if row % SEED_ROWS == 0 {
            crate::interrupt::checkpoint()?;
        }
        let proxy = if outcome[row] > 0.0 {
            outcome[row].ln()
        } else {
            zero_proxy
        };
        response[row] = proxy - offset[row];
        if !response[row].is_finite() {
            return Ok(None);
        }
    }
    let mut linear = ppml_linear_config(config, strict_inner_tolerance(config.ppml))?;
    // The seed needs the ordinary certified projection, with no optional
    // inference/caching products. In Low mode its finite rows remain on disk.
    linear.execution.mode = config.linear.execution.mode;
    let frame = LinearSolveFrame::from_parts(
        &response,
        BorrowedMapWeights::Provided(weights),
        ppml_bread_weight_scale(weights)?,
    )?;
    let design = prepared.fit_owned_fe_design()?;
    let mut session = CertifiedLinearSession::new_with_fe_design(
        prepared,
        frame,
        linear,
        LinearSessionGenerations::new(0, 0, 0, 0, 0),
        design,
    )?;
    let output = session.solve_fresh()?;
    if !output.certified() {
        return Ok(None);
    }
    let mut eta = fallible_filled(
        outcome.len(),
        0.0,
        "unable to reserve offset start predictor",
    )?;
    fill_candidate_eta_from_linear_output(&output, offset, &mut eta)?;
    if eta
        .iter()
        .any(|value| !usable_seed_value(*value, config.ppml))
    {
        return Ok(None);
    }
    let mut coefficients = fallible_filled(
        prepared.fit_model().coefficients.len(),
        0.0,
        "unable to reserve offset start coefficients",
    )?;
    fill_candidate_coefficients_from_linear_output(&output, &mut coefficients)?;
    crate::interrupt::checkpoint()?;
    Ok(Some((coefficients, eta)))
}
