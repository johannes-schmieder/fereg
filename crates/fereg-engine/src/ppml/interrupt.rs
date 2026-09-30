//! Synchronous numerical checkpoints; the host keeps SPI on its calling thread.
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptPoint {
    SeparationRound,
    SimplexModel,
    SimplexReduction,
    SimplexPivot,
    RectifierIteration,
    MuIteration,
    IrlsIteration,
    LineSearch,
    LinearFrame,
    StrictCheckpoint,
}

/// No Send/Sync bound: a Stata host may borrow its main-thread-only SPI handle.
/// Every callback error aborts, even when its code also names a numerical stall.
pub type Interrupt<'a> = dyn FnMut(InterruptPoint) -> Result<()> + 'a;
