//! Requested-only output banks, certified before a single scatter to original rows.
mod fixed;
mod ols;
mod ppml;
use crate::{
    data::{DropReason, Prepared},
    error::{reserved, ErrorCode, FeregError, Result},
    fe::Certificate,
};
pub use ols::{ols, OlsFrame};
pub use ppml::{ppml, PpmlRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Residuals,
    Xb,
    /// PPML eta/xbd aliases share this one bank.
    Xbd,
    Mean,
    Absorbed,
    DropReason,
    /// Channel zero is the intercept when present; remaining channels are slopes.
    Saved {
        effect: usize,
        channel: usize,
    },
    /// Bias uncertainty uses the fixed component anchor, never ordinary centering.
    Bias {
        effect: usize,
        channel: crate::ppml::bias::Channel,
    },
}
#[derive(Debug, Clone, Default)]
pub struct Request {
    pub residuals: bool,
    pub xb: bool,
    pub xbd: bool,
    pub absorbed: bool,
    pub drop_reason: bool,
    pub saved: Vec<(usize, usize)>,
}
impl Request {
    fn kinds(&self, mean: bool) -> Result<Vec<Kind>> {
        let mut kinds = reserved(
            self.saved
                .len()
                .checked_add(6)
                .ok_or_else(|| invalid("output count overflows"))?,
        )?;
        for (include, kind) in [
            (self.residuals, Kind::Residuals),
            (self.xb, Kind::Xb),
            (self.xbd, Kind::Xbd),
            (mean, Kind::Mean),
            (self.absorbed, Kind::Absorbed),
            (self.drop_reason, Kind::DropReason),
        ] {
            if include {
                kinds.push(kind);
            }
        }
        for &(effect, channel) in &self.saved {
            let kind = Kind::Saved { effect, channel };
            if kinds.contains(&kind) {
                return Err(invalid("saved FE channel requested more than once"));
            }
            kinds.push(kind);
        }
        Ok(kinds)
    }
}
#[derive(Debug, PartialEq)]
pub struct Column {
    pub kind: Kind,
    /// Nonretained rows are IEEE NaN; the plugin translates them to Stata missing.
    /// DropReason is defined on every original row.
    pub values: Vec<f64>,
}
#[derive(Debug, PartialEq)]
pub struct OutputCertificate {
    pub fixed_effects: Certificate,
    pub relative_reconstruction: f64,
}
#[derive(Debug, PartialEq)]
pub struct Outputs {
    pub columns: Vec<Column>,
    /// Absent when no numerical observation output was requested.
    pub certificate: Option<OutputCertificate>,
}
impl Outputs {
    #[must_use]
    pub fn get(&self, kind: Kind) -> Option<&[f64]> {
        self.columns
            .iter()
            .find(|c| c.kind == kind)
            .map(|c| c.values.as_slice())
    }
}
pub(crate) fn allocate(sample: &Prepared, kinds: Vec<Kind>) -> Result<Outputs> {
    let mut columns = reserved(kinds.len())?;
    for kind in kinds {
        let mut values = reserved(sample.n_original())?;
        if kind == Kind::DropReason {
            values.extend(sample.drop_reasons.iter().map(|r| f64::from(r.code())));
        } else {
            values.resize(sample.n_original(), f64::NAN);
        }
        columns.push(Column { kind, values });
    }
    Ok(Outputs {
        columns,
        certificate: None,
    })
}
pub(crate) fn validate_scatter(sample: &Prepared, outputs: &Outputs) -> Result<()> {
    for column in &outputs.columns {
        if column.values.len() != sample.n_original() {
            return Err(invalid("output does not cover the original axis"));
        }
        for (row, (&value, &reason)) in column.values.iter().zip(&sample.drop_reasons).enumerate() {
            let valid = if column.kind == Kind::DropReason {
                value == f64::from(reason.code())
            } else if reason == DropReason::Retained {
                value.is_finite()
            } else {
                value.is_nan()
            };
            if !valid {
                return Err(numerical(format!(
                    "output {:?} has invalid original row {row}",
                    column.kind
                )));
            }
        }
    }
    Ok(())
}
fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
