//! Main-thread SPI production with bounded, Rust-owned transpose work.

use std::sync::{Condvar, Mutex, MutexGuard};

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::parallel::ThreadTeam;

use crate::input::SpiApi;
use crate::input_memory::InputMemory;

const TILE_ROWS: usize = 4096;
const MAX_COLUMNS: usize = 64;

#[derive(Debug, Clone, Copy)]
pub(crate) struct NumericRhsLayout {
    pub width: usize,
    pub append_intercept: bool,
}

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
        self.slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn wait<'a>(&self, slots: MutexGuard<'a, Slots>) -> MutexGuard<'a, Slots> {
        self.changed
            .wait(slots)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
#[cfg(test)]
pub(crate) fn try_read(
    spi: &mut dyn SpiApi,
    positions: &[u32],
    columns: &mut [&mut [f64]],
    missing: &mut [&mut [bool]],
    team: ThreadTeam,
    memory: &mut InputMemory,
) -> Result<bool> {
    try_read_with_rhs(spi, positions, columns, missing, team, memory, None)
}

pub(crate) fn try_read_with_rhs(
    spi: &mut dyn SpiApi,
    positions: &[u32],
    columns: &mut [&mut [f64]],
    missing: &mut [&mut [bool]],
    team: ThreadTeam,
    memory: &mut InputMemory,
    mut rhs: Option<(&mut Vec<f64>, NumericRhsLayout)>,
) -> Result<bool> {
    if !spi.supports_numeric_rows()
        || !(1..=MAX_COLUMNS).contains(&positions.len())
        || columns.is_empty()
        || fereg_core::parallel::StagePlan::new(
            columns[0].len() as u128 * positions.len() as u128,
            2,
            team.threads(),
        )
        .lanes
            <= 1
    {
        return Ok(false);
    }
    let width = positions.len();
    let rows = columns[0].len();
    if columns.len() != width
        || missing.len() != width
        || columns
            .iter()
            .zip(missing.iter())
            .any(|(values, flags)| values.len() != rows || flags.len() != rows)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "numeric pipeline has an invalid shape",
        ));
    }
    if rhs.as_ref().is_some_and(|(buffer, layout)| {
        layout.width < 2
            || layout.width > width
            || !buffer.is_empty()
            || rows
                .checked_mul(layout.width)
                .is_none_or(|n| buffer.capacity() < n)
    }) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "numeric RHS reservation has an invalid shape",
        ));
    }
    read_tiles(
        spi,
        positions,
        rows,
        team,
        memory,
        |start, count, values, boundary| {
            // Column-major destination writes stay contiguous. Missing
            // classification uses the very same runtime SPI boundary.
            for (column, (destination, missing_flags)) in
                columns.iter_mut().zip(missing.iter_mut()).enumerate()
            {
                for offset in 0..count {
                    let value = values[offset * width + column];
                    destination[start + offset] = value;
                    missing_flags[start + offset] = value >= boundary;
                }
            }
            if let Some((rhs, layout)) = rhs.as_mut() {
                append_rhs(rhs, *layout, width, count, values);
            }
        },
    )
}

