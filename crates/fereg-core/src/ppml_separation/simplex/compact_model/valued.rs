//! The projected-cone search on general raw rows: binary or continuous
//! finite columns, any number of them, absorbed intercepts and slopes.
//!
//! Rows are read on demand through the caller's raw reader, so nothing is
//! stored per incidence. The positive-row null space, the coalesced boundary
//! projection and transposed phase I are the same numerical proposals as the
//! binary lane. Neither outcome is certified here: an integer ray is accepted
//! only through the caller's exact replay of every raw row, and a Farkas
//! vector is lifted to a physical dual for the requested IR certificate.
use super::super::verify_indexed_farkas_certificate;

mod revised_cone;
use super::super::{combine_basis, compact_tableau};
use super::projection::{self, Grow};
use super::sparse_echelon::{self, Echelon};
use super::{fallible_collect, fallible_filled, reserve, trace_refusal, CompactWork};
use crate::error::{ErrorCode, Result};
use crate::memory::fallible_copy;
use crate::ppml::ExactNullspace;
use crate::ppml_separation::SeparationConfig;

/// Raw reader: emits `(coordinate, value)` for one row. Zero values may be
/// emitted and are ignored.
pub(crate) type RawReader<'r> = &'r dyn Fn(usize, &mut dyn FnMut(usize, f64)) -> Result<()>;

pub(crate) struct ValuedDesign<'r> {
    pub(crate) width: usize,
    /// Upper bound on emitted coordinates per row, for work accounting.
    pub(crate) row_terms: usize,
    pub(crate) read: RawReader<'r>,
}

pub(crate) enum ValuedModel {
    /// Integer coefficients in the reader's coordinate order, already
    /// accepted by the caller's exact replay.
    Direction {
        coefficients: Vec<i64>,
        iterations: usize,
    },
    /// A physical observation dual proposal for the IR certificate.
    NumericalDual { dual: Vec<f64> },
    /// A ray whose exact integer lift failed, for the IR direction certificate.
    NumericalDirection {
        coefficients: Vec<f64>,
        iterations: usize,
    },
    /// A ray beyond 64-bit integers from the exact dyadic lift, replayed
    /// exactly by the caller.
    WideDirection {
        ray: super::WideRay,
        iterations: usize,
    },
    /// A no-separation terminal whose Farkas vector passed the indexed replay
    /// and whose exact rational dual passed the caller's exact moment check,
    /// in the form the binary lane returns it.
    Terminal(super::CompactModel),
}

/// Exact raw replay of an integer ray: true only for a certified ray.
pub(crate) type Verify<'v> = &'v mut dyn FnMut(&[i64]) -> Result<bool>;

/// Exact check of an integer dual: every zero row strictly positive and
/// every model moment exactly zero.
pub(crate) type DualCheck<'v> = &'v mut dyn FnMut(&super::ExactDual) -> Result<bool>;

/// The caller's exact checks of a proposed ray and of a proposed dual.
pub(crate) struct Verifiers<'v> {
    pub(crate) ray: Verify<'v>,
    pub(crate) wide: WideCheck<'v>,
    pub(crate) dual: DualCheck<'v>,
}

/// Exact raw replay of a wide integer ray: true only for a certified ray.
pub(crate) type WideCheck<'v> = &'v mut dyn FnMut(&super::WideRay) -> Result<bool>;

/// Column-major copy of a dense cone basis: one row's projection visits only
/// its own terms.
struct Columns {
    by_column: Vec<f64>,
    dimension: usize,
}

impl Columns {
    fn new(basis: &[Vec<f64>], width: usize) -> Result<Self> {
        let dimension = basis.len();
        let mut by_column =
            fallible_filled(width * dimension, 0., "unable to reserve valued basis")?;
        for (k, vector) in basis.iter().enumerate() {
            for (j, &value) in vector.iter().enumerate() {
                by_column[j * dimension + k] = value;
            }
        }
        Ok(Self {
            by_column,
            dimension,
        })
    }

