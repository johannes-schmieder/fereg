//! Parallel numeric input through Stata's read-only numeric window (SPEC §17).
//!
//! A phase opens the window on Stata's thread, lends a bounded set of original
//! row blocks to the action team and waits. While workers read, the main thread
//! makes no SPI call: it polls Break only between phases, after every worker
//! of the phase has returned and the window is closed. Each block writes
//! disjoint rows of reserved, Rust-owned destinations (first touch in the
//! worker, no zero-fill pass). Lengths are set only after every block of every
//! phase succeeded, so an error or unwind never exposes uninitialized storage.
//! A refusal before the first phase leaves the destinations untouched for the
//! serial reader; any failure after dispatch is returned, never retried.

use std::sync::{Mutex, PoisonError};

use fereg_core::data::UniformValuesRef;
use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::parallel::{StagePlan, ThreadTeam};
use rayon::prelude::*;

use crate::input::SpiApi;
use crate::input_memory::InputMemory;
use crate::numeric_pipeline::NumericRhsLayout;

/// The window reads at most this many slots per call (one stack pointer table).
pub(crate) const MAX_WINDOW_SLOTS: usize = 64;
/// Target values per block: a bounded worker task that still amortizes one
/// window call per slot group.
const BLOCK_VALUES: usize = 1 << 15;
const MIN_BLOCK_ROWS: usize = 256;
const MAX_BLOCK_ROWS: usize = 16_384;
/// Values per phase: Break is polled between phases, so this bounds latency
/// (about 16M accessor calls, tens of milliseconds even on one lane).
#[cfg(not(test))]
const PHASE_VALUES: usize = 1 << 24;
/// Unit tests exercise many phases on small inputs.
#[cfg(test)]
const PHASE_VALUES: usize = 1 << 16;

/// Host combinations whose numeric window passed qualification: plugin
/// target, Stata major version, edition and interface (`c(console)`). Every
/// other combination, and any request without these public facts, keeps the
/// serial reader. Safety of concurrent `vdata` calls rests on this empirical
/// qualification; StataCorp documents no thread-safety guarantee.
const QUALIFIED: &[(&str, &str, u32, &str, &str)] = &[
    ("macos", "aarch64", 18, "MP", "console"),
    ("macos", "aarch64", 18, "MP", "gui"),
    ("macos", "aarch64", 19, "MP", "console"),
    ("macos", "aarch64", 19, "MP", "gui"),
    ("linux", "x86_64", 17, "MP", "console"),
    ("linux", "x86_64", 17, "MP", "gui"),
    ("linux", "x86_64", 18, "MP", "console"),
    ("linux", "x86_64", 18, "MP", "gui"),
    ("linux", "x86_64", 19, "MP", "console"),
    ("linux", "x86_64", 19, "MP", "gui"),
];

/// Host capability decided from public request metadata before any read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Policy {
    enabled: bool,
    fault: Option<Fault>,
}

/// Developer injections (hidden adapters only) on the parallel path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    /// Park on Stata's thread after the first phase until a real Break,
    /// after creating the native marker named by the hidden token.
    BreakAfterFirstPhase { token: [u8; 32], ppml: bool },
    /// Panic inside one worker block of the first phase.
    WorkerPanic,
    /// One worker block of the first phase reports SDK return code 919.
    ReadFailure,
}

impl Policy {
    pub(crate) const SERIAL: Self = Self {
        enabled: false,
        fault: None,
    };

    #[cfg(test)]
    pub(crate) const PARALLEL: Self = Self {
        enabled: true,
        fault: None,
    };

    /// Decide from the request's public host facts (`stata`, `host_edition`,
    /// `host_interface`, `host_aliases`). Alias variables resolve through
    /// another frame inside the SDK and always keep the serial reader.
    pub(crate) fn from_host(
        stata: Option<&str>,
        edition: Option<&str>,
        interface: Option<&str>,
        aliases: Option<&str>,
    ) -> Self {
        let major = stata
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 1.0)
            .map(|value| value.floor() as u32);
        let enabled = match (major, edition, interface, aliases) {
            (Some(major), Some(edition), Some(interface), Some("0")) => {
                QUALIFIED.iter().any(|&(os, arch, version, ed, face)| {
                    os == std::env::consts::OS
                        && arch == std::env::consts::ARCH
                        && version == major
                        && ed == edition
                        && face == interface
                })
            }
            _ => false,
        };
        Self {
            enabled,
            fault: None,
        }
    }

    pub(crate) fn from_request(values: &std::collections::BTreeMap<String, String>) -> Self {
        let field = |key: &str| values.get(key).map(String::as_str);
        Self::from_host(
            field("stata"),
            field("host_edition"),
            field("host_interface"),
            field("host_aliases"),
        )
    }

    /// The developer `serial_input` injection keeps the serial reader.
    #[must_use]
    pub(crate) fn serial(self) -> Self {
        Self::SERIAL
    }

    #[must_use]
    pub(crate) fn with_fault(self, fault: Fault) -> Self {
        Self {
            fault: Some(fault),
            ..self
        }
    }

    pub(crate) fn enabled(self) -> bool {
        self.enabled
    }
}

/// The parallel reader's capability and its worker budget (`threads()`),
/// independent of whether the tiled pipeline also has a team.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Transport {
    pub(crate) policy: Policy,
    pub(crate) team: ThreadTeam,
}

impl Transport {
    pub(crate) fn new(policy: Policy, threads: fereg_core::ThreadConfig) -> Option<Self> {
        policy.enabled().then(|| Self {
            policy,
            team: ThreadTeam::new(threads),
        })
    }
}

/// One window read of `rows` original rows from zero-based `first_row`, for
/// window slots `first_slot..first_slot + slots` (`slots <= MAX_WINDOW_SLOTS`).
#[cfg_attr(not(feature = "ffi"), allow(dead_code))]
pub struct BlockRead {
    pub(crate) first_slot: usize,
    pub(crate) slots: usize,
    pub(crate) first_row: usize,
    pub(crate) rows: usize,
    /// `rows` retained flags, or null when every row is retained.
    pub(crate) retained: *const bool,
    /// `slots` fills for excluded rows; read only with `retained`.
    pub(crate) excluded_values: *const f64,
    pub(crate) excluded_missing: *const bool,
    /// `slots` destinations, each valid for `rows` writes.
    pub(crate) values: *const *mut f64,
    /// `slots` flag destinations, or null for a value-only read.
    pub(crate) missing: *const *mut bool,
}

