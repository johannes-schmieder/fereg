use crate::error::{ErrorCode, FeregError, Result};
use std::collections::BTreeMap;

pub const ADO_VERSION: &str = "0.5.0";
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MINIMUM_STATA_MAJOR: u16 = 18;
pub const SPI_MAJOR: u16 = 3;
pub const SPI_MINOR: u16 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

/// Version of the small action/diagnostics handshake accepted by the plugin.
///
/// Minor 5 adds the explicit ado-certified complete-input flag used by the
/// estimation bridge. Older diagnostic callers remain valid under the usual
/// minor-version rule; estimation continues to require an exact action wire.
pub const ACTION_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::new(1, 5);
/// Version of the data-independent expanded-model metadata wire.
pub const MODEL_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(2, 1);
/// Version of the public prepared-OLS shadow-result layout.
pub const OLS_RESULT_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(1, 4);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub ado_version: String,
    pub protocol: ProtocolVersion,
    pub stata_major: u16,
}

impl Handshake {
    #[must_use]
    pub fn current(stata_major: u16) -> Self {
        Self {
            ado_version: ADO_VERSION.to_owned(),
            protocol: ACTION_PROTOCOL_VERSION,
            stata_major,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.protocol.major != ACTION_PROTOCOL_VERSION.major {
            return Err(FeregError::new(
                ErrorCode::ProtocolMismatch,
                format!(
                    "action protocol major {} is incompatible with plugin major {}",
                    self.protocol.major, ACTION_PROTOCOL_VERSION.major
                ),
            ));
        }
        if self.protocol.minor > ACTION_PROTOCOL_VERSION.minor {
            return Err(FeregError::new(
                ErrorCode::ProtocolMismatch,
                format!(
                    "action protocol minor {} is newer than plugin minor {}",
                    self.protocol.minor, ACTION_PROTOCOL_VERSION.minor
                ),
            ));
        }
        if self.stata_major < MINIMUM_STATA_MAJOR {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                format!(
                    "Stata {} is unsupported; fereg requires Stata {} or newer",
                    self.stata_major, MINIMUM_STATA_MAJOR
                ),
            ));
        }
        if self.ado_version != ADO_VERSION {
            return Err(FeregError::new(
                ErrorCode::ProtocolMismatch,
                format!(
                    "ado contract {} is incompatible with plugin contract {}",
                    self.ado_version, ADO_VERSION
                ),
            ));
        }
        Ok(())
    }

    pub fn from_key_values(values: &BTreeMap<String, String>) -> Result<Self> {
        let ado_version = required(values, "ado")?.to_owned();
        let protocol_major = parse_u16(values, "protocol_major")?;
        let protocol_minor = parse_u16(values, "protocol_minor")?;
        let stata_major = parse_stata_major(required(values, "stata")?)?;
        Ok(Self {
            ado_version,
            protocol: ProtocolVersion::new(protocol_major, protocol_minor),
            stata_major,
        })
    }
}

pub fn parse_key_values(args: &[String]) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for argument in args {
        let (key, value) = argument.split_once('=').ok_or_else(|| {
            FeregError::new(
                ErrorCode::Syntax,
                format!("plugin argument {argument:?} must have key=value form"),
            )
        })?;
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        if key.is_empty() || value.is_empty() {
            return Err(FeregError::new(
                ErrorCode::Syntax,
                format!("plugin argument {argument:?} has an empty key or value"),
            ));
        }
        if values.insert(key.clone(), value).is_some() {
            return Err(FeregError::new(
                ErrorCode::Syntax,
                format!("duplicate plugin protocol field {key}"),
            ));
        }
    }
    Ok(values)
}

fn required<'a>(values: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str> {
    values.get(key).map(String::as_str).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Syntax,
            format!("missing required protocol field {key}=..."),
        )
    })
}

fn parse_u16(values: &BTreeMap<String, String>, key: &str) -> Result<u16> {
    required(values, key)?.parse::<u16>().map_err(|_| {
        FeregError::new(
            ErrorCode::Syntax,
            format!("protocol field {key} must be an unsigned integer"),
        )
    })
}

fn parse_stata_major(value: &str) -> Result<u16> {
    value
        .split('.')
        .next()
        .unwrap_or(value)
        .parse::<u16>()
        .map_err(|_| {
            FeregError::new(
                ErrorCode::Syntax,
                format!("invalid Stata version {value:?}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_handshake_passes() {
        Handshake::current(18)
            .validate()
            .expect("current handshake");
    }

    #[test]
    fn action_handshake_and_model_wire_are_distinct_versions() {
        assert_eq!(ACTION_PROTOCOL_VERSION, ProtocolVersion::new(1, 5));
        assert_eq!(MODEL_WIRE_VERSION, ProtocolVersion::new(2, 1));
        assert_ne!(ACTION_PROTOCOL_VERSION, MODEL_WIRE_VERSION);
    }

    #[test]
    fn newer_protocol_major_fails_closed() {
        let handshake = Handshake {
            ado_version: ADO_VERSION.to_owned(),
            protocol: ProtocolVersion::new(ACTION_PROTOCOL_VERSION.major + 1, 0),
            stata_major: 18,
        };
        let error = handshake.validate().expect_err("must reject");
        assert_eq!(error.code, ErrorCode::ProtocolMismatch);
        assert!(error.message.contains("action protocol major"));
        assert!(!error.message.contains("model protocol"));
    }

    #[test]
    fn newer_action_protocol_minor_is_labeled_unambiguously() {
        let handshake = Handshake {
            ado_version: ADO_VERSION.to_owned(),
            protocol: ProtocolVersion::new(
                ACTION_PROTOCOL_VERSION.major,
                ACTION_PROTOCOL_VERSION.minor + 1,
            ),
            stata_major: 18,
        };
        let error = handshake.validate().expect_err("must reject");
        assert_eq!(error.code, ErrorCode::ProtocolMismatch);
        assert!(error.message.contains("action protocol minor"));
        assert!(!error.message.contains("model protocol"));
    }

    #[test]
    fn old_stata_is_rejected() {
        assert_eq!(
            Handshake::current(17)
                .validate()
                .expect_err("must reject")
                .code,
            ErrorCode::Unsupported
        );
    }

    #[test]
    fn pre_one_ado_contract_must_match_exactly() {
        let handshake = Handshake {
            ado_version: "0.99.0".to_owned(),
            protocol: ACTION_PROTOCOL_VERSION,
            stata_major: 18,
        };
        assert_eq!(
            handshake.validate().expect_err("must reject").code,
            ErrorCode::ProtocolMismatch
        );
    }

    #[test]
    fn malformed_and_duplicate_key_values_are_rejected() {
        assert_eq!(
            parse_key_values(&["threads".to_owned()])
                .expect_err("must reject")
                .code,
            ErrorCode::Syntax
        );
        assert_eq!(
            parse_key_values(&["threads=1".to_owned(), "threads=2".to_owned()])
                .expect_err("must reject")
                .code,
            ErrorCode::Syntax
        );
    }
}
