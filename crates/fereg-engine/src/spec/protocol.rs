//! Model version and key/value framing harvested from legacy protocol.rs at b1700e6d.
use crate::error::{copied_string, reserved, ErrorCode, FeregError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

pub const MODEL_WIRE_VERSION: ProtocolVersion = ProtocolVersion { major: 2, minor: 1 };

/// Each caller declares its entire vocabulary; unknown and repeated keys fail closed.
pub fn parse_key_values(args: &[String], allowed: &[&str]) -> Result<Vec<(String, String)>> {
    let mut values: Vec<(String, String)> = reserved(args.len())?;
    for argument in args {
        let (key, value) = argument.split_once('=').ok_or_else(|| {
            FeregError::new(
                ErrorCode::Syntax,
                "plugin argument must have key=value form",
            )
        })?;
        let mut key = copied_string(key.trim())?;
        key.make_ascii_lowercase();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(FeregError::new(
                ErrorCode::Syntax,
                "empty protocol key or value",
            ));
        }
        if !allowed.contains(&key.as_str()) {
            return Err(FeregError::new(
                ErrorCode::Syntax,
                format!("unknown protocol field {key}"),
            ));
        }
        if values.iter().any(|(previous, _)| previous == &key) {
            return Err(FeregError::new(
                ErrorCode::Syntax,
                format!("duplicate protocol field {key}"),
            ));
        }
        values.push((key, copied_string(value)?));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_rejects_unknown_duplicate_and_empty_fields() {
        for args in [
            vec!["x"],
            vec!["=1"],
            vec!["x="],
            vec!["future=1"],
            vec!["x=1", " X =2"],
        ] {
            let args = args.into_iter().map(String::from).collect::<Vec<_>>();
            assert_eq!(
                parse_key_values(&args, &["x"]).unwrap_err().code,
                ErrorCode::Syntax
            );
        }
        assert_eq!(
            parse_key_values(&[" X =a=b ".into()], &["x"]).unwrap(),
            vec![("x".into(), "a=b".into())]
        );
    }
}
