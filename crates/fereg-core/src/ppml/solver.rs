//! Public PPML solver-family identity and bounded execution diagnostics.
//!
//! PPML keeps one canonical independently certified checkpoint session. The
//! initial frame is MAP; explicit specialized families may use current-weight
//! LSMR to compute later checkpoint projections before canonical recertification.

use crate::error::{ErrorCode, FeregError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PpmlSolverFamily {
    Auto = 1,
    Map = 2,
    Lsmr = 3,
    Cmg = 7,
}

impl PpmlSolverFamily {
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Map => "map",
            Self::Lsmr => "lsmr",
            Self::Cmg => "cmg",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PpmlSolverEngine {
    Map = 1,
    LsmrFused = 2,
    LsmrJoint = 3,
    CmgPrepared = 4,
}

impl PpmlSolverEngine {
    pub const ALL: [Self; 4] = [
        Self::Map,
        Self::LsmrFused,
        Self::LsmrJoint,
        Self::CmgPrepared,
    ];

    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Map => "map",
            Self::LsmrFused => "lsmr_fused",
            Self::LsmrJoint => "lsmr_joint",
            Self::CmgPrepared => "cmg_prepared",
        }
    }

    #[must_use]
    pub const fn family(self) -> PpmlSolverFamily {
        match self {
            Self::Map => PpmlSolverFamily::Map,
            Self::LsmrFused | Self::LsmrJoint => PpmlSolverFamily::Lsmr,
            Self::CmgPrepared => PpmlSolverFamily::Cmg,
        }
    }

    const fn index(self) -> usize {
        self as usize - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PpmlSolverFallbackReason {
    None = 0,
    CmgNumericalNonconvergence = 1,
    CheckpointLsmrNumericalNonconvergence = 2,
    CmgAndCheckpointNumericalNonconvergence = 3,
}

impl PpmlSolverFallbackReason {
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CmgNumericalNonconvergence => "cmg_numerical_nonconvergence",
            Self::CheckpointLsmrNumericalNonconvergence => {
                "checkpoint_lsmr_numerical_nonconvergence"
            }
            Self::CmgAndCheckpointNumericalNonconvergence => {
                "cmg_and_checkpoint_numerical_nonconvergence"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PpmlSolverPlan {
    pub(crate) requested: PpmlSolverFamily,
    pub(crate) selected: PpmlSolverFamily,
}

impl PpmlSolverPlan {
    pub(crate) const fn new(requested: PpmlSolverFamily) -> Self {
        let selected = match requested {
            PpmlSolverFamily::Auto => PpmlSolverFamily::Map,
            other => other,
        };
        Self {
            requested,
            selected,
        }
    }

    pub(crate) const fn selected_engine(self, active_finite_columns: usize) -> PpmlSolverEngine {
        match self.selected {
            PpmlSolverFamily::Auto | PpmlSolverFamily::Map => PpmlSolverEngine::Map,
            PpmlSolverFamily::Lsmr if active_finite_columns == 0 => PpmlSolverEngine::LsmrFused,
            PpmlSolverFamily::Lsmr => PpmlSolverEngine::LsmrJoint,
            PpmlSolverFamily::Cmg => PpmlSolverEngine::CmgPrepared,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlSolverEngineReport {
    pub engine: PpmlSolverEngine,
    pub ordinary_frames: usize,
    pub checkpoint_frames: usize,
    pub retries: usize,
    pub fallback_count: usize,
    pub iterations: usize,
    pub setup_seconds: f64,
    pub solve_seconds: f64,
    pub certification_seconds: f64,
    pub absolute_infinity_norm_max: Option<f64>,
    pub relative_infinity_norm_max: Option<f64>,
    pub relative_two_norm_max: Option<f64>,
    pub normal_equation_max: Option<f64>,
    pub projection_absolute_infinity_norm_max: Option<f64>,
    pub backward_error_max: Option<f64>,
}

impl PpmlSolverEngineReport {
    const fn empty(engine: PpmlSolverEngine) -> Self {
        Self {
            engine,
            ordinary_frames: 0,
            checkpoint_frames: 0,
            retries: 0,
            fallback_count: 0,
            iterations: 0,
            setup_seconds: 0.0,
            solve_seconds: 0.0,
            certification_seconds: 0.0,
            absolute_infinity_norm_max: None,
            relative_infinity_norm_max: None,
            relative_two_norm_max: None,
            normal_equation_max: None,
            projection_absolute_infinity_norm_max: None,
            backward_error_max: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlSolverReport {
    pub requested: PpmlSolverFamily,
    pub selected: PpmlSolverFamily,
    pub selected_engine: PpmlSolverEngine,
    pub effective: PpmlSolverFamily,
    pub effective_engine: PpmlSolverEngine,
    pub checkpoint: PpmlSolverFamily,
    pub fallback_reason: PpmlSolverFallbackReason,
    pub fallback_count: usize,
    engines: [PpmlSolverEngineReport; 4],
}

impl PpmlSolverReport {
    pub(crate) const fn new(plan: PpmlSolverPlan, selected_engine: PpmlSolverEngine) -> Self {
        Self {
            requested: plan.requested,
            selected: plan.selected,
            selected_engine,
            effective: plan.selected,
            effective_engine: selected_engine,
            checkpoint: PpmlSolverFamily::Map,
            fallback_reason: PpmlSolverFallbackReason::None,
            fallback_count: 0,
            engines: [
                PpmlSolverEngineReport::empty(PpmlSolverEngine::Map),
                PpmlSolverEngineReport::empty(PpmlSolverEngine::LsmrFused),
                PpmlSolverEngineReport::empty(PpmlSolverEngine::LsmrJoint),
                PpmlSolverEngineReport::empty(PpmlSolverEngine::CmgPrepared),
            ],
        }
    }

    #[must_use]
    pub const fn engines(&self) -> &[PpmlSolverEngineReport; 4] {
        &self.engines
    }

    pub(crate) fn enable_lsmr_checkpoint(&mut self) {
        self.checkpoint = PpmlSolverFamily::Lsmr;
    }

    pub(crate) fn observe(&mut self, observation: PpmlSolverObservation) -> Result<()> {
        observation.validate()?;
        let row = &mut self.engines[observation.engine.index()];
        match observation.frame {
            PpmlSolverFrameKind::Ordinary => increment(&mut row.ordinary_frames, 1)?,
            PpmlSolverFrameKind::Checkpoint => increment(&mut row.checkpoint_frames, 1)?,
        }
        increment(&mut row.retries, observation.retries)?;
        increment(&mut row.iterations, observation.iterations)?;
        row.setup_seconds += observation.setup_seconds;
        row.solve_seconds += observation.solve_seconds;
        row.certification_seconds += observation.certification_seconds;
        update_max(
            &mut row.absolute_infinity_norm_max,
            observation.absolute_infinity_norm,
        );
        update_max(
            &mut row.relative_infinity_norm_max,
            observation.relative_infinity_norm,
        );
        update_max(
            &mut row.relative_two_norm_max,
            observation.relative_two_norm,
        );
        update_max(&mut row.normal_equation_max, observation.normal_equation);
        update_max(
            &mut row.projection_absolute_infinity_norm_max,
            observation.projection_absolute_infinity_norm,
        );
        update_max(&mut row.backward_error_max, observation.backward_error);
        Ok(())
    }

    pub(crate) fn record_cmg_numerical_fallback(
        &mut self,
        failed_attempt: PpmlSolverObservation,
    ) -> Result<()> {
        if failed_attempt.engine != PpmlSolverEngine::CmgPrepared
            || failed_attempt.frame != PpmlSolverFrameKind::Ordinary
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML CMG fallback lacks its failed ordinary-frame observation",
            ));
        }
        self.observe(failed_attempt)?;
        increment(&mut self.fallback_count, 1)?;
        increment(
            &mut self.engines[PpmlSolverEngine::CmgPrepared.index()].fallback_count,
            1,
        )?;
        self.fallback_reason = match self.fallback_reason {
            PpmlSolverFallbackReason::None => PpmlSolverFallbackReason::CmgNumericalNonconvergence,
            PpmlSolverFallbackReason::CheckpointLsmrNumericalNonconvergence => {
                PpmlSolverFallbackReason::CmgAndCheckpointNumericalNonconvergence
            }
            reason => reason,
        };
        self.effective = PpmlSolverFamily::Map;
        self.effective_engine = PpmlSolverEngine::Map;
        Ok(())
    }

    pub(crate) fn record_checkpoint_lsmr_fallback(
        &mut self,
        fallback: PpmlSolverObservation,
    ) -> Result<()> {
        if self.checkpoint != PpmlSolverFamily::Lsmr
            || fallback.engine != PpmlSolverEngine::Map
            || fallback.frame != PpmlSolverFrameKind::Checkpoint
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML checkpoint fallback lacks its certified MAP observation",
            ));
        }
        self.observe(fallback)?;
        increment(&mut self.fallback_count, 1)?;
        increment(
            &mut self.engines[PpmlSolverEngine::Map.index()].fallback_count,
            1,
        )?;
        self.fallback_reason = match self.fallback_reason {
            PpmlSolverFallbackReason::None => {
                PpmlSolverFallbackReason::CheckpointLsmrNumericalNonconvergence
            }
            PpmlSolverFallbackReason::CmgNumericalNonconvergence => {
                PpmlSolverFallbackReason::CmgAndCheckpointNumericalNonconvergence
            }
            reason => reason,
        };
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let expected_selected = match self.requested {
            PpmlSolverFamily::Auto => PpmlSolverFamily::Map,
            requested => requested,
        };
        if self.selected != expected_selected
            || self.selected == PpmlSolverFamily::Auto
            || self.selected_engine.family() != self.selected
            || self.effective == PpmlSolverFamily::Auto
            || self.effective_engine.family() != self.effective
            || !matches!(
                self.checkpoint,
                PpmlSolverFamily::Map | PpmlSolverFamily::Lsmr
            )
            || (self.checkpoint == PpmlSolverFamily::Lsmr
                && !matches!(
                    self.selected,
                    PpmlSolverFamily::Lsmr | PpmlSolverFamily::Cmg
                ))
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML solver identity report is inconsistent",
            ));
        }
        let mut row_fallbacks = 0_usize;
        for (expected_engine, row) in PpmlSolverEngine::ALL.into_iter().zip(self.engines.iter()) {
            if row.engine != expected_engine {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "PPML solver diagnostic rows are out of canonical order",
                ));
            }
            for value in [
                row.setup_seconds,
                row.solve_seconds,
                row.certification_seconds,
            ] {
                if !value.is_finite() || value < 0.0 {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML solver timing is nonfinite or negative",
                    ));
                }
            }
            for value in [
                row.absolute_infinity_norm_max,
                row.relative_infinity_norm_max,
                row.relative_two_norm_max,
                row.normal_equation_max,
                row.projection_absolute_infinity_norm_max,
                row.backward_error_max,
            ]
            .into_iter()
            .flatten()
            {
                if !value.is_finite() || value < 0.0 {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML solver certificate metric is nonfinite or negative",
                    ));
                }
            }
            row_fallbacks = row_fallbacks
                .checked_add(row.fallback_count)
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "PPML solver fallback counter overflowed",
                    )
                })?;
        }
        if row_fallbacks != self.fallback_count {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML solver fallback counters disagree",
            ));
        }
        match self.fallback_reason {
            PpmlSolverFallbackReason::None => {
                if self.fallback_count != 0
                    || self.effective != self.selected
                    || self.effective_engine != self.selected_engine
                {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML no-fallback solver identity is inconsistent",
                    ));
                }
            }
            PpmlSolverFallbackReason::CmgNumericalNonconvergence => {
                if self.selected != PpmlSolverFamily::Cmg
                    || self.selected_engine != PpmlSolverEngine::CmgPrepared
                    || self.effective != PpmlSolverFamily::Map
                    || self.effective_engine != PpmlSolverEngine::Map
                    || self.fallback_count == 0
                    || self.engines[PpmlSolverEngine::CmgPrepared.index()].fallback_count
                        != self.fallback_count
                {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML CMG fallback identity is inconsistent",
                    ));
                }
            }
            PpmlSolverFallbackReason::CheckpointLsmrNumericalNonconvergence => {
                if self.checkpoint != PpmlSolverFamily::Lsmr
                    || self.effective != self.selected
                    || self.effective_engine != self.selected_engine
                    || self.fallback_count == 0
                    || self.engines[PpmlSolverEngine::Map.index()].fallback_count
                        != self.fallback_count
                {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML checkpoint LSMR fallback identity is inconsistent",
                    ));
                }
            }
            PpmlSolverFallbackReason::CmgAndCheckpointNumericalNonconvergence => {
                let cmg_fallbacks =
                    self.engines[PpmlSolverEngine::CmgPrepared.index()].fallback_count;
                let checkpoint_fallbacks =
                    self.engines[PpmlSolverEngine::Map.index()].fallback_count;
                if self.selected != PpmlSolverFamily::Cmg
                    || self.selected_engine != PpmlSolverEngine::CmgPrepared
                    || self.checkpoint != PpmlSolverFamily::Lsmr
                    || self.effective != PpmlSolverFamily::Map
                    || self.effective_engine != PpmlSolverEngine::Map
                    || cmg_fallbacks == 0
                    || checkpoint_fallbacks == 0
                    || cmg_fallbacks.saturating_add(checkpoint_fallbacks) != self.fallback_count
                {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "PPML combined CMG/checkpoint fallback identity is inconsistent",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PpmlSolverFrameKind {
    Ordinary,
    Checkpoint,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PpmlSolverObservation {
    pub(crate) engine: PpmlSolverEngine,
    pub(crate) frame: PpmlSolverFrameKind,
    pub(crate) retries: usize,
    pub(crate) iterations: usize,
    pub(crate) setup_seconds: f64,
    pub(crate) solve_seconds: f64,
    pub(crate) certification_seconds: f64,
    pub(crate) absolute_infinity_norm: Option<f64>,
    pub(crate) relative_infinity_norm: Option<f64>,
    pub(crate) relative_two_norm: Option<f64>,
    pub(crate) normal_equation: Option<f64>,
    pub(crate) projection_absolute_infinity_norm: Option<f64>,
    pub(crate) backward_error: Option<f64>,
}

impl PpmlSolverObservation {
    fn validate(self) -> Result<()> {
        let times = [
            self.setup_seconds,
            self.solve_seconds,
            self.certification_seconds,
        ];
        let norms = [
            self.absolute_infinity_norm,
            self.relative_infinity_norm,
            self.relative_two_norm,
            self.normal_equation,
            self.projection_absolute_infinity_norm,
            self.backward_error,
        ];
        if times.iter().any(|value| !value.is_finite() || *value < 0.0)
            || norms
                .iter()
                .flatten()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML solver diagnostics contain a nonfinite or negative metric",
            ));
        }
        Ok(())
    }
}

