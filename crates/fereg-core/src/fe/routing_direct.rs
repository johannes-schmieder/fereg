//! Optional, current-weight two-FE direct proposal; never a rank authority.
//!
//! Numerical scope: only an admitted direct candidate changes arithmetic. The
//! caller must certify its original-weight residuals before publication. A
//! declined candidate leaves the incumbent, sample and omission decisions alone.
//! Work is a conservative scalar arithmetic/access charge, not elapsed time.

use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, Result};
use crate::memory::{fallible_filled, FeSolverMemoryBudget};

use super::{BorrowedInterceptFeDesign, BorrowedMapWeights, ComponentInfo};

/// Once-per-session pure portfolio selection, charged by the caller even when
/// direct is declined. See `portfolio_preflight` for its operation envelope.
pub(crate) const PORTFOLIO_PREFLIGHT_WORK: u64 = 1024;

#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectLimits {
    /// Packed ordinary Schur iteration at current RHS width, independent of
    /// retained inactive banks and the physical session setup reference W0.
    pub reference_iteration_work: u64,
    /// Already charged pure preflight; counts toward 4W, never returned as work.
    pub preflight_work: u64,
    /// Remaining optional work AFTER charging `preflight_work`.
    pub remaining_setup_work: u64,
    /// Previously incurred census work; normally zero for `attempt_with_census`.
    pub component_census_work: u64,
    /// Full independent certificate, including any fresh norm scans.
    pub certificate_work: u64,
    pub certificate_bytes: u128,
    /// Other live solver storage; execution storage is in `memory` already.
    pub retained_live_bytes: u128,
    pub memory: Option<FeSolverMemoryBudget>,
}

