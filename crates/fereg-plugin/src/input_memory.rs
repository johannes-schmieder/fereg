//! Admission of simultaneously live input arrays and string dictionaries.
//!
//! Dictionary storage is explicit: byte keys, an entry vector and a u32 bucket
//! vector. This avoids depending on a private standard-library hash-table
//! layout to admit old/new tables during growth.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::mem::size_of;

use fereg_core::error::{ErrorCode, FeregError, Result};

#[derive(Debug, Clone, Copy)]
pub(crate) struct InputMemory {
    usable: Option<u128>,
    resident: u128,
    peak: u128,
}

impl InputMemory {
    pub(crate) fn new(usable: Option<u128>, parents_and_arrays: u128) -> Result<Self> {
        let mut result = Self {
            usable,
            resident: parents_and_arrays,
            peak: 0,
        };
        result.admit(0)?;
        Ok(result)
    }

    pub(crate) fn admit(&mut self, temporary: u128) -> Result<()> {
        let peak = add(self.resident, temporary)?;
        if self.usable.is_some_and(|usable| peak > usable) {
            return Err(allocation("input construction exceeds usable memory"));
        }
        self.peak = self.peak.max(peak);
        Ok(())
    }

    pub(crate) fn retain(&mut self, bytes: u128) -> Result<()> {
        self.admit(bytes)?;
        self.resident = add(self.resident, bytes)?;
        Ok(())
    }

    fn release(&mut self, bytes: u128) -> Result<()> {
        self.resident = self
            .resident
            .checked_sub(bytes)
            .ok_or_else(|| allocation("input ownership accounting underflows"))?;
        Ok(())
    }

    pub(crate) fn remaining(&self) -> usize {
        self.usable.map_or(usize::MAX, |usable| {
            usize::try_from(usable.saturating_sub(self.resident)).unwrap_or(usize::MAX)
        })
    }

    pub(crate) fn peak(self) -> u128 {
        self.peak
    }
}

#[derive(Debug)]
struct Entry {
    bytes: Vec<u8>,
    hash: u64,
    code: u32,
}

#[derive(Debug)]
pub(crate) struct StringDictionary {
    entries: Vec<Entry>,
    buckets: Vec<u32>,
    hash: RandomState,
    key_bytes: u128,
}

