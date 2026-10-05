//! Common lifetime accounting for development-only action diagnostics.
//!
//! A partial component bound is useful for finding an omission, but cannot be
//! a denominator for a whole-action ratio. Missing stages and unresolved owner
//! families survive every merge, including failed and discarded attempts.

use fereg_core::error::{ErrorCode, FeregError, Result};

pub(crate) const VERSION: u32 = 1;
const STAGES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum Stage {
    Metadata,
    Loading,
    Preparation,
    Separation,
    Fit,
    Iteration,
    Strict,
    Inference,
    Recovery,
    Publication,
}

pub(crate) const ALL_STAGES: [Stage; STAGES] = [
    Stage::Metadata,
    Stage::Loading,
    Stage::Preparation,
    Stage::Separation,
    Stage::Fit,
    Stage::Iteration,
    Stage::Strict,
    Stage::Inference,
    Stage::Recovery,
    Stage::Publication,
];

impl Stage {
    const fn bit(self) -> u16 {
        1 << self as usize
    }
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Loading => "loading",
            Self::Preparation => "preparation",
            Self::Separation => "separation",
            Self::Fit => "fit",
            Self::Iteration => "iteration",
            Self::Strict => "strict",
            Self::Inference => "inference",
            Self::Recovery => "recovery",
            Self::Publication => "publication",
        }
    }
}

