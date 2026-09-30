//! Pinned within adapter: fixed Schwarz reductions and canonical coefficient recovery.
use super::{design::validate_matrix, Attempt, Backend, Design, StopReason};
use crate::{
    data::{DenseMatrix, Weights},
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
    threads::ThreadTeam,
};
use within::{
    BatchSolveResult, BuildError, Effect, LocalSolverConfig, LsmrOptions, PreconditionerConfig,
    ReductionStrategy, Solver, WithinError,
};

/// Upstream convergence is only a candidate; the shared certificate owns acceptance.
#[derive(Debug, PartialEq)]
pub struct LsmrCandidate {
    pub coefficients: DenseMatrix,
    pub progress: Vec<Attempt>,
}
/// One solver owns numerical state for exactly one immutable design/weight frame.
pub struct LsmrSession<'a, 'data> {
    pub(super) design: &'a Design<'data>,
    solver: Option<Solver<'a>>,
}
impl<'a, 'data> LsmrSession<'a, 'data> {
    pub fn new(design: &'a Design<'data>, team: &ThreadTeam) -> Result<Self> {
        let mut effects = reserved(design.layout().effects.len())?;
        for (index, term) in design.topology().effects().iter().enumerate() {
            let mut slopes = reserved(term.slopes.len())?;
            for slope in &term.slopes {
                slopes.push(design.sample().regressors[slope.source.get() as usize - 1].as_slice());
            }
            effects.push(
                Effect::new(
                    design.topology().effect_partition(index)?.codes(),
                    term.intercept,
                    slopes,
                )
                .map_err(build_error)?,
            );
        }
        let mut normalized = reserved(if matches!(design.weights(), Weights::Reference(_)) {
            design.nobs()
        } else {
            0
        })?;
        let weights = match design.weights() {
            Weights::Unit => None,
            Weights::Provided(values) => Some(values),
            Weights::Reference(_) => {
                normalized.extend((0..design.nobs()).map(|row| design.weights().at(row)));
                Some(normalized.as_slice())
            }
        };
        // Rank belongs to the shared FE engine; raw construction must not trim
        // a direction before the independent certificate sees the original span.
        // Explicit static configuration avoids upstream's adaptive default and
        // its worker-dependent Auto reduction. No nested worker pool is created.
        let config = PreconditionerConfig::Additive {
            local_solver: LocalSolverConfig::default(),
            reduction: ReductionStrategy::ParallelReduction,
        };
        let solver = if effects.is_empty() {
            None
        } else {
            Some(
                team.install(|| Solver::new_raw(effects, weights, config))
                    .map_err(build_error)?,
            )
        };
        Ok(Self { design, solver })
    }
    pub fn solve(
        &self,
        rhs: &DenseMatrix,
        budget: usize,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<LsmrCandidate> {
        self.solve_polled(rhs, budget, tolerance, None, team)
    }
    pub(super) fn solve_polled(
        &self,
        rhs: &DenseMatrix,
        budget: usize,
        tolerance: f64,
        poll: Poll<'_>,
        team: &ThreadTeam,
    ) -> Result<LsmrCandidate> {
        execution::check(poll)?;
        validate_matrix(rhs, self.design.nobs())?;
        if budget == 0 || !tolerance.is_finite() || tolerance <= 0. || tolerance >= 1. {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "invalid LSMR controls",
            ));
        }
        let mut coefficients = DenseMatrix::zeros(self.design.layout().total, rhs.ncols())?;
        let mut progress = reserved(rhs.ncols())?;
        if rhs.ncols() == 0 {
            return Ok(LsmrCandidate {
                coefficients,
                progress,
            });
        }
        let Some(solver) = &self.solver else {
            progress.resize(
                rhs.ncols(),
                Attempt {
                    refinements: 0,
                    backend: Backend::Lsmr,
                    iterations: 0,
                    stop: StopReason::NoEffects,
                },
            );
            return Ok(LsmrCandidate {
                coefficients,
                progress,
            });
        };
        let mut columns = reserved(rhs.ncols())?;
        for column in 0..rhs.ncols() {
            columns.push(rhs.column(column)?);
        }
        let mut slices = reserved(rhs.ncols())?;
        slices.extend(columns.iter().map(Vec::as_slice));
        let options = LsmrOptions {
            tol: tolerance,
            maxiter: budget,
            local_size: None,
        };
        let result = team
            .install(|| solver.solve_batch_with_poll(&slices, &options, poll))
            .map_err(solve_error)?;
        recover(self.design, &result, &mut coefficients)?;
        if result.converged.len() != rhs.ncols() || result.iterations.len() != rhs.ncols() {
            return Err(internal("within returned malformed progress"));
        }
        for (&converged, &iterations) in result.converged.iter().zip(&result.iterations) {
            if iterations > budget {
                return Err(internal("within exceeded the remaining iteration budget"));
            }
            progress.push(Attempt {
                refinements: 0,
                backend: Backend::Lsmr,
                iterations,
                stop: if converged {
                    StopReason::Converged
                } else if iterations == budget {
                    StopReason::BudgetExhausted
                } else {
                    StopReason::NumericalBreakdown
                },
            });
        }
        Ok(LsmrCandidate {
            coefficients,
            progress,
        })
    }
}
fn recover(design: &Design<'_>, result: &BatchSolveResult, target: &mut DenseMatrix) -> Result<()> {
    if result.n_obs != design.nobs()
        || result.n_dofs != target.nrows()
        || result.layout.n_dofs() != target.nrows()
        || result.layout.n_terms() != design.layout().effects.len()
        || result.x.len()
            != target
                .nrows()
                .checked_mul(target.ncols())
                .ok_or_else(|| internal("LSMR dimensions overflow"))?
    {
        return Err(internal(
            "within returned an incompatible coefficient layout",
        ));
    }
    for source in 0..result.n_dofs {
        let address = result
            .layout
            .address(source)
            .ok_or_else(|| internal("within coefficient address is missing"))?;
        let destination = design.coefficient_index(
            address.channel.term,
            address.channel.column,
            address.level as usize,
        )?;
        for column in 0..target.ncols() {
            let value = result.x[column * result.n_dofs + source];
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "LSMR coefficients are not finite",
                ));
            }
            target[(destination, column)] = value;
        }
    }
    Ok(())
}
fn build_error(error: BuildError) -> FeregError {
    let code = match error {
        BuildError::UnscalableComponent { .. }
        | BuildError::SingularDiagonal { .. }
        | BuildError::LocalSolverBuild(_) => ErrorCode::NonConvergence,
        BuildError::DofSpaceExceedsU32 { .. } => ErrorCode::Unsupported,
        BuildError::ThreadPool(_) => ErrorCode::Allocation,
        _ => ErrorCode::Internal,
    };
    FeregError::new(code, format!("LSMR construction failed: {error}"))
}
fn solve_error(error: WithinError) -> FeregError {
    match error {
        WithinError::Build(error) => build_error(error),
        WithinError::Solve(within::SolveError::Interrupted) => {
            FeregError::new(ErrorCode::Interrupted, "LSMR solve interrupted")
        }
        WithinError::Solve(within::SolveError::LocalSolveFailed { .. }) => FeregError::new(
            ErrorCode::NonConvergence,
            format!("LSMR solve failed: {error}"),
        ),
        _ => FeregError::new(ErrorCode::Internal, format!("LSMR solve failed: {error}")),
    }
}
fn internal(message: &str) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}
