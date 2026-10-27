use crate::options::ThreadConfig;
use crate::protocol::{
    ACTION_PROTOCOL_VERSION, ADO_VERSION, MINIMUM_STATA_MAJOR, MODEL_WIRE_VERSION,
    OLS_RESULT_WIRE_VERSION, PLUGIN_VERSION, PPML_REQUEST_WIRE_VERSION, PPML_RESULT_WIRE_VERSION,
    SPI_MAJOR, SPI_MINOR,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

pub(crate) mod phase_ledger;

thread_local! {
    static SILENCED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Suppress ordinary diagnostic formatting while measuring a projection's
/// economic intervals. The caller emits its bounded labels explicitly. This
/// guard belongs to the executing thread and restores nesting on every exit.
pub(crate) struct Silence {
    enabled: bool,
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Silence {
    pub(crate) fn new(enabled: bool) -> Self {
        if enabled {
            SILENCED.set(SILENCED.get() + 1);
        }
        Self {
            enabled,
            _thread: std::marker::PhantomData,
        }
    }
}

impl Drop for Silence {
    fn drop(&mut self) {
        if self.enabled {
            SILENCED.set(SILENCED.get() - 1);
        }
    }
}

static SOLVER_TRACE_FILE_LOCK: Mutex<()> = Mutex::new(());

/// Developer solver tracing (`FEREG_SOLVER_TRACE`), read at each use.
#[must_use]
pub fn solver_trace_enabled() -> bool {
    std::env::var_os("FEREG_SOLVER_TRACE").is_some() && SILENCED.get() == 0
}

/// Developer phase timing (`FEREG_PHASE_TIMING`), read at each use.
#[must_use]
pub fn phase_timing_enabled() -> bool {
    std::env::var_os("FEREG_PHASE_TIMING").is_some() && SILENCED.get() == 0
}

/// Write one solver-trace line to standard error and, when
/// `FEREG_SOLVER_TRACE_FILE` names a file, append it there as well.
pub(crate) fn emit_solver_trace(line: &str) {
    eprintln!("{line}");
    let Some(path) = std::env::var_os("FEREG_SOLVER_TRACE_FILE") else {
        return;
    };
    let result = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| write_solver_trace_record(&mut file, line));
    if let Err(error) = result {
        eprintln!("FEREG_SOLVER_TRACE_IO_ERROR|{error}");
    }
}

fn write_solver_trace_record(writer: &mut impl Write, line: &str) -> std::io::Result<()> {
    // O_APPEND orders individual writes, not a formatted record or short-write
    // retries. All in-process trace producers share this record-level lock.
    let _guard = SOLVER_TRACE_FILE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub ado_version: &'static str,
    pub plugin_version: &'static str,
    pub action_protocol_major: u16,
    pub action_protocol_minor: u16,
    pub model_wire_major: u16,
    pub model_wire_minor: u16,
    pub ols_result_wire_major: u16,
    pub ols_result_wire_minor: u16,
    pub git_commit: &'static str,
    pub build_date: &'static str,
    pub operating_system: &'static str,
    pub architecture: &'static str,
    pub plugin_abi_major: u16,
    pub plugin_abi_minor: u16,
    pub cpu_dispatch_path: &'static str,
    pub available_threads: usize,
}

impl BuildInfo {
    #[must_use]
    pub fn current() -> Self {
        Self {
            ado_version: ADO_VERSION,
            plugin_version: PLUGIN_VERSION,
            action_protocol_major: ACTION_PROTOCOL_VERSION.major,
            action_protocol_minor: ACTION_PROTOCOL_VERSION.minor,
            model_wire_major: MODEL_WIRE_VERSION.major,
            model_wire_minor: MODEL_WIRE_VERSION.minor,
            ols_result_wire_major: OLS_RESULT_WIRE_VERSION.major,
            ols_result_wire_minor: OLS_RESULT_WIRE_VERSION.minor,
            git_commit: env!("FEREG_GIT_COMMIT"),
            build_date: env!("FEREG_BUILD_DATE"),
            operating_system: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            plugin_abi_major: SPI_MAJOR,
            plugin_abi_minor: SPI_MINOR,
            cpu_dispatch_path: crate::kernels::active_name(),
            available_threads: ThreadConfig::available(),
        }
    }

    /// Plugin identity is available even when a subsequent installation check fails.
    #[must_use]
    pub fn render(&self, verbose: bool) -> String {
        let mut output = format!("fereg.plugin_version={}\n", self.plugin_version);
        if verbose {
            output.push_str(&format!(
                concat!(
                    "fereg.ado_contract={}\n",
                    "fereg.action_protocol={}.{}\n",
                    "fereg.model_wire={}.{}\n",
                    "fereg.ols_result_wire={}.{}\n",
                    "fereg.ppml_request_wire={}.{}\n",
                    "fereg.ppml_result_wire={}.{}\n",
                    "fereg.plugin_abi={}.{}\n",
                    "fereg.git_commit={}\n",
                    "fereg.build_date={}\n",
                    "fereg.os={}\n",
                    "fereg.arch={}\n",
                    "fereg.cpu_dispatch={}\n",
                    "fereg.available_threads={}\n",
                    "fereg.minimum_stata={}\n",
                    "fereg.dependency_audit=build-time-required\n"
                ),
                self.ado_version,
                self.action_protocol_major,
                self.action_protocol_minor,
                self.model_wire_major,
                self.model_wire_minor,
                self.ols_result_wire_major,
                self.ols_result_wire_minor,
                PPML_REQUEST_WIRE_VERSION.major,
                PPML_REQUEST_WIRE_VERSION.minor,
                PPML_RESULT_WIRE_VERSION.major,
                PPML_RESULT_WIRE_VERSION.minor,
                self.plugin_abi_major,
                self.plugin_abi_minor,
                self.git_commit,
                self.build_date,
                self.operating_system,
                self.architecture,
                self.cpu_dispatch_path,
                self.available_threads,
                MINIMUM_STATA_MAJOR,
            ));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};

    #[test]
    fn scoped_silence_restores_nesting_after_failure() {
        assert_eq!(SILENCED.get(), 0);
        let outer = Silence::new(true);
        assert!(std::panic::catch_unwind(|| {
            let _inner = Silence::new(true);
            let _disabled = Silence::new(false);
            assert_eq!(SILENCED.get(), 2);
            panic!("test scope failure");
        })
        .is_err());
        assert_eq!(SILENCED.get(), 1);
        drop(outer);
        assert_eq!(SILENCED.get(), 0);
    }

    struct FragmentedWriter {
        bytes: Arc<Mutex<Vec<u8>>>,
        first_write: Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>,
    }

    impl Write for FragmentedWriter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            // Every fragment, including the final newline, must hold the same
            // lock. This assertion makes the regression independent of luck.
            assert!(SOLVER_TRACE_FILE_LOCK.try_lock().is_err());
            self.bytes.lock().unwrap().push(data[0]);
            if let Some((entered, release)) = self.first_write.take() {
                entered.send(()).unwrap();
                release.recv().unwrap();
            }
            Ok(1)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn concurrent_trace_records_remain_whole_across_short_writes() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let first_bytes = bytes.clone();
        let first = std::thread::spawn(move || {
            write_solver_trace_record(
                &mut FragmentedWriter {
                    bytes: first_bytes,
                    first_write: Some((entered_tx, release_rx)),
                },
                "first-ready-record",
            )
            .unwrap();
        });
        entered_rx.recv().unwrap();
        let (started_tx, started_rx) = mpsc::sync_channel(0);
        let second_bytes = bytes.clone();
        let second = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            write_solver_trace_record(
                &mut FragmentedWriter {
                    bytes: second_bytes,
                    first_write: None,
                },
                "second-ready-record",
            )
            .unwrap();
        });
        started_rx.recv().unwrap();
        assert_eq!(&*bytes.lock().unwrap(), b"f");
        release_tx.send(()).unwrap();
        first.join().unwrap();
        second.join().unwrap();
        assert_eq!(
            &*bytes.lock().unwrap(),
            b"first-ready-record\nsecond-ready-record\n"
        );
    }
}