    /// `out_j = sum_k basis_k[j] y_k`.
    fn combine(&self, y: &[f64], out: &mut [f64]) {
        let dimension = self.dimension;
        for (o, column) in out
            .iter_mut()
            .zip(self.by_column.chunks_exact(dimension.max(1)))
        {
            *o = column.iter().zip(y).map(|(&b, &c)| b * c).sum();
        }
    }

    fn project(
        &self,
        read: RawReader<'_>,
        row: usize,
        out: &mut [f64],
        (tolerance, row_terms): (f64, usize),
        work: &mut usize,
    ) -> Result<bool> {
        let dimension = self.dimension;
        let Some(left) = work.checked_sub((row_terms.max(1) + 1).saturating_mul(dimension)) else {
            return Ok(false);
        };
        *work = left;
        out.fill(0.);
        let mut scale = 0_f64;
        let width = self.by_column.len() / dimension.max(1);
        read(row, &mut |j, v| {
            if v != 0. && j < width {
                scale += v.abs();
                let column = &self.by_column[j * dimension..(j + 1) * dimension];
                for (acc, &b) in out.iter_mut().zip(column) {
                    *acc += v * b;
                }
            }
        })?;
        for value in out.iter_mut() {
            if value.abs() <= tolerance * scale.max(1.) {
                *value = 0.;
            }
        }
        Ok(true)
    }
}

/// Pivot counter, interrupt poll and memory request of one search.
/// Interrupt and storage hooks of the exact dual.
type ExactHooks<'i, 'g> = (
    &'i mut dyn FnMut() -> Result<()>,
    &'g mut dyn FnMut(usize) -> Option<usize>,
);
type Hooks<'p, 'i, 'g> = (
    &'p mut usize,
    &'i mut dyn FnMut() -> Result<()>,
    &'g mut dyn FnMut(usize) -> Option<usize>,
);

pub(crate) struct ValuedSearch<'a, 'r> {
    pub(crate) outcome: &'a [f64],
    pub(crate) design: &'a ValuedDesign<'r>,
    pub(crate) config: SeparationConfig,
    pub(crate) work: CompactWork,
    pub(crate) available: usize,
    /// Words held by the live echelon form, counted in every request.
    pub(crate) held: usize,
}

impl ValuedSearch<'_, '_> {
    fn require(&mut self, needed: usize, grow: &mut dyn FnMut(usize) -> Option<usize>) -> bool {
        let needed = needed.saturating_add(self.held);
        if needed <= self.available {
            return true;
        }
        match grow(needed).filter(|&g| g >= needed) {
            Some(granted) => {
                self.available = granted;
                true
            }
            None => false,
        }
    }

