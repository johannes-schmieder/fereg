//! Main-thread SPI production with bounded, Rust-owned transpose work.

use std::sync::{Condvar, Mutex, MutexGuard};

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::ols::{OlsRequestV1, PreparedOlsLoadHint};
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

struct ReservedColumns<'a> {
    positions: Vec<u32>,
    values: Vec<&'a mut Vec<f64>>,
    missing: Vec<&'a mut Vec<bool>>,
}

impl<'a> ReservedColumns<'a> {
    fn new(width: usize) -> Option<Self> {
        let mut result = Self {
            positions: Vec::new(),
            values: Vec::new(),
            missing: Vec::new(),
        };
        result.positions.try_reserve_exact(width).ok()?;
        result.values.try_reserve_exact(width).ok()?;
        result.missing.try_reserve_exact(width).ok()?;
        Some(result)
    }

    fn partition(
        request: &OlsRequestV1,
        positions: &[u32],
        columns: &'a mut [&mut Vec<f64>],
        missing: &'a mut [&mut Vec<bool>],
        ids: &[fereg_core::metadata::SourceId],
    ) -> Option<(Self, Self)> {
        let (mut early, mut late) = (Self::new(positions.len())?, Self::new(positions.len())?);
        for (index, ((&position, values), flags)) in
            positions.iter().zip(columns).zip(missing).enumerate()
        {
            let is_early = index
                .checked_sub(1)
                .and_then(|i| request.model.sources.get(i))
                .is_some_and(|source| ids.contains(&source.id));
            let group = if is_early { &mut early } else { &mut late };
            group.positions.push(position);
            group.values.push(&mut **values);
            group.missing.push(&mut **flags);
        }
        (early.values.len() == ids.len()).then_some((early, late))
    }
}

/// Complete the categorical columns before lending them to optional native
/// factorization. The original source registry and numeric RHS recipe survive.
#[allow(clippy::too_many_arguments)]
pub(crate) fn try_read_prepared_reserved(
    spi: &mut dyn SpiApi,
    request: &OlsRequestV1,
    positions: &[u32],
    rows: usize,
    columns: &mut [&mut Vec<f64>],
    missing: &mut [&mut Vec<bool>],
    team: ThreadTeam,
    memory: &mut InputMemory,
    rhs: Option<(&mut Vec<f64>, NumericRhsLayout)>,
    hint: &mut Option<PreparedOlsLoadHint>,
) -> Result<bool> {
    let width = positions.len();
    if !(1..=MAX_COLUMNS).contains(&width)
        || request.model.sources.len() >= width
        || columns.len() != width
        || missing.len() != width
        || !spi.supports_numeric_rows()
        || memory.admit(4096).is_err()
    {
        return Ok(false);
    }
    // Admit the bounded identity/partition descriptors before even the first
    // optional Vec allocation; a tight budget retains the ordinary loader.
    let Ok(ids) = PreparedOlsLoadHint::source_ids(request) else {
        return Ok(false);
    };
    if ids.is_empty() || ids.len() >= width {
        return Ok(false);
    }
    let copied_rhs = rhs.as_ref().map_or(0, |(_, layout)| {
        layout
            .width
            .saturating_sub(usize::from(layout.append_intercept))
    });
    if request
        .model
        .sources
        .iter()
        .enumerate()
        .any(|(index, source)| index + 1 < copied_rhs && ids.contains(&source.id))
    {
        return Ok(false);
    }
    if [ids.len(), width - ids.len()].into_iter().any(|columns| {
        fereg_core::parallel::StagePlan::new(rows as u128 * columns as u128, 2, team.threads())
            .lanes
            <= 1
    }) {
        return Ok(false);
    }
    let Some(scratch) = PreparedOlsLoadHint::workspace_bytes(rows, ids.len()) else {
        return Ok(false);
    };
    let temporary = scratch + 4096; // Both small partition descriptor banks.
    let tiles = 2 * TILE_ROWS * width * std::mem::size_of::<f64>() + std::mem::size_of::<Handoff>();
    if temporary + tiles as u128 > memory.remaining() as u128 {
        return Ok(false);
    }
    let Some((mut early, mut late)) =
        ReservedColumns::partition(request, positions, columns, missing, &ids)
    else {
        return Ok(false);
    };
    // Allocate both disjoint tile pairs before reading: together they contain
    // no more elements than the ordinary full-width pair. Optional allocation
    // failures can therefore decline without suppressing an SPI error.
    let (Ok(early_tiles), Ok(late_tiles)) = (
        reserve_tiles(early.positions.len()),
        reserve_tiles(late.positions.len()),
    ) else {
        return Ok(false);
    };
    let tile_bytes = early_tiles
        .iter()
        .chain(&late_tiles)
        .map(|buffer| buffer.capacity() as u128 * 8)
        .sum::<u128>()
        + std::mem::size_of::<Handoff>() as u128;
    if memory.admit(temporary + tile_bytes).is_err() {
        return Ok(false);
    }
    // This temporary reservation includes completed factors and the largest
    // live factorization scratch beside either input tile pair. It is released
    // after joining; accepted factors replace preparation's ordinary bank.
    memory.retain(temporary)?;
    let result = (|| {
        if !try_read_reserved_with_tiles(
            spi,
            &early.positions,
            rows,
            &mut early.values,
            &mut early.missing,
            team,
            memory,
            None,
            Some(early_tiles),
        )? {
            return Ok(false);
        }
        let (read, prepared) = crate::cancellable::while_loading(
            team,
            || {
                PreparedOlsLoadHint::prepare(
                    ids.iter()
                        .copied()
                        .zip(early.values.iter().zip(&early.missing))
                        .map(|(id, (values, flags))| (id, values.as_slice(), flags.as_slice())),
                    rows,
                    scratch,
                )
            },
            |spi| {
                try_read_reserved_with_tiles(
                    spi,
                    &late.positions,
                    rows,
                    &mut late.values,
                    &mut late.missing,
                    team,
                    memory,
                    rhs,
                    Some(late_tiles),
                )
            },
            spi,
        )?;
        if !read {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "admitted numeric preparation tail declined",
            ));
        }
        *hint = prepared;
        Ok(true)
    })();
    memory.release(temporary)?;
    result
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
    rhs: Option<(&mut Vec<f64>, NumericRhsLayout)>,
) -> Result<bool> {
    try_read_reserved_with_tiles(
        spi, positions, rows, columns, missing, team, memory, rhs, None,
    )
}