fn increment(value: &mut usize, amount: usize) -> Result<()> {
    *value = value.checked_add(amount).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Internal,
            "PPML solver diagnostic counter overflowed",
        )
    })?;
    Ok(())
}

fn update_max(target: &mut Option<f64>, value: Option<f64>) {
    if let Some(value) = value {
        *target = Some(target.as_ref().map_or(value, |current| current.max(value)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_is_reported_separately_from_its_map_selection() {
        let plan = PpmlSolverPlan::new(PpmlSolverFamily::Auto);
        let report = PpmlSolverReport::new(plan, plan.selected_engine(3));
        assert_eq!(report.requested, PpmlSolverFamily::Auto);
        assert_eq!(report.selected, PpmlSolverFamily::Map);
        assert_eq!(report.selected_engine, PpmlSolverEngine::Map);
        assert_eq!(report.checkpoint, PpmlSolverFamily::Map);
    }

    #[test]
    fn lsmr_engine_selection_uses_only_the_active_control_boundary() {
        let plan = PpmlSolverPlan::new(PpmlSolverFamily::Lsmr);
        assert_eq!(plan.selected_engine(0), PpmlSolverEngine::LsmrFused);
        assert_eq!(plan.selected_engine(1), PpmlSolverEngine::LsmrJoint);
    }

    #[test]
    fn certified_lsmr_checkpoint_has_independent_engine_identity() {
        let plan = PpmlSolverPlan::new(PpmlSolverFamily::Cmg);
        let mut report = PpmlSolverReport::new(plan, PpmlSolverEngine::CmgPrepared);
        report.enable_lsmr_checkpoint();
        report
            .observe(PpmlSolverObservation {
                engine: PpmlSolverEngine::LsmrFused,
                frame: PpmlSolverFrameKind::Checkpoint,
                retries: 0,
                iterations: 41,
                setup_seconds: 0.02,
                solve_seconds: 0.03,
                certification_seconds: 0.01,
                absolute_infinity_norm: Some(1e-13),
                relative_infinity_norm: Some(2e-13),
                relative_two_norm: Some(3e-13),
                normal_equation: Some(4e-13),
                projection_absolute_infinity_norm: None,
                backward_error: None,
            })
            .expect("checkpoint observation");
        report.validate().expect("certified checkpoint identity");
        assert_eq!(report.checkpoint, PpmlSolverFamily::Lsmr);
        assert_eq!(report.effective, PpmlSolverFamily::Cmg);
        assert_eq!(report.engines()[1].checkpoint_frames, 1);
    }

    #[test]
    fn checkpoint_lsmr_numerical_fallback_is_row_local_and_nonpermanent() {
        let plan = PpmlSolverPlan::new(PpmlSolverFamily::Lsmr);
        let mut report = PpmlSolverReport::new(plan, PpmlSolverEngine::LsmrJoint);
        report.enable_lsmr_checkpoint();
        report
            .record_checkpoint_lsmr_fallback(PpmlSolverObservation {
                engine: PpmlSolverEngine::Map,
                frame: PpmlSolverFrameKind::Checkpoint,
                retries: 1,
                iterations: 117,
                setup_seconds: 0.02,
                solve_seconds: 0.05,
                certification_seconds: 0.01,
                absolute_infinity_norm: Some(1e-13),
                relative_infinity_norm: Some(2e-13),
                relative_two_norm: Some(3e-13),
                normal_equation: Some(4e-13),
                projection_absolute_infinity_norm: None,
                backward_error: None,
            })
            .expect("checkpoint fallback");
        report.validate().expect("fallback checkpoint identity");
        assert_eq!(report.effective, PpmlSolverFamily::Lsmr);
        assert_eq!(report.fallback_count, 1);
        assert_eq!(report.engines()[0].fallback_count, 1);
        assert_eq!(
            report.fallback_reason,
            PpmlSolverFallbackReason::CheckpointLsmrNumericalNonconvergence
        );
    }

    #[test]
    fn cmg_fallback_is_visible_and_permanent_in_the_report() {
        let plan = PpmlSolverPlan::new(PpmlSolverFamily::Cmg);
        let mut report = PpmlSolverReport::new(plan, PpmlSolverEngine::CmgPrepared);
        report
            .record_cmg_numerical_fallback(PpmlSolverObservation {
                engine: PpmlSolverEngine::CmgPrepared,
                frame: PpmlSolverFrameKind::Ordinary,
                retries: 2,
                iterations: 17,
                setup_seconds: 0.25,
                solve_seconds: 0.5,
                certification_seconds: 0.125,
                absolute_infinity_norm: Some(0.01),
                relative_infinity_norm: Some(0.02),
                relative_two_norm: Some(0.03),
                normal_equation: None,
                projection_absolute_infinity_norm: Some(0.04),
                backward_error: Some(0.05),
            })
            .expect("fallback report");
        assert_eq!(report.effective, PpmlSolverFamily::Map);
        assert_eq!(report.effective_engine, PpmlSolverEngine::Map);
        assert_eq!(
            report.fallback_reason,
            PpmlSolverFallbackReason::CmgNumericalNonconvergence
        );
        assert_eq!(report.fallback_count, 1);
        let cmg = &report.engines()[PpmlSolverEngine::CmgPrepared.index()];
        assert_eq!(cmg.ordinary_frames, 1);
        assert_eq!(cmg.retries, 2);
        assert_eq!(cmg.iterations, 17);
        assert_eq!(cmg.setup_seconds, 0.25);
        assert_eq!(cmg.solve_seconds, 0.5);
        assert_eq!(cmg.certification_seconds, 0.125);
        assert_eq!(cmg.absolute_infinity_norm_max, Some(0.01));
        assert_eq!(cmg.projection_absolute_infinity_norm_max, Some(0.04));
        assert_eq!(cmg.backward_error_max, Some(0.05));
        assert_eq!(cmg.fallback_count, 1);
    }
}
