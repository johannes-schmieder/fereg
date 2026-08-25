#![no_main]

use fereg_core::protocol::{parse_key_values, Handshake};
use libfuzzer_sys::fuzz_target;

mod support;

fuzz_target!(|data: &[u8]| {
    let args = support::strings(data, 64, 256);
    if let Ok(values) = parse_key_values(&args) {
        if let Ok(handshake) = Handshake::from_key_values(&values) {
            let _ = handshake.validate();
        }
    }
});
