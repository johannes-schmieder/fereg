//! A bounded alternative for prepared models whose boundary-square tableau
//! cannot fit. Both outcomes require an independent raw-design proof.
use super::{combine_basis, compact_tableau, exact_terminal, reserve};
use super::{fallible_collect, no_separation_certificate, nullspace_basis};
use super::{fallible_filled, verify_indexed_farkas_certificate};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result};
use crate::ppml::ExactNullspace;
use crate::ppml_separation::{
    DetectionProposal, MethodStatus, SeparationCertificate, SeparationCertificateProof,
    SeparationMethod,
};

pub(crate) enum CompactModel {
    Direction {
        coefficients: Vec<i64>,
        iterations: usize,
    },
    Terminal(DetectionProposal),
    /// A numerical Farkas vector whose exact lift exceeded the exact
    /// arithmetic. It is a proposal only: a physical observation dual with
    /// least-squares positive-row weights, for the requested IR certificate.
    NumericalDual {
        dual: Vec<f64>,
    },
}

mod grouped;
pub(crate) use grouped::CompactInput;
mod positive_rows;
mod projection;

const MAX_WORK: usize = 128 << 20;
const MAX_CELL_VISITS: usize = 1 << 30;

/// Largest streamed raw support: finite ones plus one level per effect.
pub(crate) const STREAMED_ROW_TERMS: usize = 128;

/// Per-stage allowances of one compact search. The incumbent lane uses the
/// fixed constants. A scaled retry receives allowances proportional to the
/// measured raw design and admits scratch storage through the live plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompactWork {
    /// Coordinate operations allowed in each elimination, projection, lift
    /// or exact replay stage.
    pub(crate) stage: usize,
    /// Dense tableau cell visits allowed in one transposed phase-I solve.
    pub(crate) cell_visits: usize,
    /// When nonzero, phase I may take at most this many pivots per cone
    /// dimension and stops once degenerate pivots stall after Bland's rule.
    pub(crate) pivots_per_dimension: usize,
}

impl CompactWork {
    pub(crate) const INCUMBENT: Self = Self {
        stage: MAX_WORK,
        cell_visits: MAX_CELL_VISITS,
        pivots_per_dimension: 0,
    };

    /// Allowances measured from the generation. One raw pass reads each row's
    /// recipe once, `rows * (row_terms + 1)` visits. Projecting a pass onto the
    /// positive-row nullspace multiplies it by that dimension, which never
    /// exceeds the model width. Each linear stage may spend four such projected
    /// passes, so the allowance grows linearly in the rows and the width and
    /// never falls below the incumbent. The dense phase-I tableau is bounded by
    /// its own measured size instead: at most `PIVOTS_PER_DIMENSION` pivots per
    /// retained cone dimension, each charged at the full tableau, and it stops
    /// earlier when degenerate pivots make no progress.
    pub(crate) fn scaled(rows: usize, row_terms: usize, width: usize) -> Self {
        const PIVOTS_PER_DIMENSION: usize = 8;
        let pass = rows.saturating_mul(row_terms.saturating_add(1));
        let stage = pass.saturating_mul(width).saturating_mul(4);
        Self {
            stage: stage.max(MAX_WORK),
            cell_visits: usize::MAX,
            pivots_per_dimension: PIVOTS_PER_DIMENSION,
        }
    }
}

pub(crate) struct BinaryDesign<'a> {
    storage: Storage<'a>,
    columns: usize,
}

enum Storage<'a> {
    Packed {
        offsets: Vec<usize>,
        indices: Vec<u32>,
    },
    /// Rows regenerated from a bit mask of the finite binary columns and the
    /// generation's own factor codes. Construction proves that every regenerated
    /// row equals the support emitted by the raw reader.
    Streamed {
        finite: Vec<u64>,
        effects: Vec<StreamedEffect<'a>>,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct StreamedEffect<'a> {
    pub(crate) codes: &'a [u32],
    /// First design column of this effect's level indicators.
    pub(crate) offset: u32,
    pub(crate) levels: u32,
}

/// One sorted raw binary support, borrowed from a packed design or
/// regenerated inline from a streamed design. The inline bank is a short-lived
/// stack value; boxing it would allocate once per visited row.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Row<'a> {
    Stored(&'a [u32]),
    Inline {
        len: usize,
        terms: [u32; STREAMED_ROW_TERMS],
    },
}

impl std::ops::Deref for Row<'_> {
    type Target = [u32];

    fn deref(&self) -> &[u32] {
        match self {
            Self::Stored(row) => row,
            Self::Inline { len, terms } => &terms[..*len],
        }
    }
}

