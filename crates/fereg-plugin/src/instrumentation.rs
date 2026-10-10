//! Safe resource-observation boundary used by the estimation action.
//!
//! The tracking allocator and the process probes live in the FFI layer.
//! Builds without the `ffi` feature (the Stata-free harness) observe nothing:
//! snapshots stay empty and no probe or planner receipt is installed.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::ppml::{
    PpmlAllocatorSnapshot, PpmlAllocatorSnapshotProbe, PpmlMemoryPreflightEvent,
    PpmlMemoryPreflightObservation, PpmlMemoryPreflightProbe, PpmlMemoryRoute,
    PpmlPhaseAllocatorProbe,
};
use fereg_core::{LinearSessionPhaseAllocatorProbe, MemoryMode};

#[cfg(feature = "ffi")]
use crate::ffi as host;

/// Whether this build carries the tracking allocator and process probes.
const OBSERVED: bool = cfg!(feature = "ffi");

/// Stand-ins for the FFI-layer probes in Stata-free builds. Every snapshot
/// they produce equals the empty `ResourceSnapshot`, and the probe
/// constructors below return `None` before reaching them.
#[cfg(not(feature = "ffi"))]
mod host {
    use fereg_core::ppml::{PpmlPhase, PpmlPhaseAllocatorEvent};
    use fereg_core::{LinearSessionAllocationPhase, LinearSessionPhaseAllocationEvent};

    #[derive(Clone, Copy, Default)]
    pub(crate) struct AllocationUsage {
        pub(crate) current_bytes: u64,
        pub(crate) peak_bytes: u64,
        pub(crate) events: u64,
    }

    pub(crate) fn allocation_usage() -> AllocationUsage {
        AllocationUsage::default()
    }

    pub(crate) fn begin_ols_allocation_observation() -> (AllocationUsage, Option<u64>) {
        (AllocationUsage::default(), None)
    }

    pub(crate) fn begin_allocation_observation() -> (AllocationUsage, Option<u64>) {
        (AllocationUsage::default(), None)
    }

    pub(crate) const fn process_rss_bytes() -> (Option<u64>, Option<u64>) {
        (None, None)
    }

    pub(crate) const fn system_available_memory_bytes() -> Option<u64> {
        None
    }

    pub(crate) const fn abi_memory_baseline() -> Option<u64> {
        None
    }

    pub(crate) fn ppml_phase_allocation_probe(
        _phase: PpmlPhase,
        _event: PpmlPhaseAllocatorEvent,
    ) -> u64 {
        0
    }

    pub(crate) fn ppml_linear_phase_allocation_probe(
        _phase: LinearSessionAllocationPhase,
        _event: LinearSessionPhaseAllocationEvent,
    ) -> u64 {
        0
    }
}

const PPML_PREFLIGHT_RECEIPT_ENV: &str = "FEREG_PPML_PREFLIGHT_RECEIPT";
static PPML_PREFLIGHT_START_CURRENT: AtomicU64 = AtomicU64::new(0);
static PPML_PREFLIGHT_START_PEAK: AtomicU64 = AtomicU64::new(0);
static PPML_PREFLIGHT_START_EVENTS: AtomicU64 = AtomicU64::new(0);
static PPML_PREFLIGHT_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
static ACTION_OBSERVATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Unit tests execute independent fake Stata actions in parallel inside one
/// process, unlike the real single-threaded SPI entrypoint. Serialize only
/// those test actions that reset the process-global allocation peak so one
/// fixture cannot invalidate another fixture's observation window.
#[cfg(test)]
pub(crate) fn lock_action_observation() -> std::sync::MutexGuard<'static, ()> {
    ACTION_OBSERVATION_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ResourceSnapshot {
    pub(crate) allocation_current_bytes: u64,
    pub(crate) allocation_peak_bytes: u64,
    pub(crate) allocation_events: u64,
    pub(crate) allocation_previous_baseline_bytes: u64,
    pub(crate) allocation_previous_baseline_observed: bool,
    pub(crate) rss_current_bytes: Option<u64>,
    pub(crate) rss_peak_bytes: Option<u64>,
}

pub(crate) fn begin_observation() -> ResourceSnapshot {
    let (allocation, allocation_previous_baseline) = host::begin_ols_allocation_observation();
    begin_snapshot(allocation, allocation_previous_baseline)
}

