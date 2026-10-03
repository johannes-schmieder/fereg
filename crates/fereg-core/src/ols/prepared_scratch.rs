//! Bounded scratch storage for low-memory prepared OLS.
//!
//! The file is private to one fit, created with `create_new`, and removed by
//! `Drop` on success, ordinary error, panic unwinding, or caller cancellation.
//! Ordinary OLS uses column-major storage. Iterative callers may explicitly
//! create row-major storage so absorbed blocks can be patched and compacted in
//! place without owning a second observation-by-parameter scratch file.

use rayon::prelude::*;
use std::fs::{remove_file, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{ErrorCode, FeregError, Result};

const IO_BUFFER_BYTES: usize = 1 << 20;
const SHARED_READ_BUFFER_BYTES: usize = 8 << 10;
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScratchLayout {
    ColumnMajor,
    RowMajor,
}

#[derive(Debug)]
pub(crate) struct PreparedOlsScratch {
    file: Option<File>,
    path: PathBuf,
    rows: usize,
    columns: usize,
    bytes: u64,
    layout: ScratchLayout,
    poisoned: bool,
    io_buffer: Vec<u8>,
    #[cfg(test)]
    shared_read_chunks: AtomicU64,
    #[cfg(test)]
    encoded_tile_reads: u64,
    #[cfg(test)]
    encoded_tile_writes: u64,
}

/// One immutable scan owns its encoded page. Borrowing the scratch prevents
/// writes, compaction or weight-frame replacement while cached bytes are live.
/// The page replaces the same-sized temporary used by each shared row read.
pub(crate) struct PreparedOlsScratchReader<'a> {
    scratch: &'a PreparedOlsScratch,
    encoded: [u8; SHARED_READ_BUFFER_BYTES],
    start: usize,
    elements: usize,
}

impl PreparedOlsScratchReader<'_> {
    pub(crate) fn read_linear(&mut self, offset: usize, values: &mut [f64]) -> Result<()> {
        self.scratch.validate_linear_range(offset, values.len())?;
        let page_elements = self.encoded.len() / 8;
        let mut position = offset;
        let mut remaining = values;
        while !remaining.is_empty() {
            if position < self.start || position - self.start >= self.elements {
                // Revoke the previous page before fallible I/O. A failed read
                // must not authorize its partially overwritten bytes.
                self.elements = 0;
                self.start = position / page_elements * page_elements;
                let total = self.scratch.rows * self.scratch.columns;
                let elements = (total - self.start).min(page_elements);
                let byte_offset = u64::try_from(self.start)
                    .ok()
                    .and_then(|value| value.checked_mul(8))
                    .ok_or_else(|| allocation("prepared OLS shared read offset overflows u64"))?;
                #[cfg(test)]
                self.scratch
                    .shared_read_chunks
                    .fetch_add(1, Ordering::Relaxed);
                read_exact_at(
                    self.scratch.file_ref(),
                    &mut self.encoded[..elements * 8],
                    byte_offset,
                )
                .map_err(|error| {
                    io_error("unable to positionally read prepared OLS scratch", error)
                })?;
                self.elements = elements;
            }
            let local = position - self.start;
            let count = remaining.len().min(self.elements - local);
            for (value, bytes) in remaining[..count]
                .iter_mut()
                .zip(self.encoded[local * 8..(local + count) * 8].chunks_exact(8))
            {
                let mut value_bytes = [0_u8; 8];
                value_bytes.copy_from_slice(bytes);
                *value = f64::from_le_bytes(value_bytes);
                if !value.is_finite() {
                    return Err(invalid("prepared OLS scratch contains a nonfinite value"));
                }
            }
            position += count;
            remaining = &mut remaining[count..];
        }
        Ok(())
    }
}

