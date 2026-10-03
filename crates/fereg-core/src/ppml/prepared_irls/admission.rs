//! Ordered, fit-lifetime admission before any optional workspace is installed.
use super::*;

pub(super) struct LinearAdmission {
    plan: PpmlMemoryPlan,
    execution_bytes: u128,
    public: PublicSolverMemoryAdmission,
    rhs: usize,
}

struct AdmissionInputs<'a, P: PreparedOlsFitView + Sync + ?Sized> {
    prepared: &'a P,
    fe_design: &'a FeDesign,
    config: &'a PreparedPpmlIrlsConfig,
    purpose: PreparedPpmlIrlsPurpose,
    threads: usize,
    shared_prepared: bool,
}

pub(super) struct LateProfiles {
    pub(super) one_fe: bool,
    pub(super) curve: bool,
}
struct SlopeOptions {
    certificate: bool,
    coupling: bool,
    joint: bool,
}
struct ProjectionAdmission {
    coefficient_solver: bool,
    projection_rhs: usize,
    categorical_checkpoints: bool,
    parallel: Option<crate::fe::ParallelProjectionPlan>,
    geometry: bool,
}
enum ProjectionInstallation {
    Generic {
        parallel: Option<crate::fe::ParallelProjectionPlan>,
        geometry: bool,
    },
    Coefficient {
        options: SlopeOptions,
        component_checkpoint: bool,
    },
}
pub(super) struct LinearInstallation {
    profiles: LateProfiles,
    centered: bool,
    fast_products: u128,
    projection: ProjectionInstallation,
}

impl LinearAdmission {
    pub(super) fn new_public<P: PreparedOlsFitView + Sync + ?Sized>(
        prepared: &P,
        mut memory_plan: PpmlMemoryPlan,
        config: &mut PreparedPpmlIrlsConfig,
        purpose: PreparedPpmlIrlsPurpose,
    ) -> Result<Self> {
        config.linear.execution.mode = match memory_plan.selected {
            PpmlMemoryRoute::Fast => PreparedOlsExecutionMode::Fast,
            PpmlMemoryRoute::Low => PreparedOlsExecutionMode::Low,
        };
        config.linear.execution.poolsize = config.memory.poolsize;
        let solver_rhs = memory_plan
            .dimensions
            .finite_columns
            .checked_add(1)
            .ok_or_else(|| allocation("PPML FE solver RHS width overflows"))?;
        let mut solver_admission =
            public_solver_memory_admission(prepared, memory_plan, config, solver_rhs, purpose)?;
        // A modeled budget veto precedes any specialized solver construction.
        // It is selection, not recovery from allocation or numerical failure.
        if config.solver_plan.requested == crate::ppml::PpmlSolverFamily::Auto
            && config.solver_plan.selected == crate::ppml::PpmlSolverFamily::Lsmr
            && memory_plan.usable_bytes.is_some_and(|usable| {
                solver_admission.incremental_bytes
                    > usable.saturating_sub(memory_plan.selected_inventory().peak_bytes)
            })
        {
            config.solver_plan.selected = crate::ppml::PpmlSolverFamily::Map;
            solver_admission = PublicSolverMemoryAdmission::default();
        }
        memory_plan = memory_plan.with_solver_workspace(solver_admission.incremental_bytes)?;
        let solver_execution_bytes = memory_plan
            .selected_inventory()
            .peak_bytes
            .checked_add(solver_admission.owned_fe_design_shortfall_bytes)
            .ok_or_else(|| allocation("PPML FE solver execution memory overflows"))?;
        let solver_memory_budget = FeSolverMemoryBudget {
            execution_bytes: solver_execution_bytes,
            usable_bytes: memory_plan.usable_bytes.unwrap_or(u128::MAX),
        };
        config.ppml.absorb.solver_memory_budget = Some(solver_memory_budget);
        config.linear.absorb.solver_memory_budget = Some(solver_memory_budget);
        Ok(Self {
            plan: memory_plan,
            execution_bytes: solver_execution_bytes,
            public: solver_admission,
            rhs: solver_rhs,
        })
    }

    fn try_solver_increment(
        &mut self,
        bytes: Option<u128>,
        overflow: &'static str,
    ) -> Result<bool> {
        let Some((bytes, total)) = bytes.and_then(|bytes| {
            self.plan
                .solver_workspace_bytes
                .checked_add(bytes)
                .map(|total| (bytes, total))
        }) else {
            return Ok(false);
        };
        let plan = match self.plan.with_solver_workspace(total) {
            Ok(plan) => plan,
            Err(error) if error.code == ErrorCode::Allocation => return Ok(false),
            Err(error) => return Err(error),
        };
        self.execution_bytes = self
            .execution_bytes
            .checked_add(bytes)
            .ok_or_else(|| allocation(overflow))?;
        self.plan = plan;
        Ok(true)
    }