impl StringDictionary {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            buckets: Vec::new(),
            hash: RandomState::new(),
            key_bytes: 0,
        }
    }

    fn storage_bytes(&self) -> u128 {
        self.entries.capacity() as u128 * size_of::<Entry>() as u128
            + self.buckets.capacity() as u128 * size_of::<u32>() as u128
            + self.key_bytes
    }

    pub(crate) fn encode(&mut self, bytes: Vec<u8>, memory: &mut InputMemory) -> Result<u32> {
        // The SPI has already admitted this single buffer before allocating it.
        // It remains temporary unless this is a new level.
        memory.admit(bytes.capacity() as u128)?;
        let hash = self.hash.hash_one(bytes.as_slice());
        if !self.buckets.is_empty() {
            let mut slot = hash as usize & (self.buckets.len() - 1);
            while self.buckets[slot] != u32::MAX {
                let entry = &self.entries[self.buckets[slot] as usize];
                if entry.hash == hash && entry.bytes == bytes {
                    return Ok(entry.code);
                }
                slot = (slot + 1) & (self.buckets.len() - 1);
            }
        }
        let code = u32::try_from(self.entries.len())
            .ok()
            .filter(|value| *value != u32::MAX)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Unsupported,
                    "fixed-width string source has more than u32::MAX levels",
                )
            })?;
        let required = self.entries.len().checked_add(1).ok_or_else(overflow)?;
        let grow_buckets = required > self.buckets.len() / 4 * 3;
        let buckets = if grow_buckets {
            self.buckets
                .len()
                .checked_mul(2)
                .ok_or_else(overflow)?
                .max(8)
        } else {
            0
        };
        let entries = if required > self.entries.capacity() {
            self.entries
                .capacity()
                .checked_mul(2)
                .ok_or_else(overflow)?
                .max(4)
        } else {
            0
        };
        // Reserve complete new banks beside both old banks and the incoming
        // key. Reallocation is allowed to keep old and new storage live.
        let growth = add(
            mul(buckets, size_of::<u32>())?,
            mul(entries, size_of::<Entry>())?,
        )?;
        memory.admit(add(growth, bytes.capacity() as u128)?)?;
        let previous = self.storage_bytes();
        if entries != 0 {
            self.entries
                .try_reserve_exact(entries - self.entries.len())
                .map_err(|_| allocation("unable to reserve string dictionary entries"))?;
            if self.entries.capacity() != entries {
                return Err(allocation("unexpected string entry capacity"));
            }
        }
        if buckets != 0 {
            let mut next = Vec::new();
            next.try_reserve_exact(buckets)
                .map_err(|_| allocation("unable to reserve string dictionary buckets"))?;
            if next.capacity() != buckets {
                return Err(allocation("unexpected string bucket capacity"));
            }
            next.resize(buckets, u32::MAX);
            for (index, entry) in self.entries.iter().enumerate() {
                let mut slot = entry.hash as usize & (buckets - 1);
                while next[slot] != u32::MAX {
                    slot = (slot + 1) & (buckets - 1);
                }
                next[slot] = index as u32;
            }
            self.buckets = next;
        }
        let mut slot = hash as usize & (self.buckets.len() - 1);
        while self.buckets[slot] != u32::MAX {
            slot = (slot + 1) & (self.buckets.len() - 1);
        }
        self.buckets[slot] = code;
        self.key_bytes = add(self.key_bytes, bytes.capacity() as u128)?;
        self.entries.push(Entry { bytes, hash, code });
        memory.retain(self.storage_bytes() - previous)?;
        Ok(code)
    }

    pub(crate) fn recode(
        mut self,
        values: &mut [f64],
        missing: &[bool],
        memory: &mut InputMemory,
    ) -> Result<()> {
        if values.len() != missing.len() {
            return Err(allocation("string recode shape mismatch"));
        }
        let previous = self.storage_bytes();
        let bucket_bytes = self.buckets.capacity() as u128 * size_of::<u32>() as u128;
        drop(std::mem::take(&mut self.buckets));
        memory.release(bucket_bytes)?;
        // In-place unstable sorting needs no heap ordering descriptors and
        // retains each original code, so encounter order cannot affect codes.
        self.entries
            .sort_unstable_by(|left, right| left.bytes.cmp(&right.bytes));
        memory.admit(mul(self.entries.len(), size_of::<u32>())?)?;
        let mut recode = Vec::new();
        recode
            .try_reserve_exact(self.entries.len())
            .map_err(|_| allocation("unable to reserve string deterministic recode"))?;
        recode.resize(self.entries.len(), 0_u32);
        for (sorted, entry) in self.entries.iter().enumerate() {
            recode[entry.code as usize] = sorted as u32;
        }
        for (value, missing) in values.iter_mut().zip(missing) {
            if !missing {
                *value = f64::from(
                    *recode
                        .get(*value as usize)
                        .ok_or_else(|| allocation("string provisional code is out of range"))?,
                );
            }
        }
        drop(recode);
        drop(self);
        memory.release(previous - bucket_bytes)
    }
}

