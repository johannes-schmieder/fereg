//! Bipartite CMG mapping harvested from legacy fe/cmg.rs; no legacy routing or polish.
use super::{design::validate_matrix, Attempt, Backend, Design, StopReason};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    threads::ThreadTeam,
};
use cmg::{
    solve_pcg_into_with_workspace, CmgError, CmgOptions, CmgPreconditioner, Laplacian, PcgOptions,
    PcgWorkspace, PreparedLaplacianTopology,
};
use rayon::prelude::*;

/// Only the core pair is projected. Other FE coefficients remain zero until
/// the outer CMG adapter eliminates nuisance effects and applies certification.
#[derive(Debug, PartialEq)]
pub struct CmgCoreCandidate {
    pub coefficients: DenseMatrix,
    pub progress: Vec<Attempt>,
}
/// Graph, hierarchy and workspaces belong to exactly one immutable weight frame.
pub struct CmgCore<'a, 'data> {
    design: &'a Design<'data>,
    effects: [usize; 2],
    split: usize,
    graph: Laplacian,
    preconditioner: CmgPreconditioner,
    workspaces: Vec<PcgWorkspace>,
    minimum_mass_root: f64,
}
impl<'a, 'data> CmgCore<'a, 'data> {
    pub fn new(design: &'a Design<'data>, effects: [usize; 2]) -> Result<Self> {
        if effects[0] == effects[1]
            || effects.iter().any(|&effect| {
                design
                    .topology()
                    .effects()
                    .get(effect)
                    .is_none_or(|term| !term.intercept || !term.slopes.is_empty())
            })
        {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "CMG core requires two distinct categorical intercept effects",
            ));
        }
        if design.nobs() == 0 {
            return Err(FeregError::new(
                ErrorCode::InsufficientObservations,
                "empty CMG core",
            ));
        }
        let left = design.topology().effect_partition(effects[0])?;
        let right = design.topology().effect_partition(effects[1])?;
        let split = left.n_levels();
        let vertices = split
            .checked_add(right.n_levels())
            .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "CMG vertex count overflows"))?;
        let topology = PreparedLaplacianTopology::prepare(
            vertices,
            left.codes()
                .iter()
                .zip(right.codes())
                .map(|(&a, &b)| (a as usize, split + b as usize)),
        )
        .map_err(cmg_error)?;
        let mut weights = reserved(design.nobs())?;
        weights.extend((0..design.nobs()).map(|row| design.weights().at(row)));
        let graph = topology.assemble(&weights).map_err(cmg_error)?;
        let minimum_mass_root = graph
            .diagonal()
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
            .sqrt();
        if !minimum_mass_root.is_finite() || minimum_mass_root <= 0. {
            return Err(numerical("CMG core level mass is not representable"));
        }
        // The pinned API couples fallible construction to its budget entry.
        // An unbounded allowance selects fallible provisioning without adding
        // a second admission policy below the estimator's memory decision.
        let preconditioner =
            CmgPreconditioner::build_serial_with_budget(&graph, CmgOptions::default(), usize::MAX)
                .map_err(cmg_error)?;
        Ok(Self {
            design,
            effects,
            split,
            graph,
            preconditioner,
            workspaces: reserved(0)?,
            minimum_mass_root,
        })
    }
    #[must_use]
    pub fn hierarchy_levels(&self) -> usize {
        self.preconditioner.hierarchy().levels().len()
    }
    pub fn solve(
        &mut self,
        rhs: &DenseMatrix,
        budget: usize,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<CmgCoreCandidate> {
        validate_matrix(rhs, self.design.nobs())?;
        if budget == 0 || !tolerance.is_finite() || tolerance <= 0. || tolerance >= 1. {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "invalid CMG core controls",
            ));
        }
        while self.workspaces.len() < rhs.ncols() {
            self.workspaces.try_reserve(1).map_err(|_| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "unable to reserve CMG workspace bank",
                )
            })?;
            self.workspaces
                .push(PcgWorkspace::try_new(&self.preconditioner).map_err(cmg_error)?);
        }
        let moments = self.design.transpose_weighted(rhs, team)?;
        let mut results = reserved(rhs.ncols())?;
        results.resize_with(rhs.ncols(), || None);
        let context = SolveContext {
            design: self.design,
            effects: self.effects,
            split: self.split,
            graph: &self.graph,
            preconditioner: &self.preconditioner,
            rhs,
            moments: &moments,
            budget,
            tolerance,
            minimum_mass_root: self.minimum_mass_root,
        };
        // Every RHS has the same scalar CMG arithmetic in the call-owned team.
        // No dependency-owned executor or worker-count-dependent reduction is used.
        team.install(|| {
            results
                .par_iter_mut()
                .zip(&mut self.workspaces)
                .enumerate()
                .for_each(|(column, (output, workspace))| {
                    *output = Some(context.solve_column(column, workspace));
                });
        });
        let mut coefficients = DenseMatrix::zeros(self.design.layout().total, rhs.ncols())?;
        let mut progress = reserved(rhs.ncols())?;
        for (column, result) in results.into_iter().enumerate() {
            let (potential, attempt) = result.expect("visited CMG RHS")?;
            for (side, &effect) in self.effects.iter().enumerate() {
                let layout = &self.design.layout().effects[effect];
                let offset = if side == 0 { 0 } else { self.split };
                let sign = if side == 0 { 1. } else { -1. };
                for level in 0..layout.levels {
                    coefficients[(layout.offset + level, column)] =
                        sign * potential[offset + level];
                }
            }
            progress.push(attempt);
        }
        Ok(CmgCoreCandidate {
            coefficients,
            progress,
        })
    }
}
struct SolveContext<'a, 'data> {
    design: &'a Design<'data>,
    effects: [usize; 2],
    split: usize,
    graph: &'a Laplacian,
    preconditioner: &'a CmgPreconditioner,
    rhs: &'a DenseMatrix,
    moments: &'a DenseMatrix,
    budget: usize,
    tolerance: f64,
    minimum_mass_root: f64,
}
impl SolveContext<'_, '_> {
    fn solve_column(
        &self,
        column: usize,
        workspace: &mut PcgWorkspace,
    ) -> Result<(Vec<f64>, Attempt)> {
        let mut potential = reserved(self.graph.vertex_count())?;
        potential.resize(self.graph.vertex_count(), 0.);
        let mut rhs = reserved(self.graph.vertex_count())?;
        for (side, &effect) in self.effects.iter().enumerate() {
            let layout = &self.design.layout().effects[effect];
            let sign = if side == 0 { 1. } else { -1. };
            rhs.extend(
                (0..layout.levels)
                    .map(|level| sign * self.moments[(layout.offset + level, column)]),
            );
        }
        debug_assert_eq!(
            rhs.len(),
            self.split + self.design.layout().effects[self.effects[1]].levels
        );
        let mut norm = 0_f64;
        for row in 0..self.design.nobs() {
            norm = norm.hypot(self.design.weights().at(row).sqrt() * self.rhs[(row, column)]);
        }
        let mut attempt = Attempt {
            backend: Backend::Cmg,
            iterations: 0,
            refinements: 0,
            stop: StopReason::Converged,
        };
        if norm == 0. {
            return Ok((potential, attempt));
        }
        // ||D'Wr|| <= tol ||sqrt(W)y|| sqrt(min level mass) bounds every
        // normalized categorical level score. A backward-error stop that grows
        // with coefficient magnitude cannot supply this guarantee on weak graphs.
        let mut factors = [self.tolerance, norm, self.minimum_mass_root];
        factors.sort_unstable_by(f64::total_cmp);
        let absolute_tolerance = (factors[0] * factors[2]) * factors[1];
        if !absolute_tolerance.is_finite() || absolute_tolerance <= 0. {
            return Err(numerical(
                "CMG absolute residual target is not representable",
            ));
        }
        match solve_pcg_into_with_workspace(
            self.graph,
            self.preconditioner,
            &rhs,
            None,
            &mut potential,
            PcgOptions {
                relative_tolerance: 0.,
                absolute_tolerance,
                max_iterations: self.budget,
                ..PcgOptions::default()
            },
            workspace,
        ) {
            Ok(diagnostics) => attempt.iterations = diagnostics.iterations(),
            Err(error) => match numerical_stop(&error) {
                Some((iterations, stop)) => {
                    attempt.iterations = iterations;
                    attempt.stop = stop;
                }
                None => return Err(cmg_error(error)),
            },
        }
        if attempt.iterations > self.budget {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "CMG exceeded the remaining budget",
            ));
        }
        Ok((potential, attempt))
    }
}
fn numerical_stop(error: &CmgError) -> Option<(usize, StopReason)> {
    match error {
        CmgError::MaximumIterations { iterations, .. } => {
            Some((*iterations, StopReason::BudgetExhausted))
        }
        CmgError::PcgBreakdown { iteration, .. } => {
            Some((*iteration, StopReason::NumericalBreakdown))
        }
        CmgError::ResidualVerificationFailed { iteration, .. } => {
            Some((*iteration, StopReason::CertificateFailure))
        }
        _ => None,
    }
}
fn cmg_error(error: CmgError) -> FeregError {
    let code = match error {
        CmgError::AllocationFailed { .. }
        | CmgError::MemoryBudgetExceeded { .. }
        | CmgError::ParallelRuntime { .. } => ErrorCode::Allocation,
        CmgError::InvalidEdgeWeight { .. }
        | CmgError::NonFiniteMatrixValue { .. }
        | CmgError::NonPositivePivot { .. }
        | CmgError::IncompatibleLaplacianRhs { .. }
        | CmgError::PcgBreakdown { .. }
        | CmgError::MaximumIterations { .. }
        | CmgError::ResidualVerificationFailed { .. } => ErrorCode::NonConvergence,
        _ => ErrorCode::Internal,
    };
    FeregError::new(code, format!("CMG core failed: {error}"))
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
