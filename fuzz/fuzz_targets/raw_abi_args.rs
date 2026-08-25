#![no_main]

use fereg_plugin::{dispatch_args, BufferSink};
use libfuzzer_sys::fuzz_target;

mod support;

fuzz_target!(|data: &[u8]| {
    let mut args = support::strings(data, 32, 256);
    if args.first().is_some_and(|action| action == "panic-test") {
        args[0] = "panic-test-disabled".to_owned();
    }
    let _ = dispatch_args(&args, &BufferSink::default());
});
