//! One owner, canonical detector order and fresh final-sample estimation.
mod report;
mod round;
use super::{invalid, Controls, Ledger, Policy, SimplexLimits, DEFAULT_MU_TOLERANCE};
use crate::{
    data::{CompiledColumns, DenseMatrix, Prepared},
    error::{reserved, ErrorCode, FeregError, Result},
    ppml::{interrupt::Flow, irls, Interrupt, InterruptPoint},
    spec::Options,
    threads::ThreadTeam,
    wls::Role,
};
pub use report::{Event, EventKind, MethodSummary, Report, TRACE_LIMIT};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunControls {
    pub numerical: Controls,
    pub simplex: SimplexLimits,
    pub ir_iterations: usize,
    pub mu_tolerance: f64,
}
impl Default for RunControls {
    fn default() -> Self {
        Self {
            numerical: Controls::default(),
            simplex: SimplexLimits::default(),
            ir_iterations: 100,
            mu_tolerance: DEFAULT_MU_TOLERANCE,
        }
    }
}
impl RunControls {
    pub(crate) fn validate(self) -> Result<()> {
        self.numerical.validate()?;
        self.simplex.validate()?;
        if self.ir_iterations == 0
            || !self.mu_tolerance.is_finite()
            || self.mu_tolerance <= 0.
            || self.mu_tolerance >= 1.
        {
            return Err(invalid("invalid separation IR budget or Mu threshold"));
        }
        Ok(())
    }
}

/// A completed detector chain owns no exploratory fit, basis or factors.
/// Private construction prevents a prior generation becoming a final sample.
#[derive(Debug, PartialEq)]
pub struct FinalSample {
    sample: Prepared,
    raw: DenseMatrix,
    roles: Vec<Role>,
    options: Options,
    report: Report,
}
impl FinalSample {
    #[must_use]
    pub fn sample(&self) -> &Prepared {
        &self.sample
    }
    #[must_use]
    pub fn raw(&self) -> &DenseMatrix {
        &self.raw
    }
    #[must_use]
    pub fn roles(&self) -> &[Role] {
        &self.roles
    }
    #[must_use]
    pub fn report(&self) -> &Report {
        &self.report
    }
    /// Rank and final-Hessian authority are recomputed on the final sample.
    pub fn fit(&self, team: &ThreadTeam) -> Result<irls::Fit> {
        self.fit_with(team, &mut |_| Ok(()))
    }
    pub fn fit_with(&self, team: &ThreadTeam, interrupt: &mut Interrupt<'_>) -> Result<irls::Fit> {
        self.fit_polled(
            team,
            Flow {
                interrupt,
                probe: Default::default(),
            },
        )
    }
    pub(crate) fn fit_polled(&self, team: &ThreadTeam, flow: Flow<'_, '_>) -> Result<irls::Fit> {
        irls::fit_final_sample_polled(
            &irls::Problem {
                sample: &self.sample,
                raw: &self.raw,
                roles: &self.roles,
            },
            self.options,
            team,
            flow.interrupt,
            flow.probe,
        )
    }
    #[must_use]
    pub fn into_parts(self) -> (Prepared, DenseMatrix, Vec<Role>, Report) {
        (self.sample, self.raw, self.roles, self.report)
    }
}

pub fn run(
    sample: Prepared,
    columns: &CompiledColumns,
    options: Options,
    policy: Policy,
    controls: RunControls,
    team: &ThreadTeam,
) -> Result<FinalSample> {
    run_with(
        sample,
        columns,
        options,
        policy,
        controls,
        team,
        &mut |_| Ok(()),
    )
}
pub fn run_with(
    sample: Prepared,
    columns: &CompiledColumns,
    options: Options,
    policy: Policy,
    controls: RunControls,
    team: &ThreadTeam,
    interrupt: &mut Interrupt<'_>,
) -> Result<FinalSample> {
    run_polled(
        sample,
        columns,
        options,
        policy,
        controls,
        team,
        Flow {
            interrupt,
            probe: Default::default(),
        },
    )
}
pub(crate) fn run_polled(
    sample: Prepared,
    columns: &CompiledColumns,
    options: Options,
    policy: Policy,
    controls: RunControls,
    team: &ThreadTeam,
    flow: Flow<'_, '_>,
) -> Result<FinalSample> {
    let Flow { interrupt, probe } = flow;
    options.validate()?;
    controls.validate()?;
    if columns.roles.contains(&Role::AbsorbedIntercept)
        && !sample
            .topology
            .as_ref()
            .is_some_and(|t| t.effects().iter().any(|e| e.intercept))
    {
        return Err(invalid(
            "deferred constant requires the prepared intercept span",
        ));
    }
    let mut roles = reserved(columns.roles.len())?;
    roles.extend_from_slice(&columns.roles);
    let mut ledger = Ledger::new(sample, policy)?;
    let mut report = Report::new(policy, &ledger);
    interrupt(InterruptPoint::SeparationRound)?;
    if let Some(commit) = ledger.prune_initial_singletons()? {
        report.commit(None, commit)?;
    }
    loop {
        interrupt(InterruptPoint::SeparationRound)?;
        report.begin_round(ledger.view().generation())?;
        if round::exact(&mut ledger, &mut report)? {
            continue;
        }
        // Recipes read only the new owner. No old projected columns or
        // factors are compacted into the next numerical generation.
        let raw = columns.generate_polled(ledger.view().sample(), probe.dense())?;
        if (round::Runtime {
            options,
            controls,
            team,
            interrupt,
            probe,
        })
        .run(&mut ledger, &raw, &roles, &mut report)?
        {
            continue;
        }
        report.finish(&ledger)?;
        let sample = ledger.into_sample();
        if raw.nrows() != sample.n_active() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "final separation columns are stale",
            ));
        }
        return Ok(FinalSample {
            sample,
            raw,
            roles,
            options,
            report,
        });
    }
}