impl<'a> BinaryDesign<'a> {
    #[cfg(test)]
    pub(crate) fn from_rows(
        rows: usize,
        columns: usize,
        limit: usize,
        mut read: impl FnMut(usize, &mut [f64]) -> Result<()>,
    ) -> Result<Option<Self>> {
        let mut dense = vec![0.; columns];
        let mut support = |i, row: &mut Vec<u32>| {
            read(i, &mut dense)?;
            for (j, &x) in dense.iter().enumerate() {
                if x != 0. {
                    if x != 1. {
                        return Ok(false);
                    }
                    row.push(j as u32);
                }
            }
            Ok(true)
        };
        grouped::RawSupport::new((columns, columns, columns), &mut support).pack(
            rows,
            &[],
            None,
            limit.saturating_sub(columns),
        )
    }

    fn packed(offsets: Vec<usize>, indices: Vec<u32>, columns: usize) -> Self {
        Self {
            storage: Storage::Packed { offsets, indices },
            columns,
        }
    }

    /// Regenerate rows from the generation's factor codes instead of storing
    /// every incidence. `read` is the same raw-support reader used to pack a
    /// design. Every row is read once, and any disagreement with the code view
    /// or any nonbinary value leaves the caller with no streamed design.
    pub(crate) fn streamed(
        rows: usize,
        (columns, finite): (usize, usize),
        effects: Vec<StreamedEffect<'a>>,
        read: &mut dyn FnMut(usize, &mut Vec<u32>) -> Result<bool>,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Option<Self>> {
        if finite > u64::BITS as usize
            || finite.saturating_add(effects.len()) > STREAMED_ROW_TERMS
            || u32::try_from(columns).is_err()
            || u32::try_from(rows).is_err()
        {
            return Ok(None);
        }
        let mut next = finite;
        for effect in &effects {
            let start = effect.offset as usize;
            if effect.codes.len() != rows || start != next {
                return Ok(None);
            }
            next = start.saturating_add(effect.levels as usize);
        }
        if next != columns {
            return Ok(None);
        }
        let mut masks = fallible_filled(rows, 0u64, "unable to reserve streamed binary rows")?;
        let mut row = Vec::new();
        reserve(
            &mut row,
            STREAMED_ROW_TERMS,
            "unable to reserve streamed binary support",
        )?;
        for (i, mask) in masks.iter_mut().enumerate() {
            if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
                interrupt()?;
            }
            row.clear();
            if !read(i, &mut row)? {
                return Ok(None);
            }
            let split = row.partition_point(|&j| (j as usize) < finite);
            if row.len() != split + effects.len()
                || row[..split].windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Ok(None);
            }
            for &j in &row[..split] {
                *mask |= 1u64 << j;
            }
            for (&j, effect) in row[split..].iter().zip(&effects) {
                let code = effect.codes[i];
                if code >= effect.levels || j != effect.offset + code {
                    return Ok(None);
                }
            }
        }
        Ok(Some(Self {
            storage: Storage::Streamed {
                finite: masks,
                effects,
            },
            columns,
        }))
    }

    pub(crate) fn ncols(&self) -> usize {
        self.columns
    }

    pub(crate) fn row(&self, row: usize) -> Row<'_> {
        match &self.storage {
            Storage::Packed { offsets, indices } => {
                Row::Stored(&indices[offsets[row]..offsets[row + 1]])
            }
            Storage::Streamed { finite, effects } => {
                let mut terms = [0u32; STREAMED_ROW_TERMS];
                let mut len = 0;
                let mut mask = finite[row];
                while mask != 0 {
                    terms[len] = mask.trailing_zeros();
                    len += 1;
                    mask &= mask - 1;
                }
                for effect in effects {
                    terms[len] = effect.offset + effect.codes[row];
                    len += 1;
                }
                Row::Inline { len, terms }
            }
        }
    }

    pub(crate) fn cells(&self) -> usize {
        match &self.storage {
            Storage::Packed { offsets, indices } => {
                offsets.capacity() + indices.capacity().div_ceil(2)
            }
            Storage::Streamed { finite, effects } => finite.capacity() + 2 * effects.capacity(),
        }
    }
}

