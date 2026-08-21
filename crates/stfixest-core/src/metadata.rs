use crate::error::{ErrorCode, Result, StFixestError};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum VariableRole {
    CandidateSample = 1,
    Dependent = 2,
    ContinuousSource = 3,
    CategoricalSource = 4,
    MaterializedTimeSeries = 5,
    Weight = 6,
    Offset = 7,
    Exposure = 8,
    ClusterSource = 9,
    FixedEffectSource = 10,
    FixedEffectSlope = 11,
    OutputSample = 12,
    OutputResidual = 13,
    OutputFixedEffectSum = 14,
    OutputIndividualFixedEffect = 15,
    OutputDropReason = 16,
}

impl VariableRole {
    #[must_use]
    pub const fn code(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableSpec {
    pub position: usize,
    pub role: VariableRole,
    pub source_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum OperatorType {
    Continuous = 1,
    Categorical = 2,
    CategoricalByCategorical = 3,
    CategoricalByContinuous = 4,
    ContinuousByContinuous = 5,
}

impl OperatorType {
    #[must_use]
    pub const fn code(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelTerm {
    pub term_id: u32,
    pub term_order: u32,
    pub operator: OperatorType,
    pub source_positions: Vec<usize>,
    pub level_values: Vec<f64>,
    pub base: bool,
    pub omitted: bool,
    pub coefficient_column: usize,
    pub canonical_name: String,
    pub pre_materialized_time_series: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsorbTerm {
    pub term_id: u32,
    pub categorical_source_positions: Vec<usize>,
    pub intercept: bool,
    pub slope_source_positions: Vec<usize>,
    pub saved_output_positions: Vec<usize>,
    pub canonical_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterTerm {
    pub term_id: u32,
    pub source_positions: Vec<usize>,
    pub canonical_label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpec {
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub candidate_sample_position: usize,
    pub variables: Vec<VariableSpec>,
    pub model_terms: Vec<ModelTerm>,
    pub absorb_terms: Vec<AbsorbTerm>,
    pub cluster_terms: Vec<ClusterTerm>,
}

impl ModelSpec {
    pub fn validate(&self) -> Result<()> {
        if self.protocol_major != crate::protocol::PROTOCOL_VERSION.major {
            return Err(StFixestError::new(
                ErrorCode::ProtocolMismatch,
                "model specification protocol major does not match the plugin",
            ));
        }
        if self.protocol_minor > crate::protocol::PROTOCOL_VERSION.minor {
            return Err(StFixestError::new(
                ErrorCode::ProtocolMismatch,
                "model specification protocol minor is newer than the plugin",
            ));
        }
        self.validate_variables()?;
        self.validate_term_ids()?;
        self.validate_positions()?;
        self.validate_semantics()?;
        Ok(())
    }

    #[must_use]
    pub fn variable_role_rows(&self) -> Vec<[f64; 3]> {
        self.variables
            .iter()
            .map(|variable| {
                [
                    variable.position as f64,
                    f64::from(variable.role.code()),
                    0.0,
                ]
            })
            .collect()
    }

    #[must_use]
    pub fn model_term_rows(&self) -> Vec<Vec<f64>> {
        self.model_terms
            .iter()
            .map(|term| {
                let mut row = vec![
                    f64::from(term.term_id),
                    f64::from(term.term_order),
                    f64::from(term.operator.code()),
                    term.source_positions.len() as f64,
                    if term.base { 1.0 } else { 0.0 },
                    if term.omitted { 1.0 } else { 0.0 },
                    term.coefficient_column as f64,
                    if term.pre_materialized_time_series {
                        1.0
                    } else {
                        0.0
                    },
                ];
                row.extend(term.source_positions.iter().map(|value| *value as f64));
                row
            })
            .collect()
    }

    fn validate_variables(&self) -> Result<()> {
        if self.variables.is_empty() {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "model specification contains no variables",
            ));
        }
        let mut positions = BTreeSet::new();
        for variable in &self.variables {
            if variable.position == 0 {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    "variable positions are one-based and must be positive",
                ));
            }
            if !positions.insert(variable.position) {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    format!("duplicate variable position {}", variable.position),
                ));
            }
            if variable.source_name.trim().is_empty() {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    "variable source name may not be empty",
                ));
            }
        }
        let candidate = self
            .variables
            .iter()
            .find(|variable| variable.position == self.candidate_sample_position)
            .ok_or_else(|| {
                StFixestError::new(
                    ErrorCode::InvalidArgument,
                    "candidate sample position is not present in the variable table",
                )
            })?;
        if candidate.role != VariableRole::CandidateSample {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "candidate sample position does not have the candidate-sample role",
            ));
        }
        Ok(())
    }

    fn validate_term_ids(&self) -> Result<()> {
        validate_unique_ids(
            self.model_terms.iter().map(|term| term.term_id),
            "ordinary model term",
        )?;
        validate_unique_ids(
            self.absorb_terms.iter().map(|term| term.term_id),
            "absorbed term",
        )?;
        validate_unique_ids(
            self.cluster_terms.iter().map(|term| term.term_id),
            "cluster term",
        )?;
        Ok(())
    }

    fn validate_positions(&self) -> Result<()> {
        let positions: BTreeSet<usize> = self.variables.iter().map(|v| v.position).collect();
        let check = |position: usize, context: &str| -> Result<()> {
            if positions.contains(&position) {
                Ok(())
            } else {
                Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    format!("{context} references missing variable position {position}"),
                ))
            }
        };
        for term in &self.model_terms {
            for position in &term.source_positions {
                check(*position, "ordinary model term")?;
            }
        }
        for term in &self.absorb_terms {
            for position in &term.categorical_source_positions {
                check(*position, "absorbed categorical source")?;
            }
            for position in &term.slope_source_positions {
                check(*position, "absorbed slope source")?;
            }
            for position in &term.saved_output_positions {
                check(*position, "absorbed output")?;
            }
        }
        for term in &self.cluster_terms {
            for position in &term.source_positions {
                check(*position, "cluster source")?;
            }
        }
        Ok(())
    }

    fn validate_semantics(&self) -> Result<()> {
        for term in &self.model_terms {
            if term.source_positions.is_empty() {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    format!("ordinary model term {} has no sources", term.term_id),
                ));
            }
            if term.canonical_name.trim().is_empty() {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    format!("ordinary model term {} has no canonical name", term.term_id),
                ));
            }
        }
        for term in &self.absorb_terms {
            if term.categorical_source_positions.is_empty() {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    format!("absorbed term {} has no categorical sources", term.term_id),
                ));
            }
            if !term.intercept && term.slope_source_positions.is_empty() {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    format!(
                        "absorbed term {} has neither an intercept nor a slope",
                        term.term_id
                    ),
                ));
            }
        }
        for term in &self.cluster_terms {
            if term.source_positions.is_empty() {
                return Err(StFixestError::new(
                    ErrorCode::InvalidArgument,
                    format!("cluster term {} has no sources", term.term_id),
                ));
            }
        }
        Ok(())
    }
}