    fn try_slope_increment(&mut self, bytes: u128) -> bool {
        self.plan
            .reusable_slope_bytes
            .checked_add(bytes)
            .is_some_and(|total| self.plan.admit_reusable_slope(total))
    }

    fn admit_one_fe<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
    ) -> Result<bool> {
        let prepared = inputs.prepared;
        let fe_design = inputs.fe_design;
        let config = inputs.config;
        let purpose = inputs.purpose;
        let admitted = if matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && config.solver_plan.selected == crate::ppml::PpmlSolverFamily::Map
            && prepared.fit_has_one_ordinary_intercept_effect()
            && fe_design.effects().len() == 1
            && fe_design.effects()[0].has_intercept()
            && fe_design.effects()[0].slopes().is_empty()
        {
            let profile_bytes = ProfiledOneFe::bytes(
                fe_design.nobs(),
                prepared.fit_model().columns.len(),
                fe_design.effects()[0].n_levels(),
            )
            .ok()
            .and_then(|bytes| {
                if config.allocation_snapshot_probe.is_some() {
                    ProfiledOneFe::audit_bytes(
                        fe_design.nobs(),
                        prepared.fit_model().columns.len(),
                        fe_design.effects()[0].n_levels(),
                    )
                    .ok()
                    .and_then(|audit| bytes.checked_add(audit))
                } else {
                    Some(bytes)
                }
            });
            self.try_solver_increment(profile_bytes, "one-FE profile workspace size overflows")?
        } else {
            false
        };
        Ok(admitted)
    }
    fn admit_curve<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
    ) -> Result<bool> {
        let prepared = inputs.prepared;
        let fe_design = inputs.fe_design;
        let purpose = inputs.purpose;
        let admitted = if profile_curve_enabled()
            && matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && ProfiledInterceptCurve::selected_effect(fe_design).is_some()
        {
            let effect =
                &fe_design.effects()[ProfiledInterceptCurve::selected_effect(fe_design).unwrap()];
            let bytes = ProfiledInterceptCurve::bytes(
                fe_design.nobs(),
                prepared.fit_model().columns.len(),
                effect.n_levels(),
                effect.slopes().len(),
            );
            self.try_solver_increment(Some(bytes), "profile curve size overflows")?
        } else {
            false
        };
        Ok(admitted)
    }
    fn admit_centered<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
        one_fe_profile_admitted: bool,
    ) -> Result<bool> {
        let prepared = inputs.prepared;
        let fe_design = inputs.fe_design;
        let effective_threads = inputs.threads;
        let admitted = if one_fe_profile_admitted && inputs.shared_prepared {
            let bytes = crate::ols::ImplicitCenteredDesign::bytes(
                fe_design.effects()[0].n_levels(),
                prepared.fit_model().columns.len(),
                effective_threads,
            )
            .ok();
            self.try_solver_increment(bytes, "implicit one-FE workspace size overflows")?
        } else {
            false
        };
        Ok(admitted)
    }
    fn admit_fast_products<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
    ) -> Result<u128> {
        let fe_design = inputs.fe_design;
        let config = inputs.config;
        let purpose = inputs.purpose;
        let effective_threads = inputs.threads;
        let admitted = if matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && config.linear.execution.mode == PreparedOlsExecutionMode::Fast
            && config.linear.absorb.solver == FeSolverKind::Map
            && matches!(fe_design.effects().len(), 2 | 3)
            && fe_design
                .effects()
                .iter()
                .all(|e| e.has_intercept() && e.slopes().is_empty())
        {
            let pool = config
                .linear
                .execution
                .poolsize
                .max(1)
                .min(self.plan.dimensions.finite_columns.max(1));
            let bytes = (fe_design.nobs() as u128)
                .checked_mul(pool as u128)
                .and_then(|n| n.checked_mul(16))
                .filter(|&allowance| {
                    fe_design.effects().len() == 2
                        || crate::ols::canonical_three_workspace_bytes(
                            fe_design.nobs(),
                            self.plan.dimensions.finite_columns.saturating_add(1),
                            std::array::from_fn(|e| fe_design.effects()[e].n_levels()),
                            effective_threads,
                        )
                        .is_some_and(|bytes| bytes <= allowance)
                });
            if self.try_solver_increment(bytes, "fast canonical product workspace overflows")? {
                bytes.unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };
        Ok(admitted)
    }
    fn admit_projection<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
    ) -> Result<ProjectionAdmission> {
        let fe_design = inputs.fe_design;
        let config = inputs.config;
        let purpose = inputs.purpose;
        let effective_threads = inputs.threads;
        let solver_rhs = self.rhs;
        let projection_rhs = match config.linear.execution.mode {
            PreparedOlsExecutionMode::Fast => solver_rhs,
            PreparedOlsExecutionMode::Low => config
                .memory
                .poolsize
                .max(1)
                .min(self.plan.dimensions.finite_columns.max(1)),
        };
        let categorical_checkpoints = config.linear.execution.mode == PreparedOlsExecutionMode::Low
            && matches!(fe_design.effects().len(), 2 | 3)
            && fe_design
                .effects()
                .iter()
                .all(|e| e.has_intercept() && e.slopes().is_empty());
        let coefficient_solver = if matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && config.solver_plan.selected == crate::ppml::PpmlSolverFamily::Map
            && fe_design.effects().len() > 1
            && (fe_design
                .effects()
                .iter()
                .any(|effect| !effect.slopes().is_empty())
                || ((config.linear.execution.mode == PreparedOlsExecutionMode::Low
                    // The categorical checkpoint/LSMR engines cover at most
                    // three partitions. Higher-dimensional fast fits otherwise
                    // repeat generic observation-space projection for every RHS.
                    || fe_design.effects().len() > 3)
                    && solver_rhs > 1
                    && fe_design
                        .effects()
                        .iter()
                        .all(|effect| effect.has_intercept())))
        {
            let mut candidate = self.plan;
            match crate::fe::ReusableMapAbsorber::certificate_geometry_bytes(fe_design).and_then(
                |geometry| {
                    crate::fe::ReusableMapAbsorber::coefficient_solver_bytes(
                        fe_design,
                        projection_rhs,
                        effective_threads,
                    )
                    .and_then(|workspace| {
                        let checkpoint = if categorical_checkpoints {
                            crate::fe::ReusableCategoricalMapAbsorber::coexisting_owner_bytes(
                                fe_design.effects().len(),
                                projection_rhs,
                            )?
                        } else {
                            0
                        };
                        Ok((
                            geometry,
                            workspace + 8 * fe_design.nobs() as u128 + checkpoint,
                        ))
                    })
                },
            ) {
                Ok((geometry, workspace))
                    if candidate.admit_reusable_certificate(geometry)
                        && candidate.admit_reusable_slope(workspace) =>
                {
                    self.plan = candidate;
                    true
                }
                Ok(_) => false,
                Err(error) if error.code == ErrorCode::Allocation => false,
                Err(error) => return Err(error),
            }
        } else {
            false
        };
        // Mu exploration has a separate parent separation-phase envelope.
        let projection_plan = if !coefficient_solver
            && matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && config.solver_plan.selected == crate::ppml::PpmlSolverFamily::Map
        {
            match crate::fe::ParallelProjectionPlan::for_design_and_threads(
                fe_design,
                projection_rhs,
                effective_threads,
            ) {
                Ok(Some(plan)) if self.plan.admit_reusable_projection(plan.bytes()?) => Some(plan),
                Ok(_) => None,
                Err(error) if error.code == ErrorCode::Allocation => None,
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        let certificate_geometry = if coefficient_solver {
            true
        } else if matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && config.solver_plan.selected == crate::ppml::PpmlSolverFamily::Map
            && fe_design.effects().len() > 1
            && fe_design
                .effects()
                .iter()
                .any(|effect| !effect.slopes().is_empty())
        {
            match crate::fe::ReusableMapAbsorber::certificate_geometry_bytes(fe_design) {
                Ok(bytes) => self.plan.admit_reusable_certificate(bytes),
                Err(error) if error.code == ErrorCode::Allocation => false,
                Err(error) => return Err(error),
            }
        } else {
            false
        };

        Ok(ProjectionAdmission {
            coefficient_solver,
            projection_rhs,
            categorical_checkpoints,
            parallel: projection_plan,
            geometry: certificate_geometry,
        })
    }
    fn try_slope_estimate(&mut self, estimate: Result<Option<u128>>) -> Result<bool> {
        match estimate {
            Ok(Some(bytes)) => Ok(self.try_slope_increment(bytes)),
            Ok(None) => Ok(false),
            Err(error) if error.code == ErrorCode::Allocation => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn admit_slope_options<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
        coefficient_solver: bool,
        projection_rhs: usize,
    ) -> Result<SlopeOptions> {
        if !coefficient_solver {
            return Ok(SlopeOptions {
                certificate: false,
                coupling: false,
                joint: false,
            });
        }
        let design = inputs.fe_design;
        let certificate = self.try_slope_estimate(
            crate::fe::ReusableMapAbsorber::coefficient_certificate_bytes(design, projection_rhs)
                .map(Some),
        )?;
        let coupling = self.try_slope_estimate(
            crate::fe::ReusableMapAbsorber::coefficient_coupling_bytes(design, projection_rhs),
        )?;
        let joint = self.rhs > 1
            && self.try_slope_estimate(
                crate::fe::ReusableMapAbsorber::joint_slope_bytes(design, self.rhs - 1).map(Some),
            )?;
        Ok(SlopeOptions {
            certificate,
            coupling,
            joint,
        })
    }
    fn admit_low_checkpoint<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
        coefficient_solver: bool,
        categorical_checkpoints: bool,
        projection_rhs: usize,
    ) -> Result<bool> {
        let fe_design = inputs.fe_design;
        let effective_threads = inputs.threads;
        let admitted =
            if coefficient_solver && categorical_checkpoints && fe_design.effects().len() == 3 {
                let mut level_storage = [0; 3];
                for (slot, effect) in level_storage.iter_mut().zip(fe_design.effects()) {
                    *slot = effect.n_levels();
                }
                let levels = &level_storage[..fe_design.effects().len()];
                match estimate_checkpoint_lsmr_workspace_bytes(
                    fe_design.nobs(),
                    projection_rhs,
                    levels,
                    effective_threads,
                )
                .and_then(|bytes| {
                    // Scaling overlaps the LSMR frame: N weights, component labels
                    // and maxima. Topology construction uses <32 bytes/level plus
                    // <512 bytes of descriptors for these three effects.
                    let component_bytes = 8 * fe_design.nobs() as u128
                        + 32 * levels.iter().map(|&n| n as u128).sum::<u128>()
                        + 512;
                    let bytes = bytes
                        .checked_add(component_bytes)
                        .ok_or_else(|| allocation("component checkpoint workspace overflows"))?;
                    self.plan
                        .solver_workspace_bytes
                        .checked_add(bytes)
                        .ok_or_else(|| allocation("low PPML checkpoint workspace overflows"))
                        .and_then(|total| {
                            self.plan
                                .with_solver_workspace(total)
                                .map(|plan| (plan, bytes))
                        })
                }) {
                    Ok((plan, bytes)) => {
                        self.plan = plan;
                        self.execution_bytes = self
                            .execution_bytes
                            .checked_add(bytes)
                            .ok_or_else(|| allocation("low PPML checkpoint overlap overflows"))?;
                        true
                    }
                    Err(error) if error.code == ErrorCode::Allocation => false,
                    Err(error) => return Err(error),
                }
            } else {
                false
            };
        Ok(admitted)
    }

    fn prepare_installation<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        prepared: &P,
        fe_design: &FeDesign,
        config: &PreparedPpmlIrlsConfig,
        purpose: PreparedPpmlIrlsPurpose,
        threads: usize,
        shared_prepared: bool,
    ) -> Result<LinearInstallation> {
        let inputs = AdmissionInputs {
            prepared,
            fe_design,
            config,
            purpose,
            threads,
            shared_prepared,
        };
        let one_fe = self.admit_one_fe(&inputs)?;
        let curve = self.admit_curve(&inputs)?;
        let centered = self.admit_centered(&inputs, one_fe)?;
        let fast_products = self.admit_fast_products(&inputs)?;
        let projection = self.admit_projection(&inputs)?;
        let options = self.admit_slope_options(
            &inputs,
            projection.coefficient_solver,
            projection.projection_rhs,
        )?;
        let low = self.admit_low_checkpoint(
            &inputs,
            projection.coefficient_solver,
            projection.categorical_checkpoints,
            projection.projection_rhs,
        )?;
        let projection = if projection.coefficient_solver {
            ProjectionInstallation::Coefficient {
                options,
                component_checkpoint: low,
            }
        } else {
            ProjectionInstallation::Generic {
                parallel: projection.parallel,
                geometry: projection.geometry,
            }
        };
        Ok(LinearInstallation {
            profiles: LateProfiles { one_fe, curve },
            centered,
            fast_products,
            projection,
        })
    }

    pub(super) fn admit_installation<P: PreparedOlsFitView + Sync + ?Sized>(
        mut self,
        prepared: &P,
        design: &FeDesign,
        config: &mut PreparedPpmlIrlsConfig,
        purpose: PreparedPpmlIrlsPurpose,
        threads: usize,
        shared_prepared: bool,
    ) -> Result<(
        PpmlMemoryPlan,
        PublicSolverMemoryAdmission,
        FeSolverMemoryBudget,
        LinearInstallation,
    )> {
        let installation =
            self.prepare_installation(prepared, design, config, purpose, threads, shared_prepared)?;
        let (plan, public, budget) = self.finish(config)?;
        Ok((plan, public, budget, installation))
    }

    fn finish(
        self,
        config: &mut PreparedPpmlIrlsConfig,
    ) -> Result<(
        PpmlMemoryPlan,
        PublicSolverMemoryAdmission,
        FeSolverMemoryBudget,
    )> {
        // Execution overlap differs from the plan peak; do not derive one from the other.
        let budget = FeSolverMemoryBudget {
            execution_bytes: self
                .execution_bytes
                .checked_add(self.plan.reusable_projection_bytes)
                .and_then(|n| n.checked_add(self.plan.reusable_certificate_bytes))
                .and_then(|n| n.checked_add(self.plan.reusable_slope_bytes))
                .ok_or_else(|| allocation("PPML projection execution memory overflows"))?,
            usable_bytes: self.plan.usable_bytes.unwrap_or(u128::MAX),
        };
        config.ppml.absorb.solver_memory_budget = Some(budget);
        config.linear.absorb.solver_memory_budget = Some(budget);
        Ok((self.plan, self.public, budget))
    }
}

