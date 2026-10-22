//! Selected structural two-gate policy. No progress banks, prices or clocks.
//!
//! The immutable offline fit selects feature16 (final role) <=0 at local16,
//! followed by constant Defer. Owner economics is separate from SetupLedger.
use super::routing_remaining::Demand;
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Unknown,
    Packed,
    Outcome,
    Final,
}

pub(crate) struct GateState {
    terminal: bool,
    role: Role,
}

impl GateState {
    pub(crate) fn new() -> Self {
        Self {
            terminal: false,
            role: Role::Unknown,
        }
    }

    pub(crate) fn shell_bytes() -> u128 {
        std::mem::size_of::<Self>() as u128
    }

    pub(crate) fn begin(&mut self, demand: Demand, width: usize, stride: usize, final_phase: bool) {
        // Validate caller-owned demand before extracting the selected final bit.
        self.role = if width == 0 || width > stride {
            Role::Unknown
        } else {
            match demand {
                Demand::Packed { regressors }
                    if !final_phase && regressors.checked_add(1) == Some(width) =>
                {
                    Role::Packed
                }
                Demand::Outcome { block, .. }
                    if !final_phase && width == 1 && block > 0 && block <= stride =>
                {
                    Role::Outcome
                }
                Demand::Final if final_phase && width == 1 => Role::Final,
                // First regressor/one-off episodes have no treatment evidence.
                _ => Role::Unknown,
            }
        };
    }

    pub(crate) fn checkpoint(
        &mut self,
        age: usize,
        remaining: usize,
        eligible: bool,
    ) -> Result<bool> {
        if self.terminal || !eligible || age != 16 || remaining == 0 {
            return Ok(false);
        }
        crate::interrupt::checkpoint()?;
        self.terminal = true;
        let grant = matches!(self.role, Role::Packed | Role::Outcome);
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=two_fe_gates|event=decision|local=16|role={:?}|grant={}|terminal=1|proxy_entries=0|bank_bytes=0",
                self.role, u8::from(grant)));
        }
        crate::interrupt::checkpoint()?;
        Ok(grant)
    }

    pub(crate) fn correction(&mut self) -> Result<()> {
        crate::interrupt::checkpoint()?;
        self.terminal = true;
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(
                "FEREG_SOLVER_TRACE|phase=two_fe_gates|event=correction|grant=0|terminal=1|proxy_entries=0|bank_bytes=0");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