#[allow(clippy::too_many_arguments)]
fn try_read_reserved_with_tiles(
    spi: &mut dyn SpiApi,
    positions: &[u32],
    rows: usize,
    columns: &mut [&mut Vec<f64>],
    missing: &mut [&mut Vec<bool>],
    team: ThreadTeam,
    memory: &mut InputMemory,
    mut rhs: Option<(&mut Vec<f64>, NumericRhsLayout)>,
    tiles: Option<[Vec<f64>; 2]>,
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
                || layout.width - usize::from(layout.append_intercept) > width
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
        tiles,
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
    if rhs_width > width {
        for row in values[..count * width].chunks_exact(width) {
            rhs.extend_from_slice(&row[..copied]);
            rhs.push(1.0);
        }
        return;
    }
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

fn reserve_tiles(width: usize) -> Result<[Vec<f64>; 2]> {
    let elements = TILE_ROWS * width;
    let mut buffers = [Vec::new(), Vec::new()];
    for buffer in &mut buffers {
        buffer.try_reserve_exact(elements).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve numeric input tile",
            )
        })?;
    }
    Ok(buffers)
}

fn read_tiles(
    spi: &mut dyn SpiApi,
    positions: &[u32],
    rows: usize,
    team: ThreadTeam,
    memory: &mut InputMemory,
    tiles: Option<[Vec<f64>; 2]>,
    mut consume: impl FnMut(usize, usize, &mut [f64], f64) + Send,
) -> Result<bool> {
    let width = positions.len();
    let elements = TILE_ROWS * width;
    let bytes = 2 * elements * std::mem::size_of::<f64>() + std::mem::size_of::<Handoff>();
    if bytes > memory.remaining() {
        return Ok(false);
    }
    memory.admit(bytes as u128)?;
    let mut buffers = match tiles {
        Some(tiles) => tiles,
        None => reserve_tiles(width)?,
    };
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
        missing_modulo: usize,
        batches: Vec<Vec<u32>>,
        interrupt: bool,
        polled: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    }

    impl Input {
        fn new() -> Self {
            Self {
                caller: std::thread::current().id(),
                calls: 0,
                failure: usize::MAX,
                panic: false,
                boundary: 1e100,
                missing_modulo: 197,
                batches: Vec::new(),
                interrupt: false,
                polled: None,
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
            self.batches.push(variables.to_vec());
            if self.calls == self.failure {
                assert!(!self.panic, "injected producer panic");
                return Err(FeregError::new(
                    ErrorCode::Interrupted,
                    "injected read failure",
                ));
            }
            for (row, chunk) in values.chunks_exact_mut(variables.len()).enumerate() {
                for (column, value) in chunk.iter_mut().enumerate() {
                    *value = if self.missing_modulo != 0 && (first + row) % self.missing_modulo == 0
                    {
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
            if let Some(polled) = &self.polled {
                polled.store(true, std::sync::atomic::Ordering::Release);
            }
            if self.interrupt {
                return Err(FeregError::new(
                    ErrorCode::Interrupted,
                    "injected caller interrupt",
                ));
            }
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
        let mut values: Vec<Vec<f64>> = (0..width).map(|_| Vec::with_capacity(rows)).collect();
        let mut flags: Vec<Vec<bool>> = (0..width).map(|_| Vec::with_capacity(rows)).collect();
        let mut columns: Vec<_> = values.iter_mut().collect();
        let mut missing: Vec<_> = flags.iter_mut().collect();
        let read = try_read_reserved(
            input,
            &positions,
            rows,
            &mut columns,
            &mut missing,
            ThreadTeam::new(ThreadConfig::from_requested(threads, false).unwrap()),
            memory,
            None,
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

    fn isolate_cancellation_test(name: &str) -> bool {
        const CHILD_ENV: &str = "FEREG_INPUT_PREPARATION_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_some() {
            return false;
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD_ENV, "1")
            .args([name, "--exact", "--test-threads=1"])
            .status()
            .expect("launch isolated input preparation test");
        assert!(status.success());
        true
    }

    #[test]
    fn prepared_input_overlap_preserves_slots_rhs_and_optional_fallback() {
        if isolate_cancellation_test("numeric_pipeline::tests::prepared_input_overlap_preserves_slots_rhs_and_optional_fallback") { return; }
        use fereg_core::metadata::{
            AbsorbTermId, AbsorbedTerm, CompiledTermGraph, SourceEntry, SourceId,
        };
        let rows = 16387;
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: (1..=3)
                    .map(|id| SourceEntry {
                        id: SourceId::new(id).unwrap(),
                        canonical_name: format!("x{id}"),
                    })
                    .collect(),
                terms: Vec::new(),
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![SourceId::new(3).unwrap()],
                slope_sources: Vec::new(),
                label: "g".into(),
                saved_name: String::new(),
            }],
            bindings: Vec::new(),
            include_intercept: true,
            keep_singletons: false,
        };
        for workers in [2, 4, 16] {
            for case in 0..6 {
                let mut request = request.clone();
                if case == 2 {
                    request.absorbed_terms[0].categorical_sources[0] = SourceId::new(1).unwrap();
                }
                let team = ThreadTeam::new(ThreadConfig::from_requested(workers, false).unwrap());
                let mut input = Input::new();
                input.missing_modulo = if case == 3 { 197 } else { 0 };
                if case >= 4 {
                    input.failure = 6;
                    input.panic = case == 5;
                }
                let mut memory = InputMemory::new(
                    (case == 1).then_some(2 * TILE_ROWS as u128 * 4 * 8 + 4096),
                    0,
                )
                .unwrap();
                let remaining = memory.remaining();
                let mut values = (0..4).map(|_| Vec::with_capacity(rows)).collect::<Vec<_>>();
                let mut flags = (0..4).map(|_| Vec::with_capacity(rows)).collect::<Vec<_>>();
                let mut rhs = Vec::with_capacity(rows * 4);
                let mut hint = None;
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    try_read_prepared_reserved(
                        &mut input,
                        &request,
                        &[2, 3, 4, 5],
                        rows,
                        &mut values.iter_mut().collect::<Vec<_>>(),
                        &mut flags.iter_mut().collect::<Vec<_>>(),
                        team,
                        &mut memory,
                        Some((
                            &mut rhs,
                            NumericRhsLayout {
                                width: 4,
                                append_intercept: true,
                            },
                        )),
                        &mut hint,
                    )
                }));
                if case == 5 {
                    assert!(outcome.is_err());
                    assert!(!fereg_core::interrupt::cancellation_requested());
                    continue;
                }
                let result = outcome.unwrap();
                assert_eq!(remaining, memory.remaining());
                assert!(!fereg_core::interrupt::cancellation_requested());
                if case == 4 {
                    assert_eq!(result.unwrap_err().code, ErrorCode::Interrupted);
                    assert!(hint.is_none());
                    assert_eq!(input.calls, 6);
                    continue;
                }
                let accepted = !matches!(case, 1 | 2);
                assert_eq!(result.unwrap(), accepted);
                if !accepted {
                    assert_eq!(input.calls, 0);
                    assert!(values.iter().all(Vec::is_empty));
                    assert!(flags.iter().all(Vec::is_empty));
                    assert!(rhs.is_empty());
                    assert!(hint.is_none());
                    continue;
                }
                assert_eq!(hint.is_some(), case != 3);
                assert_eq!(
                    input.batches,
                    (0..5)
                        .map(|_| vec![5])
                        .chain((0..5).map(|_| vec![2, 3, 4]))
                        .collect::<Vec<_>>()
                );
                for row in 0..rows {
                    let missing = case == 3 && (row + 1) % 197 == 0;
                    for column in 0..4 {
                        let expected = if missing {
                            input.boundary
                        } else {
                            (row + 1) as f64 + (column + 2) as f64 * 100000.
                        };
                        assert_eq!(values[column][row], expected);
                        assert_eq!(flags[column][row], missing);
                        assert_eq!(
                            rhs[row * 4 + column],
                            if column == 3 { 1. } else { expected }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn input_preparation_tail_polls_caller_and_joins_cancellation_or_unwind() {
        if isolate_cancellation_test("numeric_pipeline::tests::input_preparation_tail_polls_caller_and_joins_cancellation_or_unwind") { return; }
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        for mode in 0..4 {
            let signal = Arc::new(AtomicBool::new(false));
            let joined = AtomicBool::new(false);
            let mut input = Input::new();
            input.polled = Some(Arc::clone(&signal));
            input.interrupt = mode == 1;
            let caller = std::thread::current().id();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::cancellable::while_loading(
                    ThreadTeam::new(ThreadConfig::from_requested(2, false).unwrap()),
                    || {
                        assert_ne!(caller, std::thread::current().id());
                        let start = std::time::Instant::now();
                        while if mode == 0 {
                            !signal.load(Ordering::Acquire)
                        } else {
                            !fereg_core::interrupt::cancellation_requested()
                        } {
                            assert!(
                                start.elapsed().as_secs() < 30,
                                "caller failed to poll or cancel"
                            );
                            std::thread::yield_now();
                        }
                        joined.store(true, Ordering::Release);
                        23
                    },
                    |_| -> Result<usize> {
                        assert_eq!(caller, std::thread::current().id());
                        assert_ne!(mode, 3, "injected caller panic");
                        if mode == 2 {
                            Err(FeregError::new(
                                ErrorCode::InvalidData,
                                "authoritative input failure",
                            ))
                        } else {
                            Ok(17)
                        }
                    },
                    &mut input,
                )
            }));
            assert!(joined.load(Ordering::Acquire));
            assert!(!fereg_core::interrupt::cancellation_requested());
            match mode {
                0 => assert_eq!(result.unwrap().unwrap(), (17, 23)),
                1 => assert_eq!(result.unwrap().unwrap_err().code, ErrorCode::Interrupted),
                2 => assert_eq!(
                    result.unwrap().unwrap_err().message,
                    "authoritative input failure"
                ),
                _ => assert!(result.is_err()),
            }
        }
    }

    #[test]
    fn reserved_pipeline_preserves_values_capacities_and_failure_shutdown() {
        for append_intercept in [false, true] {
            for (width, threads) in [
                (3_usize, 4),
                (4, 4),
                (6, 4),
                (8, 8),
                (9, 4),
                (16, 4),
                (52, 4),
                (64, 4),
            ] {
                for rhs_width in [
                    2,
                    width.div_ceil(2),
                    width,
                    width + usize::from(append_intercept),
                ] {
                    for failure in [usize::MAX, 1, 3, 5] {
                        let rows = 16387;
                        let positions: Vec<u32> = (1..=width as u32).rev().collect();
                        let mut values: Vec<Vec<f64>> =
                            (0..width).map(|_| Vec::with_capacity(rows)).collect();
                        let mut flags: Vec<Vec<bool>> =
                            (0..width).map(|_| Vec::with_capacity(rows)).collect();
                        let pointers: Vec<_> = values.iter().map(Vec::as_ptr).collect();
                        let mut rhs = Vec::with_capacity(rows * rhs_width);
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
                                    width: rhs_width,
                                    append_intercept,
                                },
                            )),
                        );
                        if failure == usize::MAX {
                            assert_eq!(result, Ok(true));
                        } else {
                            assert_eq!(result.unwrap_err().code, ErrorCode::Interrupted);
                        }
                        let initialized =
                            rows.min(failure.saturating_sub(1).saturating_mul(TILE_ROWS));
                        assert_eq!(rhs.len(), initialized * rhs_width);
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