impl DirectLimits {
    /// Common certificate credit never funds optional prefixes. The same
    /// immutable original-design certificate appears on both alternatives,
    /// while a failed direct certificate remains part of spent setup work.
    fn admits_work(self, noncertificate: u64) -> bool {
        self.reference_iteration_work
            .checked_mul(4)
            .is_some_and(|cap| {
                self.preflight_work
                    .checked_add(noncertificate)
                    .is_some_and(|total| total <= cap)
            })
            && noncertificate
                .checked_add(self.certificate_work)
                .is_some_and(|total| total <= self.remaining_setup_work)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectDecline {
    Work,
    Memory,
    Geometry,
    Numerical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DirectProfile {
    pub eliminated_effect: usize,
    pub dense_elements: usize,
    /// Census through output, excluding preflight and the common certificate.
    pub noncertificate_work: u64,
    /// Both profiles through caller certification, excluding spent preflight.
    pub work_bound: u64,
    /// Additional peak, including output and caller certification scratch.
    pub peak_bytes: u128,
}

#[derive(Debug)]
pub(crate) struct DirectCandidate {
    pub residualized: DenseMatrix,
    /// RHS-major, then original effect order; one intercept slot per level.
    pub coefficients: Vec<f64>,
    pub eliminated_effect: usize,
}

#[derive(Debug)]
pub(crate) struct DirectAttempt {
    pub profiles: [Option<DirectProfile>; 2],
    pub candidate: Option<DirectCandidate>,
    pub decline: Option<DirectDecline>,
    /// Conservative work charged, excluding the already charged preflight.
    /// Numerical failure may leave reserved work unused.
    pub work_used: u64,
    /// Additional storage admitted; other live storage is not included here.
    pub peak_bytes: u128,
}

impl DirectAttempt {
    fn decline(&mut self, reason: DirectDecline) {
        self.decline = Some(reason);
    }
}

fn add(a: u64, b: u64) -> Option<u64> {
    a.checked_add(b)
}

fn product(a: usize, b: usize) -> Option<usize> {
    a.checked_mul(b)
}

fn work(n: usize, unit: u64) -> Option<u64> {
    u64::try_from(n).ok()?.checked_mul(unit)
}

/// Fresh fixed-leaf norm scan in the same scalar-work currency as incumbent W0.
/// Includes scaled update, root-weight validation, leaf merges and final checks.
pub(crate) fn fresh_norm_work(rows: usize, rhs: usize) -> Option<u64> {
    work(product(rows, rhs)?, 24)?
        .checked_add(work(rows, 16)?)?
        .checked_add(work(rhs, 128)?)?
        .checked_add(work(product(rows.div_ceil(512), rhs)?, 64)?)
}

/// Full original-design certificate, including a fresh residual norm scan.
/// Per effect, reserve the shifted compensated pass AND the possible scaled
/// two-pass fallback: 64 row/RHS units plus 48 shared row units. The level
/// allowance covers initialization, representability checks and certificate
/// folds. This deliberately does not assume that small residuals skip fallback.
pub(crate) fn original_certificate_work(
    rows: usize,
    total_levels: usize,
    effects: usize,
    rhs: usize,
) -> Option<u64> {
    fresh_norm_work(rows, rhs)?
        .checked_add(work(product(product(rows, rhs)?, effects)?, 64)?)?
        .checked_add(work(product(rows, effects)?, 48)?)?
        .checked_add(work(product(total_levels, rhs)?, 128)?)
}

fn geometry_row_work(weights: BorrowedMapWeights<'_>) -> u64 {
    match weights {
        BorrowedMapWeights::Unit => 18,
        BorrowedMapWeights::Provided(_) => 34,
    }
}

fn rhs_row_work(weights: BorrowedMapWeights<'_>) -> u64 {
    match weights {
        // Four passes use at most 30 scalar steps per row. Reserve another
        // six for output initialization, the final finite scan and guards.
        // The first three passes read a normalized weight when provided:
        // weight/code/component/scale reads plus one division, at most 15.
        BorrowedMapWeights::Unit => 36,
        BorrowedMapWeights::Provided(_) => 52,
    }
}

/// Direct-only caller preparation is not part of common certificate credit:
/// optional retained-coefficient validation reads/checks every coefficient,
/// and scratch-bank loans plus two-effect layout construction have fixed cost.
fn output_preparation_work(levels: usize, rhs: usize) -> Option<u64> {
    work(product(levels, rhs)?, 2)?.checked_add(128)
}

fn fits(limits: DirectLimits, bytes: u128) -> bool {
    limits
        .retained_live_bytes
        .checked_add(bytes)
        .is_some_and(|total| limits.memory.is_none_or(|budget| budget.fits(total)))
}

/// Sufficient admission before any census, membership scan or allocation.
/// Weights must already satisfy the caller's positive/finite validation. Only
/// their mode is inspected. Validated designs already reject empty declared
/// levels. No numerical success or rank conclusion follows from this bound.
///
/// Scalar-work audit: the two fixed orientation evaluations use fewer than
/// 160 checked arithmetic/conversion operations, including their work/memory
/// gates. Packed Wref, the existing certificate bound and caller ledger/state
/// bookkeeping have a further 96-operation allowance. Each checked operation
/// costs at most arithmetic plus its overflow predicate (512 total); reserve
/// 256 more reads/writes/comparisons and 256 entry/exit/guard units. No scan or
/// allocation is hidden in this 1,024-unit envelope. The caller charges it once
/// and passes the post-charge remainder. Returned profile work excludes it.
pub(crate) fn portfolio_preflight(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    rhs: usize,
    limits: DirectLimits,
) -> Option<DirectProfile> {
    if design.effects().len() != 2
        || design.nobs() == 0
        || rhs == 0
        || limits.preflight_work != PORTFOLIO_PREFLIGHT_WORK
        || limits.component_census_work != 0
    {
        return None;
    }
    let p = design.effects()[0].n_levels();
    let q = design.effects()[1].n_levels();
    // Evaluate exactly both orientations; neither evaluation inspects data.
    [
        portfolio_bound(design.nobs(), p, q, rhs, weights, 0, limits),
        portfolio_bound(design.nobs(), q, p, rhs, weights, 1, limits),
    ]
    .into_iter()
    .flatten()
    .filter(|profile| {
        limits.admits_work(profile.noncertificate_work) && fits(limits, profile.peak_bytes)
    })
    .min_by_key(|profile| (profile.work_bound, profile.eliminated_effect))
}

#[allow(clippy::too_many_arguments)]
fn portfolio_bound(
    n: usize,
    p: usize,
    q: usize,
    rhs: usize,
    weights: BorrowedMapWeights<'_>,
    eliminated_effect: usize,
    limits: DirectLimits,
) -> Option<DirectProfile> {
    if n == 0 || p == 0 || q == 0 || rhs == 0 {
        return None;
    }
    let levels = p.checked_add(q)?;
    let components = p.min(q);
    let z = q.checked_sub(1)?;
    let cells = product(z, z)?;
    let level_rhs = product(levels, rhs)?;
    let row_rhs = product(n, rhs)?;
    // For actual component sizes q_i, sum(q_i-1)^2 <= (q-1)^2,
    // sum(q_i-1)^2*q_i <= (q-1)^2*q. For eliminated touches d_g,
    // d_g<=q and sum d_g<=n, so both clique-pair bounds are valid.
    let pairs = (product(product(p, q)?, z)? / 2).min(product(n, z)? / 2);
    // Census 8n+48L; BOTH profiles/validation 5n+24L+64c; then
    // existing geometry, RHS, level/output, factor/solve and clique tariffs.
    let operations = work(n, 13 + geometry_row_work(weights))?
        .checked_add(work(row_rhs, rhs_row_work(weights))?)?
        .checked_add(work(levels, 72)?)?
        .checked_add(work(components, 64)?)?
        .checked_add(work(level_rhs, 18)?)?
        .checked_add(128)?
        .checked_add(work(product(cells, q.checked_add(rhs)?)?, 32)?)?
        .checked_add(work(pairs, 48)?)?;
    let indices = components
        .checked_mul(5)?
        .checked_add(1)?
        .checked_add(q.checked_mul(2)?)?;
    let floats = cells
        .checked_add(p)?
        .checked_add(q.checked_mul(2)?)?
        .checked_add(components)?
        .checked_add(level_rhs)?
        .checked_add(row_rhs)?;
    let labels = (levels as u128)
        .checked_mul(std::mem::size_of::<usize>() as u128)?
        .checked_add(2 * std::mem::size_of::<Vec<usize>>() as u128)?;
    let census_peak = labels
        .checked_add((levels as u128).checked_mul(std::mem::size_of::<usize>() as u128)?)?
        .checked_add(1024)?;
    let profile_peak = labels
        .checked_add((indices as u128).checked_mul(std::mem::size_of::<usize>() as u128)?)?
        .checked_add((floats as u128).checked_mul(8)?)?
        .checked_add(limits.certificate_bytes)?
        .checked_add(1024)?;
    Some(DirectProfile {
        eliminated_effect,
        dense_elements: cells,
        noncertificate_work: operations,
        work_bound: operations.checked_add(limits.certificate_work)?,
        peak_bytes: census_peak.max(profile_peak),
    })
}

/// Own the bounded topology census when the caller has no retained components.
/// Breadth-first traversal reuses validated level membership: exactly two visits
/// per observation and at most one queue entry per level. Labels follow the
/// existing component routine's effect/level ordering; no numerical rank test.
pub(crate) fn attempt_with_census(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    rhs: &DenseMatrix,
    mut limits: DirectLimits,
) -> Result<DirectAttempt> {
    if design.effects().len() != 2 || rhs.nrows() != design.nobs() || rhs.ncols() == 0 {
        return Err(invalid_argument(
            "direct proposal requires two effects and aligned RHS",
        ));
    }
    let mut declined = DirectAttempt {
        profiles: [None, None],
        candidate: None,
        decline: Some(DirectDecline::Work),
        work_used: limits.component_census_work,
        peak_bytes: 0,
    };
    let (p, q) = (
        design.effects()[0].n_levels(),
        design.effects()[1].n_levels(),
    );
    let Some(levels) = p.checked_add(q) else {
        return Ok(declined);
    };
    // Each row is visited from both endpoints: membership read, other code
    // read, label read and comparison (8n). Queue/label bookkeeping is 48L.
    let census_work = work(design.nobs(), 8)
        .and_then(|v| add(v, work(levels, 48)?))
        .and_then(|v| add(v, limits.component_census_work));
    // A necessary lower bound before any topology allocation. Component sizes
    // are deliberately unknown here; no connectedness assumption is made.
    let minimum = census_work
        .and_then(|v| add(v, work(design.nobs(), geometry_row_work(weights))?))
        .and_then(|v| {
            add(
                v,
                work(product(design.nobs(), rhs.ncols())?, rhs_row_work(weights))?,
            )
        })
        .and_then(|v| add(v, output_preparation_work(levels, rhs.ncols())?));
    let (Some(census_work), Some(minimum)) = (census_work, minimum) else {
        return Ok(declined);
    };
    if !limits.admits_work(minimum) {
        return Ok(declined);
    }
    let retained_bytes = levels as u128 * std::mem::size_of::<usize>() as u128
        + 2 * std::mem::size_of::<Vec<usize>>() as u128;
    let census_peak = retained_bytes + levels as u128 * std::mem::size_of::<usize>() as u128 + 1024;
    if !fits(limits, census_peak) {
        declined.decline(DirectDecline::Memory);
        return Ok(declined);
    }
    declined.work_used = census_work;
    declined.peak_bytes = census_peak;
    let components = census(design, levels)?;
    if components
        .component_by_effect_level
        .iter()
        .flatten()
        .any(|&v| v == usize::MAX)
    {
        declined.decline(DirectDecline::Geometry);
        return Ok(declined);
    }
    limits.component_census_work = census_work;
    limits.retained_live_bytes = limits
        .retained_live_bytes
        .checked_add(retained_bytes)
        .ok_or_else(|| allocation("direct census live storage overflows"))?;
    let mut result = attempt(design, weights, &components, rhs, limits)?;
    result.peak_bytes = result
        .peak_bytes
        .checked_add(retained_bytes)
        .ok_or_else(|| allocation("direct census peak storage overflows"))?
        .max(census_peak);
    for profile in result.profiles.iter_mut().flatten() {
        profile.peak_bytes += retained_bytes;
    }
    Ok(result)
}

fn census(design: &BorrowedInterceptFeDesign<'_>, levels: usize) -> Result<ComponentInfo> {
    let p = design.effects()[0].n_levels();
    let mut labels = reserved(2, Vec::<usize>::new())?;
    labels[0] = reserved(p, usize::MAX)?;
    labels[1] = reserved(design.effects()[1].n_levels(), usize::MAX)?;
    let mut queue = reserved(levels, 0_usize)?;
    let mut n_components = 0;
    for node in 0..levels {
        checkpoint(node)?;
        let effect = usize::from(node >= p);
        let level = if effect == 0 { node } else { node - p };
        if labels[effect][level] != usize::MAX
            || design.effects()[effect]
                .level_index()
                .members(level)
                .is_empty()
        {
            continue;
        }
        labels[effect][level] = n_components;
        queue[0] = node;
        let (mut head, mut tail) = (0, 1);
        while head < tail {
            checkpoint(head)?;
            let current = queue[head];
            head += 1;
            let side = usize::from(current >= p);
            let other = 1 - side;
            let level = if side == 0 { current } else { current - p };
            for (position, &row) in design.effects()[side]
                .level_index()
                .members(level)
                .iter()
                .enumerate()
            {
                checkpoint(position)?;
                let neighbor = design.effects()[other].codes()[row] as usize;
                if labels[other][neighbor] == usize::MAX {
                    labels[other][neighbor] = n_components;
                    queue[tail] = neighbor + if other == 0 { 0 } else { p };
                    tail += 1;
                }
            }
        }
        n_components += 1;
    }
    Ok(ComponentInfo {
        n_components,
        component_by_effect_level: labels,
    })
}

#[allow(clippy::too_many_arguments)]
fn profile(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    components: &ComponentInfo,
    counts: &[usize],
    eliminated: usize,
    rhs: usize,
    common_work: u64,
    limits: DirectLimits,
) -> Option<DirectProfile> {
    let n = design.nobs();
    let p = design.effects()[eliminated].n_levels();
    let q = design.effects()[1 - eliminated].n_levels();
    let c = components.n_components;
    let kept_counts = &counts[(1 - eliminated) * c..(2 - eliminated) * c];
    let mut cells = 0_usize;
    let mut operations = common_work;
    // Same currency as incumbent W0: scalar arithmetic/predicate or indexed
    // read/write = 1, FMA = 1; address arithmetic and loop mechanics excluded.
    // Geometry bounds aggregation/reset plus a max-weight scan for provided
    // weights. RHS bounds two anchored assembly passes, one recovery pass,
    // reconstruction, output initialization and finite checks.
    operations = add(operations, work(n, geometry_row_work(weights))?)?;
    operations = add(operations, work(product(n, rhs)?, rhs_row_work(weights))?)?;
    operations = add(operations, work(product(p.checked_add(q)?, rhs)?, 16)?)?;
    operations = add(operations, output_preparation_work(p.checked_add(q)?, rhs)?)?;
    for &count in kept_counts {
        let r = count.checked_sub(1)?;
        let square = product(r, r)?;
        cells = cells.checked_add(square)?;
        // Fixed upper bounds for equilibration, Cholesky and both triangular
        // solves; sqrt/division and scalar accesses are included.
        operations = add(operations, work(product(square, r.checked_add(1)?)?, 32)?)?;
        operations = add(operations, work(product(square, rhs)?, 32)?)?;
    }
    for g in 0..p {
        let component = components.component_by_effect_level[eliminated][g];
        let d = design.effects()[eliminated]
            .level_index()
            .members(g)
            .len()
            .min(kept_counts[component]);
        let pairs = product(d, d.saturating_sub(1))? / 2;
        operations = add(operations, work(pairs, 48)?)?;
    }
    // Both orientation counts remain live, together with selected component
    // offsets/anchors/counters, local indices and touched-cell scratch.
    let indices = c
        .checked_mul(5)?
        .checked_add(1)?
        .checked_add(q.checked_mul(2)?)?;
    let floats = cells
        .checked_add(p)?
        .checked_add(q.checked_mul(2)?)?
        .checked_add(c)?
        .checked_add(product(p.checked_add(q)?, rhs)?)?
        .checked_add(product(n, rhs)?)?;
    let bytes = (indices as u128)
        .checked_mul(std::mem::size_of::<usize>() as u128)?
        .checked_add((floats as u128).checked_mul(8)?)?
        .checked_add(limits.certificate_bytes)?
        .checked_add(1024)?;
    Some(DirectProfile {
        eliminated_effect: eliminated,
        dense_elements: cells,
        noncertificate_work: operations,
        work_bound: operations.checked_add(limits.certificate_work)?,
        peak_bytes: bytes,
    })
}

/// Try exactly one admitted orientation. The component labels must come from
/// the existing final-sample topology. Neither weights nor RHS are mutated.
/// The caller performs the reserved independent certificate even after a
/// successful return; this helper never labels a candidate as certified.
pub(crate) fn attempt(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    components: &ComponentInfo,
    rhs: &DenseMatrix,
    limits: DirectLimits,
) -> Result<DirectAttempt> {
    let mut result = DirectAttempt {
        profiles: [None, None],
        candidate: None,
        decline: None,
        work_used: limits.component_census_work,
        peak_bytes: 0,
    };
    if design.effects().len() != 2 || rhs.nrows() != design.nobs() || rhs.ncols() == 0 {
        return Err(invalid_argument(
            "direct proposal requires two effects and aligned RHS",
        ));
    }
    let levels = [
        design.effects()[0].n_levels(),
        design.effects()[1].n_levels(),
    ];
    let c = components.n_components;
    if components.component_by_effect_level.len() != 2
        || (0..2).any(|j| components.component_by_effect_level[j].len() != levels[j])
    {
        return Err(invalid_argument(
            "direct proposal component layout is inconsistent",
        ));
    }
    if let BorrowedMapWeights::Provided(w) = weights {
        if w.len() != design.nobs() {
            return Err(invalid_argument(
                "direct proposal weight count is inconsistent",
            ));
        }
    }
    // Per row: two code reads, two component reads and one comparison.
    // Per level: label validation/counts (five steps), the orientation's
    // degree-bound profile (at most fifteen), and four bookkeeping steps.
    // Each component reserves both dense-block profiles and their guards.
    let Some(common_work) = work(design.nobs(), 5)
        .and_then(|v| add(v, work(levels[0].checked_add(levels[1])?, 24)?))
        .and_then(|v| add(v, work(c, 64)?))
        .and_then(|v| v.checked_add(result.work_used))
    else {
        result.decline(DirectDecline::Work);
        return Ok(result);
    };
    if !limits.admits_work(common_work) {
        result.decline(DirectDecline::Work);
        return Ok(result);
    }
    let Some(count_len) = c.checked_mul(2) else {
        result.decline(DirectDecline::Memory);
        return Ok(result);
    };
    let count_bytes = count_len as u128 * std::mem::size_of::<usize>() as u128 + 1024;
    if !fits(limits, count_bytes) {
        result.decline(DirectDecline::Memory);
        return Ok(result);
    }
    result.work_used = common_work;
    result.peak_bytes = count_bytes;
    let mut counts = reserved(count_len, 0_usize)?;
    for effect in 0..2 {
        for &component in &components.component_by_effect_level[effect] {
            if component >= c {
                return Err(invalid_argument(
                    "direct proposal component label is out of range",
                ));
            }
            counts[effect * c + component] += 1;
        }
    }
    if design.nobs() == 0 || counts.contains(&0) {
        result.decline(DirectDecline::Geometry);
        return Ok(result);
    }
    // Validate the supplied cross-effect partition without deriving new ranks.
    for row in 0..design.nobs() {
        if row % 4096 == 0 {
            crate::interrupt::checkpoint()?;
        }
        if components.component_by_effect_level[0][design.effects()[0].codes()[row] as usize]
            != components.component_by_effect_level[1][design.effects()[1].codes()[row] as usize]
        {
            return Err(invalid_argument(
                "direct proposal component labels split an observation",
            ));
        }
    }
    for effect in 0..2 {
        result.profiles[effect] = profile(
            design,
            weights,
            components,
            &counts,
            effect,
            rhs.ncols(),
            common_work,
            limits,
        );
    }
    let chosen = result
        .profiles
        .iter()
        .flatten()
        .copied()
        .filter(|p| limits.admits_work(p.noncertificate_work) && fits(limits, p.peak_bytes))
        .min_by_key(|p| (p.work_bound, p.eliminated_effect));
    let Some(chosen) = chosen else {
        result.decline(
            if result
                .profiles
                .iter()
                .flatten()
                .any(|p| limits.admits_work(p.noncertificate_work))
            {
                DirectDecline::Memory
            } else {
                DirectDecline::Work
            },
        );
        return Ok(result);
    };
    // Reserve the entire deterministic attempt before numerical work. This also
    // charges numerical failures without making wall-clock progress authoritative.
    result.work_used = chosen.work_bound;
    result.peak_bytes = chosen.peak_bytes;
    result.candidate = solve(design, weights, components, rhs, &counts, chosen)?;
    if result.candidate.is_none() {
        result.decline(DirectDecline::Numerical);
    }
    Ok(result)
}

fn reserved<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let values = fallible_filled(n, value, "unable to reserve admitted direct proposal")?;
    if values.capacity() != n {
        return Err(allocation(
            "direct proposal allocation exceeds admitted capacity",
        ));
    }
    Ok(values)
}

fn checkpoint(position: usize) -> Result<()> {
    if position % 4096 == 0 {
        crate::interrupt::checkpoint()?;
    }
    Ok(())
}

/// Borrowed component grounding, shared by assembly, factorization and solves.
struct GroundedLayout<'a> {
    component: &'a [usize],
    sizes: &'a [usize],
    offsets: &'a [usize],
    anchors: &'a [usize],
    local: &'a [usize],
}

/// The already admitted numeric and index banks, with no additional allocation.
struct DirectFactors<'a> {
    layout: GroundedLayout<'a>,
    matrix: &'a [f64],
    roots: &'a [f64],
    mass: &'a [f64],
    next: &'a [usize],
    touched: &'a [usize],
}

