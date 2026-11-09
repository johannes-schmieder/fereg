//! Ordered, fit-lifetime admission before any optional workspace is installed.
use super::*;
use crate::ppml::one_fe_profile::OneFeProfileLayout;
use crate::ppml::profiled_intercept_curve::InterceptCurveLayout;

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

pub(super) struct AdmittedProfiles {
    pub(super) one_fe: Option<OneFeProfileLayout>,
    pub(super) curve: Option<InterceptCurveLayout>,
}
enum ProjectionInstallation {
    Generic {
        parallel: Option<crate::fe::ParallelProjectionPlan>,
        geometry: Option<u128>,
    },
    Coefficient {
        geometry: u128,
        workspace: u128,
        certificate: Option<u128>,
        coupling: Option<u128>,
        joint: Option<u128>,
        cmg: Option<u128>,
        component_checkpoint: Option<u128>,
    },
}
pub(super) struct LinearInstallation {
    topology_sharing: Option<u128>,
    preconditioner_reuse_enabled: bool,
    profiles: AdmittedProfiles,
    centered: Option<u128>,
    fast_products: Option<u128>,
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
        let solver_admission =
            public_solver_memory_admission(prepared, memory_plan, config, solver_rhs, purpose)?;
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
    ) -> Result<Option<OneFeProfileLayout>> {
        let design = inputs.fe_design;
        if !matches!(inputs.purpose, PreparedPpmlIrlsPurpose::Publication)
            || inputs.config.solver_plan.selected != crate::ppml::PpmlSolverFamily::Map
            || !inputs.prepared.fit_has_one_ordinary_intercept_effect()
            || design.effects().len() != 1
            || !design.effects()[0].has_intercept()
            || !design.effects()[0].slopes().is_empty()
        {
            return Ok(None);
        }
        let layout = OneFeProfileLayout::new(
            design.nobs(),
            inputs.prepared.fit_model().columns.len(),
            design.effects()[0].n_levels(),
        )
        .ok();
        let bytes = layout.and_then(|plan| {
            plan.bytes(inputs.config.allocation_snapshot_probe.is_some())
                .ok()
        });
        Ok(
            if self.try_solver_increment(bytes, "one-FE profile workspace size overflows")? {
                layout
            } else {
                None
            },
        )
    }
    fn admit_curve<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
    ) -> Result<Option<InterceptCurveLayout>> {
        let layout = if profile_curve_enabled()
            && matches!(inputs.purpose, PreparedPpmlIrlsPurpose::Publication)
        {
            InterceptCurveLayout::for_design(
                inputs.fe_design,
                inputs.prepared.fit_model().columns.len(),
            )
        } else {
            None
        };
        Ok(
            if self.try_solver_increment(
                layout.map(InterceptCurveLayout::bytes),
                "profile curve size overflows",
            )? {
                layout
            } else {
                None
            },
        )
    }
    fn admit_centered<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
        one_fe_profile_admitted: bool,
    ) -> Result<Option<u128>> {
        let bytes = if one_fe_profile_admitted && inputs.shared_prepared {
            crate::ols::ImplicitCenteredDesign::bytes(
                inputs.fe_design.effects()[0].n_levels(),
                inputs.prepared.fit_model().columns.len(),
                inputs.threads,
            )
            .ok()
        } else {
            None
        };
        Ok(
            if self.try_solver_increment(bytes, "implicit one-FE workspace size overflows")? {
                bytes
            } else {
                None
            },
        )
    }
    fn admit_fast_products<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
    ) -> Result<Option<u128>> {
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
                bytes.filter(|&n| n > 0)
            } else {
                None
            }
        } else {
            None
        };
        Ok(admitted)
    }
    fn admit_projection<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
    ) -> Result<ProjectionInstallation> {
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
        let categorical_checkpoints = matches!(fe_design.effects().len(), 2 | 3)
            && fe_design
                .effects()
                .iter()
                .all(|e| e.has_intercept() && e.slopes().is_empty());
        let coefficient_solver = if matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && config.solver_plan.selected == crate::ppml::PpmlSolverFamily::Map
            && fe_design.effects().len() > 1
        {
            let mut candidate = self.plan;
            match crate::fe::ReusableMapAbsorber::certificate_geometry_bytes(fe_design).and_then(
                |geometry| {
                    crate::ols::ppml_coefficient_lifetime_bytes(
                        fe_design,
                        projection_rhs,
                        effective_threads,
                    )
                    .map(|workspace| (geometry, workspace))
                },
            ) {
                Ok((geometry, workspace))
                    if candidate.admit_reusable_certificate(geometry)
                        && candidate.admit_reusable_slope(workspace) =>
                {
                    self.plan = candidate;
                    Some((geometry, workspace))
                }
                Ok(_) => None,
                Err(error) if error.code == ErrorCode::Allocation => None,
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        // Mu exploration has a separate parent separation-phase envelope.
        let projection_plan = if coefficient_solver.is_none()
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
        let certificate_geometry = if coefficient_solver.is_some() {
            None
        } else if matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && config.solver_plan.selected == crate::ppml::PpmlSolverFamily::Map
            && fe_design.effects().len() > 1
            && fe_design
                .effects()
                .iter()
                .any(|effect| !effect.slopes().is_empty())
        {
            match crate::fe::ReusableMapAbsorber::certificate_geometry_bytes(fe_design) {
                Ok(bytes) => self.plan.admit_reusable_certificate(bytes).then_some(bytes),
                Err(error) if error.code == ErrorCode::Allocation => None,
                Err(error) => return Err(error),
            }
        } else {
            None
        };

        if let Some((geometry, workspace)) = coefficient_solver {
            self.admit_coefficient_options(
                inputs,
                categorical_checkpoints,
                projection_rhs,
                geometry,
                workspace,
            )
        } else {
            Ok(ProjectionInstallation::Generic {
                parallel: projection_plan,
                geometry: certificate_geometry,
            })
        }
    }
    fn admit_coefficient_options<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
        categorical_checkpoints: bool,
        projection_rhs: usize,
        geometry: u128,
        workspace: u128,
    ) -> Result<ProjectionInstallation> {
        // Preserve this order: geometry/solver, certificate, coupling,
        // joint finite bank, then the coexisting checkpoint reserve.
        let certificate = self.try_slope_estimate(
            crate::fe::ReusableMapAbsorber::certificate_workspace_bytes(
                inputs.fe_design,
                projection_rhs,
            )
            .map(Some),
        )?;
        let coupling =
            self.try_slope_estimate(crate::fe::ReusableMapAbsorber::coefficient_coupling_bytes(
                inputs.fe_design,
                projection_rhs,
            ))?;
        let joint = if self.rhs > 1 {
            self.try_slope_estimate(
                crate::fe::ReusableMapAbsorber::joint_slope_bytes(inputs.fe_design, self.rhs - 1)
                    .map(Some),
            )?
        } else {
            None
        };
        let cmg = self.admit_cmg(inputs, joint.is_some())?;
        let component_checkpoint =
            self.admit_component_checkpoint(inputs, categorical_checkpoints, projection_rhs)?;
        Ok(ProjectionInstallation::Coefficient {
            geometry,
            workspace,
            certificate,
            coupling,
            joint,
            cmg,
            component_checkpoint,
        })
    }

    /// Auto two-FE reuse of one CMG hierarchy; its own construction is admitted
    /// against the final solver budget when it is attempted.
    fn admit_cmg<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
        joint: bool,
    ) -> Result<Option<u128>> {
        let effects = inputs.fe_design.effects();
        if !joint
            || inputs.config.solver_plan.requested != crate::ppml::PpmlSolverFamily::Auto
            || effects.len() != 2
            || effects
                .iter()
                .any(|e| !e.has_intercept() || !e.slopes().is_empty() || e.channels() != 1)
        {
            return Ok(None);
        }
        let levels = effects[0].n_levels().min(effects[1].n_levels());
        match crate::fe::PpmlCmg::bytes(levels) {
            Ok(bytes) => Ok(self
                .try_solver_increment(Some(bytes), "PPML CMG application banks overflow")?
                .then_some(bytes)),
            Err(error) if error.code == ErrorCode::Allocation => Ok(None),
            Err(error) => Err(error),
        }
    }
    fn try_slope_estimate(&mut self, estimate: Result<Option<u128>>) -> Result<Option<u128>> {
        match estimate {
            Ok(Some(bytes)) => Ok(self.try_slope_increment(bytes).then_some(bytes)),
            Ok(None) => Ok(None),
            Err(error) if error.code == ErrorCode::Allocation => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn admit_component_checkpoint<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        inputs: &AdmissionInputs<'_, P>,
        categorical_checkpoints: bool,
        projection_rhs: usize,
    ) -> Result<Option<u128>> {
        let fe_design = inputs.fe_design;
        let effective_threads = inputs.threads;
        if !categorical_checkpoints || fe_design.effects().len() != 3 {
            return Ok(None);
        }
        let estimate = crate::ols::ppml_component_checkpoint_bytes(
            fe_design,
            projection_rhs,
            effective_threads,
        );
        match estimate {
            Ok(bytes) => Ok(self
                .try_solver_increment(Some(bytes), "low PPML checkpoint overlap overflows")?
                .then_some(bytes)),
            Err(error) if error.code == ErrorCode::Allocation => Ok(None),
            Err(error) => Err(error),
        }
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
        let centered = self.admit_centered(&inputs, one_fe.is_some())?;
        let fast_products = self.admit_fast_products(&inputs)?;
        let projection = self.admit_projection(&inputs)?;
        // Retained indexing now overlaps every later phase. Its temporary
        // construction is already inside the admitted component checkpoint;
        // reserve the retained owner separately before enabling the experiment.
        let topology_sharing = if config.topology_sharing
            && matches!(
                projection,
                ProjectionInstallation::Coefficient {
                    component_checkpoint: Some(_),
                    ..
                }
            ) {
            let bytes = crate::fe::ppml_topology::retained_bytes(
                fe_design.effects().iter().map(|effect| effect.n_levels()),
                fe_design.effects().len(),
            )?;
            self.try_solver_increment(Some(bytes), "PPML topology retention overflows")?
                .then_some(bytes)
        } else {
            None
        };
        // Historical storage shares the already persistent Fast-products
        // reservation. It never expands the ordinary solver execution budget.
        let preconditioner_reuse_enabled = fast_products.is_some()
            && self.plan.dimensions.requested_saved_fe_channels == 0
            && inputs.config.solver_plan.requested == crate::ppml::PpmlSolverFamily::Auto
            && inputs.fe_design.effects().len() == 2;
        Ok(LinearInstallation {
            topology_sharing,
            preconditioner_reuse_enabled,
            profiles: AdmittedProfiles { one_fe, curve },
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
        generations: LinearSessionGenerations,
        fe_design: FeDesign,
        shared_prepared: &mut Option<crate::ols::prepared_fit_view::SharedPreparedOlsData>,
        config: &PreparedPpmlIrlsConfig,
        purpose: PreparedPpmlIrlsPurpose,
        effective_threads: usize,
    ) -> Result<(CertifiedLinearSession<'a, P>, AdmittedProfiles, bool)> {
        let mut session = CertifiedLinearSession::new_with_fe_design(
            prepared,
            frame,
            initial_config,
            generations,
            fe_design,
        )?;
        // A refused centered option retains the caller's shared owner through the fit.
        if let Some(allowance) = self.centered {
            session.enable_group_centered_one_fe(
                shared_prepared
                    .take()
                    .ok_or_else(|| invalid_argument("implicit prepared owner is absent"))?,
                allowance,
            )?;
        }
        if let Some(allowance) = self.fast_products {
            session.enable_fast_canonical_products(allowance)?;
        }
        if self.preconditioner_reuse_enabled {
            session.enable_fast_preconditioner_reuse();
        }
        let low_checkpoint = match self.projection {
            ProjectionInstallation::Generic { parallel, geometry } => {
                if let Some(plan) = parallel {
                    session.enable_ppml_parallel_projection(plan)?;
                }
                if let Some(allowance) = geometry {
                    session.enable_ppml_certificate_geometry(allowance)?;
                }
                None
            }
            ProjectionInstallation::Coefficient {
                geometry,
                workspace,
                certificate,
                coupling,
                joint,
                cmg,
                component_checkpoint,
            } => {
                session.enable_ppml_certificate_geometry(geometry)?;
                let started = crate::diagnostics::phase_timing_enabled().then(Instant::now);
                let installed = session.enable_ppml_coefficient_solver(
                    effective_threads,
                    workspace,
                    certificate,
                    coupling,
                );
                if let Some(started) = started {
                    eprintln!(
                        "FEREG_PHASE|ppml_coefficient_install|seconds={:.9}|rows={}|threads={effective_threads}|call_succeeded={}",
                        started.elapsed().as_secs_f64(),
                        session.fe_design().nobs(),
                        usize::from(installed.is_ok()),
                    );
                }
                installed?;
                if let Some(allowance) = joint {
                    session.enable_ppml_joint_slopes(allowance)?;
                }
                if let Some(allowance) = cmg {
                    session.enable_ppml_cmg(allowance)?;
                }
                component_checkpoint
            }
        };
        session.set_phase_allocator_probe(config.linear_phase_allocation_probe);
        let initial_checkpoint = low_checkpoint.is_some()
            || use_initial_checkpoint_lsmr_accelerator(
                config,
                purpose,
                session.fe_design().nobs(),
                session.fe_design().effects().len(),
            );
        if initial_checkpoint {
            session.enable_checkpoint_lsmr(config.linear.execution.threads.deterministic);
            if let Some(allowance) = low_checkpoint {
                session.enable_component_checkpoint_scaling(allowance, effective_threads)?;
            }
        }
        if let Some(allowance) = self.topology_sharing {
            session.enable_ppml_topology(allowance)?;
        }
        Ok((session, self.profiles, initial_checkpoint))
    }
}