pub(crate) fn compact_model_admitted(
    rows: usize,
    positive: usize,
    width: usize,
    limit: usize,
) -> bool {
    width > 0
        && positive > 0
        && positive < rows
        && width.saturating_mul(width).saturating_add(rows) <= limit
}

fn dense_positive_admitted(
    rows: usize,
    positive: usize,
    width: usize,
    (limit, work): (usize, usize),
) -> bool {
    positive_elimination(positive, width) <= work
        && dense_positive_cells(rows, positive, width) <= limit
}

fn positive_elimination(positive: usize, width: usize) -> usize {
    positive
        .saturating_mul(width)
        .saturating_mul(positive.min(width))
}

/// Record why a compact stage gave no proposal: the stage, the measured
/// requirement and the allowance it exceeded (cells or operations).
pub(crate) fn trace_refusal(stage: &'static str, needed: usize, available: usize) {
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=compact_refusal|stage={stage}|needed={needed}|available={available}"
        );
    }
}

fn dense_positive_cells(rows: usize, positive: usize, width: usize) -> usize {
    positive
        .saturating_mul(width)
        .saturating_mul(2)
        .saturating_add(width.saturating_mul(width.saturating_add(8)))
        .saturating_add(rows.saturating_mul(3))
}

pub(crate) fn compact_model(
    outcome: &[f64],
    design: &BinaryDesign,
    config: crate::ppml_separation::SeparationConfig,
    pivots: &mut usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<CompactModel>> {
    CompactSearch::new(
        outcome,
        design,
        config,
        None,
        (config.simplex_max_tableau_cells, CompactWork::INCUMBENT),
    )
    .run(pivots, interrupt, &mut |_| None)
}

/// The same search and certificates with allowances scaled to the measured
/// design. `grow` admits scratch storage beyond `limit` by its exact size.
pub(crate) fn compact_model_scaled(
    outcome: &[f64],
    design: &BinaryDesign,
    config: crate::ppml_separation::SeparationConfig,
    (limit, work, dual_only): (usize, CompactWork, bool),
    pivots: &mut usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
    grow: &mut dyn FnMut(usize) -> Option<usize>,
) -> Result<Option<CompactModel>> {
    let mut search = CompactSearch::new(outcome, design, config, None, (limit, work));
    search.scaled = true;
    search.dual_only = dual_only;
    search.run(pivots, interrupt, grow)
}

struct CompactSearch<'a> {
    outcome: &'a [f64],
    design: &'a BinaryDesign<'a>,
    groups: Option<&'a grouped::RowGroups>,
    tolerance: f64,
    certificate_tolerance: f64,
    max_iterations: usize,
    available: usize,
    work: CompactWork,
    /// A scaled retry: the streamed cone may request its measured distinct-row
    /// bank, the tableau stops on measured stall, and an inexact Farkas vector
    /// is returned for the IR certificate instead of None.
    scaled: bool,
    /// For an IR certificate: return the physical Farkas proposal without an
    /// exact simplex terminal, whose proof type IR cannot replay as its own.
    dual_only: bool,
}

impl<'a> CompactSearch<'a> {
    fn new(
        outcome: &'a [f64],
        design: &'a BinaryDesign<'a>,
        config: crate::ppml_separation::SeparationConfig,
        groups: Option<&'a grouped::RowGroups>,
        (limit, work): (usize, CompactWork),
    ) -> Self {
        Self {
            outcome,
            design,
            groups,
            tolerance: config.projection_tolerance,
            certificate_tolerance: config.certificate_tolerance,
            max_iterations: config.simplex_max_iterations,
            available: limit.saturating_sub(design.cells()),
            work,
            scaled: false,
            dual_only: false,
        }
    }

    fn require(&mut self, needed: usize, grow: &mut dyn FnMut(usize) -> Option<usize>) -> bool {
        if needed > self.available {
            let Some(granted) = grow(needed).filter(|&grant| grant >= needed) else {
                return false;
            };
            self.available = granted;
        }
        true
    }

