//! Guarded estimation transport shared by the OLS and PPML actions.
//!
//! Stata 18 rejects a plugin call with the complete guarded estimate protocol
//! as "too many options" before native dispatch. Keep the ordinary argv form
//! for the C and Fake SPI boundaries, and additionally accept one explicitly
//! tagged, byte-length-framed bundle for the hidden licensed-Stata caller.
//! Decoding is complete and canonical before the handshake and before any SPI
//! metadata or observation read.
use crate::estimate::imposed_memory_limit_bytes;
use crate::input::SpiApi;
use crate::model_metadata::{push_reserved, reserved_empty, syntax};
use fereg_core::error::{allocation, Result};
use fereg_core::protocol::parse_key_values;
use std::collections::BTreeMap;

const MAX_LOCAL_ESTIMATE_BUNDLE_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn admit_estimate_transport(args: &[String]) -> Result<u128> {
    let bytes = crate::metadata_memory::transport_peak_bytes(args)?;
    crate::metadata_memory::admit(
        bytes,
        fereg_core::memory::usable_memory_bytes(imposed_memory_limit_bytes()?),
    )?;
    Ok(bytes)
}

pub(crate) fn read_local_estimate_bundle(
    args: &[String],
    spi: &mut dyn SpiApi,
) -> Result<Option<String>> {
    let Some(descriptor) = args
        .iter()
        .find_map(|argument| argument.strip_prefix("bundle_local="))
    else {
        return Ok(None);
    };
    if args.len() != 1 {
        return Err(syntax(
            "local estimation bundle must be the only action argument",
        ));
    }
    if descriptor.is_empty()
        || descriptor.starts_with('0')
        || !descriptor.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(syntax(
            "local estimation bundle byte count is not canonical",
        ));
    }
    let length = descriptor
        .parse::<usize>()
        .map_err(|_| allocation("local estimation bundle byte count overflows"))?;
    if length > MAX_LOCAL_ESTIMATE_BUNDLE_BYTES {
        return Err(allocation(
            "local estimation bundle exceeds the 16 MiB metadata capacity",
        ));
    }
    const PREFIX: &[u8] = b"bundle=";
    // Read one byte beyond the declaration so SPI truncation cannot make an
    // overlong macro look exact. The SPI limit excludes its NUL terminator.
    let required = PREFIX.len() + length + 2;
    crate::metadata_memory::admit(
        required as u128
            + args.len() as u128 * std::mem::size_of::<String>() as u128
            + args
                .iter()
                .map(|value| value.capacity() as u128)
                .sum::<u128>(),
        fereg_core::memory::usable_memory_bytes(imposed_memory_limit_bytes()?),
    )?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(required)
        .map_err(|_| allocation("unable to reserve local estimation bundle metadata"))?;
    if bytes.capacity() < required {
        return Err(allocation(
            "local estimation bundle reservation was undersized",
        ));
    }
    bytes.resize(required, u8::MAX);
    bytes[..PREFIX.len()].copy_from_slice(PREFIX);
    spi.read_estimate_bundle(&mut bytes[PREFIX.len()..])?;
    if bytes[PREFIX.len()..].iter().position(|&byte| byte == 0) != Some(length) {
        return Err(syntax(
            "local estimation bundle byte count or terminator is inconsistent",
        ));
    }
    bytes.truncate(PREFIX.len() + length);
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| syntax("local estimation bundle is not valid UTF-8"))
}

/// One estimator's guarded protocol: its label in messages and its field cap.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GuardedProtocol {
    label: &'static str,
    max_fields: usize,
}

pub(crate) const OLS_PROTOCOL: GuardedProtocol = GuardedProtocol {
    label: "OLS",
    max_fields: 85,
};

pub(crate) const PPML_PROTOCOL: GuardedProtocol = GuardedProtocol {
    label: "PPML",
    max_fields: 99,
};

impl GuardedProtocol {
    pub(crate) fn key_values(self, args: &[String]) -> Result<BTreeMap<String, String>> {
        let label = self.label;
        if args.len() > self.max_fields {
            return Err(syntax(format!(
                "guarded {label} protocol has more than {} fields",
                self.max_fields
            )));
        }
        let Some(bundle) = (args.len() == 1)
            .then(|| args[0].strip_prefix("bundle="))
            .flatten()
        else {
            if args.iter().any(|argument| argument.starts_with("bundle=")) {
                return Err(syntax(format!(
                    "guarded {label} bundle must be the only action argument"
                )));
            }
            return parse_key_values(args);
        };
        parse_key_values(&self.decode_bundle(bundle)?)
    }

