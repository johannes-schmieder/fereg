use crate::dispatch::dispatch_args;
use crate::estimate::{estimate_ols_test, ACTION_ESTIMATE_OLS};
use crate::input::{prepare_ols_test, PreparedMatrixStoreName, SpiApi, ACTION_PREPARE_OLS_TEST};
use crate::output::OutputSink;
use fereg_core::error::{ErrorCode, FeregError};
use std::alloc::{GlobalAlloc, Layout, System};
use std::any::Any;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

struct TrackingAllocator;

static ALLOCATION_CURRENT_BYTES: AtomicU64 = AtomicU64::new(0);
static ALLOCATION_PEAK_BYTES: AtomicU64 = AtomicU64::new(0);
static PREVIOUS_ACTION_BASELINE_BYTES: AtomicU64 = AtomicU64::new(u64::MAX);

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the request is forwarded unchanged to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the request is forwarded unchanged to the system allocator.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record_deallocation(layout.size());
        // SAFETY: the pointer/layout pair came from the system allocator.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the pointer/layout pair and new size are forwarded unchanged.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            if new_size >= layout.size() {
                record_allocation(new_size - layout.size());
            } else {
                record_deallocation(layout.size() - new_size);
            }
        }
        replacement
    }
}

fn record_allocation(bytes: usize) {
    let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
    let current = ALLOCATION_CURRENT_BYTES
        .fetch_add(bytes, Ordering::Relaxed)
        .saturating_add(bytes);
    ALLOCATION_PEAK_BYTES.fetch_max(current, Ordering::Relaxed);
}

fn record_deallocation(bytes: usize) {
    let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
    let _ =
        ALLOCATION_CURRENT_BYTES.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            Some(current.saturating_sub(bytes))
        });
}

pub(crate) fn begin_allocation_observation() -> (u64, u64) {
    let current = ALLOCATION_CURRENT_BYTES.load(Ordering::Relaxed);
    ALLOCATION_PEAK_BYTES.store(current, Ordering::Relaxed);
    let previous = PREVIOUS_ACTION_BASELINE_BYTES.swap(current, Ordering::Relaxed);
    (
        current,
        if previous == u64::MAX {
            current
        } else {
            previous
        },
    )
}

pub(crate) fn allocation_usage() -> (u64, u64) {
    (
        ALLOCATION_CURRENT_BYTES.load(Ordering::Relaxed),
        ALLOCATION_PEAK_BYTES.load(Ordering::Relaxed),
    )
}

#[cfg(target_os = "linux")]
pub(crate) fn system_available_memory_bytes() -> Option<u64> {
    parse_linux_memory_field("/proc/meminfo", "MemAvailable:", 1024)
}

#[cfg(target_os = "linux")]
pub(crate) fn process_rss_bytes() -> (Option<u64>, Option<u64>) {
    let current = parse_linux_memory_field("/proc/self/status", "VmRSS:", 1024);
    let peak = parse_linux_memory_field("/proc/self/status", "VmHWM:", 1024);
    (current, peak)
}

#[cfg(target_os = "linux")]
fn parse_linux_memory_field(path: &str, field: &str, multiplier: u64) -> Option<u64> {
    let contents = std::fs::read_to_string(path).ok()?;
    let value = contents
        .lines()
        .find_map(|line| line.strip_prefix(field))?
        .split_ascii_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    value.checked_mul(multiplier)
}

#[cfg(target_os = "macos")]
pub(crate) fn system_available_memory_bytes() -> Option<u64> {
    let page_size = apple_sysctl_u64(c"hw.pagesize")?;
    let free = apple_sysctl_u64(c"vm.page_free_count")?;
    let inactive = apple_sysctl_u64(c"vm.page_inactive_count").unwrap_or(0);
    let speculative = apple_sysctl_u64(c"vm.page_speculative_count").unwrap_or(0);
    free.checked_add(inactive)?
        .checked_add(speculative)?
        .checked_mul(page_size)
}

#[cfg(target_os = "macos")]
fn apple_sysctl_u64(name: &CStr) -> Option<u64> {
    let mut value = 0_u64;
    let mut size = std::mem::size_of::<u64>();
    // SAFETY: both output pointers are live for `size`, and no input value is supplied.
    let result = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&mut value as *mut u64).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (result == 0 && matches!(size, 4 | 8)).then_some(value)
}