pub(crate) fn begin_ppml_observation() -> ResourceSnapshot {
    let (allocation, allocation_previous_baseline) = host::begin_allocation_observation();
    begin_snapshot(allocation, allocation_previous_baseline)
}

fn begin_snapshot(
    allocation: host::AllocationUsage,
    allocation_previous_baseline: Option<u64>,
) -> ResourceSnapshot {
    let (rss_current_bytes, rss_peak_bytes) = host::process_rss_bytes();
    ResourceSnapshot {
        allocation_current_bytes: allocation.current_bytes,
        allocation_peak_bytes: allocation.peak_bytes,
        allocation_events: allocation.events,
        // Preserve the established OLS telemetry value on the first action,
        // while retaining whether a genuine preceding-action baseline exists
        // for the additive PPML result wire.
        allocation_previous_baseline_bytes: allocation_previous_baseline
            .unwrap_or(allocation.current_bytes),
        allocation_previous_baseline_observed: allocation_previous_baseline.is_some(),
        rss_current_bytes,
        rss_peak_bytes,
    }
}

pub(crate) fn snapshot() -> ResourceSnapshot {
    let allocation = host::allocation_usage();
    let (rss_current_bytes, rss_peak_bytes) = host::process_rss_bytes();
    ResourceSnapshot {
        allocation_current_bytes: allocation.current_bytes,
        allocation_peak_bytes: allocation.peak_bytes,
        allocation_events: allocation.events,
        allocation_previous_baseline_bytes: 0,
        allocation_previous_baseline_observed: false,
        rss_current_bytes,
        rss_peak_bytes,
    }
}

pub(crate) fn system_available_memory_bytes() -> Option<u128> {
    host::system_available_memory_bytes().map(u128::from)
}

/// Allocation-free probe called by the prepared PPML driver only at complete
/// warmed-iteration boundaries. It intentionally excludes RSS collection,
/// syscalls, and action-lifetime baseline mutation.
fn ppml_allocator_snapshot() -> PpmlAllocatorSnapshot {
    let usage = host::allocation_usage();
    PpmlAllocatorSnapshot {
        current_bytes: usage.current_bytes,
        peak_bytes: usage.peak_bytes,
        allocation_events: usage.events,
    }
}

pub(crate) const fn ppml_allocator_snapshot_probe(
    enabled: bool,
) -> Option<PpmlAllocatorSnapshotProbe> {
    if OBSERVED && enabled {
        Some(ppml_allocator_snapshot)
    } else {
        None
    }
}

pub(crate) fn ppml_phase_allocator_probe(enabled: bool) -> Option<PpmlPhaseAllocatorProbe> {
    if OBSERVED && enabled {
        return Some(host::ppml_phase_allocation_probe);
    }
    #[cfg(all(test, feature = "ffi"))]
    if crate::ffi::ppml_allocation_failure_armed() {
        return Some(crate::ffi::ppml_failure_injection_phase_probe);
    }
    None
}

pub(crate) fn ppml_linear_phase_allocator_probe(
    enabled: bool,
) -> Option<LinearSessionPhaseAllocatorProbe> {
    if OBSERVED && enabled {
        return Some(host::ppml_linear_phase_allocation_probe);
    }
    #[cfg(all(test, feature = "ffi"))]
    if crate::ffi::ppml_allocation_failure_armed() {
        return Some(crate::ffi::ppml_failure_injection_linear_phase_probe);
    }
    None
}

pub(crate) fn ppml_action_allocation_observation(
    start: ResourceSnapshot,
) -> (
    Option<PpmlAllocatorSnapshotProbe>,
    Option<PpmlAllocatorSnapshot>,
    Option<u64>,
) {
    if !OBSERVED {
        return (None, None, None);
    }
    (
        Some(ppml_allocator_snapshot),
        Some(PpmlAllocatorSnapshot {
            current_bytes: start.allocation_current_bytes,
            peak_bytes: start.allocation_peak_bytes,
            allocation_events: start.allocation_events,
        }),
        if start.allocation_previous_baseline_observed {
            Some(start.allocation_previous_baseline_bytes)
        } else {
            None
        },
    )
}

