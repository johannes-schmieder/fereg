//! Safe resource-observation boundary used by the estimation action.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ResourceSnapshot {
    pub(crate) allocation_current_bytes: u64,
    pub(crate) allocation_peak_bytes: u64,
    pub(crate) allocation_previous_baseline_bytes: u64,
    pub(crate) rss_current_bytes: Option<u64>,
    pub(crate) rss_peak_bytes: Option<u64>,
}

#[cfg(feature = "ffi")]
pub(crate) fn begin_observation() -> ResourceSnapshot {
    let (allocation_current_bytes, allocation_previous_baseline_bytes) =
        crate::ffi::begin_allocation_observation();
    let (_, allocation_peak_bytes) = crate::ffi::allocation_usage();
    let (rss_current_bytes, rss_peak_bytes) = crate::ffi::process_rss_bytes();
    ResourceSnapshot {
        allocation_current_bytes,
        allocation_peak_bytes,
        allocation_previous_baseline_bytes,
        rss_current_bytes,
        rss_peak_bytes,
    }
}

#[cfg(not(feature = "ffi"))]
pub(crate) fn begin_observation() -> ResourceSnapshot {
    ResourceSnapshot::default()
}

#[cfg(feature = "ffi")]
pub(crate) fn snapshot() -> ResourceSnapshot {
    let (allocation_current_bytes, allocation_peak_bytes) = crate::ffi::allocation_usage();
    let (rss_current_bytes, rss_peak_bytes) = crate::ffi::process_rss_bytes();
    ResourceSnapshot {
        allocation_current_bytes,
        allocation_peak_bytes,
        allocation_previous_baseline_bytes: 0,
        rss_current_bytes,
        rss_peak_bytes,
    }
}

#[cfg(not(feature = "ffi"))]
pub(crate) fn snapshot() -> ResourceSnapshot {
    ResourceSnapshot::default()
}

#[cfg(feature = "ffi")]
pub(crate) fn system_available_memory_bytes() -> Option<u128> {
    crate::ffi::system_available_memory_bytes().map(u128::from)
}

#[cfg(not(feature = "ffi"))]
pub(crate) const fn system_available_memory_bytes() -> Option<u128> {
    None
}