    fn run(
        &mut self,
        pivots: &mut usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<CompactModel>> {
        let width = self.design.ncols();
        let outcome = self.outcome;
        let positive_count = outcome.iter().filter(|&&y| y > 0.).count();
        if width == 0 || positive_count == 0 || positive_count == outcome.len() {
            return Ok(None);
        }
        let entry = width.saturating_mul(width).saturating_add(outcome.len());
        if !self.require(entry, grow) {
            trace_refusal("entry_cells", entry, self.available);
            return Ok(None);
        }
        if dense_positive_admitted(
            outcome.len(),
            positive_count,
            width,
            (self.available, self.work.stage),
        ) {
            let positive = fallible_collect(
                outcome
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &y)| (y > 0.).then_some(i)),
                positive_count,
                "unable to reserve compact simplex positive rows",
            )?;
            return self.solve(positive, pivots, interrupt, grow);
        }
        let Some(selected) = positive_rows::select(
            self.design,
            outcome,
            (self.available, self.work.stage),
            interrupt,
        )?
        else {
            trace_refusal("positive_select", width, self.available);
            return Ok(None);
        };
        let rank = selected.len();
        let proposal = self.solve(selected, pivots, interrupt, grow)?;
        if proposal.is_some() || rank == width {
            return Ok(proposal);
        }
        let Some(odd) = positive_rows::select_odd(
            self.design,
            outcome,
            (self.available, self.work.stage),
            interrupt,
        )?
        else {
            return Ok(None);
        };
        if odd.len() > rank {
            self.solve(odd, pivots, interrupt, grow)
        } else {
            Ok(None)
        }
    }
}