fn preflight_internal(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}

const fn memory_mode_name(mode: MemoryMode) -> &'static str {
    match mode {
        MemoryMode::Auto => "auto",
        MemoryMode::Fast => "fast",
        MemoryMode::Low => "low",
    }
}

const fn memory_route_name(route: Option<PpmlMemoryRoute>) -> &'static str {
    match route {
        Some(PpmlMemoryRoute::Fast) => "\"fast\"",
        Some(PpmlMemoryRoute::Low) => "\"low\"",
        None => "null",
    }
}

fn optional_bytes(value: Option<u128>) -> String {
    value.map_or_else(|| "null".to_owned(), |bytes| bytes.to_string())
}

fn write_ppml_preflight_receipt(
    observation: PpmlMemoryPreflightObservation,
    current_growth: i128,
    event_growth: u64,
    peak_growth: u64,
) -> Result<()> {
    if observation.workspace_allocation_started {
        return Err(preflight_internal(
            "PPML memory preflight observer ran after workspace allocation",
        ));
    }
    let path = std::env::var_os(PPML_PREFLIGHT_RECEIPT_ENV)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| preflight_internal("PPML memory preflight receipt path vanished"))?;
    let allocation_bytes = u128::from(peak_growth).max(current_growth.unsigned_abs());
    let source_commit = fereg_core::BuildInfo::current().git_commit;
    let rendered = format!(
        concat!(
            "{{\n",
            "  \"schema_version\": 1,\n",
            "  \"receipt_type\": \"feppml-memory-preflight-v1\",\n",
            "  \"requested\": \"{}\",\n",
            "  \"decision\": \"{}\",\n",
            "  \"selected_route\": {},\n",
            "  \"fast_peak_bytes\": {},\n",
            "  \"low_peak_bytes\": {},\n",
            "  \"available_bytes\": {},\n",
            "  \"embedding_resident_bytes\": {},\n",
            "  \"safety_headroom_bytes\": {},\n",
            "  \"usable_bytes\": {},\n",
            "  \"planner_allocator_event_growth\": {},\n",
            "  \"planner_allocator_current_growth_bytes\": {},\n",
            "  \"planner_allocator_peak_growth_bytes\": {},\n",
            "  \"allocation_bytes\": {},\n",
            "  \"workspace_allocation_started\": false,\n",
            "  \"embedded_git_commit\": \"{}\",\n",
            "  \"complete_sentinel\": \"FEPPML_MEMORY_PREFLIGHT_COMPLETE\"\n",
            "}}\n"
        ),
        memory_mode_name(observation.requested),
        if observation.selected.is_some() {
            "selected"
        } else {
            "rejected"
        },
        memory_route_name(observation.selected),
        observation.fast_peak_bytes,
        observation.low_peak_bytes,
        optional_bytes(observation.available_bytes),
        observation.embedding_resident_bytes,
        observation.safety_headroom_bytes,
        optional_bytes(observation.usable_bytes),
        event_growth,
        current_growth,
        peak_growth,
        allocation_bytes,
        source_commit,
    );
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(std::path::PathBuf::from(path))
        .map_err(|error| {
            preflight_internal(format!(
                "unable to create PPML memory preflight receipt: {error}"
            ))
        })?;
    output.write_all(rendered.as_bytes()).map_err(|error| {
        preflight_internal(format!(
            "unable to write PPML memory preflight receipt: {error}"
        ))
    })?;
    output.sync_all().map_err(|error| {
        preflight_internal(format!(
            "unable to sync PPML memory preflight receipt: {error}"
        ))
    })?;
    Ok(())
}

/// Return the qualification-only decision observer when explicitly armed.
///
/// The file seam is accepted only together with the existing artificial
/// memory-limit seam, so normal public fits never write a planner receipt.
pub(crate) fn ppml_memory_preflight_probe() -> Result<Option<PpmlMemoryPreflightProbe>> {
    if !OBSERVED {
        return Ok(None);
    }
    let Some(path) = std::env::var_os(PPML_PREFLIGHT_RECEIPT_ENV) else {
        return Ok(None);
    };
    if path.is_empty() {
        return Err(preflight_internal(
            "FEREG_PPML_PREFLIGHT_RECEIPT must be a nonempty path",
        ));
    }
    if std::env::var_os("FEREG_MEMORY_LIMIT_BYTES").is_none() {
        return Err(preflight_internal(
            "FEREG_PPML_PREFLIGHT_RECEIPT requires FEREG_MEMORY_LIMIT_BYTES",
        ));
    }
    Ok(Some(capture_ppml_memory_preflight))
}