impl PreparedOlsScratch {
    pub(crate) fn shared_reader(&self) -> PreparedOlsScratchReader<'_> {
        PreparedOlsScratchReader {
            scratch: self,
            encoded: [0; SHARED_READ_BUFFER_BYTES],
            start: 0,
            elements: 0,
        }
    }

    pub(crate) fn create(rows: usize, columns: usize) -> Result<Self> {
        Self::create_with_layout(rows, columns, ScratchLayout::ColumnMajor)
    }

    pub(crate) fn create_row_major(rows: usize, columns: usize) -> Result<Self> {
        Self::create_with_layout(rows, columns, ScratchLayout::RowMajor)
    }

    fn create_with_layout(rows: usize, columns: usize, layout: ScratchLayout) -> Result<Self> {
        Self::create_with_buffer(rows, columns, layout, true)
    }

    /// A retained copy borrows the working file's buffer until QR releases it.
    pub(crate) fn create_unbuffered(rows: usize, columns: usize) -> Result<Self> {
        Self::create_with_buffer(rows, columns, ScratchLayout::ColumnMajor, false)
    }

    fn create_with_buffer(
        rows: usize,
        columns: usize,
        layout: ScratchLayout,
        buffered: bool,
    ) -> Result<Self> {
        let elements = rows
            .checked_mul(columns)
            .ok_or_else(|| allocation("prepared OLS scratch dimensions overflow usize"))?;
        let bytes = u64::try_from(elements)
            .ok()
            .and_then(|value| value.checked_mul(8))
            .ok_or_else(|| allocation("prepared OLS scratch size overflows u64"))?;
        let mut io_buffer = Vec::new();
        let addressable_bytes = usize::try_from(bytes).unwrap_or(usize::MAX);
        let minimum_row_bytes = if layout == ScratchLayout::RowMajor && rows != 0 {
            columns
                .checked_mul(8)
                .ok_or_else(|| allocation("prepared OLS row-major scratch row size overflows"))?
        } else {
            0
        };
        let buffer_bytes = IO_BUFFER_BYTES
            .min(addressable_bytes)
            .max(minimum_row_bytes)
            .max(8);
        let buffer_bytes = if buffered { buffer_bytes } else { 0 };
        io_buffer
            .try_reserve_exact(buffer_bytes)
            .map_err(|_| allocation("unable to reserve prepared OLS scratch I/O buffer"))?;
        if io_buffer.capacity() < buffer_bytes {
            return Err(allocation(
                "prepared OLS scratch I/O reservation was undersized",
            ));
        }
        io_buffer.resize(buffer_bytes, 0);

        let directory = std::env::temp_dir();
        let process = std::process::id();
        let mut last_error = None;
        for _ in 0..128 {
            let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!("fereg-ols-{process}-{sequence}.scratch"));
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    if let Err(error) = file.set_len(bytes) {
                        drop(file);
                        let _ = remove_file(&path);
                        return Err(io_error("unable to size prepared OLS scratch file", error));
                    }
                    return Ok(Self {
                        file: Some(file),
                        path,
                        rows,
                        columns,
                        bytes,
                        layout,
                        poisoned: false,
                        io_buffer,
                        #[cfg(test)]
                        shared_read_chunks: AtomicU64::new(0),
                        #[cfg(test)]
                        encoded_tile_reads: 0,
                        #[cfg(test)]
                        encoded_tile_writes: 0,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    last_error = Some(error);
                }
                Err(error) => {
                    return Err(io_error(
                        "unable to create private prepared OLS scratch file",
                        error,
                    ));
                }
            }
        }
        Err(io_error(
            "unable to create a unique prepared OLS scratch file",
            last_error.unwrap_or_else(|| std::io::Error::other("name exhaustion")),
        ))
    }

    pub(crate) const fn rows(&self) -> usize {
        self.rows
    }

    pub(crate) const fn columns(&self) -> usize {
        self.columns
    }

    pub(crate) const fn bytes(&self) -> u64 {
        self.bytes
    }

    #[cfg(test)]
    pub(crate) const fn layout(&self) -> ScratchLayout {
        self.layout
    }

    pub(crate) fn io_buffer_capacity(&self) -> usize {
        self.io_buffer.capacity()
    }

    #[cfg(test)]
    pub(crate) fn identity_path(&self) -> &std::path::Path {
        &self.path
    }

    pub(crate) fn write_column(&mut self, column: usize, values: &[f64]) -> Result<()> {
        self.require_layout(ScratchLayout::ColumnMajor)?;
        self.validate_column(column, values.len())?;
        let offset = self.column_element_offset(column, 0)?;
        self.write_values_at(offset, values)
    }

    pub(crate) fn write_column_with_borrowed_buffer(
        &mut self,
        column: usize,
        values: &[f64],
        donor: &mut Self,
    ) -> Result<()> {
        self.take_io_buffer_from(donor)?;
        let result = self.write_column(column, values);
        // Ordinary errors return the bank; panic unwinding drops both private
        // files and their buffers with the enclosing failed fit.
        std::mem::swap(&mut self.io_buffer, &mut donor.io_buffer);
        result
    }

    pub(crate) fn take_io_buffer_from(&mut self, donor: &mut Self) -> Result<()> {
        if !self.io_buffer.is_empty() || donor.io_buffer.len() < 8 {
            return Err(invalid(
                "scratch buffer transfer requires an empty recipient",
            ));
        }
        self.require_healthy()?;
        donor.require_healthy()?;
        std::mem::swap(&mut self.io_buffer, &mut donor.io_buffer);
        Ok(())
    }

    pub(crate) fn read_column(&mut self, column: usize, values: &mut [f64]) -> Result<()> {
        self.require_layout(ScratchLayout::ColumnMajor)?;
        self.validate_column(column, values.len())?;
        let offset = self.column_element_offset(column, 0)?;
        self.read_values_at(offset, values)
    }

    pub(crate) fn read_column_range(
        &mut self,
        column: usize,
        start_row: usize,
        values: &mut [f64],
    ) -> Result<()> {
        self.require_layout(ScratchLayout::ColumnMajor)?;
        let end_row = start_row
            .checked_add(values.len())
            .ok_or_else(|| allocation("prepared OLS scratch row range overflows"))?;
        if column >= self.columns || end_row > self.rows {
            return Err(invalid("prepared OLS scratch row range is out of bounds"));
        }
        let offset = self.column_element_offset(column, start_row)?;
        self.read_values_at(offset, values)
    }

    /// Replace a complete current-frame design with one contiguous write per
    /// existing tile. Publication callers must certify before this mutation;
    /// any callback or I/O failure poisons the private file until it is dropped.
    pub(crate) fn rewrite_row_major_with(
        &mut self,
        value: impl Fn(usize, usize) -> Result<f64> + Sync,
    ) -> Result<()> {
        self.rewrite_row_major_prepared_with(
            &mut (),
            usize::MAX,
            |_, _, _| Ok(()),
            |_, row, column| value(row, column),
        )
    }

    /// Prepare one bounded immutable row tile before parallel encoding. The
    /// preparation state is private to this rewrite; failure poisons scratch
    /// before either preparation or publication of the first encoded tile.
    pub(crate) fn rewrite_row_major_prepared_with<T: Sync>(
        &mut self,
        state: &mut T,
        maximum_rows: usize,
        prepare: impl Fn(&mut T, usize, usize) -> Result<()>,
        value: impl Fn(&T, usize, usize) -> Result<f64> + Sync,
    ) -> Result<()> {
        if self.layout != ScratchLayout::RowMajor || self.poisoned || maximum_rows == 0 {
            return Err(invalid(
                "prepared OLS rewrite requires valid row-major scratch",
            ));
        }
        if self.rows == 0 || self.columns == 0 {
            return Ok(());
        }
        let tile_rows = self.row_major_tile_rows()?.min(maximum_rows);
        let row_bytes = self.row_bytes()?;
        self.poisoned = true;
        for first in (0..self.rows).step_by(tile_rows) {
            let count = (self.rows - first).min(tile_rows);
            prepare(state, first, count)?;
            let byte_count = count
                .checked_mul(row_bytes)
                .ok_or_else(|| allocation("prepared OLS rewrite tile overflows"))?;
            self.io_buffer[..byte_count]
                .par_chunks_mut(row_bytes)
                .enumerate()
                .try_for_each(|(local, bytes)| -> Result<()> {
                    for (column, target) in bytes.chunks_exact_mut(8).enumerate() {
                        let x = value(state, first + local, column)?;
                        if !x.is_finite() {
                            return Err(invalid("prepared OLS rewrite value is nonfinite"));
                        }
                        target.copy_from_slice(&x.to_le_bytes());
                    }
                    Ok(())
                })?;
            self.write_encoded_tile(first * self.columns, byte_count)?;
        }
        self.poisoned = false;
        Ok(())
    }

    /// Patch a row-major `rows x block_columns` block into canonical column
    /// positions without allocating another full scratch file. Each I/O pair
    /// reads and rewrites a contiguous full-width row tile.
    pub(crate) fn patch_row_major_block(
        &mut self,
        start_column: usize,
        block_columns: usize,
        values: &[f64],
    ) -> Result<()> {
        self.validate_row_major_block(start_column, block_columns, values.len())?;
        if values.iter().any(|value| !value.is_finite()) {
            return Err(invalid(
                "prepared OLS row-major scratch patch received a nonfinite value",
            ));
        }
        if self.rows == 0 || block_columns == 0 {
            return Ok(());
        }

        let tile_rows = self.row_major_tile_rows()?;
        let row_bytes = self.row_bytes()?;
        let mut mutation_started = false;
        for start_row in (0..self.rows).step_by(tile_rows) {
            let rows_in_tile = (self.rows - start_row).min(tile_rows);
            let byte_count = rows_in_tile
                .checked_mul(row_bytes)
                .ok_or_else(|| allocation("prepared OLS row-major patch tile overflows"))?;
            let element_offset = start_row
                .checked_mul(self.columns)
                .ok_or_else(|| allocation("prepared OLS row-major patch offset overflows"))?;
            self.read_encoded_tile(element_offset, byte_count)?;

            for local_row in 0..rows_in_tile {
                let source_start = (start_row + local_row)
                    .checked_mul(block_columns)
                    .ok_or_else(|| {
                        allocation("prepared OLS row-major patch source offset overflows")
                    })?;
                let target_start = local_row
                    .checked_mul(self.columns)
                    .and_then(|offset| offset.checked_add(start_column))
                    .and_then(|offset| offset.checked_mul(8))
                    .ok_or_else(|| {
                        allocation("prepared OLS row-major patch target offset overflows")
                    })?;
                for (local_column, value) in values[source_start..source_start + block_columns]
                    .iter()
                    .enumerate()
                {
                    let byte_start = target_start + local_column * 8;
                    self.io_buffer[byte_start..byte_start + 8]
                        .copy_from_slice(&value.to_le_bytes());
                }
            }
            if !mutation_started {
                self.poisoned = true;
                mutation_started = true;
            }
            self.write_encoded_tile(element_offset, byte_count)?;
        }
        if mutation_started {
            self.poisoned = false;
        }
        Ok(())
    }

    /// Extract a row-major `rows x block_columns` block into caller-owned
    /// storage while reading contiguous full-width row tiles.
    pub(crate) fn read_row_major_block(
        &mut self,
        start_column: usize,
        block_columns: usize,
        values: &mut [f64],
    ) -> Result<()> {
        self.validate_row_major_block(start_column, block_columns, values.len())?;
        if self.rows == 0 || block_columns == 0 {
            return Ok(());
        }

        let tile_rows = self.row_major_tile_rows()?;
        let row_bytes = self.row_bytes()?;
        for start_row in (0..self.rows).step_by(tile_rows) {
            let rows_in_tile = (self.rows - start_row).min(tile_rows);
            let byte_count = rows_in_tile
                .checked_mul(row_bytes)
                .ok_or_else(|| allocation("prepared OLS row-major read tile overflows"))?;
            let element_offset = start_row
                .checked_mul(self.columns)
                .ok_or_else(|| allocation("prepared OLS row-major read offset overflows"))?;
            self.read_encoded_tile(element_offset, byte_count)?;

            for local_row in 0..rows_in_tile {
                let source_start = local_row
                    .checked_mul(self.columns)
                    .and_then(|offset| offset.checked_add(start_column))
                    .and_then(|offset| offset.checked_mul(8))
                    .ok_or_else(|| {
                        allocation("prepared OLS row-major read source offset overflows")
                    })?;
                let target_start = (start_row + local_row)
                    .checked_mul(block_columns)
                    .ok_or_else(|| {
                        allocation("prepared OLS row-major read target offset overflows")
                    })?;
                for local_column in 0..block_columns {
                    let byte_start = source_start + local_column * 8;
                    let mut encoded = [0_u8; 8];
                    encoded.copy_from_slice(&self.io_buffer[byte_start..byte_start + 8]);
                    let value = f64::from_le_bytes(encoded);
                    if !value.is_finite() {
                        return Err(invalid(
                            "prepared OLS row-major scratch contains a nonfinite value",
                        ));
                    }
                    values[target_start + local_column] = value;
                }
            }
        }
        Ok(())
    }

    /// Read selected positions from one row without allocation or mutation of
    /// the shared file cursor. Selected positions must be strictly increasing.
    #[cfg(test)]
    pub(crate) fn read_selected_row_shared(
        &self,
        row: usize,
        selected_columns: &[usize],
        values: &mut [f64],
    ) -> Result<()> {
        self.require_layout(ScratchLayout::RowMajor)?;
        if row >= self.rows || values.len() != selected_columns.len() {
            return Err(invalid(
                "prepared OLS selected row dimensions are inconsistent",
            ));
        }
        self.validate_selected_columns(selected_columns)?;
        if selected_columns.is_empty() {
            return Ok(());
        }

        let row_start = row
            .checked_mul(self.columns)
            .ok_or_else(|| allocation("prepared OLS selected row offset overflows"))?;
        let mut encoded = [0_u8; SHARED_READ_BUFFER_BYTES];
        let values_per_chunk = encoded.len() / 8;
        let mut selected = 0;
        while selected < selected_columns.len() {
            let chunk_start = (selected_columns[selected] / values_per_chunk) * values_per_chunk;
            let chunk_end = chunk_start
                .saturating_add(values_per_chunk)
                .min(self.columns);
            let chunk_values = chunk_end - chunk_start;
            let byte_count = chunk_values
                .checked_mul(8)
                .ok_or_else(|| allocation("prepared OLS selected row read size overflows"))?;
            let byte_offset = row_start
                .checked_add(chunk_start)
                .and_then(|offset| u64::try_from(offset).ok())
                .and_then(|offset| offset.checked_mul(8))
                .ok_or_else(|| allocation("prepared OLS selected row offset overflows u64"))?;
            #[cfg(test)]
            self.shared_read_chunks.fetch_add(1, Ordering::Relaxed);
            read_exact_at(self.file_ref(), &mut encoded[..byte_count], byte_offset).map_err(
                |error| {
                    io_error(
                        "unable to positionally read prepared OLS scratch row",
                        error,
                    )
                },
            )?;
            while selected < selected_columns.len() && selected_columns[selected] < chunk_end {
                let byte_start = (selected_columns[selected] - chunk_start) * 8;
                let mut value_bytes = [0_u8; 8];
                value_bytes.copy_from_slice(&encoded[byte_start..byte_start + 8]);
                values[selected] = f64::from_le_bytes(value_bytes);
                if !values[selected].is_finite() {
                    return Err(invalid(
                        "prepared OLS row-major scratch contains a nonfinite value",
                    ));
                }
                selected += 1;
            }
        }
        Ok(())
    }

    /// Forward-compact selected canonical columns in the same row-major file.
    /// The source tile is read completely before its compacted prefix is
    /// written leftward, so no later unread source row can be overwritten.
    pub(crate) fn compact_row_major_columns_forward(
        &mut self,
        selected_columns: &[usize],
    ) -> Result<()> {
        self.require_layout(ScratchLayout::RowMajor)?;
        self.validate_selected_columns(selected_columns)?;
        let old_columns = self.columns;
        let new_columns = selected_columns.len();
        if new_columns == old_columns {
            return Ok(());
        }
        let new_elements = self
            .rows
            .checked_mul(new_columns)
            .ok_or_else(|| allocation("prepared OLS compact scratch dimensions overflow usize"))?;
        let new_bytes = u64::try_from(new_elements)
            .ok()
            .and_then(|value| value.checked_mul(8))
            .ok_or_else(|| allocation("prepared OLS compact scratch size overflows u64"))?;

        if self.rows != 0 && new_columns != 0 {
            let tile_rows = self.row_major_tile_rows()?;
            let row_bytes = self.row_bytes()?;
            for start_row in (0..self.rows).step_by(tile_rows) {
                let rows_in_tile = (self.rows - start_row).min(tile_rows);
                let source_bytes = rows_in_tile.checked_mul(row_bytes).ok_or_else(|| {
                    allocation("prepared OLS compact scratch source tile overflows")
                })?;
                let source_offset = start_row.checked_mul(old_columns).ok_or_else(|| {
                    allocation("prepared OLS compact scratch source offset overflows")
                })?;
                self.read_encoded_tile(source_offset, source_bytes)?;
                for local_row in 0..rows_in_tile {
                    for (target_column, source_column) in
                        selected_columns.iter().copied().enumerate()
                    {
                        let source_start = local_row
                            .checked_mul(old_columns)
                            .and_then(|offset| offset.checked_add(source_column))
                            .and_then(|offset| offset.checked_mul(8))
                            .ok_or_else(|| {
                                allocation(
                                    "prepared OLS compact scratch tile source offset overflows",
                                )
                            })?;
                        let target_start = local_row
                            .checked_mul(new_columns)
                            .and_then(|offset| offset.checked_add(target_column))
                            .and_then(|offset| offset.checked_mul(8))
                            .ok_or_else(|| {
                                allocation(
                                    "prepared OLS compact scratch tile target offset overflows",
                                )
                            })?;
                        self.io_buffer
                            .copy_within(source_start..source_start + 8, target_start);
                    }
                }
                let target_bytes = rows_in_tile
                    .checked_mul(new_columns)
                    .and_then(|elements| elements.checked_mul(8))
                    .ok_or_else(|| {
                        allocation("prepared OLS compact scratch target tile overflows")
                    })?;
                let target_offset = start_row.checked_mul(new_columns).ok_or_else(|| {
                    allocation("prepared OLS compact scratch target offset overflows")
                })?;
                self.poisoned = true;
                self.write_encoded_tile(target_offset, target_bytes)?;
            }
        }

        self.poisoned = true;
        self.file_mut()
            .set_len(new_bytes)
            .map_err(|error| io_error("unable to truncate prepared OLS scratch file", error))?;
        self.columns = new_columns;
        self.bytes = new_bytes;
        self.poisoned = false;
        Ok(())
    }

    pub(crate) fn write_linear(&mut self, offset: usize, values: &[f64]) -> Result<()> {
        self.validate_linear_range(offset, values.len())?;
        self.write_values_at(offset, values)
    }

    pub(crate) fn read_linear(&mut self, offset: usize, values: &mut [f64]) -> Result<()> {
        self.validate_linear_range(offset, values.len())?;
        self.read_values_at(offset, values)
    }

    /// Read without changing the shared file cursor. This is the read-only,
    /// allocation-free seam used by parallel final-inference row adapters.
    pub(crate) fn read_linear_shared(&self, offset: usize, values: &mut [f64]) -> Result<()> {
        self.validate_linear_range(offset, values.len())?;
        let mut element_offset = offset;
        let mut encoded = [0_u8; SHARED_READ_BUFFER_BYTES];
        let values_per_chunk = encoded.len() / 8;
        for chunk in values.chunks_mut(values_per_chunk) {
            let byte_count = chunk
                .len()
                .checked_mul(8)
                .ok_or_else(|| allocation("prepared OLS shared read size overflows"))?;
            let byte_offset = u64::try_from(element_offset)
                .ok()
                .and_then(|value| value.checked_mul(8))
                .ok_or_else(|| allocation("prepared OLS shared read offset overflows u64"))?;
            #[cfg(test)]
            self.shared_read_chunks.fetch_add(1, Ordering::Relaxed);
            read_exact_at(self.file_ref(), &mut encoded[..byte_count], byte_offset).map_err(
                |error| io_error("unable to positionally read prepared OLS scratch", error),
            )?;
            for (value, bytes) in chunk.iter_mut().zip(encoded[..byte_count].chunks_exact(8)) {
                let mut value_bytes = [0_u8; 8];
                value_bytes.copy_from_slice(bytes);
                *value = f64::from_le_bytes(value_bytes);
                if !value.is_finite() {
                    return Err(invalid("prepared OLS scratch contains a nonfinite value"));
                }
            }
            element_offset = element_offset
                .checked_add(chunk.len())
                .ok_or_else(|| allocation("prepared OLS shared read offset overflows usize"))?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn shared_read_chunks_for_test(&self) -> u64 {
        self.shared_read_chunks.load(Ordering::Relaxed)
    }

    fn read_encoded_tile(&mut self, element_offset: usize, byte_count: usize) -> Result<()> {
        if byte_count > self.io_buffer.len() {
            return Err(allocation(
                "prepared OLS scratch tile exceeds its fixed I/O buffer",
            ));
        }
        self.seek_element(element_offset)?;
        #[cfg(test)]
        {
            self.encoded_tile_reads += 1;
        }
        let file = self
            .file
            .as_mut()
            .expect("prepared OLS scratch file is open before Drop");
        file.read_exact(&mut self.io_buffer[..byte_count])
            .map_err(|error| io_error("unable to read prepared OLS scratch tile", error))
    }

    fn write_encoded_tile(&mut self, element_offset: usize, byte_count: usize) -> Result<()> {
        if byte_count > self.io_buffer.len() {
            return Err(allocation(
                "prepared OLS scratch tile exceeds its fixed I/O buffer",
            ));
        }
        self.seek_element(element_offset)?;
        #[cfg(test)]
        {
            self.encoded_tile_writes += 1;
        }
        let file = self
            .file
            .as_mut()
            .expect("prepared OLS scratch file is open before Drop");
        file.write_all(&self.io_buffer[..byte_count])
            .map_err(|error| io_error("unable to write prepared OLS scratch tile", error))
    }

    fn write_values_at(&mut self, offset: usize, values: &[f64]) -> Result<()> {
        self.require_healthy()?;
        if values.iter().any(|value| !value.is_finite()) {
            return Err(invalid(
                "prepared OLS scratch write received a nonfinite value",
            ));
        }
        if values.is_empty() {
            return Ok(());
        }
        self.seek_element(offset)?;
        let values_per_chunk = self.io_buffer.len() / 8;
        if values_per_chunk == 0 {
            return Err(invalid("prepared OLS scratch has no active I/O buffer"));
        }
        for chunk in values.chunks(values_per_chunk) {
            let byte_count = chunk
                .len()
                .checked_mul(8)
                .ok_or_else(|| allocation("prepared OLS scratch write size overflows"))?;
            for (value, bytes) in chunk
                .iter()
                .zip(self.io_buffer[..byte_count].chunks_exact_mut(8))
            {
                bytes.copy_from_slice(&value.to_le_bytes());
            }
            self.poisoned = true;
            let file = self
                .file
                .as_mut()
                .expect("prepared OLS scratch file is open before Drop");
            file.write_all(&self.io_buffer[..byte_count])
                .map_err(|error| io_error("unable to write prepared OLS scratch column", error))?;
        }
        self.poisoned = false;
        Ok(())
    }

    fn read_values_at(&mut self, offset: usize, values: &mut [f64]) -> Result<()> {
        self.seek_element(offset)?;
        let values_per_chunk = self.io_buffer.len() / 8;
        if values_per_chunk == 0 {
            return Err(invalid("prepared OLS scratch has no active I/O buffer"));
        }
        for chunk in values.chunks_mut(values_per_chunk) {
            let byte_count = chunk
                .len()
                .checked_mul(8)
                .ok_or_else(|| allocation("prepared OLS scratch read size overflows"))?;
            let file = self
                .file
                .as_mut()
                .expect("prepared OLS scratch file is open before Drop");
            file.read_exact(&mut self.io_buffer[..byte_count])
                .map_err(|error| io_error("unable to read prepared OLS scratch column", error))?;
            for (value, bytes) in chunk
                .iter_mut()
                .zip(self.io_buffer[..byte_count].chunks_exact(8))
            {
                let mut encoded = [0_u8; 8];
                encoded.copy_from_slice(bytes);
                *value = f64::from_le_bytes(encoded);
                if !value.is_finite() {
                    return Err(invalid("prepared OLS scratch contains a nonfinite value"));
                }
            }
        }
        Ok(())
    }

    fn validate_column(&self, column: usize, values: usize) -> Result<()> {
        if column >= self.columns || values != self.rows {
            return Err(invalid(
                "prepared OLS scratch column dimensions are inconsistent",
            ));
        }
        Ok(())
    }

    fn validate_row_major_block(
        &self,
        start_column: usize,
        block_columns: usize,
        values: usize,
    ) -> Result<()> {
        self.require_layout(ScratchLayout::RowMajor)?;
        let end_column = start_column
            .checked_add(block_columns)
            .ok_or_else(|| allocation("prepared OLS row-major block range overflows"))?;
        let expected_values = self
            .rows
            .checked_mul(block_columns)
            .ok_or_else(|| allocation("prepared OLS row-major block dimensions overflow"))?;
        if end_column > self.columns || values != expected_values {
            return Err(invalid(
                "prepared OLS row-major block dimensions are inconsistent",
            ));
        }
        Ok(())
    }

    fn validate_selected_columns(&self, selected_columns: &[usize]) -> Result<()> {
        if selected_columns
            .iter()
            .copied()
            .any(|column| column >= self.columns)
            || selected_columns
                .windows(2)
                .any(|columns| columns[0] >= columns[1])
        {
            return Err(invalid(
                "prepared OLS selected columns must be strictly increasing and in range",
            ));
        }
        Ok(())
    }

    fn require_layout(&self, expected: ScratchLayout) -> Result<()> {
        self.require_healthy()?;
        if self.layout == expected {
            Ok(())
        } else {
            Err(invalid("prepared OLS scratch layout is inconsistent"))
        }
    }

    fn require_healthy(&self) -> Result<()> {
        if self.poisoned {
            Err(invalid(
                "prepared OLS scratch is unavailable after an incomplete in-place mutation",
            ))
        } else {
            Ok(())
        }
    }

    fn row_bytes(&self) -> Result<usize> {
        self.columns
            .checked_mul(8)
            .ok_or_else(|| allocation("prepared OLS scratch row size overflows"))
    }

    fn row_major_tile_rows(&self) -> Result<usize> {
        self.require_layout(ScratchLayout::RowMajor)?;
        let row_bytes = self.row_bytes()?;
        if row_bytes == 0 {
            return Ok(self.rows.max(1));
        }
        let tile_rows = self.io_buffer.len() / row_bytes;
        if tile_rows == 0 {
            return Err(allocation(
                "prepared OLS row-major scratch row exceeds its fixed I/O buffer",
            ));
        }
        Ok(tile_rows.min(self.rows.max(1)))
    }

    fn validate_linear_range(&self, offset: usize, values: usize) -> Result<()> {
        self.require_healthy()?;
        let end = offset
            .checked_add(values)
            .ok_or_else(|| allocation("prepared OLS scratch linear range overflows"))?;
        let elements = self
            .rows
            .checked_mul(self.columns)
            .ok_or_else(|| allocation("prepared OLS scratch dimensions overflow"))?;
        if end > elements {
            return Err(invalid(
                "prepared OLS scratch linear range is out of bounds",
            ));
        }
        Ok(())
    }

    fn column_element_offset(&self, column: usize, row: usize) -> Result<usize> {
        column
            .checked_mul(self.rows)
            .and_then(|value| value.checked_add(row))
            .ok_or_else(|| allocation("prepared OLS scratch column offset overflows"))
    }

    fn seek_element(&mut self, element: usize) -> Result<()> {
        let offset = u64::try_from(element)
            .ok()
            .and_then(|value| value.checked_mul(8))
            .ok_or_else(|| allocation("prepared OLS scratch offset overflows u64"))?;
        self.file_mut()
            .seek(SeekFrom::Start(offset))
            .map_err(|error| io_error("unable to seek prepared OLS scratch file", error))?;
        Ok(())
    }

    fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("prepared OLS scratch file is open before Drop")
    }

    fn file_ref(&self) -> &File {
        self.file
            .as_ref()
            .expect("prepared OLS scratch file is open before Drop")
    }
}