/// Worker-callable numeric reads of one open window.
///
/// # Safety
///
/// On `Ok`, an implementation has written every addressed row of every
/// requested slot (value, and flag when requested): retained rows receive the
/// host's numeric value and `value >= missing boundary`, excluded rows receive
/// the slot fills. It may be called concurrently for disjoint destinations and
/// makes no other host call. `Err` carries the host's return code.
pub unsafe trait NumericBlockSource: Sync {
    /// # Safety
    ///
    /// Every pointer in `request` is valid for the extent it describes, and no
    /// other thread accesses the addressed destinations during the call.
    unsafe fn read(&self, request: &BlockRead) -> std::result::Result<(), i32>;
}

/// A reserved destination: empty, with capacity for every original row.
pub(crate) struct Target<'a> {
    pub(crate) position: u32,
    pub(crate) values: &'a mut Vec<f64>,
    pub(crate) missing: &'a mut Vec<bool>,
    /// Value and flag stored for excluded rows (the existing storage contract).
    pub(crate) excluded: (f64, bool),
}

pub(crate) enum Candidate<'a> {
    All,
    Known(UniformValuesRef<'a, bool>),
    /// Read and validate the Stata marker; `output` is reserved and empty.
    Marker {
        position: u32,
        output: &'a mut Vec<bool>,
    },
}

/// Read every target in parallel, or decline before reading any observation.
/// `operation` names the replaced serial SPI read for error parity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn try_read(
    spi: &mut dyn SpiApi,
    transport: Transport,
    memory: &mut InputMemory,
    rows: usize,
    targets: &mut [Target<'_>],
    candidate: Candidate<'_>,
    mut rhs: Option<(&mut Vec<f64>, NumericRhsLayout)>,
    operation: &'static str,
) -> Result<bool> {
    let Transport { policy, team } = transport;
    let width = targets.len();
    let marker = matches!(candidate, Candidate::Marker { .. });
    if !policy.enabled()
        || rows == 0
        || width == 0
        || !spi.supports_numeric_window()
        || matches!(
            candidate,
            Candidate::Known(UniformValuesRef::Repeated { value: false, .. })
        )
    {
        return Ok(false);
    }
    check_reserved(rows, targets, &candidate, rhs.as_ref())?;
    let slots = width + usize::from(marker);
    let block_rows = (BLOCK_VALUES / slots).clamp(MIN_BLOCK_ROWS, MAX_BLOCK_ROWS);
    let blocks = rows.div_ceil(block_rows);
    if StagePlan::new(rows as u128 * slots as u128, blocks, team.threads()).lanes < 2 {
        return Ok(false);
    }
    // Bounded descriptors: positions, pointer tables and fills. A tight budget
    // keeps the serial reader before any observation is read.
    let descriptor_bytes = slots as u128
        * (std::mem::size_of::<u32>()
            + 2 * std::mem::size_of::<*mut f64>()
            + std::mem::size_of::<f64>()
            + std::mem::size_of::<bool>()) as u128
        + std::mem::size_of::<Mutex<Option<Failure>>>() as u128;
    if memory.retain(descriptor_bytes).is_err() {
        return Ok(false);
    }
    let (retained, mut candidate_output) = match candidate {
        Candidate::All | Candidate::Known(UniformValuesRef::Repeated { .. }) => {
            (Retained::All, None)
        }
        Candidate::Known(UniformValuesRef::Explicit(flags)) => (Retained::Known(flags), None),
        Candidate::Marker { position, output } => (
            Retained::Marker {
                position,
                output: std::ptr::null_mut(),
            },
            Some(output),
        ),
    };
    let result = (|| {
        let Some(bank) = Bank::new(
            rows,
            block_rows,
            targets,
            retained,
            candidate_output.as_deref_mut(),
            rhs.as_mut()
                .map(|(values, layout)| (&mut **values, *layout)),
        ) else {
            return Ok(false);
        };
        let Some(positions) = bank.positions(targets) else {
            return Ok(false);
        };
        let trace = Trace::new(rows, width, marker);
        let failures = Mutex::new(None::<Failure>);
        let Some(phases) = run_phases(
            spi,
            Transport { policy, team },
            &bank,
            &positions,
            (rows, blocks, block_rows * slots),
            &failures,
        )?
        else {
            return Ok(false);
        };
        drop(bank);
        if let Some(failure) = failures
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
        {
            return Err(failure.error(operation));
        }
        spi.check_interrupted()?;
        // Every block of every phase returned without failure, and a successful
        // block initializes all of its rows in every destination.
        for target in targets.iter_mut() {
            // SAFETY: capacity >= rows was checked; every row is initialized.
            unsafe {
                target.values.set_len(rows);
                target.missing.set_len(rows);
            }
        }
        if let Some(output) = candidate_output.as_mut() {
            // SAFETY: as above, for the marker-derived candidate.
            unsafe { output.set_len(rows) };
        }
        if let Some((values, layout)) = rhs.as_mut() {
            // SAFETY: as above; every row wrote `layout.width` values.
            unsafe { values.set_len(rows * layout.width) };
        }
        trace.finish(phases, blocks, team.threads());
        Ok(true)
    })();
    memory.release(descriptor_bytes)?;
    result
}