fn validate_unique_ids<I>(ids: I, label: &str) -> Result<()>
where
    I: IntoIterator<Item = u32>,
{
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                format!("duplicate {label} id {id}"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_spec() -> ModelSpec {
        ModelSpec {
            protocol_major: crate::protocol::PROTOCOL_VERSION.major,
            protocol_minor: crate::protocol::PROTOCOL_VERSION.minor,
            candidate_sample_position: 1,
            variables: vec![
                VariableSpec {
                    position: 1,
                    role: VariableRole::CandidateSample,
                    source_name: "touse".to_owned(),
                },
                VariableSpec {
                    position: 2,
                    role: VariableRole::Dependent,
                    source_name: "y".to_owned(),
                },
                VariableSpec {
                    position: 3,
                    role: VariableRole::ContinuousSource,
                    source_name: "x".to_owned(),
                },
            ],
            model_terms: vec![ModelTerm {
                term_id: 1,
                term_order: 1,
                operator: OperatorType::Continuous,
                source_positions: vec![3],
                level_values: vec![],
                base: false,
                omitted: false,
                coefficient_column: 1,
                canonical_name: "x".to_owned(),
                pre_materialized_time_series: false,
            }],
            absorb_terms: vec![],
            cluster_terms: vec![],
        }
    }

    #[test]
    fn minimal_spec_is_valid() {
        minimal_spec().validate().expect("valid spec");
    }

    #[test]
    fn missing_source_position_is_rejected() {
        let mut spec = minimal_spec();
        spec.model_terms[0].source_positions = vec![99];
        assert_eq!(
            spec.validate().expect_err("must reject").code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn newer_minor_protocol_is_rejected() {
        let mut spec = minimal_spec();
        spec.protocol_minor = crate::protocol::PROTOCOL_VERSION.minor + 1;
        assert_eq!(
            spec.validate().expect_err("must reject").code,
            ErrorCode::ProtocolMismatch
        );
    }
}