/// Stable bits in the private audit schema, never inferred from a sampled peak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Gaps(u16);
impl Gaps {
    #[cfg(test)]
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const TRANSPORT_COMPILER: Self = Self(1 << 0);
    pub(crate) const STRING_LOADING: Self = Self(1 << 1);
    pub(crate) const CONSTRUCTION: Self = Self(1 << 2);
    pub(crate) const NUMERICAL_FRAMES: Self = Self(1 << 3);
    pub(crate) const RETAINED_FE_PARENT: Self = Self(1 << 4);
    pub(crate) const DEPENDENCY_CONTAINERS: Self = Self(1 << 5);
    pub(crate) const WORKER_RUNTIME: Self = Self(1 << 6);
    pub(crate) const PPML_GENERATION_PARENT: Self = Self(1 << 7);
    pub(crate) const RECOVERY: Self = Self(1 << 8);
    pub(crate) const PUBLICATION: Self = Self(1 << 9);
    pub(crate) const SOLVER_ATTEMPT: Self = Self(1 << 10);
    pub(crate) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub(crate) const fn bits(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Estimator {
    Ols,
    Ppml,
}

/// Bytes retained throughout one nested frame and its incremental workspace.
/// Separate sequential frames must be recorded separately, not summed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Frame {
    pub(crate) parents: u128,
    pub(crate) workspace: u128,
}
impl Frame {
    pub(crate) fn peak(self) -> Result<u128> {
        self.parents.checked_add(self.workspace).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "action memory parent/frame sum overflows",
            )
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Entry {
    peak: u128,
    parents: u128,
    workspace: u128,
    gaps: u16,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Ledger {
    required: u16,
    observed: u16,
    entries: [Entry; STAGES],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Summary {
    /// Maximum of the supplied component bounds, even if some scopes are unknown.
    pub(crate) accounted_peak: u128,
    pub(crate) complete_peak: Option<u128>,
    pub(crate) missing_stages: u16,
    pub(crate) gaps: u16,
}
impl Summary {
    pub(crate) fn ratio(self, observed_bytes: u64) -> Option<f64> {
        self.complete_peak
            .filter(|bytes| *bytes != 0)
            .map(|bytes| observed_bytes as f64 / bytes as f64)
    }
}

impl Ledger {
    pub(crate) fn new(estimator: Estimator) -> Self {
        let all = (1 << STAGES) - 1;
        let required = match estimator {
            Estimator::Ppml => all,
            Estimator::Ols => {
                all & !(Stage::Separation.bit() | Stage::Iteration.bit() | Stage::Strict.bit())
            }
        };
        Self {
            required,
            observed: 0,
            entries: [Entry::default(); STAGES],
        }
    }

    pub(crate) fn record(&mut self, stage: Stage, frame: Frame, gaps: Gaps) -> Result<()> {
        if self.required & stage.bit() == 0 {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "action memory stage is not applicable",
            ));
        }
        let peak = frame.peak()?;
        let entry = &mut self.entries[stage as usize];
        if peak >= entry.peak {
            entry.peak = peak;
            entry.parents = frame.parents;
            entry.workspace = frame.workspace;
        }
        // A later smaller/known attempt cannot erase an earlier unknown owner.
        entry.gaps |= gaps.bits();
        self.observed |= stage.bit();
        Ok(())
    }

    pub(crate) fn summary(&self) -> Summary {
        let missing_stages = self.required & !self.observed;
        let mut accounted_peak = 0;
        let mut gaps = 0;
        for entry in self.entries {
            accounted_peak = accounted_peak.max(entry.peak);
            gaps |= entry.gaps;
        }
        Summary {
            accounted_peak,
            missing_stages,
            gaps,
            complete_peak: (missing_stages == 0 && gaps == 0).then_some(accounted_peak),
        }
    }

    pub(crate) fn write(
        &self,
        mut writer: impl std::io::Write,
        observed_bytes: u64,
    ) -> std::io::Result<()> {
        let summary = self.summary();
        writeln!(writer, "FEREG_ACTION_MEMORY|version={VERSION}|accounted_peak={}|complete_peak={:?}|prefix_ratio={:?}|observation=precommit|observed_prefix_bytes={observed_bytes}|missing_stages={}|gaps={}",
            summary.accounted_peak, summary.complete_peak, summary.ratio(observed_bytes), summary.missing_stages, summary.gaps)?;
        for stage in ALL_STAGES {
            if self.required & stage.bit() == 0 {
                continue;
            }
            let e = self.entries[stage as usize];
            writeln!(writer, "FEREG_ACTION_MEMORY_STAGE|version={VERSION}|stage={}|recorded={}|parents={}|workspace={}|peak={}|gaps={}",
                stage.name(), self.observed & stage.bit() != 0, e.parents, e.workspace, e.peak, e.gaps)?;
        }
        Ok(())
    }
}

/// This opt-in development trace precedes every public store. It is not a
/// completion receipt or a change to either estimator's public memory wire.
/// One lifetime timeline, with independent coverage/admission and expected
/// heap channels. Missing expected data remains unknown through composition.
pub(crate) struct Composition {
    pub(crate) ledger: Ledger,
    pub(crate) expected_peak: Option<u128>,
}
impl Composition {
    pub(crate) fn new(estimator: Estimator) -> Self {
        Self {
            ledger: Ledger::new(estimator),
            expected_peak: Some(0),
        }
    }
    pub(crate) fn record(
        &mut self,
        stage: Stage,
        admission: Frame,
        expected: Option<Frame>,
        gaps: Gaps,
    ) -> Result<()> {
        self.ledger.record(stage, admission, gaps)?;
        self.expected_peak = match (self.expected_peak, expected) {
            (Some(peak), Some(frame)) => Some(peak.max(frame.peak()?)),
            _ => None,
        };
        Ok(())
    }
}

pub(crate) fn emit(ledger: &Ledger, observed_bytes: u64) -> Result<()> {
    ledger
        .write(std::io::stderr().lock(), observed_bytes)
        .map_err(|_| FeregError::new(ErrorCode::Internal, "unable to write action memory audit"))
}

/// The ABI samples this after the action and its owned frames have returned
/// (or unwound), before error reporting. It supplies the terminal half of the
/// opt-in trace; it cannot turn an incomplete model into complete coverage.
#[derive(Debug, Clone, Copy)]
#[cfg(any(test, feature = "ffi"))]
pub(crate) struct TerminalObservation {
    pub(crate) estimator: &'static str,
    pub(crate) return_code: i32,
    pub(crate) start_bytes: u64,
    pub(crate) current_bytes: u64,
    pub(crate) peak_bytes: u64,
}

#[cfg(any(test, feature = "ffi"))]
impl TerminalObservation {
    #[cfg(feature = "ffi")]
    pub(crate) fn write_whole_action(self, mut writer: impl std::io::Write) -> std::io::Result<()> {
        writeln!(writer,
            "FEREG_WHOLE_ACTION_MEMORY_TERMINAL|version=1|estimator={}|return_code={}|observation=abi_return|start_bytes={}|current_bytes={}|peak_bytes={}|peak_delta_bytes={}",
            self.estimator, self.return_code, self.start_bytes, self.current_bytes,
            self.peak_bytes, self.peak_bytes.saturating_sub(self.start_bytes))
    }