/// Fill reserved empty columns directly from owned tiles. Each push initializes
/// one element; the preflight guarantees capacity for the complete input.
// Keep admitted buffers separate from the main-thread SPI handle.
#[allow(clippy::too_many_arguments)]
pub(crate) fn try_read_reserved(
    spi: &mut dyn SpiApi,
    positions: &[u32],
    rows: usize,
    columns: &mut [&mut Vec<f64>],
    missing: &mut [&mut Vec<bool>],
    team: ThreadTeam,
    memory: &mut InputMemory,
    mut rhs: Option<(&mut Vec<f64>, NumericRhsLayout)>,
) -> Result<bool> {
    let width = positions.len();
    if !spi.supports_numeric_rows()
        || !(1..=MAX_COLUMNS).contains(&width)
        || fereg_core::parallel::StagePlan::new(rows as u128 * width as u128, 2, team.threads())
            .lanes
            <= 1
    {
        return Ok(false);
    }
    if columns.len() != width
        || missing.len() != width
        || columns.iter().zip(missing.iter()).any(|(v, m)| {
            !v.is_empty() || !m.is_empty() || v.capacity() < rows || m.capacity() < rows
        })
        || rhs.as_ref().is_some_and(|(v, layout)| {
            layout.width < 2
                || layout.width > width
                || !v.is_empty()
                || rows
                    .checked_mul(layout.width)
                    .is_none_or(|n| v.capacity() < n)
        })
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "reserved numeric pipeline has an invalid shape",
        ));
    }
    read_tiles(
        spi,
        positions,
        rows,
        team,
        memory,
        |start, count, values, boundary| {
            for (column, (destination, flags)) in
                columns.iter_mut().zip(missing.iter_mut()).enumerate()
            {
                debug_assert_eq!(destination.len(), start);
                debug_assert_eq!(flags.len(), start);
                for offset in 0..count {
                    let value = values[offset * width + column];
                    destination.push(value);
                    flags.push(value >= boundary);
                }
            }
            if let Some((rhs, layout)) = rhs.as_mut() {
                append_rhs(rhs, *layout, width, count, values);
            }
        },
    )
}

// The admitted recipe is outcome, ordered continuous sources and an optional
// constant. In particular, a public slope-only model has no constant column.
// FE and extra cluster sources have already reached their authoritative columns.
// Compact within the owned tile, requiring no additional row-scale allocation.
fn append_rhs(
    rhs: &mut Vec<f64>,
    layout: NumericRhsLayout,
    width: usize,
    count: usize,
    values: &mut [f64],
) {
    let rhs_width = layout.width;
    let copied = rhs_width - usize::from(layout.append_intercept);
    for row in 0..count {
        if rhs_width < width {
            values.copy_within(row * width..row * width + copied, row * rhs_width);
        }
        if layout.append_intercept {
            values[(row + 1) * rhs_width - 1] = 1.0;
        }
    }
    rhs.extend_from_slice(&values[..count * rhs_width]);
}

