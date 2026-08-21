use crate::data::weights::{validate_weight, WeightKind};
use crate::error::{ErrorCode, Result, StFixestError};

/// Authoritative observation-level exclusion reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DropReason {
    Retained = 0,
    ExcludedByIfIn = 1,
    MissingOutcome = 2,
    MissingRegressor = 3,
    InvalidOrMissingWeight = 4,
    MissingFixedEffect = 5,
    MissingCluster = 6,
    MissingOffsetOrExposure = 7,
    Singleton = 8,
    FixedEffectSeparation = 9,
    SimplexSeparation = 10,
    IteratedRectifierSeparation = 11,
    MuSeparation = 12,
    OtherModelExclusion = 13,
}

impl DropReason {
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Borrowed numeric columns needed to construct the initial native sample.
#[derive(Debug)]
pub struct NumericPreparationInput<'a> {
    pub candidate: &'a [bool],
    pub outcome: &'a [f64],
    pub regressors: &'a [&'a [f64]],
    pub weight_kind: WeightKind,
    pub weights: Option<&'a [f64]>,
    pub offset: Option<&'a [f64]>,
    pub exposure: Option<&'a [f64]>,
    /// Missing values in categorical sources must be represented as `None`.
    pub fixed_effect_sources: &'a [&'a [Option<u32>]],
    pub cluster_sources: &'a [&'a [Option<u32>]],
    pub poisson: bool,
}

/// Compact native data after initial missing-value and domain validation.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedData {
    pub n_original: usize,
    pub n_candidate: usize,
    pub obs_map: Vec<usize>,
    pub outcome: Vec<f64>,
    pub regressors: Vec<Vec<f64>>,
    pub weights: Vec<f64>,
    pub offset: Vec<f64>,
    pub fixed_effect_sources: Vec<Vec<u32>>,
    pub cluster_sources: Vec<Vec<u32>>,
    pub drop_reasons: Vec<DropReason>,
}

impl PreparedData {
    #[must_use]
    pub fn n_active(&self) -> usize {
        self.obs_map.len()
    }

    #[must_use]
    pub fn retained_mask(&self) -> Vec<bool> {
        self.drop_reasons
            .iter()
            .map(|reason| *reason == DropReason::Retained)
            .collect()
    }

    #[must_use]
    pub fn drop_reason_codes(&self) -> Vec<u8> {
        self.drop_reasons
            .iter()
            .map(|reason| reason.code())
            .collect()
    }
}

/// Construct the authoritative initial sample from original-order numeric data.
///
/// Reason precedence follows the specification: `if`/`in`, outcome,
/// regressors, weights, fixed effects, clusters, then offset/exposure.
pub fn prepare_numeric_data(input: &NumericPreparationInput<'_>) -> Result<PreparedData> {
    let n = input.candidate.len();
    validate_lengths(input, n)?;
    if input.offset.is_some() && input.exposure.is_some() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "offset and exposure are mutually exclusive",
        ));
    }

    let n_candidate = input.candidate.iter().filter(|value| **value).count();
    let mut drop_reasons = vec![DropReason::Retained; n];
    let mut obs_map = Vec::with_capacity(n_candidate);
    let mut outcome = Vec::with_capacity(n_candidate);
    let mut regressors: Vec<Vec<f64>> = input
        .regressors
        .iter()
        .map(|_| Vec::with_capacity(n_candidate))
        .collect();
    let mut weights = Vec::with_capacity(n_candidate);
    let mut offset = Vec::with_capacity(n_candidate);
    let mut fixed_effect_sources: Vec<Vec<u32>> = input
        .fixed_effect_sources
        .iter()
        .map(|_| Vec::with_capacity(n_candidate))
        .collect();
    let mut cluster_sources: Vec<Vec<u32>> = input
        .cluster_sources
        .iter()
        .map(|_| Vec::with_capacity(n_candidate))
        .collect();

    for observation in 0..n {
        let reason = initial_drop_reason(input, observation)?;
        drop_reasons[observation] = reason;
        if reason != DropReason::Retained {
            continue;
        }

        let y = input.outcome[observation];
        let weight = match input.weights {
            Some(column) => validate_weight(input.weight_kind, column[observation])?,
            None => validate_weight(WeightKind::None, 1.0)?,
        };
        let observation_offset = match (input.offset, input.exposure) {
            (Some(column), None) => column[observation],
            (None, Some(column)) => column[observation].ln(),
            (None, None) => 0.0,
            (Some(_), Some(_)) => unreachable!("validated above"),
        };

        obs_map.push(observation);
        outcome.push(y);
        weights.push(weight);
        offset.push(observation_offset);
        for (destination, source) in regressors.iter_mut().zip(input.regressors) {
            destination.push(source[observation]);
        }
        for (destination, source) in fixed_effect_sources
            .iter_mut()
            .zip(input.fixed_effect_sources)
        {
            destination.push(source[observation].expect("validated fixed-effect source"));
        }
        for (destination, source) in cluster_sources.iter_mut().zip(input.cluster_sources) {
            destination.push(source[observation].expect("validated cluster source"));
        }
    }

    if obs_map.is_empty() {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "no observations remain after native sample validation",
        ));
    }

    Ok(PreparedData {
        n_original: n,
        n_candidate,
        obs_map,
        outcome,
        regressors,
        weights,
        offset,
        fixed_effect_sources,
        cluster_sources,
        drop_reasons,
    })
}

