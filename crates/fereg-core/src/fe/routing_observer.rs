//! Borrowed transport for mutually exclusive, separately owned Auto sidecars.
use super::routing_gates::GateState;
use super::routing_remaining::{Demand, State};

pub(crate) enum ObserverRef<'a> {
    Remaining(&'a mut State),
    Gates(&'a mut GateState),
}

impl ObserverRef<'_> {
    pub(crate) fn reborrow(&mut self) -> ObserverRef<'_> {
        match self {
            Self::Remaining(state) => ObserverRef::Remaining(state),
            Self::Gates(state) => ObserverRef::Gates(state),
        }
    }

    pub(crate) fn remaining(&self) -> Option<&State> {
        match self {
            Self::Remaining(state) => Some(state),
            Self::Gates(_) => None,
        }
    }

    pub(crate) fn remaining_mut(&mut self) -> Option<&mut State> {
        match self {
            Self::Remaining(state) => Some(state),
            Self::Gates(_) => None,
        }
    }

    pub(crate) fn begin(
        &mut self,
        demand: Demand,
        width: usize,
        stride: usize,
        tolerance: f64,
        final_phase: bool,
    ) {
        match self {
            Self::Remaining(state) => state.begin(demand, width, stride, tolerance, final_phase),
            Self::Gates(state) => state.begin(demand, width, stride, final_phase),
        }
    }
}
