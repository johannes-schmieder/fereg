#![no_main]

use fereg_core::{factorize_f64, factorize_i64, factorize_strings};
use libfuzzer_sys::fuzz_target;

mod support;
use support::Cursor;

fuzz_target!(|data: &[u8]| {
    let mut cursor = Cursor::new(data);
    let count = usize::from(cursor.byte()).min(128);
    let integers: Vec<i64> = (0..count).map(|_| cursor.i64()).collect();
    let floats: Vec<f64> = (0..count).map(|_| cursor.f64()).collect();
    let strings: Vec<String> = (0..count).map(|_| cursor.string(64)).collect();
    let _ = factorize_i64(&integers);
    let _ = factorize_f64(&floats);
    let _ = factorize_strings(&strings);
});
