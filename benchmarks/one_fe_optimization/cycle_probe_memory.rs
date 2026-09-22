//! Descriptor reservations for the captured-source cycle experiment only.
use crate::error::{ErrorCode, FeregError, Result};
use crate::memory::{FeSolverMemoryBudget, SolverWorkspacePeak};

pub(crate) struct CycleProbeReservation {
    pub(crate) budget: Option<FeSolverMemoryBudget>,
    pub(crate) peak: SolverWorkspacePeak,
    live: u128,
}

impl CycleProbeReservation {
    pub(crate) fn new(budget: Option<FeSolverMemoryBudget>) -> Self {
        Self { budget, peak: SolverWorkspacePeak::ZERO, live: 0 }
    }

    // The descriptor constructors keep all their small tables until return.
    // Accounting includes actual capacities, including a failed last reserve.
    pub(crate) fn reserve<T>(&mut self, target: &mut Vec<T>, additional: usize, label: &'static str) -> Result<()> {
        let error = || FeregError::new(ErrorCode::Allocation, label);
        let old = target.capacity() as u128 * std::mem::size_of::<T>() as u128;
        let capacity = target.len().checked_add(additional).ok_or_else(error)?.max(target.capacity());
        let planned = self.live.checked_sub(old).and_then(|v| v.checked_add(capacity as u128 * std::mem::size_of::<T>() as u128)).ok_or_else(error)?;
        if self.budget.is_some_and(|b| !b.fits(planned)) { return Err(error()); }
        target.try_reserve_exact(additional).map_err(|_| error())?;
        self.live = self.live.checked_sub(old).and_then(|v| v.checked_add(target.capacity() as u128 * std::mem::size_of::<T>() as u128)).ok_or_else(error)?;
        self.peak = self.peak.max(SolverWorkspacePeak::from_bytes(self.live)?);
        if self.budget.is_some_and(|b| !b.fits(self.live)) { return Err(error()); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cycle_descriptor_reservation_charges_joint_capacity_and_rejects_before_growth() {
        let mut reservation = CycleProbeReservation::new(Some(FeSolverMemoryBudget { execution_bytes: 100, usable_bytes: 164 }));
        let mut first = Vec::<u64>::new();
        reservation.reserve(&mut first, 4, "first").unwrap();
        first.resize(4, 0);
        let mut second = Vec::<u64>::new();
        reservation.reserve(&mut second, 4, "second").unwrap();
        assert_eq!(reservation.peak.bytes(), Some(64));
        assert_eq!(reservation.reserve(&mut first, 1, "growth").unwrap_err().code, ErrorCode::Allocation);
        assert_eq!(first.capacity(), 4);
        assert_eq!(second.capacity(), 4);
        assert_eq!(reservation.peak.bytes(), Some(64));
    }
}
