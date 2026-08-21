use crate::dispatch::dispatch_args;
use crate::output::OutputSink;
use std::any::Any;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::panic::{catch_unwind, AssertUnwindSafe};
use stfixest_core::error::{ErrorCode, StFixestError};

#[cfg(not(test))]
extern "C" {
    fn stfixest_spi_display(message: *const c_char) -> c_int;
    fn stfixest_spi_error(message: *const c_char) -> c_int;
}

#[cfg(test)]
unsafe fn stfixest_spi_display(_message: *const c_char) -> c_int {
    0
}

#[cfg(test)]
unsafe fn stfixest_spi_error(_message: *const c_char) -> c_int {
    0
}

struct SpiSink;

impl OutputSink for SpiSink {
    fn display(&self, message: &str) {
        let sanitized = sanitize_c_string(message);
        // SAFETY: `sanitized` is a live NUL-terminated C string for the duration
        // of the call. The C bridge executes this SPI call on Stata's calling
        // thread and does not retain the pointer.
        unsafe {
            let _ = stfixest_spi_display(sanitized.as_ptr());
        }
    }

    fn error(&self, message: &str) {
        let sanitized = sanitize_c_string(message);
        // SAFETY: same lifetime and thread argument as `display` above.
        unsafe {
            let _ = stfixest_spi_error(sanitized.as_ptr());
        }
    }
}

/// Dispatch one Stata plugin action across the C ABI.
///
/// # Safety
///
/// When `argc` is positive, `argv` must point to an array of at least `argc`
/// non-null pointers to NUL-terminated byte strings. The pointers and strings
/// must remain valid for the duration of the call. Stata must have initialized
/// the C SPI bridge with `pginit` before actions that emit output.
#[no_mangle]
pub unsafe extern "C" fn stfixest_dispatch(argc: c_int, argv: *const *const c_char) -> c_int {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the exported ABI validates argc/argv before dereferencing.
        let args = unsafe { read_arguments(argc, argv) }?;
        let sink = SpiSink;
        dispatch_args(&args, &sink)
    }));

    match outcome {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            report_error(&error);
            error.return_code()
        }
        Err(payload) => {
            let message = panic_message(payload.as_ref());
            let error = StFixestError::new(
                ErrorCode::Panic,
                format!("caught Rust panic at the Stata ABI boundary: {message}"),
            );
            report_error(&error);
            error.return_code()
        }
    }
}

unsafe fn read_arguments(
    argc: c_int,
    argv: *const *const c_char,
) -> Result<Vec<String>, StFixestError> {
    if argc < 0 {
        return Err(StFixestError::new(
            ErrorCode::Syntax,
            "plugin received a negative argument count",
        ));
    }
    if argc > 0 && argv.is_null() {
        return Err(StFixestError::new(
            ErrorCode::Syntax,
            "plugin received a null argv pointer",
        ));
    }
    let mut args = Vec::with_capacity(argc as usize);
    for index in 0..argc as usize {
        // SAFETY: argv is non-null for argc > 0 and the caller promises
        // an array containing argc pointers. Each pointee is validated below.
        let pointer = unsafe { *argv.add(index) };
        if pointer.is_null() {
            return Err(StFixestError::new(
                ErrorCode::Syntax,
                format!("plugin argument {index} is null"),
            ));
        }
        // SAFETY: pointer was checked non-null and is supplied by the C ABI
        // as a NUL-terminated command argument.
        let value = unsafe { CStr::from_ptr(pointer) }.to_str().map_err(|_| {
            StFixestError::new(
                ErrorCode::Syntax,
                format!("plugin argument {index} is not valid UTF-8"),
            )
        })?;
        args.push(value.to_owned());
    }
    Ok(args)
}

fn report_error(error: &StFixestError) {
    let sink = SpiSink;
    sink.error(&format!("stfixest: {error}\n"));
}

fn sanitize_c_string(message: &str) -> CString {
    let bytes: Vec<u8> = message
        .as_bytes()
        .iter()
        .map(|byte| if *byte == 0 { b'?' } else { *byte })
        .collect();
    CString::new(bytes).expect("interior NUL bytes were replaced")
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn exported_boundary_catches_injected_panic() {
        let args = [
            CString::new("panic-test").expect("cstring"),
            CString::new("confirm=panic-boundary-test").expect("cstring"),
        ];
        let pointers: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();
        // SAFETY: pointer array and C strings are valid for the duration of the call.
        let code = unsafe { stfixest_dispatch(pointers.len() as c_int, pointers.as_ptr()) };
        assert_eq!(code, ErrorCode::Panic.stata_return_code());
    }

    #[test]
    fn exported_boundary_rejects_null_argv() {
        // SAFETY: deliberately invalid input is contained and validated by the ABI.
        let code = unsafe { stfixest_dispatch(1, std::ptr::null()) };
        assert_eq!(code, ErrorCode::Syntax.stata_return_code());
    }
}
