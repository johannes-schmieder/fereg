#![no_main]

use fereg_core::{
    ModelWireV2, NumericWireTable, ABSORB_COMPONENT_ROW_WIDTH, ABSORB_TERM_ROW_WIDTH,
    COMPONENT_ROW_WIDTH, SOURCE_ROW_WIDTH, TERM_ROW_WIDTH,
};
use libfuzzer_sys::fuzz_target;

mod support;
use support::Cursor;

fn table(cursor: &mut Cursor<'_>, canonical_width: usize) -> NumericWireTable {
    let row_count = usize::from(cursor.byte() % 9);
    let width = if cursor.byte() & 1 == 0 {
        canonical_width
    } else {
        usize::from(cursor.byte()) % (canonical_width + 3)
    };
    let rows = (0..row_count)
        .map(|_| (0..width).map(|_| cursor.f64()).collect())
        .collect();
    NumericWireTable {
        declared_count: cursor.f64(),
        rows,
    }
}

fn registry(cursor: &mut Cursor<'_>) -> Vec<String> {
    (0..usize::from(cursor.byte() % 13))
        .map(|_| cursor.string(64))
        .collect()
}

fuzz_target!(|data: &[u8]| {
    let mut cursor = Cursor::new(data);
    let wire = ModelWireV2 {
        major: cursor.f64(),
        minor: cursor.f64(),
        sources: table(&mut cursor, SOURCE_ROW_WIDTH),
        terms: table(&mut cursor, TERM_ROW_WIDTH),
        components: table(&mut cursor, COMPONENT_ROW_WIDTH),
        source_names: registry(&mut cursor),
        canonical_tokens: registry(&mut cursor),
        raw_component_operators: registry(&mut cursor),
        raw_time_series_operators: registry(&mut cursor),
        absorb_terms: table(&mut cursor, ABSORB_TERM_ROW_WIDTH),
        absorb_components: table(&mut cursor, ABSORB_COMPONENT_ROW_WIDTH),
        absorb_labels: registry(&mut cursor),
        absorb_saved_names: registry(&mut cursor),
    };
    let _ = wire.decode_specification();
});
