//! PPML Auto two-FE CMG reuse.
//!
//! One bounded Schur hierarchy, built on one IRLS frame's weights, serves as a
//! fixed preconditioner for later frames' exact current-weight operators: the
//! retained block of joint ordinary frames and the categorical strict
//! reprojection. It is solve acceleration only (SPEC section 13). Every
//! operator, stopping target and certificate uses the current frame, and an
//! estimation makes at most two construction attempts, refusals included.
use super::*;
use crate::fe::cmg_schur::CmgSchurPreconditioner;
use crate::fe::slope_coefficients::joint::JointBlockPreconditioner;
use crate::fe::MapGeometryGeneration;

const MAX_ATTEMPTS: u8 = 2;

pub(crate) struct PpmlCmg {
    budget: Option<FeSolverMemoryBudget>,
    attempts: u8,
    /// Weight generation of the current categorical frame.
    frame: Option<MapGeometryGeneration>,
    /// Weight generation the installed hierarchy was built on.
    built: Option<MapGeometryGeneration>,
    /// A stale hierarchy and its generation, set aside while a categorical
    /// projection may refresh it, and restored unless replaced.
    aside: Option<(Box<CmgSchurPreconditioner>, Option<MapGeometryGeneration>)>,
    roots: Vec<f64>,
    raw: Vec<f64>,
    solved: Vec<f64>,
}

impl PpmlCmg {
    /// Retained banks for joint applications over the smaller effect.
    pub(crate) fn bytes(levels: usize) -> Result<u128> {
        (levels as u128)
            .checked_mul(3 * std::mem::size_of::<f64>() as u128)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>() as u128))
            .ok_or_else(|| allocation("PPML CMG application banks overflow"))
    }
}

impl ReusableCategoricalMapAbsorber<'_> {
    /// Enable reuse for a PPML Auto owner with exactly two intercept effects.
    /// The h16/k1 route builds for a strict reprojection; joint frames build
    /// through [`JointCmgBlock`].
    pub(crate) fn enable_ppml_cmg(
        &mut self,
        budget: Option<FeSolverMemoryBudget>,
        allowance: u128,
    ) -> Result<()> {
        let Some(graph) = self.two_fe_pcg.as_ref() else {
            return Ok(());
        };
        let levels = graph.right_weights.len();
        crate::memory::require_admitted_capacity(PpmlCmg::bytes(levels)?, allowance)?;
        let mut reservation = SystemWorkspaceReservation;
        let message = "unable to reserve PPML CMG application banks";
        self.ppml_cmg = Some(Box::new(PpmlCmg {
            budget,
            attempts: 0,
            frame: None,
            built: None,
            aside: None,
            roots: reserved_zeros(levels, &mut reservation, message)?,
            raw: reserved_zeros(levels, &mut reservation, message)?,
            solved: reserved_zeros(levels, &mut reservation, message)?,
        }));
        self.request_auto_routing(crate::fe::RoutingPreset::new(16, 1)?, budget, 0);
        Ok(())
    }

    pub(super) fn observe_ppml_cmg_frame(&mut self, generation: MapGeometryGeneration) {
        if let Some(state) = self.ppml_cmg.as_mut() {
            state.frame = Some(generation);
        }
    }

    /// Before a categorical PPML projection. While an attempt remains, a
    /// hierarchy from an earlier weight generation is set aside, so the h16
    /// route can rebuild it on current weights; its bytes stay charged to that
    /// rebuild. Without an attempt the route is withheld.
    pub(super) fn begin_ppml_cmg_projection(&mut self) -> Result<Option<AutoSchurRoute>> {
        let (Some(state), Some(graph)) = (self.ppml_cmg.as_mut(), self.two_fe_pcg.as_mut()) else {
            return Ok(None);
        };
        if state.attempts >= MAX_ATTEMPTS {
            return Ok(self.auto_route.take());
        }
        let mut resident = 0;
        if state.built != state.frame {
            if let Some(stale) = graph.cmg_schur.take() {
                resident = stale.retained_bytes()?;
                state.aside = Some((stale, state.built.take()));
            }
        }
        if graph.cmg_schur.is_some() {
            return Ok(self.auto_route.take());
        }
        if let Some(route) = self.auto_route.as_mut() {
            route.setup = crate::fe::routing::SetupLedger::new();
            route.request.budget = state
                .budget
                .map(|budget| -> Result<FeSolverMemoryBudget> {
                    Ok(FeSolverMemoryBudget {
                        execution_bytes: budget
                            .execution_bytes
                            .checked_add(resident)
                            .ok_or_else(|| allocation("PPML CMG refresh admission overflows"))?,
                        ..budget
                    })
                })
                .transpose()?;
        }
        Ok(None)
    }

    /// Restore a withheld route; otherwise count the route's attempt and
    /// reinstate a set-aside hierarchy that was not replaced.
    pub(super) fn finish_ppml_cmg_projection(&mut self, withheld: Option<AutoSchurRoute>) {
        if withheld.is_some() {
            self.auto_route = withheld;
            return;
        }
        let (Some(state), Some(graph)) = (self.ppml_cmg.as_mut(), self.two_fe_pcg.as_mut()) else {
            return;
        };
        if let Some(route) = self.auto_route.as_mut() {
            route.request.budget = state.budget;
            if route.setup.attempted {
                state.attempts = state.attempts.saturating_add(1);
                if graph.cmg_schur.is_some() {
                    state.built = state.frame;
                }
            }
        }
        if let Some((stale, built)) = state.aside.take() {
            if graph.cmg_schur.is_none() {
                graph.cmg_schur = Some(stale);
                state.built = built;
            }
        }
    }

    /// Joint-frame construction on the current frame's weights, when none is
    /// installed and an attempt remains. A refusal consumes the attempt.
    fn install_ppml_cmg(&mut self, weights: &[f64]) -> Result<bool> {
        let (Some(state), Some(graph)) = (self.ppml_cmg.as_mut(), self.two_fe_pcg.as_mut()) else {
            return Ok(false);
        };
        if graph.cmg_schur.is_some() {
            return Ok(true);
        }
        if state.attempts >= MAX_ATTEMPTS || weights.len() != self.design.nobs() {
            return Ok(false);
        }
        state.attempts += 1;
        let built = CmgSchurPreconditioner::try_build_bounded(
            &self.design,
            BorrowedMapWeights::Provided(weights),
            graph.left_effect,
            5,
            crate::fe::cmg_schur::Sampling {
                samples: 1,
                seed: 0,
            },
            self.max_rhs,
            state.budget,
            false,
        )?;
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=ppml_joint_cmg|attempt={}|installed={}|refusal={:?}",
                state.attempts,
                u8::from(built.preconditioner.is_some()),
                built.refusal
            ));
        }
        state.built = built.preconditioner.as_ref().and(state.frame);
        graph.cmg_schur = built.preconditioner.map(Box::new);
        Ok(graph.cmg_schur.is_some())
    }
}

