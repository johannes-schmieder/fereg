use crate::error::{ErrorCode, FeregError, Result};
use crate::factorize::factorize_f64;
use crate::fe::{factorize_code_interaction, CategoricalCodeView};
use crate::metadata::{CompiledTermGraph, ModelWireV2};
use crate::options::ThreadConfig;
use crate::parallel::ThreadTeam;
use crate::protocol::Handshake;
use rayon::prelude::*;
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
                "fereg.selftest.{}={} {}",
                case.name,
                if case.passed { "pass" } else { "fail" },
                case.detail
            )
            .expect("writing to a String cannot fail");
        }
        writeln!(
            output,
            "fereg.selftest.overall={}",
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
            Err(FeregError::new(
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
        run_case("factor_f64_dense", || {
            let encoded = factorize_f64(&[4.0, 2.0, 4.0, -3.0])?;
            require(
                encoded.codes == vec![2, 1, 2, 0],
                "unexpected dense integer codes",
            )
        }),
        run_case("factor_f64_hashed", || {
            let encoded = factorize_f64(&[-0.0, 0.0, 1.5])?;
            require(
                encoded.codes == vec![0, 0, 1],
                "floating zero was not canonicalized",
            )
        }),
        run_case("factor_f64_ordered", || {
            // At least 16,384 sorted values select the ordered-run path.
            let values: Vec<f64> = (0..16_384_u32)
                .map(|row| f64::from(row / 2) + 0.5)
                .collect();
            let encoded = factorize_f64(&values)?;
            require(
                encoded.levels.len() == 8_192
                    && encoded
                        .codes
                        .iter()
                        .enumerate()
                        .all(|(row, &code)| code as usize == row / 2),
                "unexpected ordered numeric codes",
            )
        }),
        run_case("code_interaction", || {
            let first = [0, 0, 1, 1];
            let second = [0, 1, 0, 1];
            let encoded = factorize_code_interaction(&[
                CategoricalCodeView::new(&first, 2),
                CategoricalCodeView::new(&second, 2),
            ])?;
            require(
                encoded.n_levels() == 4,
                "interaction tuple count is incorrect",
            )
        }),
        run_case("metadata", || {
            let graph = CompiledTermGraph {
                sources: Vec::new(),
                terms: Vec::new(),
            };
            let decoded = ModelWireV2::from_graph(&graph)?.decode()?;
            require(
                decoded == graph,
                "model-wire v2 empty round trip changed graph",
            )
        }),
        run_case("threads", || {
            let config = ThreadConfig::from_requested(threads.max(1), true)?;
            let team = ThreadTeam::new(config);
            let values: Vec<u64> = (0_u64..100_000).collect();
            let expected = chunk_checksum(&values, false);
            // Exercise the cached Rayon team that estimation actions install.
            let (first, workers) =
                team.install(|| (chunk_checksum(&values, true), rayon::current_num_threads()))?;
            let second = team.install(|| chunk_checksum(&values, true))?;
            require(
                workers == team.threads(),
                "worker team has the wrong thread count",
            )?;
            require(
                first == expected && second == expected,
                "parallel checksum differs from the serial checksum",
            )
        }),
    ];
    SelfTestReport { cases }
}

const CHECKSUM_CHUNK: usize = 1_024;

/// Chunk partials depend only on chunk position and are XOR-combined, so the
/// result cannot depend on how a worker team schedules the chunks.
fn chunk_checksum(values: &[u64], parallel: bool) -> u64 {
    let partial = |(chunk, block): (usize, &[u64])| {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ chunk as u64;
        for (index, value) in block.iter().enumerate() {
            let absolute = chunk * CHECKSUM_CHUNK + index;
            state ^= value
                .wrapping_add((absolute as u64).rotate_left((absolute % 63) as u32))
                .rotate_left((absolute % 61) as u32);
        }
        state.rotate_left((chunk % 63) as u32)
    };
    let partials: Vec<u64> = if parallel {
        values
            .par_chunks(CHECKSUM_CHUNK)
            .enumerate()
            .map(partial)
            .collect()
    } else {
        values
            .chunks(CHECKSUM_CHUNK)
            .enumerate()
            .map(partial)
            .collect()
    };
    partials.into_iter().fold(0, |state, value| state ^ value)
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
        Err(FeregError::new(ErrorCode::SelfTestFailed, message))
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