fn solve(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    components: &ComponentInfo,
    rhs: &DenseMatrix,
    counts: &[usize],
    profile: DirectProfile,
) -> Result<Option<DirectCandidate>> {
    let eliminated = profile.eliminated_effect;
    let kept = 1 - eliminated;
    let a = design.effects()[eliminated];
    let b = design.effects()[kept];
    let (p, q, c) = (a.n_levels(), b.n_levels(), components.n_components);
    let component = &components.component_by_effect_level[kept];
    let sizes = &counts[kept * c..(kept + 1) * c];
    let mut offsets = reserved(c + 1, 0_usize)?;
    let mut anchors = reserved(c, usize::MAX)?;
    let mut next = reserved(c, 0_usize)?;
    let mut local = reserved(q, usize::MAX)?;
    for k in 0..c {
        offsets[k + 1] = offsets[k] + (sizes[k] - 1) * (sizes[k] - 1);
    }
    for h in 0..q {
        let k = component[h];
        if anchors[k] == usize::MAX {
            anchors[k] = h;
        } else {
            local[h] = next[k];
            next[k] += 1;
        }
    }
    let unit = matches!(weights, BorrowedMapWeights::Unit);
    let mut scales = reserved(c, if unit { 1_f64 } else { 0_f64 })?;
    let raw_weight = |i| match weights {
        BorrowedMapWeights::Unit => 1.0,
        BorrowedMapWeights::Provided(w) => w[i],
    };
    if !unit {
        for row in 0..design.nobs() {
            checkpoint(row)?;
            let w = raw_weight(row);
            if !w.is_finite() || w <= 0.0 {
                return Ok(None);
            }
            let k = component[b.codes()[row] as usize];
            scales[k] = scales[k].max(w);
        }
    }
    let weight = |i| {
        if unit {
            1.0
        } else {
            raw_weight(i) / scales[component[b.codes()[i] as usize]]
        }
    };
    let mut mass = reserved(p, 0_f64)?;
    let mut totals = reserved(q, 0_f64)?;
    let mut touched = reserved(q, 0_usize)?;
    let mut matrix = reserved(profile.dense_elements, 0_f64)?;
    for (g, group_mass) in mass.iter_mut().enumerate() {
        crate::interrupt::checkpoint()?;
        let mut count = 0;
        for (position, &row) in a.level_index().members(g).iter().enumerate() {
            checkpoint(position)?;
            let w = weight(row);
            if !unit && (!w.is_finite() || w <= 0.0) {
                return Ok(None);
            }
            let h = b.codes()[row] as usize;
            if totals[h] == 0.0 {
                touched[count] = h;
                count += 1;
            }
            totals[h] += w;
            *group_mass += w;
        }
        if !group_mass.is_finite() || *group_mass <= 0.0 {
            return Ok(None);
        }
        for i in 0..count {
            checkpoint(i)?;
            let h = touched[i];
            let k = component[h];
            let r = sizes[k] - 1;
            for &j in &touched[i + 1..count] {
                let edge = (totals[h].max(totals[j]) / *group_mass) * totals[h].min(totals[j]);
                if !edge.is_finite() || edge <= 0.0 {
                    return Ok(None);
                }
                let (u, v) = (local[h], local[j]);
                let block = offsets[k];
                if u != usize::MAX {
                    matrix[block + u * r + u] += edge;
                }
                if v != usize::MAX {
                    matrix[block + v * r + v] += edge;
                }
                if u != usize::MAX && v != usize::MAX {
                    matrix[block + u * r + v] -= edge;
                    matrix[block + v * r + u] -= edge;
                }
            }
        }
        for &h in &touched[..count] {
            totals[h] = 0.0;
        }
    }
    let layout = GroundedLayout {
        component,
        sizes,
        offsets: &offsets,
        anchors: &anchors,
        local: &local,
    };
    let Some(roots) = factor_components(&layout, &mut matrix, &mut next, &mut touched)? else {
        return Ok(None);
    };
    let factors = DirectFactors {
        layout,
        matrix: &matrix,
        roots: &roots,
        mass: &mass,
        next: &next,
        touched: &touched,
    };
    solve_rhs(design, rhs, eliminated, weight, &factors)
}

