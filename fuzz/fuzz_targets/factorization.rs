#![no_main]

use fereg_core::factorize_f64;
use libfuzzer_sys::fuzz_target;

mod support;
use support::Cursor;

// factorize_f64 has three paths: ordered runs (at least 16,384 sorted values),
// dense integers and the hashed fallback. The selector byte steers the input
// shape toward each one; every path must agree on canonical codes.
fuzz_target!(|data: &[u8]| {
    let mut cursor = Cursor::new(data);
    let selector = cursor.byte() % 3;
    let count = match selector {
        0 => 16_384 + usize::from(cursor.byte()),
        _ => usize::from(cursor.byte()).min(128),
    };
    let values: Vec<f64> = match selector {
        0 => {
            let mut sorted: Vec<f64> = (0..count)
                .map(|_| f64::from(cursor.byte() % 64) * 0.5)
                .collect();
            sorted.sort_by(f64::total_cmp);
            sorted
        }
        1 => (0..count)
            .map(|_| f64::from(cursor.byte()) - 64.0)
            .collect(),
        _ => (0..count).map(|_| cursor.f64()).collect(),
    };
    if let Ok(encoded) = factorize_f64(&values) {
        assert_eq!(encoded.codes.len(), values.len());
        for (value, &code) in values.iter().zip(&encoded.codes) {
            let level = encoded.levels[code as usize];
            assert!(level == *value || (level == 0.0 && *value == 0.0));
        }
        assert!(encoded.levels.windows(2).all(|pair| pair[0] < pair[1]));
    }
});
