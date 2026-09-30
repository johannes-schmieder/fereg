//! Comprehensive iterated rectification; guesses never authorize a deletion.
mod iteration;
use super::{invalid, Controls, DirectionProposal, Model, Status, View};
use crate::{
    error::{ErrorCode, FeregError, Result},
    ppml::{Interrupt, InterruptPoint},
    spec::Solver,
    threads::ThreadTeam,
    wls::Work,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IrFixedPoint {
    pub direction_scale: f64,
    pub interior_zero_relative: f64,
    pub boundary_violation_relative: f64,
    pub boundary_maximum_relative: f64,
}
/// Detector evidence is generation-bound and must be replayed or independently
/// certified before the coordinator can accept a terminal state or deletion.
#[derive(Debug, Clone, PartialEq)]
pub struct IrReport {
    pub generation: usize,
    pub sample_identity: u64,
    pub controls: Controls,
    pub solver: Solver,
    pub linear_tolerance: f64,
    pub span_tolerance: f64,
    pub budget: usize,
    pub iterations: usize,
    pub span_frames: usize,
    pub status: Status,
    pub direction: Option<DirectionProposal>,
    pub fixed_point: Option<IrFixedPoint>,
    pub work: Work,
}
/// Only fresh deterministic replay can construct terminal IR evidence.
#[derive(Debug, PartialEq)]
pub struct IrTerminal {
    generation: usize,
    sample_identity: u64,
    status: Status,
    iterations: usize,
    work: Work,
}
impl IrTerminal {
    #[must_use]
    pub const fn generation(&self) -> usize {
        self.generation
    }
    #[must_use]
    pub const fn sample_identity(&self) -> u64 {
        self.sample_identity
    }
    #[must_use]
    pub const fn status(&self) -> Status {
        self.status
    }
    #[must_use]
    pub const fn iterations(&self) -> usize {
        self.iterations
    }
    #[must_use]
    pub const fn work(&self) -> Work {
        self.work
    }
}
impl View<'_> {
    pub fn iterated_rectifier(
        self,
        model: &Model<'_>,
        max_iterations: usize,
        team: &ThreadTeam,
    ) -> Result<IrReport> {
        self.iterated_rectifier_with(model, max_iterations, team, &mut |_| Ok(()))
    }
    pub fn iterated_rectifier_with(
        self,
        model: &Model<'_>,
        max_iterations: usize,
        team: &ThreadTeam,
        interrupt: &mut Interrupt<'_>,
    ) -> Result<IrReport> {
        model.validate(self)?;
        if max_iterations == 0 {
            return Err(invalid("IR iteration budget must be positive"));
        }
        let options = model.projection_options();
        let report = IrReport {
            generation: self.generation(),
            sample_identity: self.numerical_identity(),
            controls: model.controls,
            solver: options.solver,
            linear_tolerance: options.tolerance,
            span_tolerance: model.span_options()?.tolerance,
            budget: max_iterations,
            iterations: 0,
            span_frames: 0,
            status: Status::NoCandidates,
            direction: None,
            fixed_point: None,
            work: Work::default(),
        };
        iteration::run(self, model, options, report, team, interrupt)
    }
    pub fn replay_ir(
        self,
        observed: &IrReport,
        model: &Model<'_>,
        max_iterations: usize,
        team: &ThreadTeam,
    ) -> Result<IrTerminal> {
        self.replay_ir_with(observed, model, max_iterations, team, &mut |_| Ok(()))
    }
    pub fn replay_ir_with(
        self,
        observed: &IrReport,
        model: &Model<'_>,
        max_iterations: usize,
        team: &ThreadTeam,
        interrupt: &mut Interrupt<'_>,
    ) -> Result<IrTerminal> {
        if observed.generation != self.generation()
            || observed.sample_identity != self.numerical_identity()
            || observed.budget != max_iterations
        {
            return Err(invalid(
                "terminal IR report belongs to a different generation or budget",
            ));
        }
        if observed.direction.is_some()
            || !matches!(observed.status, Status::NoCandidates | Status::NoSeparation)
        {
            return Err(failed(
                "terminal IR replay requires a no-drop detector result",
            ));
        }
        // Restart with the initial rectifier and fresh factors, rather than
        // accepting a producer's fixed-point flag or saved projected state.
        let replayed = self.iterated_rectifier_with(model, max_iterations, team, interrupt)?;
        if &replayed != observed {
            return Err(failed(
                "terminal IR evidence failed fresh deterministic replay",
            ));
        }
        Ok(IrTerminal {
            generation: replayed.generation,
            sample_identity: replayed.sample_identity,
            status: replayed.status,
            iterations: replayed.iterations,
            work: replayed.work,
        })
    }
}
fn failed(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