fn initial_drop_reason(
    input: &NumericPreparationInput<'_>,
    observation: usize,
) -> Result<DropReason> {
    if !input.candidate[observation] {
        return Ok(DropReason::ExcludedByIfIn);
    }
    let outcome = input.outcome[observation];
    if !outcome.is_finite() {
        return Ok(DropReason::MissingOutcome);
    }
    if input.poisson && outcome < 0.0 {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            format!(
                "Poisson outcome is negative at observation {}",
                observation + 1
            ),
        ));
    }
    if input
        .regressors
        .iter()
        .any(|column| !column[observation].is_finite())
    {
        return Ok(DropReason::MissingRegressor);
    }
    if let Some(column) = input.weights {
        let value = column[observation];
        if !value.is_finite() {
            return Ok(DropReason::InvalidOrMissingWeight);
        }
        validate_weight(input.weight_kind, value)?;
    }
    if input
        .fixed_effect_sources
        .iter()
        .any(|column| column[observation].is_none())
    {
        return Ok(DropReason::MissingFixedEffect);
    }
    if input
        .cluster_sources
        .iter()
        .any(|column| column[observation].is_none())
    {
        return Ok(DropReason::MissingCluster);
    }
    if let Some(column) = input.offset {
        if !column[observation].is_finite() {
            return Ok(DropReason::MissingOffsetOrExposure);
        }
    }
    if let Some(column) = input.exposure {
        let value = column[observation];
        if !value.is_finite() || value <= 0.0 {
            return Ok(DropReason::MissingOffsetOrExposure);
        }
    }
    Ok(DropReason::Retained)
}

fn validate_lengths(input: &NumericPreparationInput<'_>, n: usize) -> Result<()> {
    let mut named_lengths = vec![("outcome", input.outcome.len())];
    named_lengths.extend(input.regressors.iter().enumerate().map(|(index, column)| {
        (
            if index == 0 {
                "regressor"
            } else {
                "regressors"
            },
            column.len(),
        )
    }));
    if let Some(column) = input.weights {
        named_lengths.push(("weights", column.len()));
    }
    if let Some(column) = input.offset {
        named_lengths.push(("offset", column.len()));
    }
    if let Some(column) = input.exposure {
        named_lengths.push(("exposure", column.len()));
    }
    named_lengths.extend(
        input
            .fixed_effect_sources
            .iter()
            .map(|column| ("fixed-effect source", column.len())),
    );
    named_lengths.extend(
        input
            .cluster_sources
            .iter()
            .map(|column| ("cluster source", column.len())),
    );
    for (name, length) in named_lengths {
        if length != n {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                format!("{name} has length {length}, expected {n}"),
            ));
        }
    }
    if input.weight_kind != WeightKind::None && input.weights.is_none() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "a non-default weight kind requires a weight column",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_reason_precedence_and_compacts_in_original_order() {
        let candidate = [false, true, true, true, true, true, true, true];
        let outcome = [1.0, f64::NAN, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let x = [1.0, 2.0, f64::NAN, 4.0, 5.0, 6.0, 7.0, 8.0];
        let weights = [1.0, 1.0, 1.0, f64::NAN, 1.0, 1.0, 1.0, 2.0];
        let fe = [
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            None,
            Some(1),
            Some(1),
            Some(1),
        ];
        let cluster = [
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            None,
            Some(1),
            Some(1),
        ];
        let exposure = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 2.0];
        let input = NumericPreparationInput {
            candidate: &candidate,
            outcome: &outcome,
            regressors: &[&x],
            weight_kind: WeightKind::Analytic,
            weights: Some(&weights),
            offset: None,
            exposure: Some(&exposure),
            fixed_effect_sources: &[&fe],
            cluster_sources: &[&cluster],
            poisson: true,
        };
        let prepared = prepare_numeric_data(&input).expect("prepared data");
        assert_eq!(prepared.obs_map, vec![7]);
        assert_eq!(prepared.outcome, vec![7.0]);
        assert_eq!(prepared.offset, vec![2.0_f64.ln()]);
        assert_eq!(
            prepared.drop_reasons,
            vec![
                DropReason::ExcludedByIfIn,
                DropReason::MissingOutcome,
                DropReason::MissingRegressor,
                DropReason::InvalidOrMissingWeight,
                DropReason::MissingFixedEffect,
                DropReason::MissingCluster,
                DropReason::MissingOffsetOrExposure,
                DropReason::Retained,
            ]
        );
    }

    #[test]
    fn rejects_negative_poisson_outcome() {
        let candidate = [true];
        let outcome = [-1.0];
        let input = NumericPreparationInput {
            candidate: &candidate,
            outcome: &outcome,
            regressors: &[],
            weight_kind: WeightKind::None,
            weights: None,
            offset: None,
            exposure: None,
            fixed_effect_sources: &[],
            cluster_sources: &[],
            poisson: true,
        };
        assert!(prepare_numeric_data(&input).is_err());
    }
}
