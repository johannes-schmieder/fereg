use crate::error::{ErrorCode, Result, StFixestError};
use crate::factorize::{factorize_f64, factorize_i64, factorize_strings, interaction_codes};
use crate::metadata::{ModelSpec, ModelTerm, OperatorType, VariableRole, VariableSpec};
use crate::options::ThreadConfig;
use crate::parallel::ThreadTeam;
use crate::protocol::Handshake;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfTestCase {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfTestReport {
    pub cases: Vec<SelfTestCase>,
}

impl SelfTestReport {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.cases.iter().all(|case| case.passed)
    }

    #[must_use]
    pub fn render(&self) -> String {
        let mut output = String::new();
        for case in &self.cases {
            writeln!(
                output,
                "stfixest.selftest.{}={} {}",
                case.name,
                if case.passed { "pass" } else { "fail" },
                case.detail
            )
            .expect("writing to a String cannot fail");
        }
        writeln!(
            output,
            "stfixest.selftest.overall={}",
            if self.passed() { "pass" } else { "fail" }
        )
        .expect("writing to a String cannot fail");
        output
    }

    pub fn require_pass(&self) -> Result<()> {
        if self.passed() {
            Ok(())
        } else {
            let failed = self
                .cases
                .iter()
                .filter(|case| !case.passed)
                .map(|case| case.name)
                .collect::<Vec<_>>()
                .join(", ");
            Err(StFixestError::new(
                ErrorCode::SelfTestFailed,
                format!("native self-test failed: {failed}"),
            ))
        }
    }
}

#[must_use]
pub fn run_native_selftest(threads: usize) -> SelfTestReport {
    let cases = vec![
        run_case("protocol", || Handshake::current(18).validate()),
        run_case("factor_i64", || {
            let encoded = factorize_i64(&[4, 2, 4, -3])?;
            require(
                encoded.codes == vec![2, 1, 2, 0],
                "unexpected integer codes",
            )
        }),
        run_case("factor_f64", || {
            let encoded = factorize_f64(&[-0.0, 0.0, 1.5])?;
            require(
                encoded.codes == vec![0, 0, 1],
                "floating zero was not canonicalized",
            )
        }),
        run_case("factor_string", || {
            let encoded = factorize_strings(&["firm-2", "firm-1", "firm-2"])?;
            require(encoded.codes == vec![1, 0, 1], "unexpected string codes")
        }),
        run_case("interaction", || {
            let first = [0, 0, 1, 1];
            let second = [0, 1, 0, 1];
            let encoded = interaction_codes(&[&first, &second])?;
            require(
                encoded.n_levels() == 4,
                "interaction tuple count is incorrect",
            )
        }),
        run_case("metadata", || minimal_model_spec().validate()),
        run_case("threads", || {
            let config = ThreadConfig::from_requested(threads.max(1), true)?;
            let team = ThreadTeam::new(config);
            let values: Vec<u64> = (0_u64..100_000).collect();
            let first = team.xor_checksum(&values);
            let second = team.xor_checksum(&values);
            require(first == second, "parallel checksum is not repeatable")
        }),
    ];
    SelfTestReport { cases }
}

fn run_case<F>(name: &'static str, function: F) -> SelfTestCase
where
    F: FnOnce() -> Result<()>,
{
    match function() {
        Ok(()) => SelfTestCase {
            name,
            passed: true,
            detail: "ok".to_owned(),
        },
        Err(error) => SelfTestCase {
            name,
            passed: false,
            detail: error.to_string(),
        },
    }
}

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(StFixestError::new(ErrorCode::SelfTestFailed, message))
    }
}

fn minimal_model_spec() -> ModelSpec {
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
            level_values: Vec::new(),
            base: false,
            omitted: false,
            coefficient_column: 1,
            canonical_name: "x".to_owned(),
            pre_materialized_time_series: false,
        }],
        absorb_terms: Vec::new(),
        cluster_terms: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_selftest_passes_single_and_many_threads() {
        for threads in [1, 2, 8, 32] {
            let report = run_native_selftest(threads);
            assert!(report.passed(), "{}", report.render());
        }
    }
}