/// Developer Break injection: with the window closed and the phase joined,
/// publish the native marker and poll until Stata delivers a real Break.
fn park_for_break(spi: &mut dyn SpiApi, token: &[u8; 32], ppml: bool) -> Result<()> {
    let token = std::str::from_utf8(token)
        .map_err(|_| FeregError::new(ErrorCode::Internal, "invalid input Break token"))?;
    let (prefix, marker) = if ppml {
        (
            "feppml",
            crate::developer_injection::PPML_NATIVE_BREAK_MARKER,
        )
    } else {
        ("fereg", crate::developer_injection::OLS_NATIVE_BREAK_MARKER)
    };
    crate::developer_injection::create_native_break_marker(prefix, marker, token)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while std::time::Instant::now() < deadline {
        spi.check_interrupted()?;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    Err(FeregError::new(
        ErrorCode::Internal,
        "parallel input Break injection was not interrupted",
    ))
}

/// Run every phase: poll Break, open the window, read the phase's blocks on
/// the team and close the window once every worker has returned. `Ok(None)`:
/// the first window was refused, so nothing was read. `shape` is (rows,
/// blocks, values per block); failures accumulate in serial precedence.
fn run_phases(
    spi: &mut dyn SpiApi,
    transport: Transport,
    bank: &Bank<'_>,
    positions: &[u32],
    shape: (usize, usize, usize),
    failures: &Mutex<Option<Failure>>,
) -> Result<Option<usize>> {
    let Transport { policy, team } = transport;
    let (rows, blocks, block_values) = shape;
    // A phase holds a bounded number of values (Break latency), but at least
    // a few blocks per worker (join imbalance).
    let phase_blocks = (PHASE_VALUES / block_values.max(1))
        .max(team.threads().saturating_mul(4))
        .max(1);
    let mut payload = true;
    let mut phases = 0_usize;
    for phase_start in (0..blocks).step_by(phase_blocks) {
        spi.check_interrupted()?;
        let phase = phase_start..(phase_start + phase_blocks).min(blocks);
        let fault = (phases == 0).then_some(policy.fault).flatten();
        let opened = spi.with_numeric_window(positions, rows, &mut |source| {
            team.install(|| {
                phase
                    .clone()
                    .into_par_iter()
                    .with_max_len(1)
                    .for_each(|block| {
                        let failure = match fault {
                            Some(Fault::WorkerPanic) if block == phase.end - 1 => {
                                panic!("injected parallel input worker panic")
                            }
                            Some(Fault::ReadFailure) if block == phase.end - 1 => {
                                Some(payload_failure(bank.block(block).0, 0, 919))
                            }
                            _ => read_block(source, bank, block, payload),
                        };
                        if let Some(failure) = failure {
                            record(failures, failure);
                        }
                    });
            })
        })?;
        if !opened {
            if phases == 0 {
                return Ok(None);
            }
            return Err(FeregError::new(
                ErrorCode::Internal,
                "Stata numeric window was refused after parallel loading began",
            ));
        }
        phases += 1;
        let class = failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|failure| failure.key.class);
        match class {
            // Later marker rows cannot precede this one; stop reading.
            Some(MARKER) => break,
            // Serial loading validates the complete marker before payload,
            // so a later marker error still dominates a payload error.
            Some(_) if bank.marker() => payload = false,
            Some(_) => break,
            None => {}
        }
        if let Some(Fault::BreakAfterFirstPhase { token, ppml }) = fault {
            park_for_break(spi, &token, ppml)?;
        }
    }
    Ok(Some(phases))
}

fn check_reserved(
    rows: usize,
    targets: &[Target<'_>],
    candidate: &Candidate<'_>,
    rhs: Option<&(&mut Vec<f64>, NumericRhsLayout)>,
) -> Result<()> {
    let reserved = |len: usize, capacity: usize, required: usize| len == 0 && capacity >= required;
    let candidate_ok = match candidate {
        Candidate::All => true,
        Candidate::Known(flags) => flags.len() == rows,
        Candidate::Marker { output, .. } => reserved(output.len(), output.capacity(), rows),
    };
    let rhs_ok = rhs.is_none_or(|(values, layout)| {
        matches!(candidate, Candidate::All)
            && layout.width >= 2
            && layout.width - usize::from(layout.append_intercept) <= targets.len()
            && rows
                .checked_mul(layout.width)
                .is_some_and(|elements| reserved(values.len(), values.capacity(), elements))
    });
    if candidate_ok
        && rhs_ok
        && targets.iter().all(|target| {
            reserved(target.values.len(), target.values.capacity(), rows)
                && reserved(target.missing.len(), target.missing.capacity(), rows)
        })
    {
        Ok(())
    } else {
        Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "parallel numeric input has an invalid reserved shape",
        ))
    }
}

const MARKER: u8 = 0;
const PAYLOAD: u8 = 1;

/// Serial precedence: marker rows first (path B validates the whole marker
/// before any payload), then payload in row-major original order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FailureKey {
    class: u8,
    row: usize,
    slot: usize,
}

#[derive(Debug)]
struct Failure {
    key: FailureKey,
    kind: FailureKind,
}

#[derive(Debug)]
enum FailureKind {
    MarkerRead(i32),
    Marker(FeregError),
    Payload(i32),
}

impl Failure {
    fn error(self, operation: &str) -> FeregError {
        match self.kind {
            FailureKind::MarkerRead(rc) => spi_failure("numeric read", rc),
            FailureKind::Marker(error) => error,
            FailureKind::Payload(rc) => spi_failure(operation, rc),
        }
    }
}

fn spi_failure(operation: &str, rc: i32) -> FeregError {
    FeregError::new(
        ErrorCode::Internal,
        format!("Stata SPI {operation} failed with return code {rc}"),
    )
}

fn record(failures: &Mutex<Option<Failure>>, failure: Failure) {
    let mut current = failures.lock().unwrap_or_else(PoisonError::into_inner);
    if current
        .as_ref()
        .is_none_or(|existing| failure.key < existing.key)
    {
        *current = Some(failure);
    }
}

enum Retained<'a> {
    All,
    Known(&'a [bool]),
    Marker { position: u32, output: *mut bool },
}

/// Raw views of the reserved destinations. Every block owns a disjoint row
/// range of each; the tables themselves are read-only during a phase. The
/// owning vectors stay borrowed by the caller and are touched again only to
/// set their lengths after the bank is dropped.
struct Bank<'a> {
    rows: usize,
    block_rows: usize,
    values: Vec<*mut f64>,
    missing: Vec<*mut bool>,
    excluded_values: Vec<f64>,
    excluded_missing: Vec<bool>,
    retained: Retained<'a>,
    rhs: Option<(*mut f64, NumericRhsLayout)>,
}

// SAFETY: workers dereference the raw pointers only within the rows of their
// own block (`Bank::block`), and the pointer/fill tables are not mutated while
// a phase runs.
unsafe impl Sync for Bank<'_> {}

impl<'a> Bank<'a> {
    fn new(
        rows: usize,
        block_rows: usize,
        targets: &mut [Target<'_>],
        mut retained: Retained<'a>,
        candidate: Option<&mut Vec<bool>>,
        rhs: Option<(&mut Vec<f64>, NumericRhsLayout)>,
    ) -> Option<Self> {
        let width = targets.len();
        let mut values = reserved(width)?;
        let mut missing = reserved(width)?;
        let mut excluded_values = reserved(width)?;
        let mut excluded_missing = reserved(width)?;
        for target in targets.iter_mut() {
            values.push(target.values.spare_capacity_mut().as_mut_ptr().cast());
            missing.push(target.missing.spare_capacity_mut().as_mut_ptr().cast());
            excluded_values.push(target.excluded.0);
            excluded_missing.push(target.excluded.1);
        }
        if let (Retained::Marker { output, .. }, Some(candidate)) = (&mut retained, candidate) {
            *output = candidate.spare_capacity_mut().as_mut_ptr().cast();
        }
        Some(Self {
            rows,
            block_rows,
            values,
            missing,
            excluded_values,
            excluded_missing,
            retained,
            rhs: rhs
                .map(|(values, layout)| (values.spare_capacity_mut().as_mut_ptr().cast(), layout)),
        })
    }

    fn marker(&self) -> bool {
        matches!(self.retained, Retained::Marker { .. })
    }

    /// Window variables: the targets, then the marker in the last slot.
    fn positions(&self, targets: &[Target<'_>]) -> Option<Vec<u32>> {
        let mut positions = reserved(targets.len() + usize::from(self.marker()))?;
        positions.extend(targets.iter().map(|target| target.position));
        if let Retained::Marker { position, .. } = self.retained {
            positions.push(position);
        }
        Some(positions)
    }

    fn block(&self, block: usize) -> (usize, usize) {
        let start = block * self.block_rows;
        (start, self.block_rows.min(self.rows - start))
    }
}

fn reserved<T>(capacity: usize) -> Option<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).ok()?;
    Some(values)
}

