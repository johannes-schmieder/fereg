//! Ordinary direction storage is exclusive; the canonical checkpoint and curve coexist.
use super::admission::AdmittedProfiles;
use super::*;

// Inline storage preserves the existing allocation and lifetime envelope.
#[allow(clippy::large_enum_variant)]
pub(super) enum OrdinaryDirection {
    Canonical,
    Profile(ProfiledOneFe),
    LsmrJoint(ReusableCategoricalJointLsmr),
    LsmrFused(ReusableCategoricalJointLsmr),
    Cmg(ReusableCategoricalFwlCmg),
}

impl OrdinaryDirection {
    pub(super) fn is_canonical(&self) -> bool {
        matches!(self, Self::Canonical)
    }
    pub(super) fn profile(&self) -> Option<&ProfiledOneFe> {
        if let Self::Profile(profile) = self {
            Some(profile)
        } else {
            None
        }
    }
    pub(super) fn profile_mut(&mut self) -> Option<&mut ProfiledOneFe> {
        if let Self::Profile(profile) = self {
            Some(profile)
        } else {
            None
        }
    }
    pub(super) fn lsmr(&self) -> Option<&ReusableCategoricalJointLsmr> {
        if let Self::LsmrJoint(workspace) | Self::LsmrFused(workspace) = self {
            Some(workspace)
        } else {
            None
        }
    }
    pub(super) fn lsmr_mut(
        &mut self,
    ) -> Option<(&mut ReusableCategoricalJointLsmr, PpmlSolverEngine)> {
        match self {
            Self::LsmrJoint(workspace) => Some((workspace, PpmlSolverEngine::LsmrJoint)),
            Self::LsmrFused(workspace) => Some((workspace, PpmlSolverEngine::LsmrFused)),
            _ => None,
        }
    }
    pub(super) fn cmg(&self) -> Option<&ReusableCategoricalFwlCmg> {
        if let Self::Cmg(cmg) = self {
            Some(cmg)
        } else {
            None
        }
    }
    pub(super) fn cmg_mut(&mut self) -> Option<&mut ReusableCategoricalFwlCmg> {
        if let Self::Cmg(cmg) = self {
            Some(cmg)
        } else {
            None
        }
    }
    pub(super) fn specialized_engine(&self) -> Option<PpmlSolverEngine> {
        match self {
            Self::LsmrJoint(_) => Some(PpmlSolverEngine::LsmrJoint),
            Self::LsmrFused(_) => Some(PpmlSolverEngine::LsmrFused),
            Self::Cmg(_) => Some(PpmlSolverEngine::CmgPrepared),
            Self::Canonical | Self::Profile(_) => None,
        }
    }
    pub(super) fn active_slots(&self) -> Option<&[usize]> {
        match self {
            Self::LsmrJoint(workspace) | Self::LsmrFused(workspace) => {
                Some(workspace.active_slots())
            }
            Self::Cmg(cmg) => Some(cmg.active_slots()),
            Self::Profile(profile) => Some(profile.active_slots()),
            Self::Canonical => None,
        }
    }
    pub(super) fn retire(&mut self) {
        *self = Self::Canonical;
    }
    pub(super) fn retire_profile(&mut self) {
        if matches!(self, Self::Profile(_)) {
            self.retire();
        }
    }
    pub(super) fn after_commit(&mut self, basis_unchanged: bool) {
        if !basis_unchanged {
            self.retire_profile();
        }
        match self {
            Self::LsmrJoint(workspace) | Self::LsmrFused(workspace) => workspace.reset_warm_start(),
            Self::Cmg(cmg) => cmg.reset_warm_start(),
            Self::Canonical | Self::Profile(_) => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn initialize<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        prepared: &P,
        design: &FeDesign,
        columns: Option<&[usize]>,
        profiles: &AdmittedProfiles,
        outcome: &[f64],
        prior_weights: &[f64],
        config: PreparedPpmlIrlsConfig,
        engine: PpmlSolverEngine,
        admission: PublicSolverMemoryAdmission,
        budget: FeSolverMemoryBudget,
    ) -> Result<Option<ProfiledInterceptCurve>> {
        if let Some(layout) = profiles.one_fe {
            match layout.construct(
                prepared,
                columns.expect("profile basis"),
                design,
                outcome,
                prior_weights,
                config.linear.execution.threads,
            ) {
                Ok(profile) => *self = Self::Profile(profile),
                Err(error)
                    if matches!(
                        error.code,
                        ErrorCode::Allocation | ErrorCode::NonConvergence
                    ) => {}
                Err(error) => return Err(error),
            }
        }
        let curve = if let Some(layout) = profiles.curve {
            match layout.construct(
                prepared,
                columns.expect("curve basis"),
                design,
                outcome,
                prior_weights,
                config.linear.execution.threads,
            ) {
                Ok(curve) => Some(curve),
                Err(error)
                    if matches!(
                        error.code,
                        ErrorCode::Allocation | ErrorCode::NonConvergence
                    ) =>
                {
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        if engine != PpmlSolverEngine::Map {
            let message = if engine == PpmlSolverEngine::CmgPrepared {
                "fused PPML CMG requires the frozen active basis"
            } else {
                "joint PPML LSMR requires the frozen active basis"
            };
            *self = Self::specialized(
                engine,
                prepared,
                columns.ok_or_else(|| internal(message))?,
                design,
                config,
                admission,
                budget,
            )?;
        }
        Ok(curve)
    }

    fn specialized<P: PreparedOlsFitView + Sync + ?Sized>(
        engine: PpmlSolverEngine,
        prepared: &P,
        columns: &[usize],
        design: &FeDesign,
        config: PreparedPpmlIrlsConfig,
        admission: PublicSolverMemoryAdmission,
        budget: FeSolverMemoryBudget,
    ) -> Result<Self> {
        match engine {
            PpmlSolverEngine::LsmrJoint | PpmlSolverEngine::LsmrFused => {
                let workspace = ReusableCategoricalJointLsmr::new_with_threads(
                    prepared,
                    columns,
                    design,
                    config.linear.execution.threads,
                    joint_lsmr_policy(design.effects().len()),
                )?;
                Ok(if engine == PpmlSolverEngine::LsmrJoint {
                    Self::LsmrJoint(workspace)
                } else {
                    Self::LsmrFused(workspace)
                })
            }
            PpmlSolverEngine::CmgPrepared => Ok(Self::Cmg(ReusableCategoricalFwlCmg::new(
                prepared,
                columns,
                design,
                FusedFwlCmgConfig {
                    map: config.ppml.absorb.map,
                    threads: config.linear.execution.threads,
                    batch_size: admission.cmg_batch_capacity,
                    solver_memory_budget: budget,
                },
            )?)),
            PpmlSolverEngine::Map => Err(internal(
                "accepted PPML strict basis change lacks a specialized engine",
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn replace_basis<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        engine: Option<PpmlSolverEngine>,
        prepared: &P,
        columns: &[usize],
        design: &FeDesign,
        config: PreparedPpmlIrlsConfig,
        admission: PublicSolverMemoryAdmission,
        budget: FeSolverMemoryBudget,
    ) -> Result<()> {
        let engine = engine.ok_or_else(|| {
            internal("accepted PPML strict basis change lacks a specialized engine")
        })?;
        // Releasing the old basis before fallible construction avoids doubling live storage.
        self.retire();
        *self = Self::specialized(engine, prepared, columns, design, config, admission, budget)?;
        Ok(())
    }
}
