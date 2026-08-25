#![no_main]

use fereg_core::interaction_codes;
use libfuzzer_sys::fuzz_target;

mod support;
use support::Cursor;

fuzz_target!(|data: &[u8]| {
    let mut cursor = Cursor::new(data);
    let source_count = usize::from(cursor.byte() % 9);
    let row_count = usize::from(cursor.byte()).min(128);
    let mut sources = Vec::with_capacity(source_count);
    for source in 0..source_count {
        let mismatch = usize::from(cursor.byte() & 1) * usize::from(source > 0);
        let rows = row_count.saturating_sub(mismatch);
        sources.push((0..rows).map(|_| cursor.u32()).collect::<Vec<_>>());
    }
    let borrowed = sources.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let _ = interaction_codes(&borrowed);
});
