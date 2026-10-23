//! OLS-only cell lifetime, independent of the CMG route's attempt ledger.
use super::*;

// These owners are mutually exclusive: PPML supports changing weights;
// experimental OLS cells retain one immutable weight borrow. Reuse the existing
// storage slot, rather than changing admission for Off/explicit/PPML callers.
pub(super) enum ReuseState<'a> {
    Off,
    Ppml(super::super::ppml_topology::PpmlTopology),
    Eager(EagerCells<'a>),
}
const _: () = {
    assert!(
        std::mem::size_of::<ReuseState<'static>>()
            == std::mem::size_of::<Option<super::super::ppml_topology::PpmlTopology>>()
    );
    assert!(
        std::mem::align_of::<ReuseState<'static>>()
            == std::mem::align_of::<Option<super::super::ppml_topology::PpmlTopology>>()
    );
};
impl ReuseState<'_> {
    pub(super) fn ppml(&self) -> Option<&super::super::ppml_topology::PpmlTopology> {
        match self {
            Self::Ppml(state) => Some(state),
            _ => None,
        }
    }
    pub(super) fn ppml_mut(&mut self) -> Option<&mut super::super::ppml_topology::PpmlTopology> {
        match self {
            Self::Ppml(state) => Some(state),
            _ => None,
        }
    }
    pub(super) fn eager(&self) -> Option<&EagerCells<'_>> {
        match self {
            Self::Eager(state) => Some(state),
            _ => None,
        }
    }
}
impl<'a> ReuseState<'a> {
    pub(super) fn eager_mut(&mut self) -> Option<&mut EagerCells<'a>> {
        match self {
            Self::Eager(state) => Some(state),
            _ => None,
        }
    }
}

pub(super) struct EagerCells<'a> {
    // This immutable borrow lives as long as the design owner. Pointer equality
    // is an owner check, not a substitute for retaining the borrow's lifetime.
    weights: BorrowedMapWeights<'a>,
    scale: u64,
    pub(super) attempted: bool,
    pub(super) refusal: Option<super::super::routing_kernel::KernelRefusal>,
    peak: SolverWorkspacePeak,
}

impl EagerCells<'_> {
    pub(super) fn validate(&mut self, weights: BorrowedMapWeights<'_>, scale: f64) -> Result<()> {
        let same = match (self.weights, weights) {
            (BorrowedMapWeights::Unit, BorrowedMapWeights::Unit) => true,
            (BorrowedMapWeights::Provided(a), BorrowedMapWeights::Provided(b)) => {
                std::ptr::eq(a, b)
            }
            _ => false,
        };
        if !same || (self.scale != 0 && self.scale != scale.to_bits()) {
            return Err(invalid_argument(
                "eager cells require the same immutable OLS weight owner",
            ));
        }
        self.scale = scale.to_bits();
        Ok(())
    }

    pub(super) fn total_peak(&self, parents: u128) -> Result<SolverWorkspacePeak> {
        self.peak.with_retained(parents)
    }

    fn trace(&self, event: &str, kernel: Option<&super::super::routing_kernel::CellSchur>) {
        if crate::diagnostics::phase_timing_enabled() || crate::diagnostics::solver_trace_enabled()
        {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_CELLS|event={event}|attempted={}|admitted={}|refusal={:?}|cells={}|retained_bytes={}|peak_bytes={}",
                u8::from(self.attempted), u8::from(kernel.is_some()), self.refusal,
                kernel.map_or(0, |k| k.cell_count()), kernel.map_or(0, |k| k.retained_bytes()),
                self.peak.bytes().unwrap_or(u64::MAX),
            ));
        }
    }
}

/// Add only storage retained during this nested construction. The existing
/// request already includes the base execution/FE parents.
pub(super) fn with_retained(
    budget: Option<FeSolverMemoryBudget>,
    bytes: u128,
) -> Result<Option<FeSolverMemoryBudget>> {
    budget
        .map(|budget| {
            Ok(FeSolverMemoryBudget {
                execution_bytes: budget
                    .execution_bytes
                    .checked_add(bytes)
                    .ok_or_else(|| allocation("cell and CMG retained overlap overflows"))?,
                usable_bytes: budget.usable_bytes,
            })
        })
        .transpose()
}

impl<'a> ReusableCategoricalMapAbsorber<'a> {
    pub(crate) fn request_eager_cells(&mut self, weights: BorrowedMapWeights<'a>) -> Result<()> {
        if self.auto_route.is_none() || !matches!(self.reuse, ReuseState::Off) {
            return Err(invalid_argument(
                "eager cells require a fresh OLS routing owner",
            ));
        }
        self.reuse = ReuseState::Eager(EagerCells {
            weights,
            scale: 0,
            attempted: false,
            refusal: None,
            peak: SolverWorkspacePeak::ZERO,
        });
        self.reuse.eager().unwrap().trace("bound", None);
        Ok(())
    }

    pub(super) fn prepare_eager_cells(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        scale: f64,
        setup_seconds: &mut f64,
        solve_started: &mut Instant,
    ) -> Result<()> {
        let Some(state) = self.reuse.eager_mut() else {
            return Ok(());
        };
        let route = self
            .auto_route
            .as_ref()
            .ok_or_else(|| invalid_argument("eager OLS route is absent"))?;
        let graph = self
            .two_fe_pcg
            .as_mut()
            .ok_or_else(|| invalid_argument("eager cells require two effects"))?;
        if state.attempted {
            state.trace("reuse", graph.kernel.as_ref());
            return Ok(());
        }
        state.attempted = true;
        let started = Instant::now();
        let cmg_bytes = graph
            .cmg_schur
            .as_ref()
            .map_or(Ok(0), |cmg| cmg.retained_bytes())?;
        let built = super::super::routing_kernel::build_occurrence(
            &self.design,
            graph.left_effect,
            weights,
            scale,
            super::super::routing_kernel::KernelLimits {
                retained_live_bytes: cmg_bytes,
                memory: route.request.budget,
            },
        )?;
        state.peak = state.peak.max(built.peak.with_retained(cmg_bytes)?);
        state.refusal = built.refusal;
        graph.kernel = built.kernel;
        graph.use_cell_operator = graph.kernel.is_some();
        let elapsed = started.elapsed();
        *setup_seconds += elapsed.as_secs_f64();
        *solve_started += elapsed;
        state.trace("attempt", graph.kernel.as_ref());
        Ok(())
    }
}