fn read_tiles(
    spi: &mut dyn SpiApi,
    positions: &[u32],
    rows: usize,
    team: ThreadTeam,
    memory: &mut InputMemory,
    mut consume: impl FnMut(usize, usize, &mut [f64], f64) + Send,
) -> Result<bool> {
    let width = positions.len();
    let elements = TILE_ROWS * width;
    let bytes = 2 * elements * std::mem::size_of::<f64>() + std::mem::size_of::<Handoff>();
    if bytes > memory.remaining() {
        return Ok(false);
    }
    memory.admit(bytes as u128)?;
    let mut buffers = [Vec::new(), Vec::new()];
    for buffer in &mut buffers {
        buffer.try_reserve_exact(elements).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve numeric input tile",
            )
        })?;
    }
    let actual = buffers
        .iter()
        .map(|buffer| buffer.capacity() as u128 * 8)
        .sum::<u128>()
        + std::mem::size_of::<Handoff>() as u128;
    memory.admit(actual)?;
    for buffer in &mut buffers {
        buffer.resize(elements, 0.0);
    }
    let [first, second] = buffers;
    let handoff = Handoff {
        slots: Mutex::new(Slots {
            free: [Some(first), Some(second)],
            ready: [None, None],
            closed: false,
        }),
        changed: Condvar::new(),
    };
    team.in_place_scope(|scope| -> Result<()> {
        scope.spawn(|_| {
            let _close = CloseOnDrop(&handoff);
            let mut index = 0;
            loop {
                let mut tile = {
                    let mut slots = handoff.lock();
                    loop {
                        if let Some(tile) = slots.ready[index].take() {
                            break tile;
                        }
                        if slots.closed {
                            return;
                        }
                        slots = handoff.wait(slots);
                    }
                };
                consume(
                    tile.start,
                    tile.rows,
                    &mut tile.values,
                    tile.missing_boundary,
                );
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
                        return Err(FeregError::new(
                            ErrorCode::Internal,
                            "numeric input worker stopped",
                        ));
                    }
                    if let Some(buffer) = slots.free[index].take() {
                        break buffer;
                    }
                    slots = handoff.wait(slots);
                }
            };
            spi.check_interrupted()?;
            let count = (rows - start).min(TILE_ROWS);
            let boundary =
                spi.read_numeric_rows(positions, start + 1, &mut buffer[..count * width])?;
            if !boundary.is_finite() || boundary <= 0.0 {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "Stata SPI returned an invalid numeric missing boundary",
                ));
            }
            handoff.lock().ready[index] = Some(Tile {
                values: buffer,
                start,
                rows: count,
                missing_boundary: boundary,
            });
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
            Self {
                caller: std::thread::current().id(),
                calls: 0,
                failure: usize::MAX,
                panic: false,
                boundary: 1e100,
            }
        }
    }

    impl SpiApi for Input {
        fn supports_numeric_rows(&self) -> bool {
            true
        }
        fn read_numeric_rows(
            &mut self,
            variables: &[u32],
            first: usize,
            values: &mut [f64],
        ) -> Result<f64> {
            assert_eq!(self.caller, std::thread::current().id());
            self.calls += 1;
            if self.calls == self.failure {
                assert!(!self.panic, "injected producer panic");
                return Err(FeregError::new(
                    ErrorCode::Interrupted,
                    "injected read failure",
                ));
            }
            for (row, chunk) in values.chunks_exact_mut(variables.len()).enumerate() {
                for (column, value) in chunk.iter_mut().enumerate() {
                    *value = if (first + row) % 197 == 0 {
                        self.boundary
                    } else {
                        (first + row) as f64 + f64::from(variables[column]) * 100000.0
                    };
                }
            }
            Ok(self.boundary)
        }
        fn check_interrupted(&mut self) -> Result<()> {
            assert_eq!(self.caller, std::thread::current().id());
            Ok(())
        }
        fn nobs(&mut self) -> Result<usize> {
            unreachable!()
        }
        fn nvars(&mut self) -> Result<usize> {
            unreachable!()
        }
        fn variable_is_string(&mut self, _: u32) -> Result<bool> {
            unreachable!()
        }
        fn variable_is_strl(&mut self, _: u32) -> Result<bool> {
            unreachable!()
        }
        fn read_numeric(&mut self, _: u32, _: usize) -> Result<(f64, bool)> {
            unreachable!()
        }
        fn read_string(&mut self, _: u32, _: usize) -> Result<Vec<u8>> {
            unreachable!()
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
            _: &PreparedMatrixStoreName,
            _: usize,
            _: usize,
            _: f64,
        ) -> Result<()> {
            unreachable!()
        }
        fn display(&mut self, _: &str) -> Result<()> {
            unreachable!()
        }
    }

    fn run(
        input: &mut Input,
        rows: usize,
        width: usize,
        threads: usize,
        memory: &mut InputMemory,
    ) -> Result<bool> {
        let positions: Vec<u32> = (1..=width as u32).rev().collect();
        let mut values = vec![vec![0.0; rows]; width];
        let mut flags = vec![vec![false; rows]; width];
        let mut columns: Vec<_> = values.iter_mut().map(Vec::as_mut_slice).collect();
        let mut missing: Vec<_> = flags.iter_mut().map(Vec::as_mut_slice).collect();
        let read = try_read(
            input,
            &positions,
            &mut columns,
            &mut missing,
            ThreadTeam::new(ThreadConfig::from_requested(threads, false).unwrap()),
            memory,
        )?;
        if read {
            for column in 0..width {
                for row in 0..rows {
                    let expected_missing = (row + 1) % 197 == 0;
                    assert_eq!(flags[column][row], expected_missing);
                    assert_eq!(
                        values[column][row],
                        if expected_missing {
                            input.boundary
                        } else {
                            (row + 1) as f64 + f64::from(positions[column]) * 100000.0
                        }
                    );
                }
            }
        }
        Ok(read)
    }

    #[test]
    fn numeric_pipeline_constructs_owned_rhs_without_changing_fe_or_missing() {
        for append_intercept in [false, true] {
            for (width, threads) in [
                (3, 4),
                (4, 4),
                (6, 4),
                (8, 8),
                (9, 4),
                (12, 4),
                (52, 4),
                (64, 4),
            ] {
                for extra_sources in [0, 1, 2, 5] {
                    if extra_sources + 2 > width {
                        continue;
                    }
                    let rhs_width = width - extra_sources;
                    let rows = 16387;
                    let positions: Vec<u32> = (1..=width as u32).rev().collect();
                    let mut values = vec![vec![0.0; rows]; width];
                    let mut flags = vec![vec![false; rows]; width];
                    let mut columns: Vec<_> = values.iter_mut().map(Vec::as_mut_slice).collect();
                    let mut missing: Vec<_> = flags.iter_mut().map(Vec::as_mut_slice).collect();
                    let mut rhs = Vec::with_capacity(rows * rhs_width);
                    let capacity = rhs.capacity();
                    let mut input = Input::new();
                    assert!(try_read_with_rhs(
                        &mut input,
                        &positions,
                        &mut columns,
                        &mut missing,
                        ThreadTeam::new(ThreadConfig::from_requested(threads, false).unwrap()),
                        &mut InputMemory::new(None, (capacity * 8) as u128).unwrap(),
                        Some((
                            &mut rhs,
                            NumericRhsLayout {
                                width: rhs_width,
                                append_intercept
                            }
                        ))
                    )
                    .unwrap());
                    assert_eq!(rhs.capacity(), capacity);
                    assert_eq!(rhs.len(), rows * rhs_width);
                    for row in 0..rows {
                        for column in 0..width {
                            let is_missing = (row + 1) % 197 == 0;
                            let expected = if is_missing {
                                input.boundary
                            } else {
                                (row + 1) as f64 + f64::from(positions[column]) * 100000.0
                            };
                            assert_eq!(values[column][row], expected);
                            assert_eq!(flags[column][row], is_missing);
                            if column < rhs_width {
                                assert_eq!(
                                    rhs[row * rhs_width + column],
                                    if append_intercept && column == rhs_width - 1 {
                                        1.0
                                    } else {
                                        expected
                                    }
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn reserved_pipeline_preserves_values_capacities_and_failure_shutdown() {
        for append_intercept in [false, true] {
            for (width, threads) in [
                (3, 4),
                (4, 4),
                (6, 4),
                (8, 8),
                (9, 4),
                (16, 4),
                (52, 4),
                (64, 4),
            ] {
                for failure in [usize::MAX, 1, 3, 5] {
                    let rows = 16387;
                    let positions: Vec<u32> = (1..=width as u32).rev().collect();
                    let mut values: Vec<Vec<f64>> =
                        (0..width).map(|_| Vec::with_capacity(rows)).collect();
                    let mut flags: Vec<Vec<bool>> =
                        (0..width).map(|_| Vec::with_capacity(rows)).collect();
                    let pointers: Vec<_> = values.iter().map(Vec::as_ptr).collect();
                    let mut rhs = Vec::with_capacity(rows * width);
                    let mut input = Input::new();
                    input.failure = failure;
                    let result = try_read_reserved(
                        &mut input,
                        &positions,
                        rows,
                        &mut values.iter_mut().collect::<Vec<_>>(),
                        &mut flags.iter_mut().collect::<Vec<_>>(),
                        ThreadTeam::new(ThreadConfig::from_requested(threads, false).unwrap()),
                        &mut InputMemory::new(None, 0).unwrap(),
                        Some((
                            &mut rhs,
                            NumericRhsLayout {
                                width,
                                append_intercept,
                            },
                        )),
                    );
                    if failure == usize::MAX {
                        assert_eq!(result, Ok(true));
                    } else {
                        assert_eq!(result.unwrap_err().code, ErrorCode::Interrupted);
                    }
                    let initialized = rows.min(failure.saturating_sub(1).saturating_mul(TILE_ROWS));
                    assert_eq!(rhs.len(), initialized * width);
                    for column in 0..width {
                        assert_eq!(pointers[column], values[column].as_ptr());
                        assert_eq!(values[column].len(), initialized);
                        assert_eq!(flags[column].len(), initialized);
                        for row in 0..initialized {
                            let missing = (row + 1) % 197 == 0;
                            let expected = if missing {
                                input.boundary
                            } else {
                                (row + 1) as f64 + f64::from(positions[column]) * 100000.0
                            };
                            assert_eq!(values[column][row], expected);
                            assert_eq!(flags[column][row], missing);
                            assert_eq!(
                                rhs[row * width + column],
                                if append_intercept && column == width - 1 {
                                    1.0
                                } else {
                                    expected
                                }
                            );
                        }
                    }
                }
            }
        }
        let mut input = Input::new();
        let mut values = (0..16)
            .map(|_| Vec::with_capacity(16384))
            .collect::<Vec<_>>();
        let mut flags = (0..16)
            .map(|_| Vec::with_capacity(16384))
            .collect::<Vec<_>>();
        // Clone of an empty Vec need not preserve capacity, so explicitly reserve.
        for v in &mut values {
            v.reserve_exact(16384);
        }
        for v in &mut flags {
            v.reserve_exact(16384);
        }
        assert!(!try_read_reserved(
            &mut input,
            &(1..=16).collect::<Vec<_>>(),
            16384,
            &mut values.iter_mut().collect::<Vec<_>>(),
            &mut flags.iter_mut().collect::<Vec<_>>(),
            ThreadTeam::new(ThreadConfig::from_requested(4, false).unwrap()),
            &mut InputMemory::new(Some(1000), 0).unwrap(),
            None
        )
        .unwrap());
        assert_eq!(input.calls, 0);
        assert!(values.iter().all(Vec::is_empty));
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
            (256, 52, 4, None),
            (8192, 1, 4, None),
            (16384, 65, 4, None),
            (16384, 52, 1, None),
            (16384, 52, 4, Some(1000)),
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
                    run(
                        &mut input,
                        16387,
                        16,
                        2,
                        &mut InputMemory::new(None, 0).unwrap(),
                    )
                }));
                if panic {
                    assert!(outcome.is_err());
                } else {
                    assert_eq!(outcome.unwrap().unwrap_err().code, ErrorCode::Interrupted);
                }
            }
        }
        for boundary in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            let mut input = Input::new();
            input.boundary = boundary;
            assert!(run(
                &mut input,
                16387,
                16,
                2,
                &mut InputMemory::new(None, 0).unwrap()
            )
            .is_err());
        }
    }

    #[test]
    fn numeric_pipeline_worker_panic_wakes_producer_before_scope_join() {
        let team = ThreadTeam::new(ThreadConfig::from_requested(2, false).unwrap());
        let outcome = std::panic::catch_unwind(|| {
            let handoff = Handoff {
                slots: Mutex::new(Slots {
                    free: [None, None],
                    ready: [None, None],
                    closed: false,
                }),
                changed: Condvar::new(),
            };
            team.in_place_scope(|scope| {
                scope.spawn(|_| {
                    let _close = CloseOnDrop(&handoff);
                    panic!("injected worker panic");
                });
                let mut slots = handoff.lock();
                while !slots.closed {
                    slots = handoff.wait(slots);
                }
            })
            .unwrap();
        });
        assert!(outcome.is_err());
    }
}