impl CompactSearch<'_> {
    fn solve(
        &mut self,
        mut positive: Vec<usize>,
        pivots: &mut usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<CompactModel>> {
        let Self {
            outcome,
            design,
            tolerance,
            ..
        } = *self;
        let width = design.ncols();
        let work = self.work;
        if !dense_positive_admitted(
            outcome.len(),
            positive.len(),
            width,
            (usize::MAX, work.stage),
        ) {
            let elimination = positive_elimination(positive.len(), width);
            trace_refusal("positive_elimination_work", elimination, work.stage);
            return Ok(None);
        }
        let cells = dense_positive_cells(outcome.len(), positive.len(), width);
        if !self.require(cells, grow) {
            trace_refusal("positive_dense_cells", cells, self.available);
            return Ok(None);
        }
        // The pivot subset only reduces proposal work. A numerical direction or
        // dual still has to balance the full raw design before we can certify it.
        let Some(mut basis) = positive_nullspace(design, &mut positive, tolerance, interrupt)?
        else {
            trace_refusal("positive_nullspace", positive.len(), width);
            return Ok(None);
        };
        let boundary_count = outcome.iter().filter(|&&y| y == 0.).count();
        if basis.is_empty() && self.dual_only {
            // Positive rows span every coordinate: unit boundary mass balances
            // through least-squares positive weights for the IR certificate.
            let boundary = fallible_collect(
                outcome
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &y)| (y == 0.).then_some(i)),
                boundary_count,
                "unable to reserve compact simplex zero rows",
            )?;
            let weights = fallible_filled(boundary.len(), 1., "unable to reserve unit dual mass")?;
            return self.admitted_dual(&boundary, &weights, &positive, boundary.len(), grow);
        }
        if basis.is_empty() {
            return Ok(exact_terminal::compact_full_positive_rank(
                design, outcome, &positive, work.stage,
            )?
            .then(|| {
                terminal(
                    no_separation_certificate(
                        SeparationCertificateProof::SimplexFullPositiveRank { columns: width },
                    ),
                    *pivots,
                    "finite regressors have full column rank on the positive-outcome sample",
                )
            }));
        }

        let dimension = basis.len();
        let boundary = fallible_collect(
            outcome
                .iter()
                .enumerate()
                .filter_map(|(i, &y)| (y == 0.).then_some(i)),
            boundary_count,
            "unable to reserve compact simplex zero rows",
        )?;
        let fixed_cells = boundary
            .capacity()
            .saturating_add(positive.capacity())
            .saturating_add(width);
        let basis_workspace = projection::minimum_basis_cells(dimension, width);
        if !self.require(fixed_cells.saturating_add(basis_workspace), grow) {
            trace_refusal(
                "basis_cells",
                fixed_cells.saturating_add(basis_workspace),
                self.available,
            );
            return Ok(None);
        }
        let cone_budget = self.available - fixed_cells;
        let mut granted = None;
        let mut extend = |needed: usize| {
            let total = fixed_cells.checked_add(needed)?;
            let grant = grow(total).filter(|&g| g >= total)?;
            granted = Some(grant);
            Some(grant - fixed_cells)
        };
        let built = projection::build(
            design,
            &boundary,
            &mut basis,
            tolerance,
            (cone_budget, work.stage),
            self.scaled.then_some(&mut extend as projection::Grow<'_>),
        )?;
        if let Some(grant) = granted {
            self.available = self.available.max(grant);
        }
        let Some(projected) = built else {
            trace_refusal("projection", dimension, cone_budget);
            return Ok(None);
        };
        let retained = basis.len();
        let live = fixed_cells.saturating_add(projected.live_cells);
        let proposal = self.propose(&projected, (boundary_count, live), pivots, interrupt, grow)?;
        let remaining = self.available.saturating_sub(live);
        if crate::diagnostics::solver_trace_enabled() {
            let branch = match &proposal {
                Some(compact_tableau::Proposal::Direction { .. }) => "direction",
                Some(compact_tableau::Proposal::PositiveWeights { .. }) => "dual",
                None => "unresolved",
            };
            eprintln!("FEREG_SOLVER_TRACE|phase=compact_tableau|dimension={dimension}|retained={retained}|boundary={boundary_count}|branch={branch}");
        }
        match proposal {
            Some(compact_tableau::Proposal::Direction {
                coordinates,
                iterations,
            }) => {
                drop(projected);
                let scratch = self.available.saturating_sub(fixed_cells + retained);
                self.lift_direction(basis, &coordinates, &positive, (scratch, iterations))
            }
            Some(compact_tableau::Proposal::PositiveWeights {
                weights,
                iterations,
            }) => {
                drop(basis);
                let farkas_cells = remaining.saturating_sub(weights.capacity());
                self.certify_weights(
                    projected,
                    (&boundary, &weights, &positive),
                    (fixed_cells, farkas_cells, iterations),
                    grow,
                )
            }
            None => Ok(None),
        }
    }
    /// Combine the proposed cone coordinates and lift them to an exact
    /// integer ray on the binding raw rows; every row is replayed exactly.
    fn lift_direction(
        &self,
        basis: Vec<Vec<f64>>,
        coordinates: &[f64],
        positive: &[usize],
        (scratch, iterations): (usize, usize),
    ) -> Result<Option<CompactModel>> {
        let coefficients = combine_basis(&basis, coordinates)?;
        drop(basis);
        let lifted = lift_ray(
            self.design,
            self.outcome,
            &coefficients,
            positive,
            self.tolerance,
            (scratch, self.work.stage),
        )?;
        if lifted.is_none() {
            trace_refusal("ray_lift", coefficients.len(), scratch);
        }
        Ok(lifted.map(|coefficients| CompactModel::Direction {
            coefficients,
            iterations,
        }))
    }

    /// A numerical Farkas vector must pass the indexed numerical replay and
    /// then the exact raw-design terminal. A scaled search that cannot lift it
    /// exactly returns the physical dual for the requested IR certificate.
    fn certify_weights(
        &mut self,
        projected: projection::BoundaryCone,
        (boundary, weights, positive): (&[usize], &[f64], &[usize]),
        (fixed_cells, farkas_cells, iterations): (usize, usize, usize),
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<CompactModel>> {
        let (design, outcome, work) = (self.design, self.outcome, self.work);
        let certificate = self.farkas(&projected, boundary, weights, farkas_cells)?;
        drop(projected);
        let Some(certificate) = certificate else {
            if crate::diagnostics::solver_trace_enabled() {
                eprintln!("FEREG_SOLVER_TRACE|phase=compact_terminal|failed=numerical_farkas");
            }
            return Ok(None);
        };
        let scratch = self.available.saturating_sub(fixed_cells + weights.len());
        if !self.dual_only
            && exact_terminal::compact_no_recession(
                design, outcome, boundary, weights, positive, work.stage, scratch,
            )?
        {
            return Ok(Some(terminal(
                certificate,
                iterations,
                "model simplex phase-I infeasibility passed an independent Farkas certificate",
            )));
        }
        if !self.scaled || self.groups.is_some() {
            return Ok(None);
        }
        self.admitted_dual(
            boundary,
            weights,
            positive,
            fixed_cells.saturating_add(weights.len()),
            grow,
        )
    }

    fn admitted_dual(
        &mut self,
        boundary: &[usize],
        weights: &[f64],
        positive: &[usize],
        live: usize,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<CompactModel>> {
        let words = physical_dual_cells(self.outcome.len(), positive.len(), self.design.ncols())
            .saturating_add(live);
        if !self.require(words, grow) {
            trace_refusal("physical_dual_cells", words, self.available);
            return Ok(None);
        }
        Ok(physical_dual(
            self.design,
            self.outcome,
            boundary,
            weights,
            positive,
            self.work.stage,
        )?
        .map(|dual| CompactModel::NumericalDual { dual }))
    }

    /// Try the already funded cone first. Extend only when its exact direct
    /// tableau size was refused and the caller can admit the physical index.
    fn propose(
        &mut self,
        projected: &projection::BoundaryCone,
        (boundary, live): (usize, usize),
        pivots: &mut usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<compact_tableau::Proposal>> {
        let dual_cells = boundary.saturating_mul(2).saturating_add(1);
        if !self.require(live.saturating_add(dual_cells), grow) {
            trace_refusal(
                "dual_cells",
                live.saturating_add(dual_cells),
                self.available,
            );
            return Ok(None);
        }
        let policy = (
            self.tolerance,
            self.max_iterations,
            self.work.cell_visits,
            self.work.pivots_per_dimension,
        );
        let remaining = self.available - live;
        let initial = projected.propose(boundary, policy, pivots, remaining, interrupt)?;
        if initial.is_some() || *pivots >= self.max_iterations {
            return Ok(initial);
        }
        let dimension = projected.rows.first().map_or(0, Vec::len);
        let needed = compact_tableau::workspace_cells(projected.rows.len(), dimension);
        if needed.is_some_and(|words| {
            words > remaining && self.require(live.saturating_add(words), grow)
        }) {
            let retried =
                projected.propose(boundary, policy, pivots, self.available - live, interrupt)?;
            if retried.is_none() {
                trace_refusal("tableau", *pivots, self.work.cell_visits);
            }
            return Ok(retried);
        }
        trace_refusal("tableau_cells", needed.unwrap_or(usize::MAX), remaining);
        Ok(None)
    }

    fn farkas(
        &self,
        cone: &projection::BoundaryCone,
        boundary: &[usize],
        weights: &[f64],
        available: usize,
    ) -> Result<Option<SeparationCertificate>> {
        if let Some(groups) = self.groups {
            return groups.farkas(
                cone,
                boundary,
                weights,
                self.certificate_tolerance,
                available,
            );
        }
        let dual = fallible_collect(
            weights.iter().map(|w| -*w).chain(std::iter::once(-1.)),
            weights.len() + 1,
            "unable to reserve compact Farkas certificate",
        )?;
        verify_indexed_farkas_certificate(
            &cone.rows,
            &cone.index,
            &dual,
            self.certificate_tolerance,
        )
    }
}

fn physical_dual_cells(rows: usize, positive: usize, width: usize) -> usize {
    rows.saturating_add(positive.saturating_mul(positive.saturating_add(width).saturating_add(4)))
        .saturating_add(width.saturating_mul(4))
}

/// Lift a numerical Farkas vector on the boundary rows to every observation.
/// The boundary keeps its strictly positive weights. Independent positive
/// representatives receive the least-squares weights that balance the boundary
/// total, refined twice from compensated residuals. Every other positive row
/// has weight zero. The result is only a proposal for the IR certificate.
fn physical_dual(
    design: &BinaryDesign,
    outcome: &[f64],
    boundary: &[usize],
    weights: &[f64],
    positive: &[usize],
    stage_work: usize,
) -> Result<Option<Vec<f64>>> {
    let width = design.ncols();
    let k = positive.len();
    let cost = boundary
        .len()
        .saturating_add(k)
        .saturating_mul(STREAMED_ROW_TERMS)
        .saturating_add(
            k.saturating_mul(k)
                .saturating_mul(k.saturating_add(width))
                .saturating_mul(3),
        );
    if k == 0 || boundary.len() != weights.len() || cost > stage_work {
        return Ok(None);
    }
    let mut total = fallible_filled(width, 0., "unable to reserve boundary dual total")?;
    let mut correction = fallible_filled(width, 0., "unable to reserve boundary dual correction")?;
    for (position, (&i, &w)) in boundary.iter().zip(weights).enumerate() {
        crate::interrupt::row_checkpoint(position)?;
        if !(w.is_finite() && w > 0.) {
            return Ok(None);
        }
        for &j in design.row(i).iter() {
            crate::linalg::neumaier_add(w, &mut total[j as usize], &mut correction[j as usize]);
        }
    }
    for (value, extra) in total.iter_mut().zip(&correction) {
        *value += extra;
    }
    drop(correction);
    // Gram matrix of the independent positive rows and the normal equations
    // A A' p = -A t, where A holds those rows and t the boundary total.
    let mut gram = fallible_filled(k * k, 0., "unable to reserve positive dual Gram")?;
    let mut marks = fallible_filled(width, false, "unable to reserve positive dual row mask")?;
    for (a, &i) in positive.iter().enumerate() {
        crate::interrupt::checkpoint()?;
        let row = design.row(i);
        for &j in row.iter() {
            marks[j as usize] = true;
        }
        for (b, &other) in positive.iter().enumerate().skip(a) {
            let shared = design
                .row(other)
                .iter()
                .filter(|&&j| marks[j as usize])
                .count();
            gram[a * k + b] = shared as f64;
            gram[b * k + a] = shared as f64;
        }
        for &j in row.iter() {
            marks[j as usize] = false;
        }
    }
    let mut solution = fallible_filled(k, 0., "unable to reserve positive dual weights")?;
    let mut residual = fallible_filled(width, 0., "unable to reserve positive dual residual")?;
    let mut matrix = fallible_filled(k * k, 0., "unable to reserve positive dual system")?;
    let mut rhs = fallible_filled(k, 0., "unable to reserve positive dual rhs")?;
    for _ in 0..3 {
        // Residual of the boundary balance, t + A'p, by exact 0/1 incidences.
        residual.copy_from_slice(&total);
        let mut residual_correction = fallible_filled(
            width,
            0.,
            "unable to reserve positive dual residual correction",
        )?;
        for (&i, &value) in positive.iter().zip(&solution) {
            for &j in design.row(i).iter() {
                crate::linalg::neumaier_add(
                    value,
                    &mut residual[j as usize],
                    &mut residual_correction[j as usize],
                );
            }
        }
        for (value, extra) in residual.iter_mut().zip(&residual_correction) {
            *value += extra;
        }
        for (entry, &i) in rhs.iter_mut().zip(positive) {
            *entry = -design
                .row(i)
                .iter()
                .map(|&j| residual[j as usize])
                .sum::<f64>();
        }
        matrix.copy_from_slice(&gram);
        if crate::linalg::solve_linear_system(&mut matrix, &mut rhs, k, 1, 0.)? != k {
            return Ok(None);
        }
        for (value, step) in solution.iter_mut().zip(&rhs) {
            *value += step;
        }
    }
    if solution.iter().any(|value| !value.is_finite()) {
        return Ok(None);
    }
    let mut dual = fallible_filled(outcome.len(), 0., "unable to reserve physical dual")?;
    for (&i, &w) in boundary.iter().zip(weights) {
        dual[i] = w;
    }
    for (&i, &value) in positive.iter().zip(&solution) {
        dual[i] = value;
    }
    Ok(Some(dual))
}

fn positive_nullspace(
    design: &BinaryDesign,
    positive: &mut Vec<usize>,
    tolerance: f64,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Vec<Vec<f64>>>> {
    let width = design.ncols();
    let mut rows = fallible_filled(
        positive.len() * width,
        0.,
        "unable to reserve compact positive design",
    )?;
    for (i, &origin) in positive.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        for &j in design.row(origin).iter() {
            rows[i * width + j as usize] = 1.;
        }
    }
    let matrix = DenseMatrix::from_row_major(positive.len(), width, rows)?;
    let mut order = fallible_collect(
        0..positive.len(),
        positive.len(),
        "unable to reserve compact pivot order",
    )?;
    let basis = match nullspace_basis(&matrix, &mut order, tolerance, interrupt) {
        Ok(basis) => basis,
        Err(error) if error.code == ErrorCode::NonConvergence => return Ok(None),
        Err(error) => return Err(error),
    };
    let rank = width - basis.len();
    let pivots = fallible_collect(
        order[..rank].iter().map(|&i| positive[i]),
        rank,
        "unable to reserve compact pivot rows",
    )?;
    *positive = pivots;
    Ok(Some(basis))
}

fn terminal(
    certificate: SeparationCertificate,
    iterations: usize,
    note: &'static str,
) -> CompactModel {
    CompactModel::Terminal(DetectionProposal {
        method: SeparationMethod::Simplex,
        dropped: Vec::new(),
        iterations,
        status: MethodStatus::NoSeparation,
        certificate: Some(certificate),
        exact_fe_certificate: None,
        direction: None,
        note: note.into(),
    })
}

/// Recover an integer ray from the proposed support and its binding rows.
/// Neither the support nor numerical rank determines a sample deletion.
fn lift_ray(
    design: &BinaryDesign,
    outcome: &[f64],
    coefficients: &[f64],
    pivots: &[usize],
    tolerance: f64,
    (scratch, stage_work): (usize, usize),
) -> Result<Option<Vec<i64>>> {
    let scale = coefficients.iter().map(|v| v.abs()).fold(0., f64::max);
    if !scale.is_finite() || scale == 0. {
        return Ok(None);
    }
    let threshold = tolerance * scale;
    let active = fallible_collect(
        coefficients
            .iter()
            .enumerate()
            .filter_map(|(j, c)| (c.abs() > threshold).then_some(j)),
        coefficients.len(),
        "unable to reserve compact ray support",
    )?;
    let width = coefficients.len();
    let k = active.len();
    if k == 0 || k.saturating_mul(k + 8).saturating_add(3 * width) > scratch {
        return Ok(None);
    }
    let mut result = fallible_filled(width, 0i64, "unable to reserve compact raw ray")?;
    let smallest = active
        .iter()
        .map(|&j| coefficients[j].abs())
        .fold(scale, f64::min);
    for divisor in [1., 2., 3., 4., 6., 8., 12., 16.] {
        if scale / smallest * divisor > (1u64 << 42) as f64 {
            break;
        }
        for &j in &active {
            result[j] = (coefficients[j] / smallest * divisor).round() as i64;
        }
        if verify_ray(design, outcome, &result)? {
            return Ok(Some(result));
        }
    }

    let mut positions = fallible_filled(width, usize::MAX, "unable to reserve compact ray map")?;
    for (i, &column) in active.iter().enumerate() {
        positions[column] = i;
    }
    let mut space = ExactNullspace::new(k)?;
    let mut row = fallible_filled(k, 0., "unable to reserve compact binding row")?;
    let mut work = stage_work;
    for i in pivots.iter().copied().chain(0..outcome.len()) {
        crate::interrupt::row_checkpoint(i)?;
        if space.rank() >= k - 1 {
            break;
        }
        row.fill(0.);
        let mut score = 0.;
        for &column in design.row(i).iter() {
            let j = column as usize;
            if positions[j] != usize::MAX {
                row[positions[j]] = 1.;
                score += coefficients[j];
            }
        }
        if outcome[i] > 0. || score.abs() <= threshold * k as f64 {
            let Some(remaining) = work.checked_sub((space.rank() + 1).saturating_mul(k)) else {
                return Ok(None);
            };
            work = remaining;
            space.insert(&row)?;
        }
    }
    let mut free = fallible_filled(k, 0i64, "unable to reserve compact free coordinates")?;
    for resolution in [65536., 16_777_216., 4_294_967_296.] {
        for (&j, dest) in active.iter().zip(&mut free) {
            *dest = (coefficients[j] / scale * resolution).round() as i64;
        }
        if let Some(lift) = space.reconstruct(&free)? {
            for (&j, value) in active.iter().zip(lift) {
                result[j] = value;
            }
            if verify_ray(design, outcome, &result)? {
                return Ok(Some(result));
            }
        }
    }
    Ok(None)
}

/// Entries have already been checked to be binary. With at most usize::MAX
/// coordinates of type i64, their sum is exactly representable in i128.
fn verify_ray(design: &BinaryDesign, outcome: &[f64], coefficients: &[i64]) -> Result<bool> {
    let mut strict = false;
    for (i, &y) in outcome.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let score: i128 = design
            .row(i)
            .iter()
            .map(|&j| i128::from(coefficients[j as usize]))
            .sum();
        if score > 0 || (y > 0. && score != 0) {
            return Ok(false);
        }
        strict |= score < 0;
    }
    Ok(strict)
}

#[cfg(test)]
mod tests;