fn read_block(
    source: &dyn NumericBlockSource,
    bank: &Bank<'_>,
    block: usize,
    payload: bool,
) -> Option<Failure> {
    let (start, rows) = bank.block(block);
    let retained: *const bool = match bank.retained {
        Retained::All => std::ptr::null(),
        Retained::Known(flags) => flags[start..start + rows].as_ptr(),
        Retained::Marker { output, .. } => {
            if let Some(failure) = read_marker(source, bank, start, rows, output) {
                return Some(failure);
            }
            output.wrapping_add(start)
        }
    };
    if !payload {
        return None;
    }
    if let Some(failure) = read_payload(source, bank, start, rows, retained) {
        return Some(failure);
    }
    if let Some((pointer, layout)) = bank.rhs {
        let copied = layout.width - usize::from(layout.append_intercept);
        for row in start..start + rows {
            let output = pointer.wrapping_add(row * layout.width);
            for (column, values) in bank.values[..copied].iter().enumerate() {
                // SAFETY: the payload read above initialized this block's rows,
                // and this block alone writes RHS row `row`.
                unsafe { output.add(column).write(values.add(row).read()) };
            }
            if layout.append_intercept {
                // SAFETY: as above; the intercept is the row's last element.
                unsafe { output.add(copied).write(1.0) };
            }
        }
    }
    None
}

/// Read and validate this block's marker. The first target's value and flag
/// rows serve as scratch; the payload read overwrites every one of them.
fn read_marker(
    source: &dyn NumericBlockSource,
    bank: &Bank<'_>,
    start: usize,
    rows: usize,
    output: *mut bool,
) -> Option<Failure> {
    let marker_slot = bank.values.len();
    let scratch = bank.values[0].wrapping_add(start);
    let flags = bank.missing[0].wrapping_add(start);
    let read = |first_row: usize, count: usize| {
        let values = [scratch.wrapping_add(first_row - start)];
        let missing = [flags.wrapping_add(first_row - start)];
        let request = BlockRead {
            first_slot: marker_slot,
            slots: 1,
            first_row,
            rows: count,
            retained: std::ptr::null(),
            excluded_values: std::ptr::null(),
            excluded_missing: std::ptr::null(),
            values: values.as_ptr(),
            missing: missing.as_ptr(),
        };
        // SAFETY: the scratch rows belong to this block, and both one-element
        // pointer tables live through the call.
        unsafe { source.read(&request) }
    };
    let validate = |offset: usize| -> std::result::Result<(), Failure> {
        // SAFETY: a successful read initialized this scratch row.
        let (value, missing) = unsafe { (scratch.add(offset).read(), flags.add(offset).read()) };
        let row = start + offset;
        let retained =
            crate::estimate::candidate_marker_value(value, missing, row + 1).map_err(|error| {
                Failure {
                    key: FailureKey {
                        class: MARKER,
                        row,
                        slot: 0,
                    },
                    kind: FailureKind::Marker(error),
                }
            })?;
        // SAFETY: this block alone writes candidate row `row`.
        unsafe { output.add(row).write(retained) };
        Ok(())
    };
    if read(start, rows).is_ok() {
        for offset in 0..rows {
            if let Err(failure) = validate(offset) {
                return Some(failure);
            }
        }
        return None;
    }
    // Locate the first failing row exactly, validating earlier rows in serial
    // order (the scalar marker pass reads and validates row by row).
    for offset in 0..rows {
        if let Err(rc) = read(start + offset, 1) {
            return Some(Failure {
                key: FailureKey {
                    class: MARKER,
                    row: start + offset,
                    slot: 0,
                },
                kind: FailureKind::MarkerRead(rc),
            });
        }
        if let Err(failure) = validate(offset) {
            return Some(failure);
        }
    }
    None
}

fn read_payload(
    source: &dyn NumericBlockSource,
    bank: &Bank<'_>,
    start: usize,
    rows: usize,
    retained: *const bool,
) -> Option<Failure> {
    let width = bank.values.len();
    let mut values = [std::ptr::null_mut::<f64>(); MAX_WINDOW_SLOTS];
    let mut missing = [std::ptr::null_mut::<bool>(); MAX_WINDOW_SLOTS];
    let mut failed = None;
    for first in (0..width).step_by(MAX_WINDOW_SLOTS) {
        let slots = (width - first).min(MAX_WINDOW_SLOTS);
        for slot in 0..slots {
            values[slot] = bank.values[first + slot].wrapping_add(start);
            missing[slot] = bank.missing[first + slot].wrapping_add(start);
        }
        let request = BlockRead {
            first_slot: first,
            slots,
            first_row: start,
            rows,
            retained,
            excluded_values: bank.excluded_values[first..].as_ptr(),
            excluded_missing: bank.excluded_missing[first..].as_ptr(),
            values: values.as_ptr(),
            missing: missing.as_ptr(),
        };
        // SAFETY: the block's rows of these destinations belong to this block;
        // the tables hold `slots` entries and the retained flags `rows`.
        if let Err(rc) = unsafe { source.read(&request) } {
            failed = Some(rc);
            break;
        }
    }
    let rc = failed?;
    // Locate the first failing coordinate in the serial row-major order.
    for offset in 0..rows {
        // SAFETY: `retained` covers `rows` initialized flags when non-null.
        if !retained.is_null() && !unsafe { retained.add(offset).read() } {
            continue;
        }
        for slot in 0..width {
            let value = [bank.values[slot].wrapping_add(start + offset)];
            let flag = [bank.missing[slot].wrapping_add(start + offset)];
            let request = BlockRead {
                first_slot: slot,
                slots: 1,
                first_row: start + offset,
                rows: 1,
                retained: std::ptr::null(),
                excluded_values: std::ptr::null(),
                excluded_missing: std::ptr::null(),
                values: value.as_ptr(),
                missing: flag.as_ptr(),
            };
            // SAFETY: one row of this block's destination; tables live here.
            if let Err(rc) = unsafe { source.read(&request) } {
                return Some(payload_failure(start + offset, slot, rc));
            }
        }
    }
    Some(payload_failure(start, 0, rc))
}