    pub(crate) fn run(
        &mut self,
        pivots: &mut usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
        verify: &mut Verifiers<'_>,
    ) -> Result<Option<ValuedModel>> {
        let (outcome, width) = (self.outcome, self.design.width);
        let positive: Vec<usize> = fallible_collect(
            (0..outcome.len()).filter(|&i| outcome[i] > 0.),
            outcome.len(),
            "unable to reserve valued positive rows",
        )?;
        if width == 0 || positive.is_empty() || positive.len() == outcome.len() {
            return Ok(None);
        }
        let tolerance = self.config.projection_tolerance;
        let stage = self.work.stage;
        let base = positive.capacity();
        let read = self.design.read;
        let mut each = |i: usize, f: &mut dyn FnMut(usize, f64)| read(i, f);
        let eliminated = {
            let mut admit = |words: usize| self.require(base.saturating_add(words), grow);
            sparse_echelon::eliminate(
                &positive, width, &mut each, tolerance, stage, &mut admit, interrupt,
            )?
        };
        let echelon = match eliminated {
            Ok(echelon) => echelon,
            Err(refusal) => {
                trace_refusal(refusal.stage, refusal.needed, refusal.available);
                return Ok(None);
            }
        };
        let nullity = echelon.nullity();
        let spent = echelon.work.saturating_add(echelon.basis_work());
        if spent > stage {
            trace_refusal("valued_basis_work", spent, stage);
            return Ok(None);
        }
        let held = base
            .saturating_add(echelon.words)
            .saturating_add(nullity.saturating_mul(width));
        if !self.require(held, grow) {
            trace_refusal("valued_basis_cells", held, self.available);
            return Ok(None);
        }
        let basis = match echelon.basis(interrupt) {
            Ok(basis) => basis,
            Err(error) if error.code == ErrorCode::NonConvergence => {
                trace_refusal("valued_nullspace", nullity, width);
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let representatives = echelon.pivot_rows(&positive)?;
        drop(positive);
        if crate::diagnostics::solver_trace_enabled() {
            eprintln!(
                "FEREG_SOLVER_TRACE|phase=valued_echelon|positive={base}|rank={}|nullity={nullity}|width={width}|work={}|words={}",
                echelon.rank(),
                echelon.work,
                echelon.words
            );
        }
        self.held = echelon.words.saturating_add(nullity.saturating_mul(width));
        let result = self.search(
            basis,
            &representatives,
            &echelon,
            (pivots, interrupt, grow),
            verify,
        );
        self.held = 0;
        result
    }

    fn search(
        &mut self,
        basis: Vec<Vec<f64>>,
        representatives: &[usize],
        echelon: &Echelon,
        (pivots, interrupt, grow): Hooks<'_, '_, '_>,
        verify: &mut Verifiers<'_>,
    ) -> Result<Option<ValuedModel>> {
        let outcome = self.outcome;
        let boundary: Vec<usize> = fallible_collect(
            (0..outcome.len()).filter(|&i| outcome[i] == 0.),
            outcome.len(),
            "unable to reserve valued boundary rows",
        )?;
        if basis.is_empty() {
            // Positive rows span the model: unit boundary mass balances.
            let weights = fallible_filled(boundary.len(), 1., "unable to reserve unit dual mass")?;
            return self.dual(&boundary, &weights, representatives, echelon, grow);
        }
        self.cone(
            basis,
            (&boundary, representatives, echelon),
            pivots,
            interrupt,
            grow,
            verify,
        )
    }

    /// Project every boundary row on the column-major basis into the
    /// coalesced cone bank, requested by its measured size.
    /// Project every boundary row on the column-major basis into the
    /// coalesced cone bank, requested by its measured size. When the plan
    /// refuses the bank at the full dimension, rank the axes in two streaming
    /// passes first and rebuild it at the retained dimension.
    fn project(
        &mut self,
        basis: &mut Vec<Vec<f64>>,
        boundary: &[usize],
        fixed: usize,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<projection::BoundaryCone>> {
        let (width, tolerance) = (self.design.width, self.config.projection_tolerance);
        let columns = Columns::new(basis, width)?;
        if let Some(cone) = self.project_at(&columns, boundary, fixed, grow)? {
            return Ok(Some(cone));
        }
        let dimension = basis.len();
        if dimension > 1 {
            let (read, terms) = (self.design.read, self.design.row_terms);
            let mut projector = |i: usize, out: &mut [f64], work: &mut usize| {
                columns.project(read, boundary[i], out, (tolerance, terms), work)
            };
            let shape = (boundary.len(), dimension);
            let stage = self.work.stage;
            let keep = projection::streamed_axes_with(shape, &mut projector, tolerance, stage)?;
            if let Some(keep) = keep.filter(|keep| keep.iter().any(|&kept| !kept)) {
                drop(columns);
                projection::retain_axes(basis, &keep);
                let columns = Columns::new(basis, width)?;
                if let Some(cone) = self.project_at(&columns, boundary, fixed, grow)? {
                    return Ok(Some(cone));
                }
            }
        }
        super::trace_unprojected("valued", dimension, self.available);
        Ok(None)
    }

    fn project_at(
        &mut self,
        columns: &Columns,
        boundary: &[usize],
        fixed: usize,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<projection::BoundaryCone>> {
        let (width, dimension) = (self.design.width, columns.dimension);
        let tolerance = self.config.projection_tolerance;
        let basis_cells = dimension.saturating_mul(width.saturating_add(3));
        let (read, terms) = (self.design.read, self.design.row_terms);
        let mut projector = |i: usize, out: &mut [f64], work: &mut usize| {
            columns.project(read, boundary[i], out, (tolerance, terms), work)
        };
        let mut granted = None;
        let mut extend = |needed: usize| {
            let total = fixed.checked_add(needed)?;
            let grant = grow(total).filter(|&g| g >= total)?;
            granted = Some(grant);
            Some(grant - fixed)
        };
        let cone_budget = self
            .available
            .saturating_sub(fixed.saturating_add(self.held));
        let built = projection::streamed_with(
            (boundary.len(), dimension),
            &mut projector,
            (cone_budget, self.work.stage),
            basis_cells,
            Some(&mut extend as Grow<'_>),
        )?;
        if let Some(grant) = granted {
            self.available = self.available.max(grant);
        }
        Ok(built)
    }

    fn cone(
        &mut self,
        mut basis: Vec<Vec<f64>>,
        (boundary, representatives, echelon): (&[usize], &[usize], &Echelon),
        pivots: &mut usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
        verify: &mut Verifiers<'_>,
    ) -> Result<Option<ValuedModel>> {
        let (width, dimension) = (self.design.width, basis.len());
        let tolerance = self.config.projection_tolerance;
        let fixed = boundary
            .len()
            .saturating_add(representatives.len())
            .saturating_add(width);
        let Some(mut cone) = self.project(&mut basis, boundary, fixed, grow)? else {
            let sets = (boundary, representatives, echelon);
            return self.revised_cone(basis, sets, pivots, (interrupt, grow), verify);
        };
        let live = fixed.saturating_add(cone.live_cells);
        let mut reduced = {
            let stage = self.work.stage;
            let mut admit = |words: usize| self.require(live.saturating_add(words), grow);
            super::equalities::reduce(&cone, &basis, tolerance, stage, &mut admit, interrupt)?
        };
        // Keep only the searched cone's axes on which some distinct row is
        // independent: the reduced cone when equalities pinned the rest.
        let stage = self.work.stage;
        let kept = match reduced.as_mut() {
            Some(r) => projection::retain_independent(&mut r.cone, &mut r.basis, tolerance, stage)?,
            None => projection::retain_independent(&mut cone, &mut basis, tolerance, stage)?,
        };
        if !kept {
            return Ok(None);
        }
        let retained = reduced.as_ref().map_or(basis.len(), |r| r.basis.len());
        // Rank of the projected boundary rows: the equality rows and the
        // retained axes of the cone that phase I searches.
        let cone_rank = reduced
            .as_ref()
            .map_or(retained, |r| r.rank() + r.basis.len());
        let live = live.saturating_add(reduced.as_ref().map_or(0, |r| r.cells));
        let search = reduced.as_ref().map_or(&cone, |r| &r.cone);
        let axes = reduced.as_ref().map_or(retained, |r| r.basis.len());
        let dual_cells = boundary.len().saturating_mul(2).saturating_add(1);
        let tableau = compact_tableau::workspace_cells(search.rows.len(), axes)
            .unwrap_or(usize::MAX)
            .max(dual_cells);
        if !self.require(live.saturating_add(tableau), grow) {
            trace_refusal(
                "valued_tableau_cells",
                live.saturating_add(tableau),
                self.available,
            );
            return Ok(None);
        }
        let policy = (
            tolerance,
            self.config.simplex_max_iterations,
            self.work.cell_visits,
            self.work.pivots_per_dimension,
        );
        let proposal = if axes == 0 {
            Some(compact_tableau::Proposal::PositiveWeights {
                weights: fallible_filled(
                    boundary.len(),
                    1.,
                    "unable to reserve unit cone weights",
                )?,
                iterations: 0,
            })
        } else {
            let free = self
                .available
                .saturating_sub(live.saturating_add(self.held));
            search.propose(boundary.len(), policy, pivots, free, interrupt)?
        };
        if crate::diagnostics::solver_trace_enabled() {
            let branch = super::tableau_branch(&proposal);
            eprintln!(
                "FEREG_SOLVER_TRACE|phase=valued_tableau|dimension={dimension}|retained={retained}|boundary={}|distinct={}|branch={branch}",
                boundary.len(),
                cone.rows.len()
            );
        }
        match proposal {
            Some(compact_tableau::Proposal::Direction {
                coordinates,
                iterations,
            }) => {
                drop(cone);
                let basis = reduced.map_or(basis, |r| r.basis);
                let coefficients = combine_basis(&basis, &coordinates)?;
                drop(basis);
                let sets = (coefficients, representatives, iterations);
                self.direction_model(sets, (interrupt, grow), verify)
            }
            Some(compact_tableau::Proposal::PositiveWeights {
                weights,
                iterations,
            }) => {
                let weights = match &reduced {
                    Some(r) => r.complete(&cone, weights)?,
                    None => weights,
                };
                drop(reduced);
                let dual = fallible_collect(
                    weights.iter().map(|w| -*w).chain(std::iter::once(-1.)),
                    weights.len() + 1,
                    "unable to reserve valued Farkas vector",
                )?;
                let passed = verify_indexed_farkas_certificate(
                    &cone.rows,
                    &cone.index,
                    &dual,
                    self.config.certificate_tolerance,
                )?;
                drop(cone);
                drop(dual);
                let Some(certificate) = passed else {
                    trace_refusal("valued_numerical_farkas", boundary.len(), 0);
                    return Ok(None);
                };
                let sets = (boundary, &weights[..], representatives);
                let target = representatives.len() + cone_rank;
                let proof = (certificate, iterations);
                let terminal = self.exact_terminal(
                    sets,
                    target,
                    proof,
                    (&mut *interrupt, &mut *grow),
                    verify,
                )?;
                if let Some(model) = terminal {
                    return Ok(Some(ValuedModel::Terminal(model)));
                }
                self.dual(boundary, &weights, representatives, echelon, grow)
            }
            None => Ok(None),
        }
    }

    /// Integer coefficients from the proposed support: binding rows (all
    /// representatives and the zero rows the proposal leaves at zero) fix an
    /// exact modular relation; small integer scalings are tried first. The
    /// caller's exact replay of every raw row decides.
    fn lift(
        &mut self,
        coefficients: &[f64],
        representatives: &[usize],
        iterations: usize,
        verify: Verify<'_>,
    ) -> Result<Option<ValuedModel>> {
        let width = coefficients.len();
        let scale = coefficients.iter().map(|v| v.abs()).fold(0., f64::max);
        if !(scale.is_finite() && scale > 0.) {
            return Ok(None);
        }
        let threshold = self.config.projection_tolerance * scale;
        let active: Vec<usize> = fallible_collect(
            (0..width).filter(|&j| coefficients[j].abs() > threshold),
            width,
            "unable to reserve valued ray support",
        )?;
        let mut candidate = fallible_filled(width, 0i64, "unable to reserve valued ray")?;
        let smallest = active
            .iter()
            .map(|&j| coefficients[j].abs())
            .fold(scale, f64::min);
        for divisor in [1., 2., 3., 4., 6., 8., 12., 16.] {
            if scale / smallest * divisor > (1u64 << 42) as f64 {
                break;
            }
            for &j in &active {
                candidate[j] = (coefficients[j] / smallest * divisor).round() as i64;
            }
            if verify(&candidate)? {
                return Ok(Some(ValuedModel::Direction {
                    coefficients: candidate,
                    iterations,
                }));
            }
        }
        // Exact relation through the binding rows on the active coordinates.
        let k = active.len();
        let mut positions = fallible_filled(width, usize::MAX, "unable to reserve valued ray map")?;
        for (p, &j) in active.iter().enumerate() {
            positions[j] = p;
        }
        let mut space = ExactNullspace::new(k)?;
        let mut row = fallible_filled(k, 0., "unable to reserve valued binding row")?;
        let mut work = self.work.stage;
        let rows = self.outcome.len();
        for i in representatives.iter().copied().chain(0..rows) {
            crate::interrupt::row_checkpoint(i)?;
            if space.rank() + 1 >= k {
                break;
            }
            row.fill(0.);
            let mut score = 0.;
            let mut magnitude = 0_f64;
            (self.design.read)(i, &mut |j, v| {
                if j < width && positions[j] != usize::MAX {
                    row[positions[j]] += v;
                    score += v * coefficients[j];
                    magnitude += (v * coefficients[j]).abs();
                }
            })?;
            if self.outcome[i] > 0. || score.abs() <= threshold * magnitude.max(1.) {
                let Some(left) = work.checked_sub((space.rank() + 1).saturating_mul(k)) else {
                    trace_refusal("valued_ray_lift_work", k, self.work.stage);
                    return Ok(None);
                };
                work = left;
                space.insert(&row)?;
            }
        }
        let mut free = fallible_filled(k, 0i64, "unable to reserve valued free coordinates")?;
        for resolution in [65536., 16_777_216., 4_294_967_296.] {
            for (&j, dest) in active.iter().zip(&mut free) {
                *dest = (coefficients[j] / scale * resolution).round() as i64;
            }
            if let Some(lift) = space.reconstruct(&free)? {
                candidate.fill(0);
                for (&j, value) in active.iter().zip(lift) {
                    candidate[j] = value;
                }
                if verify(&candidate)? {
                    return Ok(Some(ValuedModel::Direction {
                        coefficients: candidate,
                        iterations,
                    }));
                }
            }
        }
        trace_refusal("valued_ray_lift", k, width);
        Ok(None)
    }

    /// A proposed ray: its small-integer lift, else its exact dyadic lift,
    /// else the numerical direction for IR.
    fn direction_model(
        &mut self,
        (coefficients, representatives, iterations): (Vec<f64>, &[usize], usize),
        (interrupt, grow): ExactHooks<'_, '_>,
        verify: &mut Verifiers<'_>,
    ) -> Result<Option<ValuedModel>> {
        let mut lifted = self.lift(&coefficients, representatives, iterations, &mut *verify.ray)?;
        if lifted.is_none() {
            lifted = self.wide_lift(
                (&coefficients, representatives, iterations),
                (interrupt, grow),
                &mut *verify.wide,
            )?;
        }
        Ok(lifted.or(Some(ValuedModel::NumericalDirection {
            coefficients,
            iterations,
        })))
    }

    /// The exact dyadic lift of a ray whose small-integer lift failed: the
    /// representatives, the zero rows the proposal leaves at zero and the
    /// remaining positive rows bind it on its support.
    fn wide_lift(
        &mut self,
        (coefficients, representatives, iterations): (&[f64], &[usize], usize),
        (interrupt, grow): ExactHooks<'_, '_>,
        verify: WideCheck<'_>,
    ) -> Result<Option<ValuedModel>> {
        let width = coefficients.len();
        let scale = coefficients.iter().map(|v| v.abs()).fold(0., f64::max);
        if !(scale.is_finite() && scale > 0.) {
            return Ok(None);
        }
        let threshold = self.config.projection_tolerance * scale;
        let active: Vec<usize> = fallible_collect(
            (0..width).filter(|&j| coefficients[j].abs() > threshold),
            width,
            "unable to reserve wide ray support",
        )?;
        let rows = self.outcome.len();
        let mut binding = fallible_copy(representatives, "unable to reserve wide ray rows")?;
        let mut positive = Vec::new();
        for i in 0..rows {
            crate::interrupt::row_checkpoint(i)?;
            let list = if self.outcome[i] > 0. {
                &mut positive
            } else {
                let (mut score, mut magnitude) = (0., 0_f64);
                (self.design.read)(i, &mut |j, v| {
                    if j < width {
                        score += v * coefficients[j];
                        magnitude += (v * coefficients[j]).abs();
                    }
                })?;
                if score.abs() > threshold * magnitude.max(1.) {
                    continue;
                }
                &mut binding
            };
            reserve(list, 1, "unable to reserve wide ray rows")?;
            list.push(i);
        }
        reserve(
            &mut binding,
            positive.len(),
            "unable to reserve wide ray rows",
        )?;
        binding.extend_from_slice(&positive);
        drop(positive);
        let (read, stage) = (self.design.read, self.work.stage);
        let mut admit = |words: usize| self.require(words, grow);
        let sets = (&active[..], &binding[..]);
        let built =
            super::exact_dyadic::exact_ray(read, coefficients, sets, stage, &mut admit, interrupt)?;
        let ray = match built {
            Ok(ray) => ray,
            Err(reason) => {
                trace_refusal(reason, active.len(), width);
                return Ok(None);
            }
        };
        if !verify(&ray)? {
            trace_refusal("exact_ray_replay", active.len(), width);
            return Ok(None);
        }
        Ok(Some(ValuedModel::WideDirection { ray, iterations }))
    }

    /// The exact rational dual of an accepted Farkas vector, or `None` when
    /// it cannot be formed or the caller's exact moment check refuses it.
    fn exact_terminal(
        &mut self,
        sets: (&[usize], &[f64], &[usize]),
        target: usize,
        (certificate, iterations): (crate::ppml_separation::SeparationCertificate, usize),
        (interrupt, grow): ExactHooks<'_, '_>,
        verify: &mut Verifiers<'_>,
    ) -> Result<Option<super::CompactModel>> {
        let (read, width) = (self.design.read, self.design.width);
        let stage = self.work.stage;
        let mut admit = |words: usize| self.require(words, grow);
        let built = super::exact_dyadic::exact_dual(
            read, width, sets, target, stage, &mut admit, interrupt,
        )?;
        let dual = match built {
            Ok(dual) => dual,
            Err(reason) => {
                trace_refusal(reason, target, width);
                return Ok(None);
            }
        };
        let certified = (verify.dual)(&dual)?;
        if crate::diagnostics::solver_trace_enabled() {
            eprintln!(
                "FEREG_SOLVER_TRACE|phase=valued_exact_dual|target={target}|basis={}|rows={}|bits={}|certified={}",
                dual.basis.len(),
                dual.basis.len() + dual.integers.len(),
                dual.basis.iter().map(|(_, d)| d.bits()).max().unwrap_or(0),
                u8::from(certified)
            );
        }
        Ok(certified.then(|| {
            super::terminal(
                certificate,
                iterations,
                "model simplex phase-I infeasibility passed an independent Farkas certificate",
            )
        }))
    }

    fn dual(
        &mut self,
        boundary: &[usize],
        weights: &[f64],
        representatives: &[usize],
        echelon: &Echelon,
        grow: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Result<Option<ValuedModel>> {
        let (rows, width) = (self.outcome.len(), self.design.width);
        let words = rows
            .saturating_add(representatives.len().saturating_mul(2))
            .saturating_add(width.saturating_mul(6))
            .saturating_add(boundary.len());
        if !self.require(words, grow) {
            trace_refusal("valued_dual_cells", words, self.available);
            return Ok(None);
        }
        let read = self.design.read;
        let mut each = |i: usize, f: &mut dyn FnMut(usize, f64)| read(i, f);
        Ok(sparse_echelon::physical_dual(
            (&mut each, width, self.design.row_terms),
            self.outcome,
            (boundary, weights, representatives),
            echelon,
            self.work.stage,
        )?
        .map(|dual| ValuedModel::NumericalDual { dual }))
    }
}