    fn decode_bundle(self, bundle: &str) -> Result<Vec<String>> {
        let label = self.label;
        let bytes = bundle.as_bytes();
        let (count, mut cursor) = self.decimal(bytes, 0, "field count")?;
        if count == 0 || count > self.max_fields {
            return Err(syntax(format!(
                "guarded {label} bundle field count must be between one and {}",
                self.max_fields
            )));
        }
        let mut fields = reserved_empty(count, "guarded bundle fields")?;
        for _ in 0..count {
            let (length, payload_start) = self.decimal(bytes, cursor, "field length")?;
            if length == 0 {
                return Err(syntax(format!("guarded {label} bundle field is empty")));
            }
            let payload_end = payload_start
                .checked_add(length)
                .filter(|end| *end <= bytes.len())
                .ok_or_else(|| syntax(format!("guarded {label} bundle field is truncated")))?;
            let payload = bundle.get(payload_start..payload_end).ok_or_else(|| {
                syntax(format!("guarded {label} bundle length splits UTF-8 data"))
            })?;
            push_reserved(
                &mut fields,
                fallible_string(payload, "guarded bundle field")?,
                "guarded bundle field capacity exhausted",
            )?;
            cursor = payload_end;
        }
        if cursor != bytes.len() {
            return Err(syntax(format!("guarded {label} bundle has trailing data")));
        }
        Ok(fields)
    }

    fn decimal(self, bytes: &[u8], start: usize, part: &str) -> Result<(usize, usize)> {
        let label = self.label;
        let mut cursor = start;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == start || cursor >= bytes.len() || bytes[cursor] != b':' {
            return Err(syntax(format!(
                "guarded {label} bundle {part} lacks a decimal length delimiter"
            )));
        }
        if cursor - start > 1 && bytes[start] == b'0' {
            return Err(syntax(format!(
                "guarded {label} bundle {part} is not canonically encoded"
            )));
        }
        let value = std::str::from_utf8(&bytes[start..cursor])
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| syntax(format!("guarded {label} bundle {part} overflows")))?;
        Ok((value, cursor + 1))
    }
}

/// Decode one saved-output position per declared channel; zero means absent.
pub(crate) fn decode_output_position_registry(
    value: &str,
    expected: usize,
) -> Result<Vec<Option<u32>>> {
    let values = crate::model_metadata::decode_registry(
        value,
        expected,
        crate::model_metadata::RegistryFraming::Delimited,
    )?;
    let mut output = reserved_empty(expected, "saved-output position registry")?;
    for value in values {
        let position = value
            .parse::<u32>()
            .map_err(|_| syntax("saved-output position must be an unsigned integer"))?;
        push_reserved(
            &mut output,
            (position != 0).then_some(position),
            "saved-output position registry capacity exhausted",
        )?;
    }
    Ok(output)
}

pub(crate) fn fallible_string(value: &str, label: &'static str) -> Result<String> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    output.push_str(value);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fereg_core::error::ErrorCode;

    #[test]
    fn ppml_bundle_decoder_accepts_the_closed_99_field_protocol_only() {
        fn encode(fields: &[String]) -> String {
            let mut bundle = format!("{}:", fields.len());
            for field in fields {
                bundle.push_str(&format!("{}:{field}", field.len()));
            }
            bundle
        }

        let fields = (0..PPML_PROTOCOL.max_fields)
            .map(|index| format!("field_{index}=value_{index}"))
            .collect::<Vec<_>>();
        assert_eq!(
            PPML_PROTOCOL
                .decode_bundle(&encode(&fields))
                .expect("99-field PPML bundle"),
            fields,
        );

        let oversized = (0..=PPML_PROTOCOL.max_fields)
            .map(|index| format!("field_{index}=value_{index}"))
            .collect::<Vec<_>>();
        let error = PPML_PROTOCOL
            .decode_bundle(&encode(&oversized))
            .expect_err("100-field PPML bundle must fail closed");
        assert_eq!(error.code, ErrorCode::Syntax);
        assert!(error.message.contains("between one and 99"));
    }
}
