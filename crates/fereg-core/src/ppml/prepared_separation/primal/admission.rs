//! Per-attempt sparse scratch. The incumbent allowance comes from the released
//! detector workspace. Only a concrete request for more storage can extend it.
use super::*;

pub(in crate::ppml::prepared_separation) struct SparseProofAdmission<'a> {
    plan: Option<&'a LiveMemoryPlan>,
    phase: PpmlPhase,
    initial: usize,
    granted: usize,
}

impl<'a> SparseProofAdmission<'a> {
    pub(in crate::ppml::prepared_separation) fn new(
        model: &'a PreparedPpmlModelGeneration,
        method: SeparationMethod,
        config: SeparationConfig,
    ) -> Self {
        let initial = ProofBudget::for_sparse(model, method, config).words;
        Self::new_with_initial(model, method, config, initial)
    }

    /// A lane may start from its own registered workspace (for example the
    /// complete tableau envelope after all dense scratch has been released).
    pub(in crate::ppml::prepared_separation) fn new_with_initial(
        model: &'a PreparedPpmlModelGeneration,
        method: SeparationMethod,
        config: SeparationConfig,
        initial: usize,
    ) -> Self {
        let phase = match method {
            SeparationMethod::Simplex => PpmlPhase::SeparationSimplex,
            SeparationMethod::IteratedRectifier => PpmlPhase::SeparationIr,
            SeparationMethod::Mu => PpmlPhase::SeparationMu,
            _ => unreachable!("sparse numerical proof method"),
        };
        Self {
            plan: if config.adaptive_sparse {
                model.separation_memory.as_deref()
            } else {
                None
            },
            phase,
            initial,
            granted: initial,
        }
    }

    pub(in crate::ppml::prepared_separation) fn words(&self) -> usize {
        self.granted
    }

    pub(in crate::ppml::prepared_separation) fn adaptive(&self) -> bool {
        self.plan.is_some()
    }

    /// Grant the measured shortage. A fixed-size request can exceed a single
    /// increment; in that case its exact minimum is all that is requested.
    /// Smaller requests share a bounded tranche to amortize frequent fill-in.
    pub(in crate::ppml::prepared_separation) fn admit(&mut self, required: usize) -> bool {
        if required <= self.granted {
            return true;
        }
        let Some(plan) = self.plan else {
            return false;
        };
        let extra = required - self.initial;
        const TRANCHE_WORDS: usize = 1_000_000;
        let preferred = extra.max((self.granted - self.initial).saturating_add(TRANCHE_WORDS));
        let granted = plan.admit_sparse(self.phase, extra, preferred);
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=sparse_workspace|method={:?}|initial_words={}|requested_words={required}|granted_extra_words={}",
                self.phase, self.initial, granted.unwrap_or(0),
            ));
        }
        if let Some(extra) = granted {
            self.granted = self.initial.saturating_add(extra);
            true
        } else {
            false
        }
    }
}