/// The retained hierarchy as the retained-block preconditioner of a joint
/// PPML frame. Joint coordinates of the retained effect are `sqrt(D_f)
/// alpha_f`, so its block operator is `D^-1/2 S D^-1/2` for the raw Schur
/// complement `S`, and a hierarchy `B` for `S` applies as `D^1/2 B D^1/2`
/// with the current level masses `D`.
pub(crate) struct JointCmgBlock<'s, 'a> {
    absorber: &'s mut ReusableCategoricalMapAbsorber<'a>,
    weights: &'s [f64],
    masses: bool,
    attempted: bool,
}

impl<'s, 'a> JointCmgBlock<'s, 'a> {
    /// `retained` is the joint solve's retained effect. A missing owner,
    /// another orientation, or an owner without enabled reuse declines.
    pub(crate) fn new(
        absorber: Option<&'s mut ReusableCategoricalMapAbsorber<'a>>,
        weights: &'s [f64],
        retained: Option<usize>,
    ) -> Option<Self> {
        let (absorber, retained) = absorber.zip(retained)?;
        let oriented = absorber
            .two_fe_pcg
            .as_ref()
            .is_some_and(|graph| graph.right_effect == retained);
        (oriented && absorber.ppml_cmg.is_some() && weights.len() == absorber.design.nobs())
            .then_some(Self {
                absorber,
                weights,
                masses: false,
                attempted: false,
            })
    }

    /// Current `sqrt(D_f)` in the retained bank, once per frame.
    fn prepare_masses(&mut self) {
        if self.masses {
            return;
        }
        let graph = self.absorber.two_fe_pcg.as_ref().expect("oriented graph");
        let codes = self.absorber.design.effects()[graph.right_effect].codes();
        let state = self.absorber.ppml_cmg.as_mut().expect("enabled reuse");
        state.roots.fill(0.0);
        for (&code, &weight) in codes.iter().zip(self.weights) {
            state.roots[code as usize] += weight;
        }
        for root in &mut state.roots {
            *root = root.sqrt();
        }
        self.masses = true;
    }
}

impl<'s> JointCmgBlock<'s, '_> {
    pub(crate) fn as_block(&mut self) -> &mut (dyn JointBlockPreconditioner + 's) {
        self
    }
}

impl JointBlockPreconditioner for JointCmgBlock<'_, '_> {
    fn installed(&self) -> bool {
        self.absorber
            .two_fe_pcg
            .as_ref()
            .is_some_and(|graph| graph.cmg_schur.is_some())
    }

    fn install(&mut self) -> Result<bool> {
        if std::mem::replace(&mut self.attempted, true) {
            return Ok(self.installed());
        }
        self.absorber.install_ppml_cmg(self.weights)
    }

    fn apply(&mut self, residual: &[f64], output: &mut [f64]) -> Result<()> {
        self.prepare_masses();
        let (Some(state), Some(graph)) = (
            self.absorber.ppml_cmg.as_mut(),
            self.absorber.two_fe_pcg.as_mut(),
        ) else {
            return Err(invalid_data("PPML CMG reuse is not enabled"));
        };
        let preconditioner = graph
            .cmg_schur
            .as_mut()
            .ok_or_else(|| invalid_data("PPML CMG hierarchy is not installed"))?;
        if residual.len() != state.roots.len() || output.len() != state.roots.len() {
            return Err(invalid_argument(
                "PPML CMG block dimensions are inconsistent",
            ));
        }
        for ((raw, &value), &root) in state.raw.iter_mut().zip(residual).zip(&state.roots) {
            *raw = value * root;
        }
        preconditioner.apply_batch(&state.raw, &mut state.solved, 1, 1, None, None)?;
        for ((value, &solved), &root) in output.iter_mut().zip(&state.solved).zip(&state.roots) {
            *value = solved * root;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
