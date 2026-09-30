use crate::{
    data::{Prepared, PreparedAdjustment},
    error::{reserved, ErrorCode, FeregError, Result},
};
use std::borrow::Cow;
pub(crate) fn offset(sample: &Prepared) -> Result<Option<Cow<'_, [f64]>>> {
    match &sample.adjustment {
        PreparedAdjustment::None => Ok(None),
        PreparedAdjustment::Offset(values) => {
            if values.iter().any(|v| !v.is_finite()) {
                return Err(invalid("PPML offset must be finite"));
            }
            Ok(Some(Cow::Borrowed(values)))
        }
        PreparedAdjustment::Exposure(values) => {
            let mut offset = reserved(values.len())?;
            for &value in values {
                if !value.is_finite() || value <= 0. {
                    return Err(invalid("PPML exposure must be finite and positive"));
                }
                offset.push(value.ln());
            }
            Ok(Some(Cow::Owned(offset)))
        }
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}
