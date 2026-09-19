//! Main-thread SPI production with bounded, Rust-owned transpose work.

use std::sync::{Condvar, Mutex, MutexGuard};

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::parallel::ThreadTeam;

use crate::input::SpiApi;
use crate::input_memory::InputMemory;

const TILE_ROWS: usize = 4096;
const MAX_COLUMNS: usize = 64;

struct Tile {
    values: Vec<f64>,
    start: usize,
    rows: usize,
    missing_boundary: f64,
}

struct Slots {
    free: [Option<Vec<f64>>; 2],
    ready: [Option<Tile>; 2],
    closed: bool,
}

struct Handoff {
    slots: Mutex<Slots>,
    changed: Condvar,
}

impl Handoff {
    fn lock(&self) -> MutexGuard<'_, Slots> {
        self.slots.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn wait<'a>(&self, slots: MutexGuard<'a, Slots>) -> MutexGuard<'a, Slots> {
        self.changed.wait(slots).unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

// On error or unwind, wake the peer before the scope joins it. A consumer
// drains already published tiles; their destinations remain action-private.
struct CloseOnDrop<'a>(&'a Handoff);

impl Drop for CloseOnDrop<'_> {
    fn drop(&mut self) {
        self.0.lock().closed = true;
        self.0.changed.notify_all();
    }
}

