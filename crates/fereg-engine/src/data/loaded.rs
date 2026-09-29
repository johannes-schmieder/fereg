//! Rust-owned input banks: no Stata access or model expansion during preparation.
use super::{
    AdjustmentInput, NumericColumnView, NumericDomainPolicy, NumericPreparationInput,
    NumericSourceUse, Prepared, WeightInput, WeightKind,
};
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    factor::factorize_strings,
};

#[derive(Debug)]
pub struct NumericColumn {
    pub values: Vec<f64>,
    pub missing: Option<Vec<bool>>,
}
impl NumericColumn {
    #[must_use]
    pub fn view(&self) -> NumericColumnView<'_> {
        NumericColumnView {
            values: &self.values,
            missing: self.missing.as_deref(),
        }
    }
}
#[derive(Debug)]
pub enum LoadedSource {
    Numeric(NumericColumn),
    Strings(Vec<String>),
}
#[derive(Debug)]
pub enum LoadedAdjustment {
    None,
    Offset(NumericColumn),
    Exposure(NumericColumn),
}
#[derive(Debug)]
pub struct Loaded {
    pub candidate: Vec<bool>,
    pub outcome: NumericColumn,
    pub sources: Vec<LoadedSource>,
    pub weights: Option<(WeightKind, NumericColumn)>,
    pub adjustment: LoadedAdjustment,
}
impl Loaded {
    /// Validate all row banks before candidate-domain checks or string encoding.
    pub fn validate_alignment(&self) -> Result<()> {
        use super::sample::validate_column;
        let n = self.candidate.len();
        validate_column("outcome", self.outcome.view(), n)?;
        for source in &self.sources {
            match source {
                LoadedSource::Numeric(column) => validate_column("source", column.view(), n)?,
                LoadedSource::Strings(values) if values.len() != n => {
                    return Err(FeregError::new(
                        ErrorCode::InvalidArgument,
                        "string source has the wrong length",
                    ));
                }
                LoadedSource::Strings(_) => {}
            }
        }
        if let Some((_, column)) = &self.weights {
            validate_column("weight", column.view(), n)?;
        }
        match &self.adjustment {
            LoadedAdjustment::None => {}
            LoadedAdjustment::Offset(column) | LoadedAdjustment::Exposure(column) => {
                validate_column("adjustment", column.view(), n)?;
            }
        }
        Ok(())
    }
    pub fn prepare<P: NumericDomainPolicy>(
        &self,
        uses: &[NumericSourceUse],
        policy: &P,
    ) -> Result<Prepared> {
        if uses.len() != self.sources.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "loaded source roles have the wrong length",
            ));
        }
        self.validate_alignment()?;
        let mut encoded = reserved(self.sources.len())?;
        for (source, usage) in self.sources.iter().zip(uses) {
            encoded.push(match source {
                LoadedSource::Numeric(_) => None,
                LoadedSource::Strings(values) => {
                    if usage.ordinary {
                        return Err(FeregError::new(
                            ErrorCode::Unsupported,
                            "string ordinary regressors are unsupported",
                        ));
                    }
                    Some(encode_strings(values)?)
                }
            });
        }
        let mut sources = reserved(self.sources.len())?;
        for (source, converted) in self.sources.iter().zip(&encoded) {
            sources.push(match source {
                LoadedSource::Numeric(column) => column.view(),
                LoadedSource::Strings(_) => {
                    converted.as_ref().expect("encoded string source").view()
                }
            });
        }
        let weights = match &self.weights {
            None => WeightInput::None,
            Some((kind, column)) => WeightInput::Provided {
                kind: *kind,
                column: column.view(),
            },
        };
        let adjustment = match &self.adjustment {
            LoadedAdjustment::None => AdjustmentInput::None,
            LoadedAdjustment::Offset(column) => AdjustmentInput::Offset(column.view()),
            LoadedAdjustment::Exposure(column) => AdjustmentInput::Exposure(column.view()),
        };
        super::prepare_numeric_sample_with_source_uses(
            &NumericPreparationInput {
                candidate: &self.candidate,
                outcome: self.outcome.view(),
                regressors: &sources,
                weights,
                adjustment,
            },
            uses,
            policy,
        )
    }
}

fn encode_strings(values: &[String]) -> Result<NumericColumn> {
    let encoded = factorize_strings(values)?;
    let mut numeric = reserved(values.len())?;
    numeric.extend(encoded.codes.into_iter().map(f64::from));
    let mut missing = reserved(values.len())?;
    missing.extend(values.iter().map(String::is_empty));
    Ok(NumericColumn {
        values: numeric,
        missing: Some(missing),
    })
}