/// Equilibrate and factor each grounded component in its original loop order.
fn factor_components(
    layout: &GroundedLayout<'_>,
    matrix: &mut [f64],
    next: &mut [usize],
    touched: &mut [usize],
) -> Result<Option<Vec<f64>>> {
    let (component, sizes, offsets, local) =
        (layout.component, layout.sizes, layout.offsets, layout.local);
    let (q, c) = (component.len(), sizes.len());
    let mut roots = reserved(q, 1_f64)?;
    for h in 0..q {
        if local[h] != usize::MAX {
            let r = sizes[component[h]] - 1;
            let diagonal = matrix[offsets[component[h]] + local[h] * r + local[h]];
            if !diagonal.is_finite() || diagonal <= 0.0 {
                return Ok(None);
            }
            roots[h] = diagonal.sqrt();
        }
    }
    // Components need not occupy contiguous level IDs. Use an explicit packed
    // level map in touched, indexed by component prefix and local index.
    let mut prefix = 0;
    for k in 0..c {
        next[k] = prefix;
        prefix += sizes[k] - 1;
    }
    for h in 0..q {
        if local[h] != usize::MAX {
            touched[next[component[h]] + local[h]] = h;
        }
    }
    for k in 0..c {
        crate::interrupt::checkpoint()?;
        let r = sizes[k] - 1;
        let factor = &mut matrix[offsets[k]..offsets[k + 1]];
        for i in 0..r {
            checkpoint(i)?;
            for j in 0..r {
                factor[i * r + j] =
                    (factor[i * r + j] / roots[touched[next[k] + i]]) / roots[touched[next[k] + j]];
            }
        }
        for j in 0..r {
            crate::interrupt::checkpoint()?;
            let mut diagonal = factor[j * r + j];
            for l in 0..j {
                diagonal -= factor[j * r + l] * factor[j * r + l];
            }
            if !diagonal.is_finite() || diagonal <= 0.0 {
                return Ok(None);
            }
            factor[j * r + j] = diagonal.sqrt();
            for i in j + 1..r {
                checkpoint(i)?;
                let mut value = factor[i * r + j];
                for l in 0..j {
                    value -= factor[i * r + l] * factor[j * r + l];
                }
                factor[i * r + j] = value / factor[j * r + j];
            }
        }
    }
    Ok(Some(roots))
}

