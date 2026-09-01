//! Bounded, column-major scratch storage for low-memory prepared OLS.
//!
//! The file is private to one fit, created with `create_new`, and removed by
//! `Drop` on success, ordinary error, panic unwinding, or caller cancellation.

use std::fs::{remove_file, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{ErrorCode, FeregError, Result};

const IO_BUFFER_BYTES: usize = 1 << 20;
const SHARED_READ_BUFFER_BYTES: usize = 8 << 10;
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) struct PreparedOlsScratch {
    file: Option<File>,
    path: PathBuf,
    rows: usize,
    columns: usize,
    bytes: u64,
    io_buffer: Vec<u8>,
}

impl PreparedOlsScratch {
    pub(crate) fn create(rows: usize, columns: usize) -> Result<Self> {
        let elements = rows
            .checked_mul(columns)
            .ok_or_else(|| allocation("prepared OLS scratch dimensions overflow usize"))?;
        let bytes = u64::try_from(elements)
            .ok()
            .and_then(|value| value.checked_mul(8))
            .ok_or_else(|| allocation("prepared OLS scratch size overflows u64"))?;
        let mut io_buffer = Vec::new();
        let addressable_bytes = usize::try_from(bytes).unwrap_or(usize::MAX);
        let buffer_bytes = IO_BUFFER_BYTES.min(addressable_bytes).max(8);
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
                        io_buffer,
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

    pub(crate) fn io_buffer_capacity(&self) -> usize {
        self.io_buffer.capacity()
    }

    #[cfg(test)]
    pub(crate) fn identity_path(&self) -> &std::path::Path {
        &self.path
    }

    pub(crate) fn write_column(&mut self, column: usize, values: &[f64]) -> Result<()> {
        self.validate_column(column, values.len())?;
        let offset = self.column_element_offset(column, 0)?;
        self.write_values_at(offset, values)
    }

    pub(crate) fn read_column(&mut self, column: usize, values: &mut [f64]) -> Result<()> {
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
        let end_row = start_row
            .checked_add(values.len())
            .ok_or_else(|| allocation("prepared OLS scratch row range overflows"))?;
        if column >= self.columns || end_row > self.rows {
            return Err(invalid("prepared OLS scratch row range is out of bounds"));
        }
        let offset = self.column_element_offset(column, start_row)?;
        self.read_values_at(offset, values)
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

    fn write_values_at(&mut self, offset: usize, values: &[f64]) -> Result<()> {
        self.seek_element(offset)?;
        let values_per_chunk = self.io_buffer.len() / 8;
        for chunk in values.chunks(values_per_chunk) {
            let byte_count = chunk
                .len()
                .checked_mul(8)
                .ok_or_else(|| allocation("prepared OLS scratch write size overflows"))?;
            for (value, bytes) in chunk
                .iter()
                .zip(self.io_buffer[..byte_count].chunks_exact_mut(8))
            {
                if !value.is_finite() {
                    return Err(invalid(
                        "prepared OLS scratch write received a nonfinite value",
                    ));
                }
                bytes.copy_from_slice(&value.to_le_bytes());
            }
            let file = self
                .file
                .as_mut()
                .expect("prepared OLS scratch file is open before Drop");
            file.write_all(&self.io_buffer[..byte_count])
                .map_err(|error| io_error("unable to write prepared OLS scratch column", error))?;
        }
        Ok(())
    }

    fn read_values_at(&mut self, offset: usize, values: &mut [f64]) -> Result<()> {
        self.seek_element(offset)?;
        let values_per_chunk = self.io_buffer.len() / 8;
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

    fn validate_linear_range(&self, offset: usize, values: usize) -> Result<()> {
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
}