fn capture_ppml_memory_preflight(event: PpmlMemoryPreflightEvent) -> Result<()> {
    match event {
        PpmlMemoryPreflightEvent::Begin => {
            let snapshot = host::allocation_usage();
            PPML_PREFLIGHT_START_CURRENT.store(snapshot.current_bytes, Ordering::Relaxed);
            PPML_PREFLIGHT_START_PEAK.store(snapshot.peak_bytes, Ordering::Relaxed);
            PPML_PREFLIGHT_START_EVENTS.store(snapshot.events, Ordering::Relaxed);
            if PPML_PREFLIGHT_WINDOW_OPEN.swap(true, Ordering::Relaxed) {
                return Err(preflight_internal(
                    "PPML memory preflight observer window was already open",
                ));
            }
            Ok(())
        }
        PpmlMemoryPreflightEvent::Decision(observation) => {
            let end = host::allocation_usage();
            if !PPML_PREFLIGHT_WINDOW_OPEN.swap(false, Ordering::Relaxed) {
                return Err(preflight_internal(
                    "PPML memory preflight decision omitted its begin event",
                ));
            }
            let start_current = PPML_PREFLIGHT_START_CURRENT.load(Ordering::Relaxed);
            let start_peak = PPML_PREFLIGHT_START_PEAK.load(Ordering::Relaxed);
            let start_events = PPML_PREFLIGHT_START_EVENTS.load(Ordering::Relaxed);
            if end.events < start_events {
                return Err(preflight_internal(
                    "PPML memory preflight allocator event counter moved backwards",
                ));
            }
            let current_growth = i128::from(end.current_bytes) - i128::from(start_current);
            write_ppml_preflight_receipt(
                observation,
                current_growth,
                end.events - start_events,
                end.peak_bytes.saturating_sub(start_peak),
            )
        }
    }
}

#[cfg(all(test, feature = "ffi"))]
mod tests {
    use super::*;

    #[test]
    fn preflight_receipt_is_emitted_only_from_the_core_decision_event() {
        let _guard = lock_action_observation();
        let path = std::env::temp_dir().join(format!(
            "fereg-ppml-preflight-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let prior = std::env::var_os(PPML_PREFLIGHT_RECEIPT_ENV);
        std::env::set_var(PPML_PREFLIGHT_RECEIPT_ENV, &path);

        capture_ppml_memory_preflight(PpmlMemoryPreflightEvent::Begin)
            .expect("begin preflight window");
        assert!(
            !path.exists(),
            "begin event must not mint a decision receipt"
        );
        capture_ppml_memory_preflight(PpmlMemoryPreflightEvent::Decision(
            PpmlMemoryPreflightObservation {
                requested: MemoryMode::Fast,
                selected: None,
                fast_peak_bytes: 200,
                low_peak_bytes: 100,
                available_bytes: Some(150),
                embedding_resident_bytes: 25,
                safety_headroom_bytes: 50,
                usable_bytes: Some(100),
                workspace_allocation_started: false,
            },
        ))
        .expect("write decision receipt");

        let receipt = std::fs::read_to_string(&path).expect("read preflight receipt");
        assert!(receipt.contains("\"decision\": \"rejected\""));
        assert!(receipt.contains("\"selected_route\": null"));
        assert!(receipt.contains("\"workspace_allocation_started\": false"));
        assert!(receipt.contains("FEPPML_MEMORY_PREFLIGHT_COMPLETE"));
        std::fs::remove_file(&path).expect("remove preflight receipt");
        match prior {
            Some(value) => std::env::set_var(PPML_PREFLIGHT_RECEIPT_ENV, value),
            None => std::env::remove_var(PPML_PREFLIGHT_RECEIPT_ENV),
        }
    }
}

/// None in mock actions that do not cross the actual ABI boundary.
pub(crate) fn native_live_before() -> Option<u64> {
    host::abi_memory_baseline()
}
