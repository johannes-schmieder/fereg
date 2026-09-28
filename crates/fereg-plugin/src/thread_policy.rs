//! Freeze public-command concurrency before team creation and solver admission.
use fereg_core::{MemoryMode, ThreadConfig};
use std::num::NonZeroU16;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Estimator {
    Ols,
    Ppml,
}

pub(crate) struct Workload {
    pub estimator: Estimator,
    pub automatic_solver: bool,
    pub memory: MemoryMode,
    pub effects: usize,
    pub intercept_only: bool,
    pub finite_columns: usize,
}

#[inline]
pub(crate) fn plan(request: ThreadConfig, work: Workload) -> ThreadConfig {
    // Narrow three-FE PPML directions saturate before the largest teams;
    // wide finite work still benefits from them. Resolve once for the whole
    // action so preparation, memory admission, IRLS and publication agree.
    if work.estimator == Estimator::Ppml
        && work.automatic_solver
        && work.memory != MemoryMode::Low
        && work.effects == 3
        && work.intercept_only
        && work.finite_columns.saturating_add(1) <= 16
    {
        request.with_ceiling(NonZeroU16::new(8).expect("positive worker ceiling"))
    } else {
        request
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn narrow() -> Workload {
        Workload {
            estimator: Estimator::Ppml,
            automatic_solver: true,
            memory: MemoryMode::Fast,
            effects: 3,
            intercept_only: true,
            finite_columns: 6,
        }
    }

    #[test]
    fn narrow_ppml_ceiling_keeps_request_and_respects_lower_counts() {
        for count in [1, 2, 4, 8, 16, 28] {
            for memory in [MemoryMode::Auto, MemoryMode::Fast] {
                for deterministic in [false, true] {
                    let request = ThreadConfig::from_requested(count, deterministic).unwrap();
                    let actual = plan(request, Workload { memory, ..narrow() });
                    assert_eq!(actual.resolved(), count.min(8));
                    assert_eq!(actual.requested, request.requested);
                    assert_eq!(actual.deterministic, deterministic);
                }
            }
        }
        let automatic = plan(ThreadConfig::default(), narrow());
        assert_eq!(automatic.requested, None);
        assert_eq!(automatic.resolved(), ThreadConfig::available().min(8));
    }

    #[test]
    fn excluded_workloads_keep_their_requested_team() {
        let request = ThreadConfig::from_requested(28, false).unwrap();
        for work in [
            Workload {
                estimator: Estimator::Ols,
                ..narrow()
            },
            Workload {
                automatic_solver: false,
                ..narrow()
            },
            Workload {
                memory: MemoryMode::Low,
                ..narrow()
            },
            Workload {
                effects: 2,
                ..narrow()
            },
            Workload {
                effects: 4,
                ..narrow()
            },
            Workload {
                intercept_only: false,
                ..narrow()
            },
            Workload {
                finite_columns: 16,
                ..narrow()
            },
            Workload {
                finite_columns: 32,
                ..narrow()
            },
            Workload {
                finite_columns: usize::MAX,
                ..narrow()
            },
        ] {
            assert_eq!(plan(request, work), request);
        }
        assert_eq!(
            plan(
                request,
                Workload {
                    finite_columns: 15,
                    ..narrow()
                }
            )
            .resolved(),
            8
        );
    }
}