    pub(crate) fn write(self, mut writer: impl std::io::Write) -> std::io::Result<()> {
        writeln!(writer,
            "FEREG_ACTION_MEMORY_TERMINAL|version=1|estimator={}|return_code={}|observation=action_return|start_bytes={}|current_bytes={}|peak_bytes={}|peak_delta_bytes={}",
            self.estimator, self.return_code, self.start_bytes, self.current_bytes,
            self.peak_bytes, self.peak_bytes.saturating_sub(self.start_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_trace_uses_a_fixed_buffer_and_preserves_error_status() {
        let mut bytes = [0_u8; 512];
        for return_code in [0, 1, 430, 909] {
            let mut buffer = std::io::Cursor::new(bytes.as_mut_slice());
            TerminalObservation {
                estimator: "ppml",
                return_code,
                start_bytes: 100,
                current_bytes: 40,
                peak_bytes: 700,
            }
            .write(&mut buffer)
            .unwrap();
            let length = buffer.position() as usize;
            let text = std::str::from_utf8(&bytes[..length]).unwrap();
            assert!(text.contains("observation=action_return"));
            assert!(text.contains("peak_delta_bytes=600"));
            assert!(text.contains(&format!("return_code={return_code}|")));
        }
        let mut short = &mut bytes[..0];
        assert!(TerminalObservation {
            estimator: "ols",
            return_code: 0,
            start_bytes: 0,
            current_bytes: 0,
            peak_bytes: 0
        }
        .write(&mut short)
        .is_err());
    }
    fn complete(estimator: Estimator) -> Ledger {
        let mut l = Ledger::new(estimator);
        for stage in ALL_STAGES {
            if l.required & stage.bit() != 0 {
                l.record(
                    stage,
                    Frame {
                        parents: 100,
                        workspace: 20,
                    },
                    Gaps::NONE,
                )
                .unwrap();
            }
        }
        l
    }
    #[test]
    fn sums_simultaneous_owners_and_maxes_sequential_attempts() {
        let mut l = complete(Estimator::Ols);
        l.record(
            Stage::Fit,
            Frame {
                parents: 300,
                workspace: 800,
            },
            Gaps::NONE,
        )
        .unwrap();
        l.record(
            Stage::Fit,
            Frame {
                parents: 700,
                workspace: 200,
            },
            Gaps::NONE,
        )
        .unwrap();
        l.record(
            Stage::Publication,
            Frame {
                parents: 200,
                workspace: 500,
            },
            Gaps::NONE,
        )
        .unwrap();
        assert_eq!(l.summary().complete_peak, Some(1100));
        assert_eq!(l.summary().ratio(550), Some(0.5));
        assert_eq!(l.entries[Stage::Fit as usize].parents, 300);
    }
    #[test]
    fn an_unrecorded_stage_is_not_zero_or_complete() {
        for estimator in [Estimator::Ols, Estimator::Ppml] {
            let full = complete(estimator);
            for stage in ALL_STAGES {
                if full.required & stage.bit() == 0 {
                    continue;
                }
                let mut l = full;
                l.observed &= !stage.bit();
                assert_eq!(l.summary().missing_stages, stage.bit());
                assert_eq!(l.summary().complete_peak, None);
                assert_eq!(l.summary().ratio(1), None);
            }
        }
    }
    #[test]
    fn failed_attempt_and_dependency_gaps_cannot_be_cleared_by_a_later_fit() {
        let mut l = complete(Estimator::Ppml);
        l.record(
            Stage::Strict,
            Frame {
                parents: 500,
                workspace: 400,
            },
            Gaps::DEPENDENCY_CONTAINERS.union(Gaps::PPML_GENERATION_PARENT),
        )
        .unwrap();
        l.record(
            Stage::Strict,
            Frame {
                parents: 10,
                workspace: 1,
            },
            Gaps::NONE,
        )
        .unwrap();
        assert_eq!(l.summary().accounted_peak, 900);
        assert_eq!(l.summary().gaps, (1 << 5) | (1 << 7));
        assert_eq!(l.summary().ratio(900), None);
    }
    #[test]
    fn overflow_is_transactional_and_not_applicable_stages_fail_closed() {
        let mut l = complete(Estimator::Ols);
        let before = l.summary();
        assert_eq!(
            l.record(
                Stage::Fit,
                Frame {
                    parents: u128::MAX,
                    workspace: 1
                },
                Gaps::NONE
            )
            .unwrap_err()
            .code,
            ErrorCode::Allocation
        );
        assert_eq!(l.summary(), before);
        assert!(l
            .record(Stage::Iteration, Frame::default(), Gaps::NONE)
            .is_err());
        assert_eq!(l.summary(), before);
    }
    #[test]
    fn the_report_includes_every_required_stage_and_undefined_ratios() {
        let mut l = Ledger::new(Estimator::Ols);
        l.record(
            Stage::Fit,
            Frame {
                parents: 100,
                workspace: 20,
            },
            Gaps::WORKER_RUNTIME,
        )
        .unwrap();
        let mut bytes = Vec::new();
        l.write(&mut bytes, 100).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("complete_peak=None|prefix_ratio=None"));
        assert_eq!(
            text.lines()
                .filter(|l| l.starts_with("FEREG_ACTION_MEMORY_STAGE|"))
                .count(),
            7
        );
        assert!(text.contains("stage=metadata|recorded=false"));
    }
}