fn payload_failure(row: usize, slot: usize, rc: i32) -> Failure {
    Failure {
        key: FailureKey {
            class: PAYLOAD,
            row,
            slot,
        },
        kind: FailureKind::Payload(rc),
    }
}

/// Developer witness (`FEREG_SOLVER_TRACE`): FNV-1a over the complete loaded
/// input in fixed order (candidate, outcome, sources by id, weights, extra
/// PPML columns), independent of which reader produced it. Columns that a
/// prepared-load capsule took over are digested from the capsule.
pub(crate) fn trace_loaded_input(
    action: &str,
    loaded: &fereg_core::ols::LoadedOlsInput,
    owned: Option<&fereg_core::ols::PreparedOlsLoadColumns>,
    extras: &[Option<&fereg_core::ols::LoadedNumericColumn>],
) {
    if !fereg_core::diagnostics::solver_trace_enabled() {
        return;
    }
    struct Fnv(u64);
    impl Fnv {
        fn bytes(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.0 ^= u64::from(*byte);
                self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        fn column(&mut self, column: &fereg_core::ols::LoadedNumericColumn) {
            self.bytes(&(column.values.len() as u64).to_le_bytes());
            for value in &column.values {
                self.bytes(&value.to_bits().to_le_bytes());
            }
            for missing in &column.missing {
                self.bytes(&[u8::from(*missing)]);
            }
        }
    }
    let mut digest = Fnv(0xcbf2_9ce4_8422_2325);
    for retained in loaded.candidate.iter() {
        digest.bytes(&[u8::from(*retained)]);
    }
    digest.column(&loaded.outcome);
    for (id, column) in &loaded.numeric_sources {
        digest.bytes(&id.get().to_le_bytes());
        let taken = owned.and_then(|capsule| {
            capsule
                .columns()
                .find(|(owned_id, _)| owned_id == id)
                .map(|(_, column)| column)
        });
        digest.column(taken.filter(|_| column.values.is_empty()).unwrap_or(column));
    }
    for column in [
        &loaded.analytic_weight,
        &loaded.frequency_weight,
        &loaded.probability_weight,
    ]
    .into_iter()
    .flatten()
    .chain(extras.iter().copied().flatten())
    {
        digest.column(column);
    }
    fereg_core::diagnostics::emit_solver_trace(&format!(
        "FEREG_SOLVER_TRACE|phase=input_digest|action={action}|rows={}|digest={:016x}",
        loaded.candidate.len(),
        digest.0
    ));
}

/// Developer trace (`FEREG_SOLVER_TRACE`): shows that the window path ran.
struct Trace {
    started: Option<std::time::Instant>,
    rows: usize,
    width: usize,
    marker: bool,
}

impl Trace {
    fn new(rows: usize, width: usize, marker: bool) -> Self {
        Self {
            started: fereg_core::diagnostics::solver_trace_enabled().then(std::time::Instant::now),
            rows,
            width,
            marker,
        }
    }

    fn finish(&self, phases: usize, blocks: usize, workers: usize) {
        if let Some(started) = self.started {
            fereg_core::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=input_transport|mode=parallel|rows={}|width={}|marker={}|workers={workers}|phases={phases}|blocks={blocks}|seconds={:.9}",
                self.rows,
                self.width,
                u8::from(self.marker),
                started.elapsed().as_secs_f64()
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fereg_core::ThreadConfig;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Stata's runtime `.`; every numeric missing code is at or above it.
    const MISSING: f64 = 8.988_465_674_311_579e307;

    /// A Rust-owned dataset with a worker-callable window that follows the C
    /// bridge contract (values, `>=` flags, slot fills, first failing call).
    struct TestSpi {
        columns: Vec<Vec<f64>>,
        window: bool,
        opens: usize,
        open: std::sync::atomic::AtomicBool,
        polls: usize,
        interrupt_at_poll: Option<usize>,
        fail: Option<(usize, u32, i32)>,
        workers: std::sync::Mutex<HashSet<std::thread::ThreadId>>,
        reads: AtomicUsize,
    }

    impl TestSpi {
        fn new(columns: Vec<Vec<f64>>) -> Self {
            Self {
                columns,
                window: true,
                opens: 0,
                open: std::sync::atomic::AtomicBool::new(false),
                polls: 0,
                interrupt_at_poll: None,
                fail: None,
                workers: std::sync::Mutex::new(HashSet::new()),
                reads: AtomicUsize::new(0),
            }
        }

        fn rows(&self) -> usize {
            self.columns[0].len()
        }

        fn value(&self, variable: u32, row: usize) -> f64 {
            self.columns[variable as usize - 1][row]
        }
    }

    struct TestWindow<'a> {
        spi: &'a TestSpi,
        variables: Vec<u32>,
    }

    // SAFETY: on Ok every addressed row of every requested slot is written,
    // exactly as the C bridge does; destinations are disjoint per call.
    unsafe impl NumericBlockSource for TestWindow<'_> {
        unsafe fn read(&self, request: &BlockRead) -> std::result::Result<(), i32> {
            self.spi
                .workers
                .lock()
                .unwrap()
                .insert(std::thread::current().id());
            for offset in 0..request.rows {
                let row = request.first_row + offset;
                // SAFETY: the caller's pointers cover `rows` and `slots`.
                unsafe {
                    let excluded =
                        !request.retained.is_null() && !request.retained.add(offset).read();
                    for slot in 0..request.slots {
                        let value_out = request.values.add(slot).read().add(offset);
                        let flag_out = (!request.missing.is_null())
                            .then(|| request.missing.add(slot).read().add(offset));
                        if excluded {
                            value_out.write(request.excluded_values.add(slot).read());
                            if let Some(flag) = flag_out {
                                flag.write(request.excluded_missing.add(slot).read());
                            }
                            continue;
                        }
                        let variable = self.variables[request.first_slot + slot];
                        if self
                            .spi
                            .fail
                            .is_some_and(|(r, v, _)| r == row && v == variable)
                        {
                            return Err(self.spi.fail.unwrap().2);
                        }
                        self.spi.reads.fetch_add(1, Ordering::Relaxed);
                        let value = self.spi.value(variable, row);
                        value_out.write(value);
                        if let Some(flag) = flag_out {
                            flag.write(value >= MISSING);
                        }
                    }
                }
            }
            Ok(())
        }
    }

    struct CloseOnDrop<'a>(&'a std::sync::atomic::AtomicBool);

    impl Drop for CloseOnDrop<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }

    impl TestSpi {
        fn is_open(&self) -> bool {
            self.open.load(Ordering::SeqCst)
        }
    }

    impl SpiApi for TestSpi {
        fn nobs(&mut self) -> Result<usize> {
            assert!(!self.is_open());
            Ok(self.rows())
        }
        fn nvars(&mut self) -> Result<usize> {
            Ok(self.columns.len())
        }
        fn variable_is_string(&mut self, _: u32) -> Result<bool> {
            Ok(false)
        }
        fn variable_is_strl(&mut self, _: u32) -> Result<bool> {
            Ok(false)
        }
        fn read_numeric(&mut self, variable: u32, observation: usize) -> Result<(f64, bool)> {
            assert!(!self.is_open(), "no SPI call while the window is open");
            let value = self.value(variable, observation - 1);
            Ok((value, value >= MISSING))
        }
        fn supports_numeric_window(&self) -> bool {
            self.window
        }
        fn with_numeric_window(
            &mut self,
            variables: &[u32],
            rows: usize,
            run: &mut dyn FnMut(&dyn NumericBlockSource) -> Result<()>,
        ) -> Result<bool> {
            assert!(!self.is_open());
            if rows != self.rows()
                || variables
                    .iter()
                    .any(|&v| v == 0 || v as usize > self.columns.len())
            {
                return Ok(false);
            }
            self.opens += 1;
            self.open.store(true, Ordering::SeqCst);
            let _close = CloseOnDrop(&self.open);
            let window = TestWindow {
                spi: self,
                variables: variables.to_vec(),
            };
            run(&window).map(|()| true)
        }
        fn read_string(&mut self, _: u32, _: usize) -> Result<Vec<u8>> {
            unreachable!("numeric fixture")
        }
        fn matrix_dimensions(&mut self, _: &str) -> Result<(usize, usize)> {
            unreachable!()
        }
        fn matrix_element(&mut self, _: &str, _: usize, _: usize) -> Result<f64> {
            unreachable!()
        }
        fn store_numeric(&mut self, _: u32, _: usize, _: f64) -> Result<()> {
            unreachable!()
        }
        fn store_matrix(
            &mut self,
            _: &crate::input::PreparedMatrixStoreName,
            _: usize,
            _: usize,
            _: f64,
        ) -> Result<()> {
            unreachable!()
        }
        fn display(&mut self, _: &str) -> Result<()> {
            Ok(())
        }
        fn check_interrupted(&mut self) -> Result<()> {
            assert!(
                !self.is_open(),
                "Break is polled only with the window closed"
            );
            self.polls += 1;
            if self.interrupt_at_poll == Some(self.polls) {
                return Err(FeregError::new(ErrorCode::Interrupted, "test Break"));
            }
            Ok(())
        }
    }

    fn transport(policy: Policy, threads: usize) -> Transport {
        Transport {
            policy,
            team: ThreadTeam::new(ThreadConfig::from_requested(threads, true).unwrap()),
        }
    }

    /// Every storage-visible value class: finite, signed zero, huge, and the
    /// 27 missing codes (`.`, `.a` ... `.z` are increasing above `.`).
    fn dataset(rows: usize, width: usize, seed: u64) -> Vec<Vec<f64>> {
        (0..width)
            .map(|column| {
                (0..rows)
                    .map(|row| {
                        let mix = (row as u64)
                            .wrapping_mul(6_364_136_223_846_793_005)
                            .wrapping_add(seed ^ column as u64)
                            >> 33;
                        match mix % 11 {
                            0 => MISSING * (1.0 + (mix % 27) as f64 / 64.0),
                            1 => -0.0,
                            2 => f64::MAX / 3.0,
                            _ => (mix as f64 - 2e9) / 7.0,
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn marker_column(rows: usize, pattern: &str) -> Vec<f64> {
        (0..rows)
            .map(|row| {
                f64::from(match pattern {
                    "all" => true,
                    "none" => false,
                    "runs" => (row / 97) % 3 != 1,
                    "scattered" => row % 3 == 0 || row % 7 == 5,
                    "tail" => row + 1 == rows,
                    _ => unreachable!(),
                })
            })
            .collect()
    }

    struct Loaded {
        values: Vec<Vec<f64>>,
        missing: Vec<Vec<bool>>,
        candidate: Option<Vec<bool>>,
        rhs: Option<Vec<f64>>,
    }

    #[allow(clippy::too_many_arguments)]
    fn run(
        spi: &mut TestSpi,
        transport: Transport,
        width: usize,
        marker: Option<u32>,
        known: Option<&[bool]>,
        fill: (f64, bool),
        rhs_layout: Option<NumericRhsLayout>,
        budget: Option<u128>,
    ) -> (Result<bool>, Loaded) {
        let rows = spi.rows();
        let mut values: Vec<Vec<f64>> = (0..width).map(|_| Vec::with_capacity(rows)).collect();
        let mut missing: Vec<Vec<bool>> = (0..width).map(|_| Vec::with_capacity(rows)).collect();
        let mut candidate = marker.map(|_| Vec::with_capacity(rows));
        let mut rhs = rhs_layout.map(|layout| Vec::with_capacity(rows * layout.width));
        let mut memory = InputMemory::new(budget, 0).unwrap();
        let result = {
            let mut targets: Vec<Target<'_>> = values
                .iter_mut()
                .zip(missing.iter_mut())
                .enumerate()
                .map(|(index, (values, missing))| Target {
                    position: index as u32 + 1,
                    values,
                    missing,
                    excluded: fill,
                })
                .collect();
            let candidate_arg = match (marker, candidate.as_mut(), known) {
                (Some(position), Some(output), _) => Candidate::Marker { position, output },
                (_, _, Some(flags)) => Candidate::Known(UniformValuesRef::Explicit(flags)),
                _ => Candidate::All,
            };
            try_read(
                spi,
                transport,
                &mut memory,
                rows,
                &mut targets,
                candidate_arg,
                rhs.as_mut().zip(rhs_layout),
                "numeric column read",
            )
        };
        (
            result,
            Loaded {
                values,
                missing,
                candidate,
                rhs,
            },
        )
    }

    /// The serial readers' semantics, written independently.
    fn expected(
        spi: &TestSpi,
        width: usize,
        retained: &dyn Fn(usize) -> bool,
        fill: (f64, bool),
    ) -> (Vec<Vec<u64>>, Vec<Vec<bool>>) {
        let rows = spi.rows();
        let values = (0..width)
            .map(|column| {
                (0..rows)
                    .map(|row| {
                        if retained(row) {
                            spi.columns[column][row].to_bits()
                        } else {
                            fill.0.to_bits()
                        }
                    })
                    .collect()
            })
            .collect();
        let missing = (0..width)
            .map(|column| {
                (0..rows)
                    .map(|row| {
                        if retained(row) {
                            spi.columns[column][row] >= MISSING
                        } else {
                            fill.1
                        }
                    })
                    .collect()
            })
            .collect();
        (values, missing)
    }

    fn bits(values: &[Vec<f64>]) -> Vec<Vec<u64>> {
        values
            .iter()
            .map(|column| column.iter().map(|value| value.to_bits()).collect())
            .collect()
    }

    #[test]
    fn transported_values_flags_and_candidates_equal_the_serial_semantics() {
        let caller = std::thread::current().id();
        for &(rows, width) in &[
            (40_003_usize, 1_usize),
            (16_387, 1),
            (12_000, 3),
            (513, 63),
            (2_049, 64),
            (1_031, 65),
            (300, 130),
        ] {
            for threads in [2, 3, 8] {
                for pattern in ["all", "runs", "scattered", "tail", "none"] {
                    let mut columns = dataset(rows, width, rows as u64 * 31 + width as u64);
                    columns.push(marker_column(rows, pattern));
                    let mut spi = TestSpi::new(columns);
                    let marker = width as u32 + 1;
                    let (result, loaded) = run(
                        &mut spi,
                        transport(Policy::PARALLEL, threads),
                        width,
                        Some(marker),
                        None,
                        (0.0, false),
                        None,
                        None,
                    );
                    assert_eq!(result, Ok(true), "rows={rows} width={width} {pattern}");
                    let flags = marker_column(rows, pattern);
                    let (values, missing) =
                        expected(&spi, width, &|row| flags[row] == 1.0, (0.0, false));
                    assert_eq!(bits(&loaded.values), values, "{rows}x{width} {pattern}");
                    assert_eq!(loaded.missing, missing, "{rows}x{width} {pattern}");
                    let candidate: Vec<bool> = flags.iter().map(|flag| *flag == 1.0).collect();
                    assert_eq!(loaded.candidate.as_deref(), Some(candidate.as_slice()));
                    let workers = spi.workers.lock().unwrap();
                    assert!(!workers.is_empty() && !workers.contains(&caller));
                    assert!(spi.opens >= 1 && !spi.is_open());
                }
            }
        }
    }

    #[test]
    fn known_candidates_and_all_rows_keep_their_storage_contract() {
        for &(rows, width) in &[(20_000_usize, 1_usize), (5_000, 7), (777, 70)] {
            let columns = dataset(rows, width, 5);
            let flags: Vec<bool> = (0..rows).map(|row| row % 5 != 2).collect();
            for (known, fill) in [(Some(flags.as_slice()), (0.0, true)), (None, (0.0, false))] {
                let mut spi = TestSpi::new(columns.clone());
                let (result, loaded) = run(
                    &mut spi,
                    transport(Policy::PARALLEL, 4),
                    width,
                    None,
                    known,
                    fill,
                    None,
                    None,
                );
                assert_eq!(result, Ok(true));
                let retained = |row: usize| known.is_none_or(|flags| flags[row]);
                let (values, missing) = expected(&spi, width, &retained, fill);
                assert_eq!(bits(&loaded.values), values);
                assert_eq!(loaded.missing, missing);
                assert!(loaded.candidate.is_none());
                if known.is_some() {
                    // Excluded rows are never read.
                    let retained_rows = flags.iter().filter(|flag| **flag).count();
                    assert_eq!(spi.reads.load(Ordering::Relaxed), retained_rows * width);
                }
            }
        }
    }

    #[test]
    fn packed_rows_match_the_tile_recipe() {
        for append_intercept in [false, true] {
            let (rows, width) = (9_001, 5);
            let mut spi = TestSpi::new(dataset(rows, width, 9));
            let layout = NumericRhsLayout {
                width: if append_intercept { 4 } else { 3 },
                append_intercept,
            };
            let (result, loaded) = run(
                &mut spi,
                transport(Policy::PARALLEL, 3),
                width,
                None,
                None,
                (0.0, false),
                Some(layout),
                None,
            );
            assert_eq!(result, Ok(true));
            let rhs = loaded.rhs.unwrap();
            assert_eq!(rhs.len(), rows * layout.width);
            for row in 0..rows {
                for column in 0..3 {
                    assert_eq!(
                        rhs[row * layout.width + column].to_bits(),
                        spi.columns[column][row].to_bits()
                    );
                }
                if append_intercept {
                    assert_eq!(rhs[row * layout.width + 3], 1.0);
                }
            }
        }
    }

    #[test]
    fn refusals_before_dispatch_leave_destinations_untouched() {
        let rows = 50_000;
        let fresh = || TestSpi::new(dataset(rows, 4, 3));
        let cases: Vec<(&str, TestSpi, Transport, Option<u128>)> = vec![
            ("serial policy", fresh(), transport(Policy::SERIAL, 4), None),
            ("one worker", fresh(), transport(Policy::PARALLEL, 1), None),
            (
                "tight budget",
                fresh(),
                transport(Policy::PARALLEL, 4),
                Some(64),
            ),
            (
                "no window",
                {
                    let mut spi = fresh();
                    spi.window = false;
                    spi
                },
                transport(Policy::PARALLEL, 4),
                None,
            ),
        ];
        for (label, mut spi, transport, budget) in cases {
            let (result, loaded) = run(
                &mut spi,
                transport,
                4,
                None,
                None,
                (0.0, false),
                None,
                budget,
            );
            assert_eq!(result, Ok(false), "{label}");
            assert!(loaded.values.iter().all(Vec::is_empty), "{label}");
            assert_eq!(spi.reads.load(Ordering::Relaxed), 0, "{label}");
            assert_eq!(spi.opens, 0, "{label}");
        }
        // Tiny inputs stay serial by the general work policy.
        let mut spi = TestSpi::new(dataset(100, 2, 3));
        let (result, _) = run(
            &mut spi,
            transport(Policy::PARALLEL, 8),
            2,
            None,
            None,
            (0.0, false),
            None,
            None,
        );
        assert_eq!(result, Ok(false));
    }

    #[test]
    fn errors_follow_serial_precedence_and_never_expose_partial_storage() {
        let (rows, width) = (40_000, 3);
        let marker = width as u32 + 1;
        let setup = |bad_rows: &[usize]| {
            let mut columns = dataset(rows, width, 11);
            let mut flags = marker_column(rows, "runs");
            for &row in bad_rows {
                flags[row] = if row % 2 == 0 { 2.0 } else { MISSING };
            }
            columns.push(flags);
            TestSpi::new(columns)
        };
        // The first invalid marker row wins, even over an earlier payload error.
        for threads in [2, 5, 8] {
            let mut spi = setup(&[39_000, 7_001, 30_000]);
            spi.fail = Some((5, 1, 917));
            let (result, loaded) = run(
                &mut spi,
                transport(Policy::PARALLEL, threads),
                width,
                Some(marker),
                None,
                (0.0, false),
                None,
                None,
            );
            let error = result.unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert!(
                error.message.contains("observation 7002"),
                "{}",
                error.message
            );
            assert!(loaded.values.iter().all(Vec::is_empty));
            assert!(loaded.candidate.unwrap().is_empty());
        }
        // Without a marker error, the first payload coordinate in row-major
        // order reports the SDK code with the replaced operation's wording.
        let mut spi = setup(&[]);
        spi.fail = Some((20_000, 2, 919));
        let (result, loaded) = run(
            &mut spi,
            transport(Policy::PARALLEL, 4),
            width,
            Some(marker),
            None,
            (0.0, false),
            None,
            None,
        );
        assert_eq!(
            result.unwrap_err().message,
            "Stata SPI numeric column read failed with return code 919"
        );
        assert!(loaded.values.iter().all(Vec::is_empty));
        // A marker read failure is a marker-pass error.
        let mut spi = setup(&[]);
        spi.fail = Some((12_345, marker, 918));
        let (result, _) = run(
            &mut spi,
            transport(Policy::PARALLEL, 4),
            width,
            Some(marker),
            None,
            (0.0, false),
            None,
            None,
        );
        assert_eq!(
            result.unwrap_err().message,
            "Stata SPI numeric read failed with return code 918"
        );
    }

    #[test]
    fn break_between_phases_returns_the_interruption_after_joining() {
        for poll in [1, 2, 5] {
            let mut spi = TestSpi::new(dataset(30_000, 64, 1));
            spi.interrupt_at_poll = Some(poll);
            let (result, loaded) = run(
                &mut spi,
                transport(Policy::PARALLEL, 4),
                64,
                None,
                None,
                (0.0, false),
                None,
                None,
            );
            assert_eq!(result.unwrap_err().code, ErrorCode::Interrupted);
            assert_eq!(spi.opens, poll - 1);
            assert!(!spi.is_open());
            assert!(loaded.values.iter().all(Vec::is_empty));
        }
    }

    #[test]
    fn injected_worker_panic_and_read_failure_close_the_window() {
        let mut spi = TestSpi::new(dataset(30_000, 8, 2));
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(
                &mut spi,
                transport(Policy::PARALLEL.with_fault(Fault::WorkerPanic), 4),
                8,
                None,
                None,
                (0.0, false),
                None,
                None,
            )
        }));
        assert!(panic.is_err());
        assert!(!spi.is_open(), "the window closes during unwind");
        // The same session reads again successfully.
        let (result, loaded) = run(
            &mut spi,
            transport(Policy::PARALLEL, 4),
            8,
            None,
            None,
            (0.0, false),
            None,
            None,
        );
        assert_eq!(result, Ok(true));
        assert_eq!(loaded.values[0].len(), 30_000);
        let (result, loaded) = run(
            &mut spi,
            transport(Policy::PARALLEL.with_fault(Fault::ReadFailure), 4),
            8,
            None,
            None,
            (0.0, false),
            None,
            None,
        );
        assert_eq!(
            result.unwrap_err().message,
            "Stata SPI numeric column read failed with return code 919"
        );
        assert!(loaded.values.iter().all(Vec::is_empty));
    }

    #[test]
    fn host_policy_requires_every_public_fact() {
        let qualified = QUALIFIED
            .iter()
            .find(|&&(os, arch, ..)| os == std::env::consts::OS && arch == std::env::consts::ARCH);
        let Some(&(_, _, version, edition, interface)) = qualified else {
            assert!(
                !Policy::from_host(Some("19"), Some("MP"), Some("console"), Some("0")).enabled()
            );
            return;
        };
        let version = format!("{version}.5");
        let host = |stata, edition, interface, aliases| {
            Policy::from_host(stata, edition, interface, aliases).enabled()
        };
        assert!(host(
            Some(&version),
            Some(edition),
            Some(interface),
            Some("0")
        ));
        assert!(!host(
            Some(&version),
            Some(edition),
            Some(interface),
            Some("1")
        ));
        assert!(!host(Some(&version), Some(edition), Some(interface), None));
        assert!(!host(
            Some(&version),
            Some("BE"),
            Some(interface),
            Some("0")
        ));
        assert!(!host(
            Some(&version),
            Some(edition),
            Some("unknown"),
            Some("0")
        ));
        assert!(!host(Some("14"), Some(edition), Some(interface), Some("0")));
        assert!(!host(None, Some(edition), Some(interface), Some("0")));
        assert!(!host(
            Some("nan"),
            Some(edition),
            Some(interface),
            Some("0")
        ));
        assert!(!Policy::SERIAL.enabled());
        assert!(
            !Policy::from_host(Some(&version), Some(edition), Some(interface), Some("0"))
                .serial()
                .enabled()
        );
    }

    #[test]
    fn repeated_reads_are_identical_across_worker_counts() {
        let rows = 25_000;
        let mut columns = dataset(rows, 9, 77);
        columns.push(marker_column(rows, "scattered"));
        let mut reference = None;
        for threads in [2, 3, 4, 8, 2] {
            let mut spi = TestSpi::new(columns.clone());
            let (result, loaded) = run(
                &mut spi,
                transport(Policy::PARALLEL, threads),
                9,
                Some(10),
                None,
                (0.0, false),
                None,
                None,
            );
            assert_eq!(result, Ok(true));
            let snapshot = (bits(&loaded.values), loaded.missing, loaded.candidate);
            match &reference {
                None => reference = Some(snapshot),
                Some(reference) => assert_eq!(&snapshot, reference),
            }
        }
    }
}