pub(crate) fn add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or_else(overflow)
}
pub(crate) fn mul(a: usize, b: usize) -> Result<u128> {
    (a as u128).checked_mul(b as u128).ok_or_else(overflow)
}
fn overflow() -> FeregError {
    allocation("input construction memory size overflows")
}
fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "ffi")]
    #[ignore = "allocator audit; run individually with --test-threads=1"]
    fn simultaneous_dictionaries_bound_independent_allocator_peak() {
        let _guard = crate::instrumentation::lock_action_observation();
        let mut values: [Vec<f64>; 3] = std::array::from_fn(|_| vec![0.0; 257]);
        let missing = vec![false; 257];
        for _ in 0..3 {
            // Dictionary construction is serial. Libtest's separate main thread
            // can lazily allocate its result-channel context during this window.
            let start = crate::ffi::thread_allocations::begin();
            let mut dictionaries: [StringDictionary; 3] =
                std::array::from_fn(|_| StringDictionary::new());
            let mut memory = InputMemory::new(None, 0).unwrap();
            for i in 0..257_u32 {
                for (dictionary, values) in dictionaries.iter_mut().zip(&mut values) {
                    values[i as usize] = f64::from(
                        dictionary
                            .encode(i.to_be_bytes().to_vec(), &mut memory)
                            .unwrap(),
                    );
                }
                let actual = crate::ffi::thread_allocations::usage();
                assert_eq!(
                    actual.current_bytes - start.current_bytes,
                    memory.resident as u64
                );
                assert!(actual.peak_bytes - start.current_bytes <= memory.peak() as u64);
            }
            for (dictionary, values) in dictionaries.into_iter().zip(&mut values) {
                dictionary.recode(values, &missing, &mut memory).unwrap();
            }
            let actual = crate::ffi::thread_allocations::usage();
            assert_eq!(actual.current_bytes, start.current_bytes);
            assert!(actual.peak_bytes - start.current_bytes <= memory.peak() as u64);
        }
    }

    #[test]
    fn independent_sorted_oracle_and_repeated_growth() {
        for count in [0, 1, 3, 4, 7, 64, 513] {
            let mut dictionary = StringDictionary::new();
            let mut memory = InputMemory::new(None, 123).unwrap();
            let strings: Vec<Vec<u8>> = (0..count)
                .map(|i| format!("key-{}#é", (i * 71) % 97).into_bytes())
                .collect();
            let mut values: Vec<f64> = strings
                .iter()
                .map(|key| f64::from(dictionary.encode(key.clone(), &mut memory).unwrap()))
                .collect();
            let missing = vec![false; count];
            let mut levels = strings.clone();
            levels.sort();
            levels.dedup();
            let expected: Vec<f64> = strings
                .iter()
                .map(|key| levels.binary_search(key).unwrap() as f64)
                .collect();
            dictionary
                .recode(&mut values, &missing, &mut memory)
                .unwrap();
            assert_eq!(values, expected);
            assert_eq!(memory.resident, 123);
        }
    }

    #[test]
    fn exact_and_one_byte_short_growth_admission_is_transactional() {
        let key = b"first".to_vec();
        let required = size_of::<[u32; 8]>() + 4 * size_of::<Entry>() + key.capacity();
        for budget in [required - 1, required] {
            let mut memory = InputMemory::new(Some(budget as u128), 0).unwrap();
            let mut dictionary = StringDictionary::new();
            let result = dictionary.encode(key.clone(), &mut memory);
            if budget == required {
                assert_eq!(result.unwrap(), 0);
                assert_eq!(memory.peak(), required as u128);
            } else {
                assert_eq!(result.unwrap_err().code, ErrorCode::Allocation);
                assert_eq!(dictionary.storage_bytes(), 0);
                assert_eq!(memory.resident, 0);
            }
        }
    }

    #[test]
    fn budgets_cover_growth_of_multiple_live_dictionaries() {
        fn run(limit: Option<u128>, count: u32) -> Result<u128> {
            let mut memory = InputMemory::new(limit, 100)?;
            let mut tables = [StringDictionary::new(), StringDictionary::new()];
            for value in 0..count {
                for table in &mut tables {
                    table.encode(value.to_be_bytes().to_vec(), &mut memory)?;
                }
            }
            Ok(memory.peak())
        }
        for count in [1, 4, 7, 13, 25, 49, 97, 513] {
            let peak = run(None, count).unwrap();
            assert_eq!(run(Some(peak), count).unwrap(), peak);
            assert_eq!(
                run(Some(peak - 1), count).unwrap_err().code,
                ErrorCode::Allocation
            );
        }
    }
}