/// Return false before reading when the optional path does not fit. Worker
/// scratch is admitted before reservation; no SPI object crosses the scope.
pub(crate) fn try_read(
    spi: &mut dyn SpiApi,
    positions: &[u32],
    columns: &mut [&mut [f64]],
    missing: &mut [&mut [bool]],
    team: ThreadTeam,
    memory: &mut InputMemory,
) -> Result<bool> {
    if !spi.supports_numeric_rows() || team.threads() <= 1
        || !(16..=MAX_COLUMNS).contains(&positions.len())
        || columns.is_empty() || columns[0].len() < 16_384
    {
        return Ok(false);
    }
    let width = positions.len();
    let rows = columns[0].len();
    if columns.len() != width || missing.len() != width
        || columns.iter().zip(missing.iter()).any(|(values, flags)| values.len() != rows || flags.len() != rows)
    {
        return Err(FeregError::new(ErrorCode::InvalidArgument, "numeric pipeline has an invalid shape"));
    }
    let elements = TILE_ROWS * width;
    let bytes = 2 * elements * std::mem::size_of::<f64>() + std::mem::size_of::<Handoff>();
    if bytes > memory.remaining() {
        return Ok(false);
    }
    memory.admit(bytes as u128)?;
    let mut buffers = [Vec::new(), Vec::new()];
    for buffer in &mut buffers {
        buffer.try_reserve_exact(elements).map_err(|_| {
            FeregError::new(ErrorCode::Allocation, "unable to reserve numeric input tile")
        })?;
    }
    let actual = buffers.iter().map(|buffer| buffer.capacity() as u128 * 8).sum::<u128>()
        + std::mem::size_of::<Handoff>() as u128;
    memory.admit(actual)?;
    for buffer in &mut buffers {
        buffer.resize(elements, 0.0);
    }
    let [first, second] = buffers;
    let handoff = Handoff {
        slots: Mutex::new(Slots { free: [Some(first), Some(second)], ready: [None, None], closed: false }),
        changed: Condvar::new(),
    };
    team.in_place_scope(|scope| -> Result<()> {
        scope.spawn(|_| {
            let _close = CloseOnDrop(&handoff);
            let mut index = 0;
            loop {
                let tile = {
                    let mut slots = handoff.lock();
                    loop {
                        if let Some(tile) = slots.ready[index].take() {
                            break tile;
                        }
                        if slots.closed { return; }
                        slots = handoff.wait(slots);
                    }
                };
                // Column-major destination writes stay contiguous. Missing
                // classification uses the very same runtime SPI boundary.
                for (column, (destination, missing_flags)) in columns.iter_mut().zip(missing.iter_mut()).enumerate() {
                    for offset in 0..tile.rows {
                        let value = tile.values[offset * width + column];
                        destination[tile.start + offset] = value;
                        missing_flags[tile.start + offset] = value >= tile.missing_boundary;
                    }
                }
                handoff.lock().free[index] = Some(tile.values);
                handoff.changed.notify_all();
                index ^= 1;
            }
        });
        let _close = CloseOnDrop(&handoff);
        for (sequence, start) in (0..rows).step_by(TILE_ROWS).enumerate() {
            let index = sequence % 2;
            let mut buffer = {
                let mut slots = handoff.lock();
                loop {
                    if slots.closed {
                        return Err(FeregError::new(ErrorCode::Internal, "numeric input worker stopped"));
                    }
                    if let Some(buffer) = slots.free[index].take() { break buffer; }
                    slots = handoff.wait(slots);
                }
            };
            spi.check_interrupted()?;
            let count = (rows - start).min(TILE_ROWS);
            let boundary = spi.read_numeric_rows(positions, start + 1, &mut buffer[..count * width])?;
            if !boundary.is_finite() || boundary <= 0.0 {
                return Err(FeregError::new(ErrorCode::Internal, "Stata SPI returned an invalid numeric missing boundary"));
            }
            handoff.lock().ready[index] = Some(Tile { values: buffer, start, rows: count, missing_boundary: boundary });
            handoff.changed.notify_all();
        }
        Ok(())
    })??;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::PreparedMatrixStoreName;
    use fereg_core::ThreadConfig;

    struct Input {
        caller: std::thread::ThreadId,
        calls: usize,
        failure: usize,
        panic: bool,
        boundary: f64,
    }

    impl Input {
        fn new() -> Self {
            Self { caller: std::thread::current().id(), calls: 0, failure: usize::MAX,
                panic: false, boundary: 1e100 }
        }
    }

    impl SpiApi for Input {
        fn supports_numeric_rows(&self) -> bool { true }
        fn read_numeric_rows(&mut self, variables: &[u32], first: usize, values: &mut [f64]) -> Result<f64> {
            assert_eq!(self.caller, std::thread::current().id());
            self.calls += 1;
            if self.calls == self.failure {
                assert!(!self.panic, "injected producer panic");
                return Err(FeregError::new(ErrorCode::Interrupted, "injected read failure"));
            }
            for (row, chunk) in values.chunks_exact_mut(variables.len()).enumerate() {
                for (column, value) in chunk.iter_mut().enumerate() {
                    *value = if (first + row) % 197 == 0 { self.boundary }
                        else { (first + row) as f64 + f64::from(variables[column]) * 100000.0 };
                }
            }
            Ok(self.boundary)
        }
        fn check_interrupted(&mut self) -> Result<()> {
            assert_eq!(self.caller, std::thread::current().id()); Ok(())
        }
        fn nobs(&mut self) -> Result<usize> { unreachable!() }
        fn nvars(&mut self) -> Result<usize> { unreachable!() }
        fn variable_is_string(&mut self, _: u32) -> Result<bool> { unreachable!() }
        fn variable_is_strl(&mut self, _: u32) -> Result<bool> { unreachable!() }
        fn read_numeric(&mut self, _: u32, _: usize) -> Result<(f64, bool)> { unreachable!() }
        fn read_string(&mut self, _: u32, _: usize) -> Result<Vec<u8>> { unreachable!() }
        fn matrix_dimensions(&mut self, _: &str) -> Result<(usize, usize)> { unreachable!() }
        fn matrix_element(&mut self, _: &str, _: usize, _: usize) -> Result<f64> { unreachable!() }
        fn store_numeric(&mut self, _: u32, _: usize, _: f64) -> Result<()> { unreachable!() }
        fn store_matrix(&mut self, _: &PreparedMatrixStoreName, _: usize, _: usize, _: f64) -> Result<()> { unreachable!() }
        fn display(&mut self, _: &str) -> Result<()> { unreachable!() }
    }

    fn run(input: &mut Input, rows: usize, width: usize, threads: usize, memory: &mut InputMemory) -> Result<bool> {
        let positions: Vec<u32> = (1..=width as u32).rev().collect();
        let mut values = vec![vec![0.0; rows]; width];
        let mut flags = vec![vec![false; rows]; width];
        let mut columns: Vec<_> = values.iter_mut().map(Vec::as_mut_slice).collect();
        let mut missing: Vec<_> = flags.iter_mut().map(Vec::as_mut_slice).collect();
        let read = try_read(input, &positions, &mut columns, &mut missing,
            ThreadTeam::new(ThreadConfig::from_requested(threads, false).unwrap()), memory)?;
        if read {
            for column in 0..width {
                for row in 0..rows {
                    let expected_missing = (row + 1) % 197 == 0;
                    assert_eq!(flags[column][row], expected_missing);
                    assert_eq!(values[column][row], if expected_missing { input.boundary }
                        else { (row + 1) as f64 + f64::from(positions[column]) * 100000.0 });
                }
            }
        }
        Ok(read)
    }

    #[test]
    fn numeric_pipeline_transposes_tail_and_preserves_missing_on_caller_thread() {
        for width in [16, 52, 64] {
            let mut input = Input::new();
            let mut memory = InputMemory::new(None, 1234).unwrap();
            assert!(run(&mut input, 16387, width, 4, &mut memory).unwrap());
            assert_eq!(input.calls, 5);
            assert!(memory.peak() >= 1234 + 2 * 4096 * width as u128 * 8);
        }
    }

    #[test]
    fn numeric_pipeline_declines_before_reads_and_accounts_scratch() {
        for (rows, width, threads, usable) in [
            (16383, 52, 4, None), (16384, 15, 4, None), (16384, 65, 4, None),
            (16384, 52, 1, None), (16384, 52, 4, Some(1000)),
        ] {
            let mut input = Input::new();
            let mut memory = InputMemory::new(usable, 0).unwrap();
            assert!(!run(&mut input, rows, width, threads, &mut memory).unwrap());
            assert_eq!(input.calls, 0);
            assert_eq!(memory.peak(), 0);
        }
    }

    #[test]
    fn numeric_pipeline_joins_after_read_failure_invalid_boundary_and_producer_panic() {
        for failure in 1..=5 {
            for panic in [false, true] {
                let mut input = Input::new();
                input.failure = failure;
                input.panic = panic;
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(&mut input, 16387, 16, 2, &mut InputMemory::new(None, 0).unwrap())
                }));
                if panic { assert!(outcome.is_err()); }
                else { assert_eq!(outcome.unwrap().unwrap_err().code, ErrorCode::Interrupted); }
            }
        }
        for boundary in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            let mut input = Input::new();
            input.boundary = boundary;
            assert!(run(&mut input, 16387, 16, 2, &mut InputMemory::new(None, 0).unwrap()).is_err());
        }
    }

    #[test]
    fn numeric_pipeline_worker_panic_wakes_producer_before_scope_join() {
        let team = ThreadTeam::new(ThreadConfig::from_requested(2, false).unwrap());
        let outcome = std::panic::catch_unwind(|| {
            let handoff = Handoff { slots: Mutex::new(Slots { free: [None, None], ready: [None, None], closed: false }), changed: Condvar::new() };
            team.in_place_scope(|scope| {
                scope.spawn(|_| { let _close = CloseOnDrop(&handoff); panic!("injected worker panic"); });
                let mut slots = handoff.lock();
                while !slots.closed { slots = handoff.wait(slots); }
            }).unwrap();
        });
        assert!(outcome.is_err());
    }
}