#[cfg(target_os = "macos")]
pub(crate) fn process_rss_bytes() -> (Option<u64>, Option<u64>) {
    let mut current = std::mem::MaybeUninit::<libc::rusage_info_v0>::zeroed();
    // SAFETY: `current` has the exact V0 layout and remains live for the call.
    let current_result = unsafe {
        libc::proc_pid_rusage(
            libc::getpid(),
            libc::RUSAGE_INFO_V0,
            current.as_mut_ptr().cast(),
        )
    };
    let current = (current_result == 0).then(|| {
        // SAFETY: proc_pid_rusage initialized the V0 structure on success.
        unsafe { current.assume_init().ri_resident_size }
    });

    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` points to writable storage for an exact `rusage` value.
    let peak_result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    let peak = (peak_result == 0).then(|| {
        // SAFETY: getrusage initialized the structure on success; macOS reports bytes.
        u64::try_from(unsafe { usage.assume_init().ru_maxrss }).ok()
    });
    (current, peak.flatten())
}

#[cfg(windows)]
pub(crate) fn system_available_memory_bytes() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).ok()?,
        ..Default::default()
    };
    // SAFETY: `status` is initialized with the required structure length.
    (unsafe { GlobalMemoryStatusEx(&mut status) } != 0).then_some(status.ullAvailPhys)
}

#[cfg(windows)]
pub(crate) fn process_rss_bytes() -> (Option<u64>, Option<u64>) {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    counters.cb = match u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS>()) {
        Ok(value) => value,
        Err(_) => return (None, None),
    };
    // SAFETY: the pseudo-handle is valid in this process and `counters` has its exact size.
    let result =
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    if result == 0 {
        (None, None)
    } else {
        (
            u64::try_from(counters.WorkingSetSize).ok(),
            u64::try_from(counters.PeakWorkingSetSize).ok(),
        )
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(crate) const fn system_available_memory_bytes() -> Option<u64> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(crate) const fn process_rss_bytes() -> (Option<u64>, Option<u64>) {
    (None, None)
}

#[cfg(not(test))]
extern "C" {
    fn fereg_spi_display(message: *const c_char) -> c_int;
    fn fereg_spi_error(message: *const c_char) -> c_int;
    fn fereg_spi_poll() -> c_int;
    fn fereg_spi_stop_requested() -> c_int;
    fn fereg_spi_nobs() -> c_int;
    fn fereg_spi_nvars() -> c_int;
    fn fereg_spi_is_missing(value: f64) -> c_int;
    fn fereg_spi_var_is_string(variable: c_int) -> c_int;
    fn fereg_spi_var_is_strl(variable: c_int) -> c_int;
    fn fereg_spi_sdatalen(variable: c_int, observation: c_int) -> c_int;
    fn fereg_spi_read_string(
        variable: c_int,
        observation: c_int,
        buffer: *mut c_char,
        length: c_int,
    ) -> c_int;
    fn fereg_spi_vdata(variable: c_int, observation: c_int, value: *mut f64) -> c_int;
    fn fereg_spi_vstore(variable: c_int, observation: c_int, value: f64) -> c_int;
    fn fereg_spi_matrix_rows(name: *const c_char) -> c_int;
    fn fereg_spi_matrix_cols(name: *const c_char) -> c_int;
    fn fereg_spi_mat_el(name: *const c_char, row: c_int, column: c_int, value: *mut f64) -> c_int;
    fn fereg_spi_mat_store(name: *const c_char, row: c_int, column: c_int, value: f64) -> c_int;
    fn fereg_spi_is_main_thread() -> c_int;
}

#[cfg(test)]
unsafe fn fereg_spi_display(_message: *const c_char) -> c_int {
    0
}

#[cfg(test)]
unsafe fn fereg_spi_error(_message: *const c_char) -> c_int {
    0
}

#[cfg(test)]
unsafe fn fereg_spi_poll() -> c_int {
    0
}
#[cfg(test)]
unsafe fn fereg_spi_stop_requested() -> c_int {
    0
}
#[cfg(test)]
unsafe fn fereg_spi_nobs() -> c_int {
    -1
}
#[cfg(test)]
unsafe fn fereg_spi_nvars() -> c_int {
    -1
}
#[cfg(test)]
unsafe fn fereg_spi_is_missing(_value: f64) -> c_int {
    0
}
#[cfg(test)]
unsafe fn fereg_spi_var_is_string(_variable: c_int) -> c_int {
    -1
}
#[cfg(test)]
unsafe fn fereg_spi_var_is_strl(_variable: c_int) -> c_int {
    -1
}
#[cfg(test)]
unsafe fn fereg_spi_sdatalen(_variable: c_int, _observation: c_int) -> c_int {
    -1
}
#[cfg(test)]
unsafe fn fereg_spi_read_string(
    _variable: c_int,
    _observation: c_int,
    _buffer: *mut c_char,
    _length: c_int,
) -> c_int {
    498
}
#[cfg(test)]
unsafe fn fereg_spi_vdata(_variable: c_int, _observation: c_int, _value: *mut f64) -> c_int {
    498
}
#[cfg(test)]
unsafe fn fereg_spi_vstore(_variable: c_int, _observation: c_int, _value: f64) -> c_int {
    498
}
#[cfg(test)]
unsafe fn fereg_spi_matrix_rows(_name: *const c_char) -> c_int {
    -1
}
#[cfg(test)]
unsafe fn fereg_spi_matrix_cols(_name: *const c_char) -> c_int {
    -1
}
#[cfg(test)]
unsafe fn fereg_spi_mat_el(
    _name: *const c_char,
    _row: c_int,
    _column: c_int,
    _value: *mut f64,
) -> c_int {
    498
}
#[cfg(test)]
unsafe fn fereg_spi_mat_store(
    _name: *const c_char,
    _row: c_int,
    _column: c_int,
    _value: f64,
) -> c_int {
    498
}
#[cfg(test)]
unsafe fn fereg_spi_is_main_thread() -> c_int {
    0
}

struct SpiSink;

const NUMERIC_STORE_ERROR: &str = "Stata SPI numeric store failed during atomic result commit";
const MATRIX_STORE_ERROR: &str = "Stata SPI matrix store failed during atomic result commit";
const NUMERIC_STORE_ERROR_REPORT: &CStr =
    c"fereg: internal: Stata SPI numeric store failed during atomic result commit\n";
const MATRIX_STORE_ERROR_REPORT: &CStr =
    c"fereg: internal: Stata SPI matrix store failed during atomic result commit\n";

/// A token proving that all SPI calls remain on Stata's bound calling thread.
/// `Rc` makes the session neither `Send` nor `Sync` without any unsafe marker impl.
struct SpiSession {
    _not_send_or_sync: PhantomData<Rc<()>>,
    numeric_store_error: Option<FeregError>,
    matrix_store_error: Option<FeregError>,
}

impl SpiSession {
    fn new() -> Result<Self, FeregError> {
        // SAFETY: the C bridge exposes this predicate for calls from any thread.
        if unsafe { fereg_spi_is_main_thread() } != 1 {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "Stata SPI session is unavailable or not on the bound calling thread",
            ));
        }
        Ok(Self {
            _not_send_or_sync: PhantomData,
            numeric_store_error: Some(FeregError::new(ErrorCode::Internal, NUMERIC_STORE_ERROR)),
            matrix_store_error: Some(FeregError::new(ErrorCode::Internal, MATRIX_STORE_ERROR)),
        })
    }

    fn position(value: u32, label: &str) -> Result<c_int, FeregError> {
        c_int::try_from(value).map_err(|_| {
            FeregError::new(
                ErrorCode::InvalidArgument,
                format!("{label} exceeds the Stata SPI integer range"),
            )
        })
    }

    fn observation(value: usize) -> Result<c_int, FeregError> {
        c_int::try_from(value).map_err(|_| {
            FeregError::new(
                ErrorCode::InvalidArgument,
                "observation number exceeds the Stata SPI integer range",
            )
        })
    }

    fn spi_rc(rc: c_int, operation: &str) -> Result<(), FeregError> {
        if rc == 0 {
            Ok(())
        } else {
            Err(FeregError::new(
                ErrorCode::Internal,
                format!("Stata SPI {operation} failed with return code {rc}"),
            ))
        }
    }

    fn matrix_name(name: &str) -> CString {
        sanitize_c_string(name)
    }

    fn store_rc(rc: c_int, prepared_error: &mut Option<FeregError>) -> Result<(), FeregError> {
        if rc == 0 {
            Ok(())
        } else {
            Err(prepared_error
                .take()
                .expect("atomic store error was prepared before commit"))
        }
    }
}

impl SpiApi for SpiSession {
    fn nobs(&mut self) -> Result<usize, FeregError> {
        // SAFETY: `SpiSession` exists only on the bound Stata thread.
        let value = unsafe { fereg_spi_nobs() };
        usize::try_from(value).map_err(|_| {
            FeregError::new(
                ErrorCode::Internal,
                format!("Stata SPI returned invalid observation count {value}"),
            )
        })
    }

    fn nvars(&mut self) -> Result<usize, FeregError> {
        // SAFETY: `SpiSession` exists only on the bound Stata thread.
        let value = unsafe { fereg_spi_nvars() };
        usize::try_from(value).map_err(|_| {
            FeregError::new(
                ErrorCode::Internal,
                format!("Stata SPI returned invalid plugin-varlist width {value}"),
            )
        })
    }

    fn variable_is_string(&mut self, variable: u32) -> Result<bool, FeregError> {
        let variable = Self::position(variable, "variable position")?;
        // SAFETY: position is representable and session is thread-confined.
        match unsafe { fereg_spi_var_is_string(variable) } {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(FeregError::new(
                ErrorCode::Internal,
                format!("Stata SPI returned invalid string status {value}"),
            )),
        }
    }

    fn variable_is_strl(&mut self, variable: u32) -> Result<bool, FeregError> {
        let variable = Self::position(variable, "variable position")?;
        // SAFETY: position is representable and the session is thread-confined.
        match unsafe { fereg_spi_var_is_strl(variable) } {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(FeregError::new(
                ErrorCode::Internal,
                format!("Stata SPI returned invalid strL status {value}"),
            )),
        }
    }

    fn read_numeric(
        &mut self,
        variable: u32,
        observation: usize,
    ) -> Result<(f64, bool), FeregError> {
        let variable = Self::position(variable, "variable position")?;
        let observation = Self::observation(observation)?;
        let mut value = 0.0_f64;
        // SAFETY: output pointer is live; indices are checked and the session is confined.
        Self::spi_rc(
            unsafe { fereg_spi_vdata(variable, observation, &mut value) },
            "numeric read",
        )?;
        // SAFETY: missing-value classification occurs on the same bound thread.
        let missing = unsafe { fereg_spi_is_missing(value) };
        match missing {
            0 => Ok((value, false)),
            1 => Ok((value, true)),
            other => Err(FeregError::new(
                ErrorCode::Internal,
                format!("Stata SPI returned invalid missing status {other}"),
            )),
        }
    }

    fn read_string(&mut self, variable: u32, observation: usize) -> Result<Vec<u8>, FeregError> {
        let variable = Self::position(variable, "variable position")?;
        let observation = Self::observation(observation)?;
        // SAFETY: indices are checked and the session is thread-confined.
        let needed = unsafe { fereg_spi_sdatalen(variable, observation) };
        if needed < 0 {
            return Err(FeregError::new(
                ErrorCode::Internal,
                format!("Stata SPI returned invalid string length {needed}"),
            ));
        }
        let length = needed.checked_add(1).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "string buffer length overflows")
        })?;
        let length_usize = usize::try_from(length).map_err(|_| {
            FeregError::new(ErrorCode::Allocation, "string buffer length exceeds usize")
        })?;
        let mut buffer = Vec::<c_char>::new();
        buffer.try_reserve_exact(length_usize).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve Stata string buffer",
            )
        })?;
        buffer.resize(length_usize, 0);
        // SAFETY: the buffer is writable for `length` bytes; indices are checked
        // and the session remains confined to Stata's calling thread.
        Self::spi_rc(
            unsafe { fereg_spi_read_string(variable, observation, buffer.as_mut_ptr(), length) },
            "string read",
        )?;
        // SAFETY: the bridge guarantees NUL termination within the supplied buffer.
        let bytes = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_bytes();
        let mut output = Vec::new();
        output.try_reserve_exact(bytes.len()).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve copied Stata string",
            )
        })?;
        output.extend_from_slice(bytes);
        Ok(output)
    }

    fn matrix_dimensions(&mut self, name: &str) -> Result<(usize, usize), FeregError> {
        let name = Self::matrix_name(name);
        // SAFETY: name is NUL-terminated and session is thread-confined.
        let rows = unsafe { fereg_spi_matrix_rows(name.as_ptr()) };
        // SAFETY: same argument as above.
        let columns = unsafe { fereg_spi_matrix_cols(name.as_ptr()) };
        let rows = usize::try_from(rows).map_err(|_| {
            FeregError::new(ErrorCode::InvalidArgument, "Stata matrix was not found")
        })?;
        let columns = usize::try_from(columns).map_err(|_| {
            FeregError::new(ErrorCode::InvalidArgument, "Stata matrix was not found")
        })?;
        Ok((rows, columns))
    }

    fn matrix_element(&mut self, name: &str, row: usize, column: usize) -> Result<f64, FeregError> {
        let name = Self::matrix_name(name);
        let row = Self::observation(row)?;
        let column = Self::observation(column)?;
        let mut value = 0.0_f64;
        // SAFETY: output pointer/name are live and indices/session are checked.
        Self::spi_rc(
            unsafe { fereg_spi_mat_el(name.as_ptr(), row, column, &mut value) },
            "matrix read",
        )?;
        Ok(value)
    }

    fn store_numeric(
        &mut self,
        variable: u32,
        observation: usize,
        value: f64,
    ) -> Result<(), FeregError> {
        // The action preflight proved both coordinates representable before the
        // first store. Keep the commit phase free of fallible conversions.
        let variable = variable as c_int;
        let observation = observation as c_int;
        // SAFETY: indices were preflighted and the session remains on Stata's thread.
        Self::store_rc(
            unsafe { fereg_spi_vstore(variable, observation, value) },
            &mut self.numeric_store_error,
        )
    }

    fn store_matrix(
        &mut self,
        name: &PreparedMatrixStoreName,
        row: usize,
        column: usize,
        value: f64,
    ) -> Result<(), FeregError> {
        debug_assert!(row >= 1 && row <= name.rows());
        debug_assert!(column >= 1 && column <= name.columns());
        // The prepared handle proves matrix bounds and SPI representability.
        let row = row as c_int;
        let column = column as c_int;
        let name = name.as_c_str();
        // SAFETY: name/coordinates were preflighted; the session is confined.
        Self::store_rc(
            unsafe { fereg_spi_mat_store(name.as_ptr(), row, column, value) },
            &mut self.matrix_store_error,
        )
    }

    fn check_interrupted(&mut self) -> Result<(), FeregError> {
        // SAFETY: polling and stop inspection occur on the bound Stata thread.
        let poll = unsafe { fereg_spi_poll() };
        // SAFETY: same argument as above.
        let stopped = unsafe { fereg_spi_stop_requested() };
        if poll != 0 || stopped != 0 {
            Err(FeregError::new(
                ErrorCode::Interrupted,
                "Stata requested interruption during OLS input preparation",
            ))
        } else {
            Ok(())
        }
    }
}

impl OutputSink for SpiSink {
    fn display(&self, message: &str) {
        let sanitized = sanitize_c_string(message);
        // SAFETY: `sanitized` is a live NUL-terminated C string for the duration
        // of the call. The C bridge executes this SPI call on Stata's calling
        // thread and does not retain the pointer.
        unsafe {
            let _ = fereg_spi_display(sanitized.as_ptr());
        }
    }

    fn error(&self, message: &str) {
        let sanitized = sanitize_c_string(message);
        // SAFETY: same lifetime and thread argument as `display` above.
        unsafe {
            let _ = fereg_spi_error(sanitized.as_ptr());
        }
    }
}

/// Dispatch one Stata plugin action across the C ABI.
///
/// # Safety
///
/// When `argc` is positive, `argv` must point to an array of at least `argc`
/// non-null pointers to NUL-terminated byte strings. The pointers and strings
/// must remain valid for the duration of the call. Stata must have initialized
/// the C SPI bridge with `pginit` before actions that emit output.
#[no_mangle]
pub unsafe extern "C" fn fereg_dispatch(argc: c_int, argv: *const *const c_char) -> c_int {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the exported ABI validates argc/argv before dereferencing.
        let args = unsafe { read_arguments(argc, argv) }?;
        let sink = SpiSink;
        if args
            .first()
            .is_some_and(|action| action.eq_ignore_ascii_case(ACTION_PREPARE_OLS_TEST))
        {
            let mut session = SpiSession::new()?;
            prepare_ols_test(&args[1..], &sink, &mut session)
        } else if args
            .first()
            .is_some_and(|action| action.eq_ignore_ascii_case(ACTION_ESTIMATE_OLS))
        {
            let mut session = SpiSession::new()?;
            estimate_ols_test(&args[1..], &mut session)
        } else {
            dispatch_args(&args, &sink)
        }
    }));

    match outcome {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            report_error(&error);
            error.return_code()
        }
        Err(payload) => {
            let message = panic_message(payload.as_ref());
            let error = FeregError::new(
                ErrorCode::Panic,
                format!("caught Rust panic at the Stata ABI boundary: {message}"),
            );
            report_error(&error);
            error.return_code()
        }
    }
}

unsafe fn read_arguments(
    argc: c_int,
    argv: *const *const c_char,
) -> Result<Vec<String>, FeregError> {
    if argc < 0 {
        return Err(FeregError::new(
            ErrorCode::Syntax,
            "plugin received a negative argument count",
        ));
    }
    if argc > 0 && argv.is_null() {
        return Err(FeregError::new(
            ErrorCode::Syntax,
            "plugin received a null argv pointer",
        ));
    }
    let mut args = Vec::with_capacity(argc as usize);
    for index in 0..argc as usize {
        // SAFETY: argv is non-null for argc > 0 and the caller promises
        // an array containing argc pointers. Each pointee is validated below.
        let pointer = unsafe { *argv.add(index) };
        if pointer.is_null() {
            return Err(FeregError::new(
                ErrorCode::Syntax,
                format!("plugin argument {index} is null"),
            ));
        }
        // SAFETY: pointer was checked non-null and is supplied by the C ABI
        // as a NUL-terminated command argument.
        let value = unsafe { CStr::from_ptr(pointer) }.to_str().map_err(|_| {
            FeregError::new(
                ErrorCode::Syntax,
                format!("plugin argument {index} is not valid UTF-8"),
            )
        })?;
        args.push(value.to_owned());
    }
    Ok(args)
}

fn report_error(error: &FeregError) {
    if let Some(message) = prepared_store_error_report(error) {
        // Store failures occur after logical commit has begun. Their complete
        // C reports are prepared statically, so unwinding the failed commit
        // does not allocate or attempt another logical output store.
        // SAFETY: message is static, NUL-terminated, and the call is on Stata's
        // bound thread at the ABI boundary.
        unsafe {
            let _ = fereg_spi_error(message.as_ptr());
        }
        return;
    }
    let sink = SpiSink;
    sink.error(&format!("fereg: {error}\n"));
}

fn prepared_store_error_report(error: &FeregError) -> Option<&'static CStr> {
    if error.code == ErrorCode::Internal {
        match error.message.as_str() {
            NUMERIC_STORE_ERROR => Some(NUMERIC_STORE_ERROR_REPORT),
            MATRIX_STORE_ERROR => Some(MATRIX_STORE_ERROR_REPORT),
            _ => None,
        }
    } else {
        None
    }
}

fn sanitize_c_string(message: &str) -> CString {
    let bytes: Vec<u8> = message
        .as_bytes()
        .iter()
        .map(|byte| if *byte == 0 { b'?' } else { *byte })
        .collect();
    CString::new(bytes).expect("interior NUL bytes were replaced")
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn exported_boundary_catches_injected_panic() {
        let args = [
            CString::new("panic-test").expect("cstring"),
            CString::new("confirm=panic-boundary-test").expect("cstring"),
        ];
        let pointers: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();
        // SAFETY: pointer array and C strings are valid for the duration of the call.
        let code = unsafe { fereg_dispatch(pointers.len() as c_int, pointers.as_ptr()) };
        assert_eq!(code, ErrorCode::Panic.stata_return_code());
    }

    #[test]
    fn exported_boundary_rejects_null_argv() {
        // SAFETY: deliberately invalid input is contained and validated by the ABI.
        let code = unsafe { fereg_dispatch(1, std::ptr::null()) };
        assert_eq!(code, ErrorCode::Syntax.stata_return_code());
    }

    #[test]
    fn pinned_estimation_domain_errors_keep_their_ffi_return_codes() {
        for (kind, expected) in [
            (ErrorCode::NegativeWeight, 402),
            (ErrorCode::InvalidFrequencyWeight, 401),
            (ErrorCode::InsufficientObservations, 2001),
            (ErrorCode::InvalidData, 459),
        ] {
            let error = FeregError::new(kind, "injected return-code mapping");
            assert_eq!(error.return_code(), expected);
        }
    }

    #[test]
    fn atomic_store_errors_have_static_preformatted_reports() {
        for (message, expected) in [
            (NUMERIC_STORE_ERROR, NUMERIC_STORE_ERROR_REPORT),
            (MATRIX_STORE_ERROR, MATRIX_STORE_ERROR_REPORT),
        ] {
            let error = FeregError::new(ErrorCode::Internal, message);
            assert_eq!(
                prepared_store_error_report(&error).expect("prepared report"),
                expected,
            );
        }
        let unrelated = FeregError::new(ErrorCode::Internal, "unrelated");
        assert!(prepared_store_error_report(&unrelated).is_none());
    }
}