impl LinearInstallation {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn install<'a, P: PreparedOlsFitView + Sync + ?Sized>(
        self,
        prepared: &'a P,
        frame: LinearSolveFrame<'_>,
        initial_config: PreparedOlsFitConfig,
        fe_design: FeDesign,
        shared_prepared: &mut Option<crate::prepared_fit_view::SharedPreparedOlsData>,
        config: &PreparedPpmlIrlsConfig,
        purpose: PreparedPpmlIrlsPurpose,
        effective_threads: usize,
    ) -> Result<(CertifiedLinearSession<'a, P>, LateProfiles, bool)> {
        let mut session = if matches!(self.projection, ProjectionInstallation::Coefficient { .. }) {
            CertifiedLinearSession::new_with_fe_design(
                prepared,
                frame,
                initial_config,
                LinearSessionGenerations::new(1, 1, 1, 1, 1),
                fe_design,
            )?
        } else {
            CertifiedLinearSession::new_with_ppml_fe_design(
                prepared,
                frame,
                initial_config,
                LinearSessionGenerations::new(1, 1, 1, 1, 1),
                fe_design,
            )?
        };
        // A refused centered option retains the caller's shared owner through the fit.
        if self.centered {
            session.enable_group_centered_one_fe(
                shared_prepared
                    .take()
                    .ok_or_else(|| invalid_argument("implicit prepared owner is absent"))?,
            )?;
        }
        if self.fast_products > 0 {
            session.enable_fast_canonical_products(self.fast_products)?;
        }
        let low_checkpoint = match self.projection {
            ProjectionInstallation::Generic { parallel, geometry } => {
                if let Some(plan) = parallel {
                    session.enable_ppml_parallel_projection(plan)?;
                }
                if geometry {
                    session.enable_ppml_certificate_geometry()?;
                }
                false
            }
            ProjectionInstallation::Coefficient {
                options,
                component_checkpoint,
            } => {
                session.enable_ppml_certificate_geometry()?;
                session.enable_ppml_coefficient_solver(
                    effective_threads,
                    options.certificate,
                    options.coupling,
                )?;
                if options.joint {
                    session.enable_ppml_joint_slopes()?;
                }
                component_checkpoint
            }
        };
        session.set_phase_allocator_probe(config.linear_phase_allocation_probe);
        let initial_checkpoint = low_checkpoint
            || use_initial_checkpoint_lsmr_accelerator(
                config,
                purpose,
                session.fe_design().nobs(),
                session.fe_design().effects().len(),
            );
        if initial_checkpoint {
            session.enable_checkpoint_lsmr();
            if low_checkpoint {
                session.enable_component_checkpoint_scaling();
            }
        }
        Ok((session, self.profiles, initial_checkpoint))
    }
}
