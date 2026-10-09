#![no_main]

use fereg_core::{factorize_code_interaction, CategoricalCodeView};
use libfuzzer_sys::fuzz_target;

mod support;
use support::Cursor;

// Production categorical interactions: collision-free tuple codes with a
// declared level count per component, including mismatched lengths and codes
// outside the declared levels (which must fail closed).
fuzz_target!(|data: &[u8]| {
    let mut cursor = Cursor::new(data);
    let source_count = usize::from(cursor.byte() % 9);
    let row_count = usize::from(cursor.byte()).min(128);
    let mut sources = Vec::with_capacity(source_count);
    for source in 0..source_count {
        let mismatch = usize::from(cursor.byte() & 1) * usize::from(source > 0);
        let levels = usize::from(cursor.byte() % 16) + 1;
        let rows = row_count.saturating_sub(mismatch);
        let codes: Vec<u32> = (0..rows).map(|_| u32::from(cursor.byte() % 20)).collect();
        sources.push((codes, levels));
    }
    let views: Vec<_> = sources
        .iter()
        .map(|(codes, levels)| CategoricalCodeView::new(codes, *levels))
        .collect();
    if let Ok(encoded) = factorize_code_interaction(&views) {
        assert_eq!(encoded.codes().len(), row_count);
        assert!(encoded
            .codes()
            .iter()
            .all(|&code| (code as usize) < encoded.n_levels()));
    }
});