/// Apply the factored Schur blocks to all committed RHS, then recover the
/// eliminated coefficients and observation residuals without changing order.
fn solve_rhs(
    design: &BorrowedInterceptFeDesign<'_>,
    rhs: &DenseMatrix,
    eliminated: usize,
    weight: impl Fn(usize) -> f64,
    factors: &DirectFactors<'_>,
) -> Result<Option<DirectCandidate>> {
    let kept = 1 - eliminated;
    let (a, b) = (design.effects()[eliminated], design.effects()[kept]);
    let (p, q, c, nrhs) = (
        a.n_levels(),
        b.n_levels(),
        factors.layout.sizes.len(),
        rhs.ncols(),
    );
    let (sizes, offsets, anchors) = (
        factors.layout.sizes,
        factors.layout.offsets,
        factors.layout.anchors,
    );
    let (matrix, roots, mass) = (factors.matrix, factors.roots, factors.mass);
    let (next, touched) = (factors.next, factors.touched);
    let width = p + q;
    let mut coefficients = reserved(width * nrhs, 0_f64)?;
    let mut residuals = reserved(design.nobs() * nrhs, 0_f64)?;
    let a_offset = if eliminated == 0 { 0 } else { q };
    let b_offset = if kept == 0 { 0 } else { p };
    for column in 0..nrhs {
        crate::interrupt::checkpoint()?;
        let values = &mut coefficients[column * width..(column + 1) * width];
        for (g, &group_mass) in mass.iter().enumerate() {
            let members = a.level_index().members(g);
            let anchor = rhs[(members[0], column)];
            let mut sum = 0.0;
            for (position, &row) in members.iter().enumerate() {
                checkpoint(position)?;
                sum += weight(row) * (rhs[(row, column)] - anchor);
            }
            let center = sum / group_mass;
            for (position, &row) in members.iter().enumerate() {
                checkpoint(position)?;
                let h = b.codes()[row] as usize;
                values[b_offset + h] += weight(row) * ((rhs[(row, column)] - anchor) - center);
            }
        }
        for h in 0..q {
            values[b_offset + h] /= roots[h];
        }
        for k in 0..c {
            values[b_offset + anchors[k]] = 0.0;
            let r = sizes[k] - 1;
            let factor = &matrix[offsets[k]..offsets[k + 1]];
            for i in 0..r {
                checkpoint(i)?;
                let h = touched[next[k] + i];
                let mut value = values[b_offset + h];
                for j in 0..i {
                    value -= factor[i * r + j] * values[b_offset + touched[next[k] + j]];
                }
                values[b_offset + h] = value / factor[i * r + i];
            }
            for i in (0..r).rev() {
                checkpoint(i)?;
                let h = touched[next[k] + i];
                let mut value = values[b_offset + h];
                for j in i + 1..r {
                    value -= factor[j * r + i] * values[b_offset + touched[next[k] + j]];
                }
                values[b_offset + h] = value / factor[i * r + i];
            }
        }
        for h in 0..q {
            values[b_offset + h] /= roots[h];
        }
        for (g, &group_mass) in mass.iter().enumerate() {
            let members = a.level_index().members(g);
            let anchor = rhs[(members[0], column)];
            let mut sum = 0.0;
            for (position, &row) in members.iter().enumerate() {
                checkpoint(position)?;
                sum += weight(row)
                    * ((rhs[(row, column)] - anchor) - values[b_offset + b.codes()[row] as usize]);
            }
            let center = sum / group_mass;
            values[a_offset + g] = anchor + center;
            for (position, &row) in members.iter().enumerate() {
                checkpoint(position)?;
                residuals[row * nrhs + column] = ((rhs[(row, column)] - anchor)
                    - values[b_offset + b.codes()[row] as usize])
                    - center;
            }
        }
    }
    if coefficients
        .iter()
        .chain(residuals.iter())
        .any(|v| !v.is_finite())
    {
        return Ok(None);
    }
    Ok(Some(DirectCandidate {
        residualized: DenseMatrix::from_row_major(design.nobs(), nrhs, residuals)?,
        coefficients,
        eliminated_effect: eliminated,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{connected_components_all, BorrowedInterceptEffect, LevelIndex};

    fn limits() -> DirectLimits {
        DirectLimits {
            reference_iteration_work: 1_000_000,
            preflight_work: 0,
            remaining_setup_work: 4_000_000,
            component_census_work: 100,
            certificate_work: 1_000,
            certificate_bytes: 256,
            retained_live_bytes: 512,
            memory: None,
        }
    }

    fn with_rectangle(
        f: impl FnOnce(&BorrowedInterceptFeDesign<'_>, &ComponentInfo, &DenseMatrix),
    ) {
        let a = [0, 0, 1, 1];
        let b = [0, 1, 0, 1];
        let ia = LevelIndex::from_codes(&a, 2).unwrap();
        let ib = LevelIndex::from_codes(&b, 2).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            4,
            vec![
                BorrowedInterceptEffect::new(&a, 2, &ia).unwrap(),
                BorrowedInterceptEffect::new(&b, 2, &ib).unwrap(),
            ],
        )
        .unwrap();
        let components = connected_components_all(&[&a, &b], &[2, 2]).unwrap();
        let rhs = DenseMatrix::from_columns(&[vec![1., 2., 4., 8.]]).unwrap();
        f(&design, &components, &rhs);
    }

    fn portfolio_limits() -> DirectLimits {
        DirectLimits {
            preflight_work: PORTFOLIO_PREFLIGHT_WORK,
            component_census_work: 0,
            ..limits()
        }
    }

    #[test]
    fn portfolio_dimension_envelope_has_independent_admission_and_decline_examples() {
        // Hand-counted global envelopes; no synthetic graph is needed to
        // exercise the allocation-free dimension calculation or its overflow.
        for (n, p, q, rhs, expected_a, four_w, expected_c) in [
            (20_000, 100, 10, 1, 1_595_180, 1_608_616, 5_296_768),
            (10_000, 1_000, 10, 65, 27_319_588, 27_923_320, 108_414_720),
            (60_000, 3_000, 20, 6, 43_025_360, 17_290_656, 63_805_440),
        ] {
            let certificate = original_certificate_work(n, p + q, 2, rhs).unwrap();
            assert_eq!(certificate, expected_c);
            let bound = DirectLimits {
                reference_iteration_work: four_w / 4,
                remaining_setup_work: 8 * four_w - PORTFOLIO_PREFLIGHT_WORK,
                certificate_work: certificate,
                certificate_bytes: 0,
                retained_live_bytes: 0,
                ..portfolio_limits()
            };
            let profile =
                portfolio_bound(n, p, q, rhs, BorrowedMapWeights::Unit, 0, bound).unwrap();
            assert_eq!(profile.noncertificate_work, expected_a);
            assert_eq!(bound.admits_work(expected_a), q == 10);
            let weighted =
                portfolio_bound(n, p, q, rhs, BorrowedMapWeights::Provided(&[]), 0, bound).unwrap();
            assert_eq!(
                weighted.noncertificate_work - expected_a,
                16 * n as u64 * (rhs as u64 + 1)
            );
            assert!(!bound.admits_work(weighted.noncertificate_work));
        }
        let one =
            portfolio_bound(12, 3, 1, 2, BorrowedMapWeights::Unit, 0, portfolio_limits()).unwrap();
        assert_eq!((one.dense_elements, one.noncertificate_work), (0, 1860));
        for (n, p, q, rhs) in [
            (0, 1, 1, 1),
            (1, 0, 1, 1),
            (1, 1, 0, 1),
            (1, 1, 1, 0),
            (usize::MAX, 2, 2, 2),
            (1, usize::MAX, 2, 1),
            (1, 1, usize::MAX, 1),
        ] {
            assert!(portfolio_bound(
                n,
                p,
                q,
                rhs,
                BorrowedMapWeights::Unit,
                0,
                portfolio_limits()
            )
            .is_none());
        }
    }

    #[test]
    fn portfolio_preflight_enforces_exact_work_and_joint_memory_boundaries() {
        with_rectangle(|design, _, rhs| {
            // Global unknown-component upper bound is A1076 (actual A1012).
            let mut bound = DirectLimits {
                reference_iteration_work: 525, // (1076 + preflight1024)/4
                remaining_setup_work: 2836,    // A1076 + C1760; F already spent.
                certificate_work: 1760,
                ..portfolio_limits()
            };
            let admitted =
                portfolio_preflight(design, BorrowedMapWeights::Unit, rhs.ncols(), bound).unwrap();
            assert_eq!(admitted.noncertificate_work, 1076);
            assert_eq!(admitted.work_bound, 2836);
            bound.memory = Some(FeSolverMemoryBudget {
                execution_bytes: 100,
                usable_bytes: 100 + bound.retained_live_bytes + admitted.peak_bytes,
            });
            assert_eq!(
                portfolio_preflight(design, BorrowedMapWeights::Unit, 1, bound),
                Some(admitted)
            );
            for change in 0..6 {
                let mut refused = bound;
                match change {
                    0 => refused.reference_iteration_work -= 1,
                    1 => refused.remaining_setup_work -= 1,
                    2 => refused.memory.as_mut().unwrap().usable_bytes -= 1,
                    3 => refused.preflight_work = 0,
                    4 => refused.component_census_work = 1,
                    _ => refused.reference_iteration_work = u64::MAX,
                }
                assert!(
                    portfolio_preflight(design, BorrowedMapWeights::Unit, 1, refused).is_none()
                );
            }
            bound.certificate_work = u64::MAX;
            assert!(portfolio_preflight(design, BorrowedMapWeights::Unit, 1, bound).is_none());
        });
    }

    #[test]
    fn actual_direct_gate_includes_sunk_preflight_without_charging_it_again() {
        with_rectangle(|design, _, rhs| {
            let mut bound = DirectLimits {
                reference_iteration_work: 509, // (actual A1012 + F1024)/4.
                remaining_setup_work: 2772,    // Actual A+C, not A+C+F.
                certificate_work: 1760,
                ..portfolio_limits()
            };
            let accepted =
                attempt_with_census(design, BorrowedMapWeights::Unit, rhs, bound).unwrap();
            assert!(accepted.candidate.is_some());
            assert_eq!(accepted.work_used, 2772);
            bound.reference_iteration_work -= 1;
            let rejected =
                attempt_with_census(design, BorrowedMapWeights::Unit, rhs, bound).unwrap();
            assert_eq!(rejected.decline, Some(DirectDecline::Work));
            assert_eq!(rejected.work_used, 404); // Census and both profiles only.
            bound.preflight_work = 0;
            assert!(
                attempt_with_census(design, BorrowedMapWeights::Unit, rhs, bound)
                    .unwrap()
                    .candidate
                    .is_some()
            );
        });
    }

    #[test]
    fn every_small_bipartite_partition_fits_the_dimension_envelope() {
        // Every subset of a 2x3 bipartite graph, with repeated observations.
        // Includes disconnected blocks and absent declared levels. The latter
        // must fail validation before routing; valid finalized partitions have
        // census/profile storage bounded by the independent global envelope.
        for mask in 1..64 {
            let (mut a, mut b) = (Vec::new(), Vec::new());
            for edge in 0..6 {
                if mask & (1 << edge) != 0 {
                    for _ in 0..=edge % 3 {
                        a.push((edge / 3) as u32);
                        b.push((edge % 3) as u32);
                    }
                }
            }
            let ia = LevelIndex::from_codes(&a, 2).unwrap();
            let ib = LevelIndex::from_codes(&b, 3).unwrap();
            let unused_a = (0..2).any(|level| ia.members(level).is_empty());
            let unused_b = (0..3).any(|level| ib.members(level).is_empty());
            let effect_a = BorrowedInterceptEffect::new(&a, 2, &ia);
            let effect_b = BorrowedInterceptEffect::new(&b, 3, &ib);
            assert_eq!(effect_a.is_err(), unused_a);
            assert_eq!(effect_b.is_err(), unused_b);
            if unused_a || unused_b {
                continue;
            }
            let design =
                BorrowedInterceptFeDesign::new(a.len(), vec![effect_a.unwrap(), effect_b.unwrap()])
                    .unwrap();
            let rhs = DenseMatrix::from_columns(&[
                vec![1.; a.len()],
                (0..a.len()).map(|i| i as f64).collect(),
            ])
            .unwrap();
            let weighted: Vec<_> = (0..a.len()).map(|i| 1. + (i % 4) as f64).collect();
            for weights in [
                BorrowedMapWeights::Unit,
                BorrowedMapWeights::Provided(&weighted),
            ] {
                let bound = portfolio_limits();
                let preflight = portfolio_preflight(&design, weights, 2, bound).unwrap();
                let actual = attempt_with_census(&design, weights, &rhs, bound).unwrap();
                assert!(actual.work_used <= preflight.work_bound);
                let mut maximum_peak = 0;
                for (effect, actual_profile) in actual.profiles.iter().enumerate() {
                    let (p, q) = if effect == 0 { (2, 3) } else { (3, 2) };
                    let upper = portfolio_bound(a.len(), p, q, 2, weights, effect, bound).unwrap();
                    maximum_peak = maximum_peak.max(upper.peak_bytes);
                    if let Some(actual_profile) = actual_profile {
                        assert!(actual_profile.noncertificate_work <= upper.noncertificate_work);
                        assert!(actual_profile.peak_bytes <= upper.peak_bytes);
                    }
                }
                // Actual selection may choose another, cheaper orientation;
                // its own memory gate remains authoritative.
                assert!(actual.peak_bytes <= maximum_peak);
            }
        }
    }

    #[test]
    fn certificate_credit_never_funds_direct_arithmetic_or_profile_prefixes() {
        with_rectangle(|design, components, rhs| {
            // Hand-counted envelope: census224 + both profiles180 +
            // geometry72 + RHS144 + levels64 + caller preparation136 +
            // factor/solve96 + two clique edges96 = A1012. Certificate C1760.
            let mut limits = DirectLimits {
                reference_iteration_work: 253,
                remaining_setup_work: 2772,
                component_census_work: 0,
                certificate_work: 1760,
                ..limits()
            };
            let admitted =
                attempt_with_census(design, BorrowedMapWeights::Unit, rhs, limits).unwrap();
            assert!(admitted.candidate.is_some());
            assert_eq!(admitted.profiles[0].unwrap().noncertificate_work, 1012);
            assert_eq!(admitted.work_used, 2772);
            limits.reference_iteration_work = 252;
            limits.remaining_setup_work = u64::MAX;
            for certificate_work in [1760, 10_000] {
                limits.certificate_work = certificate_work;
                let refused =
                    attempt_with_census(design, BorrowedMapWeights::Unit, rhs, limits).unwrap();
                assert_eq!(refused.decline, Some(DirectDecline::Work));
                assert!(refused.candidate.is_none());
                assert_eq!(refused.work_used, 404);
                assert_eq!(refused.profiles[0].unwrap().noncertificate_work, 1012);
            }
            // Already supplied components leave a180-unit common prefix.
            // It must not allocate counts when4W=176, even with large C.
            limits.reference_iteration_work = 44;
            let refused =
                attempt(design, BorrowedMapWeights::Unit, components, rhs, limits).unwrap();
            assert_eq!(refused.decline, Some(DirectDecline::Work));
            assert_eq!(refused.profiles, [None, None]);
            assert_eq!((refused.work_used, refused.peak_bytes), (0, 0));
            limits.reference_iteration_work = 45;
            let charged =
                attempt(design, BorrowedMapWeights::Unit, components, rhs, limits).unwrap();
            assert_eq!(charged.work_used, 180);
        });
    }

    #[test]
    fn total_attempt_reserve_and_checked_overflow_refuse_without_numerical_work() {
        with_rectangle(|design, _, rhs| {
            let mut limits = DirectLimits {
                reference_iteration_work: 253,
                remaining_setup_work: 2771,
                component_census_work: 0,
                certificate_work: 1760,
                ..limits()
            };
            let refused =
                attempt_with_census(design, BorrowedMapWeights::Unit, rhs, limits).unwrap();
            assert_eq!(refused.decline, Some(DirectDecline::Work));
            assert!(refused.candidate.is_none());
            assert_eq!(refused.work_used, 404);
            // The necessary pre-census bound is576+C. One less cannot fund
            // even the census, regardless of the otherwise sufficient4W cap.
            limits.remaining_setup_work = 2335;
            let refused =
                attempt_with_census(design, BorrowedMapWeights::Unit, rhs, limits).unwrap();
            assert_eq!((refused.work_used, refused.peak_bytes), (0, 0));
            limits.remaining_setup_work = u64::MAX;
            limits.certificate_work = u64::MAX;
            let overflow =
                attempt_with_census(design, BorrowedMapWeights::Unit, rhs, limits).unwrap();
            assert_eq!((overflow.work_used, overflow.peak_bytes), (0, 0));
            limits.certificate_work = 1760;
            limits.reference_iteration_work = u64::MAX;
            let overflow =
                attempt_with_census(design, BorrowedMapWeights::Unit, rhs, limits).unwrap();
            assert_eq!((overflow.work_used, overflow.peak_bytes), (0, 0));
        });
    }

    #[test]
    fn launched_numerical_decline_keeps_complete_work_charge() {
        with_rectangle(|design, _, rhs| {
            let limits = DirectLimits {
                reference_iteration_work: 328,
                remaining_setup_work: 32 * 328,
                component_census_work: 0,
                certificate_work: 1760,
                ..limits()
            };
            let weights = [f64::MIN_POSITIVE, f64::MAX, 1., 1.];
            let failed =
                attempt_with_census(design, BorrowedMapWeights::Provided(&weights), rhs, limits)
                    .unwrap();
            assert_eq!(failed.decline, Some(DirectDecline::Numerical));
            assert!(failed.candidate.is_none());
            assert_eq!(failed.profiles[0].unwrap().noncertificate_work, 1140);
            assert_eq!(failed.work_used, 2900);
            // A spent reservation is not returned when its numerical phase
            // abstains; no caller may obtain a fresh full allowance from it.
            assert_eq!(limits.remaining_setup_work - failed.work_used, 7596);
        });
    }

    #[test]
    fn equal_work_orientations_tie_by_original_factor_after_memory_admission() {
        // At one RHS a 1x2 star costs 144 in orientation zero, and a 3x1
        // star costs 656 in orientation one: 41*144 = 9*656. All remaining
        // work is shared. Their different storage requirements must not break
        // that work tie. A final symmetric rectangle has a nonzero residual.
        let (mut a, mut b, mut p, mut q) = (Vec::new(), Vec::new(), 0, 0);
        for (components, left, right) in [(41, 1, 2), (9, 3, 1), (1, 2, 2)] {
            for _ in 0..components {
                for i in 0..left {
                    for j in 0..right {
                        a.push((p + i) as u32);
                        b.push((q + j) as u32);
                    }
                }
                p += left;
                q += right;
            }
        }
        let ia = LevelIndex::from_codes(&a, p).unwrap();
        let ib = LevelIndex::from_codes(&b, q).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            a.len(),
            vec![
                BorrowedInterceptEffect::new(&a, p, &ia).unwrap(),
                BorrowedInterceptEffect::new(&b, q, &ib).unwrap(),
            ],
        )
        .unwrap();
        let components = connected_components_all(&[&a, &b], &[p, q]).unwrap();
        let mut values = vec![0.; a.len()];
        values[a.len() - 4..].copy_from_slice(&[1., -1., -1., 1.]);
        let rhs = DenseMatrix::from_columns(&[values]).unwrap();
        let mut limits = limits();
        limits.memory = Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 65_536,
        });
        let result = attempt(&design, BorrowedMapWeights::Unit, &components, &rhs, limits).unwrap();
        let [Some(first), Some(second)] = result.profiles else {
            panic!("both complete orientation profiles are required");
        };
        assert_eq!(first.work_bound, second.work_bound);
        assert_eq!(first.peak_bytes, second.peak_bytes + 592);
        assert!(fits(limits, first.peak_bytes) && fits(limits, second.peak_bytes));
        let candidate = result.candidate.unwrap();
        assert_eq!(candidate.eliminated_effect, 0);
        assert_eq!(candidate.residualized, rhs);

        // Memory remains an independent admission condition: refusing the
        // first factor allows the equally priced second orientation.
        limits.memory.as_mut().unwrap().usable_bytes =
            limits.retained_live_bytes + second.peak_bytes;
        let result = attempt(&design, BorrowedMapWeights::Unit, &components, &rhs, limits).unwrap();
        let candidate = result.candidate.unwrap();
        assert_eq!(candidate.eliminated_effect, 1);
        assert_eq!(candidate.residualized, rhs);
    }

    #[test]
    fn weighted_rectangle_witness_and_disconnected_grounding() {
        // For each 2x2 rectangle, the one-dimensional W-orthogonal residual
        // space is (1/w00,-1/w01,-1/w10,1/w11). This is an independent analytic
        // witness, not another elimination or factorization implementation.
        let a = [0, 0, 1, 1, 2, 2, 3, 3, 4];
        let b = [0, 2, 0, 2, 1, 3, 1, 3, 4];
        let ia = LevelIndex::from_codes(&a, 5).unwrap();
        let ib = LevelIndex::from_codes(&b, 5).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            9,
            vec![
                BorrowedInterceptEffect::new(&a, 5, &ia).unwrap(),
                BorrowedInterceptEffect::new(&b, 5, &ib).unwrap(),
            ],
        )
        .unwrap();
        let components = connected_components_all(&[&a, &b], &[5, 5]).unwrap();
        assert_eq!(census(&design, 10).unwrap(), components);
        let weights = [1., 2., 3., 5., 2e80, 6e80, 3e80, 7e80, 4.];
        let y = vec![2., -1., 4., 3., -2., 5., 1., 3., 9.];
        let rhs = DenseMatrix::from_columns(&[y.clone(), y.iter().map(|v| 3. * v + 7.).collect()])
            .unwrap();
        let result = attempt(
            &design,
            BorrowedMapWeights::Provided(&weights),
            &components,
            &rhs,
            limits(),
        )
        .unwrap();
        assert_eq!(result.decline, None);
        assert!(result.profiles.iter().all(Option::is_some));
        let candidate = result.candidate.unwrap();
        let owned = attempt_with_census(
            &design,
            BorrowedMapWeights::Provided(&weights),
            &rhs,
            limits(),
        )
        .unwrap();
        assert!(owned.work_used > result.work_used);
        assert!(owned.peak_bytes > result.peak_bytes);
        assert_eq!(
            owned.candidate.unwrap().residualized,
            candidate.residualized
        );
        for start in [0, 4] {
            let contrast = y[start] - y[start + 1] - y[start + 2] + y[start + 3];
            let sum_inverse: f64 = weights[start..start + 4].iter().map(|w| 1. / w).sum();
            for (i, sign) in [1., -1., -1., 1.].into_iter().enumerate() {
                let expected = sign * contrast / (sum_inverse * weights[start + i]);
                for column in 0..2 {
                    let scale = if column == 0 { 1. } else { 3. };
                    assert!(
                        (candidate.residualized[(start + i, column)] - scale * expected).abs()
                            < 2e-13
                    );
                }
            }
        }
        assert_eq!(candidate.residualized[(8, 0)], 0.);
        assert_eq!(candidate.residualized[(8, 1)], 0.);
        for column in 0..2 {
            for row in 0..9 {
                let fitted = candidate.coefficients[column * 10 + a[row] as usize]
                    + candidate.coefficients[column * 10 + 5 + b[row] as usize];
                assert!(
                    (rhs[(row, column)] - fitted - candidate.residualized[(row, column)]).abs()
                        < 2e-13
                );
            }
        }
    }

    #[test]
    fn bounds_refuse_before_numeric_work_and_zero_weights_decline() {
        let a = [0, 0, 1, 1];
        let b = [0, 1, 0, 1];
        let ia = LevelIndex::from_codes(&a, 2).unwrap();
        let ib = LevelIndex::from_codes(&b, 2).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            4,
            vec![
                BorrowedInterceptEffect::new(&a, 2, &ia).unwrap(),
                BorrowedInterceptEffect::new(&b, 2, &ib).unwrap(),
            ],
        )
        .unwrap();
        let components = connected_components_all(&[&a, &b], &[2, 2]).unwrap();
        let rhs = DenseMatrix::from_columns(&[vec![1., 2., 4., 8.]]).unwrap();
        let mut bound = limits();
        bound.remaining_setup_work = 100;
        let refused = attempt(&design, BorrowedMapWeights::Unit, &components, &rhs, bound).unwrap();
        assert_eq!(refused.decline, Some(DirectDecline::Work));
        assert_eq!(refused.peak_bytes, 0);
        bound = limits();
        bound.memory = Some(FeSolverMemoryBudget {
            execution_bytes: 100,
            usable_bytes: 612,
        });
        let refused = attempt(&design, BorrowedMapWeights::Unit, &components, &rhs, bound).unwrap();
        assert_eq!(refused.decline, Some(DirectDecline::Memory));
        assert_eq!(refused.peak_bytes, 0);
        let refused = attempt(
            &design,
            BorrowedMapWeights::Provided(&[1., 0., 1., 1.]),
            &components,
            &rhs,
            limits(),
        )
        .unwrap();
        assert_eq!(refused.decline, Some(DirectDecline::Numerical));
        assert!(refused.candidate.is_none());
        assert!(refused.work_used <= limits().remaining_setup_work);
    }

    #[test]
    fn full_active_width_components_admit_with_immutable_certificate_reserve() {
        let n = 4_000;
        let levels = n / 2;
        let a: Vec<u32> = (0..n)
            .map(|i| (2 * (i / 4) + [0, 0, 1, 1][i % 4]) as u32)
            .collect();
        let b: Vec<u32> = (0..n)
            .map(|i| (2 * (i / 4) + [0, 1, 0, 1][i % 4]) as u32)
            .collect();
        let ia = LevelIndex::from_codes(&a, levels).unwrap();
        let ib = LevelIndex::from_codes(&b, levels).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            n,
            vec![
                BorrowedInterceptEffect::new(&a, levels, &ia).unwrap(),
                BorrowedInterceptEffect::new(&b, levels, &ib).unwrap(),
            ],
        )
        .unwrap();
        let rhs = DenseMatrix::from_columns(&[(0..n)
            .map(|i| 1_000. + [2., -1., 4., 3.][i % 4])
            .collect()])
        .unwrap();
        // The reference is packed: inactive workspace capacity cannot buy
        // direct arithmetic. Every component has a nonzero residual contrast.
        let w_ref = super::super::routing::schur_iteration_work(n, levels, levels, 1, 1).unwrap();
        let w_padded =
            super::super::routing::schur_iteration_work(n, levels, levels, 1, 64).unwrap();
        let limits = DirectLimits {
            reference_iteration_work: w_ref,
            preflight_work: 0,
            remaining_setup_work: 32 * w_ref,
            component_census_work: 0,
            certificate_work: original_certificate_work(n, 2 * levels, 2, 1).unwrap(),
            certificate_bytes: 4 * 8,
            retained_live_bytes: (n * 8) as u128,
            memory: Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: 1_000_000,
            }),
        };
        let weighted: Vec<_> = (0..n).map(|i| [1., 2., 3., 5.][i % 4]).collect();
        for weights in [
            BorrowedMapWeights::Unit,
            BorrowedMapWeights::Provided(&weighted),
        ] {
            let result = attempt_with_census(&design, weights, &rhs, limits).unwrap();
            assert_eq!(result.decline, None, "profiles: {:?}", result.profiles);
            assert!(result
                .profiles
                .iter()
                .flatten()
                .all(|p| p.noncertificate_work <= 4 * w_ref));
            assert!(result.work_used <= limits.remaining_setup_work);
            let padded = attempt_with_census(
                &design,
                weights,
                &rhs,
                DirectLimits {
                    remaining_setup_work: 32 * w_padded,
                    ..limits
                },
            )
            .unwrap();
            assert_eq!(padded.profiles, result.profiles);
            assert_eq!(padded.work_used, result.work_used);
            let candidate = result.candidate.unwrap();
            assert_eq!(
                padded.candidate.unwrap().residualized,
                candidate.residualized
            );
            for row in 0..n {
                let sign = [1., -1., -1., 1.][row % 4];
                let expected = match weights {
                    BorrowedMapWeights::Unit => 0.5 * sign,
                    BorrowedMapWeights::Provided(w) => {
                        2. * sign / ((1. + 0.5 + 1. / 3. + 0.2) * w[row])
                    }
                };
                assert!((candidate.residualized[(row, 0)] - expected).abs() < 1e-12);
            }
        }
    }
}