#[cfg(unix)]
fn read_exact_at(file: &File, mut output: &mut [u8], mut offset: u64) -> std::io::Result<()> {
    use std::os::unix::fs::FileExt;

    while !output.is_empty() {
        let read = file.read_at(output, offset)?;
        if read == 0 {
            return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        }
        offset = offset
            .checked_add(read as u64)
            .ok_or_else(|| std::io::Error::other("scratch positional-read offset overflow"))?;
        output = &mut output[read..];
    }
    Ok(())
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut output: &mut [u8], mut offset: u64) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;

    while !output.is_empty() {
        let read = file.seek_read(output, offset)?;
        if read == 0 {
            return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        }
        offset = offset
            .checked_add(read as u64)
            .ok_or_else(|| std::io::Error::other("scratch positional-read offset overflow"))?;
        output = &mut output[read..];
    }
    Ok(())
}

impl Drop for PreparedOlsScratch {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = remove_file(&self.path);
    }
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn io_error(context: &str, error: std::io::Error) -> FeregError {
    FeregError::new(ErrorCode::Allocation, format!("{context}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_row_rewrite_has_no_reads_and_poisoned_failure_cleans_up() {
        let rows = IO_BUFFER_BYTES / 24 + 7;
        let path;
        {
            let mut scratch = PreparedOlsScratch::create_row_major(rows, 3).unwrap();
            path = scratch.path.clone();
            let capacity = scratch.io_buffer_capacity();
            scratch
                .rewrite_row_major_with(|i, j| Ok((i * 3 + j) as f64))
                .unwrap();
            assert_eq!(scratch.encoded_tile_reads, 0);
            assert_eq!(
                scratch.encoded_tile_writes,
                rows.div_ceil(scratch.row_major_tile_rows().unwrap()) as u64
            );
            assert_eq!(scratch.io_buffer_capacity(), capacity);
            let mut row = [0.; 3];
            scratch
                .read_linear_shared((rows - 1) * 3, &mut row)
                .unwrap();
            assert_eq!(
                row,
                [
                    ((rows - 1) * 3) as f64,
                    ((rows - 1) * 3 + 1) as f64,
                    ((rows - 1) * 3 + 2) as f64
                ]
            );
            assert!(scratch
                .rewrite_row_major_with(|i, _| if i == rows - 1 {
                    Err(invalid("injected callback refusal"))
                } else {
                    Ok(0.)
                })
                .is_err());
            assert!(scratch.poisoned);
            assert!(scratch.read_linear_shared(0, &mut row).is_err());
            assert!(scratch.rewrite_row_major_with(|_, _| Ok(1.)).is_err());
        }
        assert!(!path.exists());
        let mut scratch = PreparedOlsScratch::create_row_major(2, 3).unwrap();
        scratch.file = Some(File::open(&scratch.path).unwrap());
        assert!(scratch.rewrite_row_major_with(|_, _| Ok(1.)).is_err());
        assert!(scratch.poisoned);
    }

    #[test]
    fn prepared_row_tiles_preserve_state_and_poison_failed_preparation() {
        let mut scratch = PreparedOlsScratch::create_row_major(11, 3).unwrap();
        let mut state = (usize::MAX, 0);
        scratch
            .rewrite_row_major_prepared_with(
                &mut state,
                4,
                |state, first, count| {
                    *state = (first, count);
                    Ok(())
                },
                |state, row, column| {
                    assert!(row >= state.0 && row < state.0 + state.1);
                    Ok((row * 3 + column) as f64)
                },
            )
            .unwrap();
        assert_eq!(scratch.encoded_tile_writes, 3);
        assert_eq!(scratch.encoded_tile_reads, 0);
        let mut row = [0.; 3];
        scratch.read_linear_shared(30, &mut row).unwrap();
        assert_eq!(row, [30., 31., 32.]);
        assert!(scratch
            .rewrite_row_major_prepared_with(
                &mut (),
                4,
                |_, first, _| if first == 4 {
                    Err(invalid("injected preparation refusal"))
                } else {
                    Ok(())
                },
                |_, _, _| Ok(0.)
            )
            .is_err());
        assert!(scratch.poisoned);
        assert!(scratch.read_linear_shared(0, &mut row).is_err());
    }

    #[test]
    fn retained_columns_survive_working_mutation_without_another_io_bank() {
        let n = IO_BUFFER_BYTES / 8 + 3;
        let values = (0..n).map(|row| row as f64 * 0.125).collect::<Vec<_>>();
        let paths;
        {
            let mut working = PreparedOlsScratch::create(n, 1).unwrap();
            let mut retained = PreparedOlsScratch::create_unbuffered(n, 1).unwrap();
            paths = [working.path.clone(), retained.path.clone()];
            let capacity = working.io_buffer_capacity();
            assert_eq!(retained.io_buffer_capacity(), 0);
            working.write_column(0, &values).unwrap();
            retained
                .write_column_with_borrowed_buffer(0, &values, &mut working)
                .unwrap();
            assert_eq!(working.io_buffer_capacity(), capacity);
            assert_eq!(retained.io_buffer_capacity(), 0);
            working.write_column(0, &vec![0.0; n]).unwrap();
            retained.take_io_buffer_from(&mut working).unwrap();
            assert_eq!(working.io_buffer_capacity(), 0);
            assert_eq!(retained.io_buffer_capacity(), capacity);
            let mut actual = vec![0.0; n];
            retained.read_column(0, &mut actual).unwrap();
            assert_eq!(actual, values);
            assert!(working.read_column(0, &mut actual).is_err());
        }
        assert!(paths.iter().all(|path| !path.exists()));
    }

    #[test]
    fn failed_retained_write_returns_the_working_buffer_and_removes_its_file() {
        let paths;
        {
            let mut working = PreparedOlsScratch::create(3, 1).unwrap();
            let mut retained = PreparedOlsScratch::create_unbuffered(3, 1).unwrap();
            paths = [working.path.clone(), retained.path.clone()];
            let capacity = working.io_buffer_capacity();
            let values = [1.0, -2.0, 3.0];
            working.write_column(0, &values).unwrap();
            retained.file = Some(File::open(&retained.path).unwrap());
            assert!(retained
                .write_column_with_borrowed_buffer(0, &values, &mut working)
                .is_err());
            assert_eq!(working.io_buffer_capacity(), capacity);
            assert_eq!(retained.io_buffer_capacity(), 0);
            let mut actual = [0.0; 3];
            working.read_column(0, &mut actual).unwrap();
            assert_eq!(actual, values);
        }
        assert!(paths.iter().all(|path| !path.exists()));
    }

    #[test]
    fn scratch_round_trips_columns_and_cleans_up() {
        let (path, bytes) = {
            let mut scratch = PreparedOlsScratch::create(4, 2).expect("scratch");
            let path = scratch.path.clone();
            scratch
                .write_column(0, &[1.0, -2.0, 3.5, 4.25])
                .expect("write first");
            scratch
                .write_column(1, &[8.0, 7.0, 6.0, 5.0])
                .expect("write second");
            let mut values = vec![0.0; 4];
            scratch.read_column(0, &mut values).expect("read first");
            assert_eq!(values, [1.0, -2.0, 3.5, 4.25]);
            scratch.read_column(1, &mut values).expect("read second");
            assert_eq!(values, [8.0, 7.0, 6.0, 5.0]);
            assert_eq!(scratch.rows(), 4);
            assert_eq!(scratch.columns(), 2);
            (path, scratch.bytes())
        };
        assert_eq!(bytes, 64);
        assert!(!path.exists());
    }

    #[test]
    fn scratch_rejects_dimension_and_nonfinite_errors() {
        let mut scratch = PreparedOlsScratch::create(2, 1).expect("scratch");
        assert_eq!(
            scratch
                .write_column(1, &[1.0, 2.0])
                .expect_err("column bound")
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            scratch.write_column(0, &[1.0]).expect_err("row bound").code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            scratch
                .write_column(0, &[1.0, f64::NAN])
                .expect_err("nonfinite")
                .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn scratch_shared_positional_reads_are_sync_and_cursor_independent() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PreparedOlsScratch>();

        let mut scratch = PreparedOlsScratch::create(4, 2).expect("scratch");
        scratch
            .write_linear(0, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])
            .expect("write rows");
        std::thread::scope(|scope| {
            for row in 0..4 {
                let scratch = &scratch;
                scope.spawn(move || {
                    let mut values = [0.0; 2];
                    scratch
                        .read_linear_shared(row * 2, &mut values)
                        .expect("shared row read");
                    assert_eq!(values, [2.0 * row as f64 + 1.0, 2.0 * row as f64 + 2.0]);
                });
            }
        });
        let mut first = [0.0; 2];
        scratch
            .read_linear_shared(0, &mut first)
            .expect("repeat shared read");
        assert_eq!(first, [1.0, 2.0]);
    }

    #[test]
    fn shared_reader_batches_narrow_and_wide_rows_without_changing_bits() {
        for columns in [1, 6, 1023, 1024, 1025, 2051] {
            let rows = 37;
            let input: Vec<_> = (0..rows * columns)
                .map(|i| {
                    if i % 19 == 0 {
                        -0.0
                    } else {
                        (i as f64 - 700.0) / 17.0
                    }
                })
                .collect();
            let mut scratch = PreparedOlsScratch::create_row_major(rows, columns).unwrap();
            scratch.write_linear(0, &input).unwrap();
            let before = scratch.shared_read_chunks_for_test();
            {
                let mut reader = scratch.shared_reader();
                let mut row = vec![0.0; columns];
                for observation in 0..rows {
                    reader.read_linear(observation * columns, &mut row).unwrap();
                    assert!(row
                        .iter()
                        .zip(&input[observation * columns..])
                        .all(|(actual, expected)| actual.to_bits() == expected.to_bits()));
                }
                assert_eq!(
                    scratch.shared_read_chunks_for_test() - before,
                    (input.len() as u64).div_ceil((SHARED_READ_BUFFER_BYTES / 8) as u64)
                );
                // Backward access and empty/out-of-range requests cannot
                // mislabel the cached page or change the shared file cursor.
                reader.read_linear(0, &mut row).unwrap();
                assert_eq!(row, input[..columns]);
                reader.read_linear(input.len(), &mut []).unwrap();
                assert!(reader.read_linear(usize::MAX, &mut row).is_err());
                reader.read_linear(columns, &mut row).unwrap();
                assert_eq!(row, input[columns..2 * columns]);
            }
            // A new scan sees writes made after the immutable borrow ends.
            scratch.write_linear(0, &[41.0]).unwrap();
            let mut value = [0.0];
            scratch.shared_reader().read_linear(0, &mut value).unwrap();
            assert_eq!(value, [41.0]);
        }
    }

    #[test]
    fn shared_reader_rejects_corrupt_bytes_and_partial_reads() {
        let mut scratch = PreparedOlsScratch::create_row_major(2049, 1).unwrap();
        scratch.write_linear(0, &vec![3.0; 2049]).unwrap();
        scratch.seek_element(1025).unwrap();
        scratch
            .file_mut()
            .write_all(&f64::NAN.to_le_bytes())
            .unwrap();
        {
            let mut reader = scratch.shared_reader();
            let mut value = [0.0];
            reader.read_linear(1024, &mut value).unwrap();
            assert_eq!(value, [3.0]);
            assert!(reader.read_linear(1025, &mut value).is_err());
        }
        scratch.file_ref().set_len(1024 * 8 + 1).unwrap();
        let mut reader = scratch.shared_reader();
        let mut value = [0.0];
        reader.read_linear(0, &mut value).unwrap();
        assert!(reader.read_linear(1024, &mut value).is_err());
        assert_eq!(reader.elements, 0);
        assert!(reader.read_linear(1024, &mut value).is_err());
    }

    #[test]
    fn shared_readers_have_independent_worker_pages() {
        let mut scratch = PreparedOlsScratch::create_row_major(2051, 6).unwrap();
        let input: Vec<_> = (0..2051 * 6).map(|i| i as f64 / 7.0).collect();
        scratch.write_linear(0, &input).unwrap();
        std::thread::scope(|scope| {
            for start in [0, 513, 1026, 1539] {
                let scratch = &scratch;
                let input = &input;
                scope.spawn(move || {
                    let mut reader = scratch.shared_reader();
                    let mut row = [0.0; 6];
                    for observation in start..(start + 513).min(2051) {
                        reader.read_linear(observation * 6, &mut row).unwrap();
                        assert_eq!(row, input[observation * 6..(observation + 1) * 6]);
                    }
                });
            }
        });
    }

    #[test]
    fn row_major_blocks_patch_extract_and_share_selected_rows() {
        let rows = 5;
        let columns = 5;
        let mut scratch = PreparedOlsScratch::create_row_major(rows, columns).expect("scratch");
        assert_eq!(scratch.layout(), ScratchLayout::RowMajor);
        let initial = (0..rows * columns)
            .map(|position| position as f64 + 0.25)
            .collect::<Vec<_>>();
        scratch.write_linear(0, &initial).expect("initialize rows");

        // Force more than one full-row tile without making the test file large.
        scratch.io_buffer.truncate(2 * columns * 8);
        let patch = vec![
            101.0, 102.0, 201.0, 202.0, 301.0, 302.0, 401.0, 402.0, 501.0, 502.0,
        ];
        scratch
            .patch_row_major_block(1, 2, &patch)
            .expect("patch block");

        let mut extracted = vec![0.0; rows * 3];
        scratch
            .read_row_major_block(1, 3, &mut extracted)
            .expect("extract block");
        assert_eq!(
            extracted,
            vec![
                101.0, 102.0, 3.25, 201.0, 202.0, 8.25, 301.0, 302.0, 13.25, 401.0, 402.0, 18.25,
                501.0, 502.0, 23.25,
            ]
        );

        std::thread::scope(|scope| {
            for row in 0..rows {
                let scratch = &scratch;
                let initial = &initial;
                let patch = &patch;
                scope.spawn(move || {
                    let mut selected = [0.0; 3];
                    scratch
                        .read_selected_row_shared(row, &[0, 2, 4], &mut selected)
                        .expect("selected row");
                    assert_eq!(
                        selected,
                        [
                            initial[row * columns],
                            patch[row * 2 + 1],
                            initial[row * columns + 4],
                        ]
                    );
                });
            }
        });

        let mut expected = vec![0.0; rows * columns];
        scratch
            .read_linear(0, &mut expected)
            .expect("snapshot before rejected patch");
        let mut nonfinite = patch.clone();
        nonfinite[4] = f64::NAN;
        assert_eq!(
            scratch
                .patch_row_major_block(1, 2, &nonfinite)
                .expect_err("nonfinite patch")
                .code,
            ErrorCode::InvalidData
        );
        let mut actual = vec![0.0; rows * columns];
        scratch
            .read_linear(0, &mut actual)
            .expect("snapshot after rejected patch");
        assert_eq!(actual, expected);
    }

    #[test]
    fn repeated_column_block_transport_preserves_bits_and_bounds_io_calls() {
        for (columns, pool, rows) in [
            (1_usize, 1_usize, 2051_usize),
            (5, 2, 419),
            (18, 4, 65_536),
            (18, 4, 131_072),
            (129, 7, 2051),
        ] {
            let mut expected = (0..rows * columns)
                .map(|i| match i % 7 {
                    0 => -0.0,
                    1 => 2.0_f64.powi(200),
                    2 => -2.0_f64.powi(-200),
                    _ => ((i % 41) as f64 - 20.0) / 16.0,
                })
                .collect::<Vec<_>>();
            let mut scratch = PreparedOlsScratch::create_row_major(rows, columns).unwrap();
            scratch.write_linear(0, &expected).unwrap();
            let identity = scratch.identity_path().to_owned();
            let capacity = scratch.io_buffer_capacity();
            let tile_count = rows.div_ceil(capacity / (columns * 8));
            let block_count = columns.div_ceil(pool);
            let mut block = vec![0.0; rows * pool];
            let mut actual = vec![0.0; rows * columns];
            for _frame in 0..2 {
                let reads_before = scratch.encoded_tile_reads;
                let writes_before = scratch.encoded_tile_writes;
                for start in (0..columns).step_by(pool) {
                    let width = (columns - start).min(pool);
                    let block = &mut block[..rows * width];
                    scratch.read_row_major_block(start, width, block).unwrap();
                    for row in 0..rows {
                        for local in 0..width {
                            let source = row * columns + start + local;
                            let slot = row * width + local;
                            assert_eq!(block[slot].to_bits(), expected[source].to_bits());
                            block[slot] = -block[slot];
                            expected[source] = -expected[source];
                        }
                    }
                    scratch.patch_row_major_block(start, width, block).unwrap();
                }
                assert_eq!(
                    scratch.encoded_tile_reads - reads_before,
                    (2 * block_count * tile_count) as u64
                );
                assert_eq!(
                    scratch.encoded_tile_writes - writes_before,
                    (block_count * tile_count) as u64
                );
                assert_eq!(scratch.io_buffer_capacity(), capacity);
                assert_eq!(scratch.identity_path(), identity);
                assert_eq!(scratch.bytes(), (rows * columns * 8) as u64);
                scratch.read_linear(0, &mut actual).unwrap();
                assert!(actual
                    .iter()
                    .zip(&expected)
                    .all(|(a, b)| a.to_bits() == b.to_bits()));
                if columns == 18 {
                    assert!(tile_count * 1000 < rows);
                }
            }
        }
    }
    #[test]
    fn scratch_layout_keeps_column_and_row_major_apis_disjoint() {
        let mut columns = PreparedOlsScratch::create(3, 2).expect("column scratch");
        assert_eq!(columns.layout(), ScratchLayout::ColumnMajor);
        columns
            .write_column(0, &[1.0, 2.0, 3.0])
            .expect("column write");
        assert_eq!(
            columns
                .patch_row_major_block(0, 1, &[1.0, 2.0, 3.0])
                .expect_err("row patch on column layout")
                .code,
            ErrorCode::InvalidData
        );

        let mut rows = PreparedOlsScratch::create_row_major(3, 2).expect("row scratch");
        rows.write_linear(0, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
            .expect("linear write remains layout neutral");
        assert_eq!(
            rows.write_column(0, &[1.0, 3.0, 5.0])
                .expect_err("column write on row layout")
                .code,
            ErrorCode::InvalidData
        );
        let mut selected = [0.0, 0.0];
        rows.read_selected_row_shared(1, &[0, 1], &mut selected)
            .expect("row read");
        assert_eq!(selected, [3.0, 4.0]);
    }

    #[test]
    fn row_major_compaction_is_forward_in_place_and_updates_metadata() {
        let rows = 4;
        let columns = 5;
        let mut scratch = PreparedOlsScratch::create_row_major(rows, columns).expect("scratch");
        let values = vec![
            10.0, 11.0, 12.0, 13.0, 14.0, 20.0, 21.0, 22.0, 23.0, 24.0, 30.0, 31.0, 32.0, 33.0,
            34.0, 40.0, 41.0, 42.0, 43.0, 44.0,
        ];
        scratch.write_linear(0, &values).expect("initialize rows");
        scratch.io_buffer.truncate(2 * columns * 8);
        let path = scratch.identity_path().to_path_buf();

        scratch
            .compact_row_major_columns_forward(&[0, 2, 4])
            .expect("compact");
        assert_eq!(scratch.identity_path(), path);
        assert!(path.exists());
        assert_eq!(scratch.layout(), ScratchLayout::RowMajor);
        assert_eq!(scratch.rows(), rows);
        assert_eq!(scratch.columns(), 3);
        assert_eq!(scratch.bytes(), (rows * 3 * 8) as u64);
        assert_eq!(
            std::fs::metadata(&path).expect("scratch metadata").len(),
            scratch.bytes()
        );

        let mut compacted = vec![0.0; rows * 3];
        scratch
            .read_linear(0, &mut compacted)
            .expect("read compact rows");
        assert_eq!(
            compacted,
            vec![10.0, 12.0, 14.0, 20.0, 22.0, 24.0, 30.0, 32.0, 34.0, 40.0, 42.0, 44.0,]
        );
    }

    #[test]
    fn row_major_compaction_failure_leaves_path_and_metadata_unchanged() {
        let rows = 3;
        let columns = 4;
        let values = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        ];
        let mut scratch = PreparedOlsScratch::create_row_major(rows, columns).expect("scratch");
        scratch.write_linear(0, &values).expect("initialize rows");
        let path = scratch.identity_path().to_path_buf();
        let bytes = scratch.bytes();

        for invalid_selection in [&[2, 1][..], &[1, 1][..], &[0, 4][..]] {
            assert_eq!(
                scratch
                    .compact_row_major_columns_forward(invalid_selection)
                    .expect_err("invalid selection")
                    .code,
                ErrorCode::InvalidData
            );
            assert_eq!(scratch.columns(), columns);
            assert_eq!(scratch.bytes(), bytes);
            assert_eq!(scratch.identity_path(), path);
            assert_eq!(std::fs::metadata(&path).expect("metadata").len(), bytes);
        }

        // A full row must fit the fixed I/O tile. This failure occurs before
        // any file write or metadata mutation.
        scratch.io_buffer.truncate(columns * 8 - 1);
        assert_eq!(
            scratch
                .compact_row_major_columns_forward(&[0, 2])
                .expect_err("undersized tile")
                .code,
            ErrorCode::Allocation
        );
        assert_eq!(scratch.columns(), columns);
        assert_eq!(scratch.bytes(), bytes);
        assert_eq!(scratch.identity_path(), path);
        assert_eq!(std::fs::metadata(&path).expect("metadata").len(), bytes);

        scratch.io_buffer.resize(columns * 8, 0);
        let mut actual = vec![0.0; values.len()];
        scratch
            .read_linear(0, &mut actual)
            .expect("read after failed compaction");
        assert_eq!(actual, values);
    }

    #[test]
    fn row_major_compaction_supports_an_empty_selected_basis() {
        let mut scratch = PreparedOlsScratch::create_row_major(2, 2).expect("scratch");
        scratch
            .write_linear(0, &[1.0, 2.0, 3.0, 4.0])
            .expect("initialize rows");
        let path = scratch.identity_path().to_path_buf();
        scratch
            .compact_row_major_columns_forward(&[])
            .expect("compact to rank zero");
        assert_eq!(scratch.columns(), 0);
        assert_eq!(scratch.bytes(), 0);
        assert_eq!(std::fs::metadata(&path).expect("metadata").len(), 0);
        let mut empty = [];
        scratch
            .read_selected_row_shared(1, &[], &mut empty)
            .expect("empty selected row");
    }

    #[test]
    fn row_major_creation_reserves_at_least_one_very_wide_row() {
        let columns = IO_BUFFER_BYTES / 8 + 1;
        let scratch = PreparedOlsScratch::create_row_major(1, columns).expect("wide scratch");
        assert!(scratch.io_buffer_capacity() >= columns * 8);
        assert_eq!(scratch.rows(), 1);
        assert_eq!(scratch.columns(), columns);
        assert_eq!(scratch.bytes(), (columns * 8) as u64);
    }

    #[test]
    fn row_major_io_failure_poisons_storage_without_publishing_new_metadata() {
        let rows = 3;
        let columns = 4;
        let values = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        ];
        let mut scratch = PreparedOlsScratch::create_row_major(rows, columns).expect("scratch");
        scratch.write_linear(0, &values).expect("initialize rows");
        let path = scratch.identity_path().to_path_buf();
        let bytes = scratch.bytes();

        drop(scratch.file.take());
        scratch.file = Some(
            OpenOptions::new()
                .read(true)
                .open(&path)
                .expect("reopen scratch read-only"),
        );
        assert_eq!(
            scratch
                .compact_row_major_columns_forward(&[0, 2])
                .expect_err("read-only compact write")
                .code,
            ErrorCode::Allocation
        );
        assert!(scratch.poisoned);
        assert_eq!(scratch.columns(), columns);
        assert_eq!(scratch.bytes(), bytes);
        assert_eq!(scratch.identity_path(), path);
        assert_eq!(std::fs::metadata(&path).expect("metadata").len(), bytes);

        let mut output = vec![0.0; values.len()];
        assert_eq!(
            scratch
                .read_linear(0, &mut output)
                .expect_err("poisoned linear read")
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            scratch
                .patch_row_major_block(0, 1, &[1.0, 2.0, 3.0])
                .expect_err("poisoned patch")
                .code,
            ErrorCode::InvalidData
        );
    }
}
