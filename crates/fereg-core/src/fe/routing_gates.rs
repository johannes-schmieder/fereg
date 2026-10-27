//! Selected structural two-gate policy. No progress banks, prices or clocks.
//!
//! The immutable offline fit selects feature16 (final role) <=0 at local16,
//! followed by constant Defer. Owner economics is separate from SetupLedger.
//! The stall rule (gate v2) may grant the same build earlier, at local ages
//! 4, 8 or 12, when the diagonal recurrence has visibly stalled; local16 and
//! everything after it are unchanged.
use super::routing_remaining::Demand;
use crate::error::Result;

mod stall;
pub(crate) use stall::STALL_THRESHOLD;

/// Which structural gate a two-FE Auto policy uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GateRule {
    /// Gate v1: the selected local16 boundary only.
    Structural16,
    /// Gate v2: v1 plus an early stall grant at local 4, 8 or 12.
    Stall { threshold: f64 },
}

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
    rule: GateRule,
}

impl GateState {
    pub(crate) fn new(rule: GateRule) -> Self {
        Self {
            terminal: false,
            role: Role::Unknown,
            rule,
        }
    }

    /// One observation of the recurrence after its `age`-th local step.
    /// `initial` is each lane's activity state (its initial energy, zero once
    /// complete, negative after an underflowed start) and `current` its
    /// present energy, both serial canonical reductions, so the decision is
    /// identical at every worker count.
    pub(crate) fn observe(
        &mut self,
        age: usize,
        remaining: usize,
        initial: &[f64],
        current: &[f64],
    ) -> Result<bool> {
        if age == 16 {
            return self.checkpoint(16, remaining, true);
        }
        match self.rule {
            GateRule::Stall { threshold } if stall::AGES.contains(&age) => {
                self.stall(age, remaining, threshold, initial, current)
            }
            _ => Ok(false),
        }
    }

    fn stall(
        &mut self,
        age: usize,
        remaining: usize,
        threshold: f64,
        initial: &[f64],
        current: &[f64],
    ) -> Result<bool> {
        if self.terminal || remaining == 0 || !matches!(self.role, Role::Packed | Role::Outcome) {
            return Ok(false);
        }
        crate::interrupt::checkpoint()?;
        // A lane without valid history leaves this age undecided.
        let Some(ratio) = stall::energy_ratio(initial, current) else {
            return Ok(false);
        };
        let grant = ratio >= stall::energy_bound(threshold, age);
        self.terminal = grant;
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=two_fe_gates|event=decision|local={age}|role={:?}|grant={}|terminal={}|proxy_entries=0|bank_bytes=0|rule=stall_v2|contraction={:.17e}",
                self.role, u8::from(grant), u8::from(grant), stall::contraction(ratio, age)));
        }
        crate::interrupt::checkpoint()?;
        Ok(grant)
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
            let rule = match self.rule {
                GateRule::Structural16 => "",
                GateRule::Stall { .. } => "|rule=stall_v2",
            };
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=two_fe_gates|event=decision|local=16|role={:?}|grant={}|terminal=1|proxy_entries=0|bank_bytes=0{rule}",
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
