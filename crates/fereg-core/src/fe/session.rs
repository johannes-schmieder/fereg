//! One immutable FE binding with independent projection and coefficient operations.
//!
//! Raw weights belong to the current call. A coefficient direction is a
//! different mathematical product from the canonical within projection, so
//! the two payloads may coexist without replacing the binding or its authority.
use super::{
    BorrowedMapWeights, CertifiedBorrowedMap, FeDesign, MapConfig, MapGeometryGeneration,
    ReusableCategoricalMapAbsorber, ReusableMapAbsorber, ReusableMapCapacities,
};
use crate::data::DenseMatrix;
use crate::error::{allocation, ErrorCode, FeregError, Result};

pub(crate) struct FeSession<'a> {
    pub(crate) general: Option<ReusableMapAbsorber>,
    pub(crate) categorical: Option<ReusableCategoricalMapAbsorber<'a>>,
    pub(crate) coefficients: bool,
    pub(crate) binding: Option<super::BorrowedInterceptFeDesign<'a>>,
    pub(crate) cmg: Option<super::CmgAbsorber<'a>>,
    pub(crate) lsmr: Option<super::BorrowedLsmrAbsorber<'a>>,
    pub(crate) owned: Option<FeDesign>,
    pub(crate) normalized: Vec<f64>,
    pub(crate) peak: crate::memory::SolverWorkspacePeak,
    pub(crate) setup_seconds: f64,
    pub(crate) resident_bytes: u128,
    norm_handoff_peak: Option<u128>,
    ols_chunked_norms: bool,
    pub(crate) structural_constant: bool,
    pub(crate) absorb_config: Option<super::AbsorbConfig>,
}

impl<'a> FeSession<'a> {
    pub(crate) fn general(projector: ReusableMapAbsorber) -> Self {
        Self {
            general: Some(projector),
            ..Self::default()
        }
    }
    pub(crate) fn categorical(
        projector: ReusableCategoricalMapAbsorber<'a>,
        general: ReusableMapAbsorber,
    ) -> Self {
        Self {
            categorical: Some(projector),
            general: Some(general),
            ..Self::default()
        }
    }
    pub(crate) fn ppml_operator_work(&self) -> Option<u128> {
        self.categorical
            .as_ref()
            .and_then(|p| p.ppml_operator_work())
    }
    pub(crate) fn design(&self) -> &FeDesign {
        self.general
            .as_ref()
            .expect("owned FE score binding")
            .design()
    }
    pub(crate) fn max_rhs(&self) -> usize {
        self.general.as_ref().map_or_else(
            || self.categorical.as_ref().expect("FE projection").max_rhs(),
            |p| p.max_rhs(),
        )
    }
    pub(crate) fn capacities(&self) -> ReusableMapCapacities {
        let mut c = self.general.as_ref().map_or_else(
            || {
                self.categorical
                    .as_ref()
                    .expect("FE projection")
                    .capacities()
            },
            |p| p.capacities(),
        );
        if self.general.is_some() {
            if let Some(p) = &self.categorical {
                let b = p.capacities();
                c.accelerator_state += b.reference_norms
                    + b.residual_norms
                    + b.rhs_scales
                    + b.norm_scales
                    + b.norm_sums
                    + b.group_sums
                    + b.group_corrections
                    + b.projection_gram
                    + b.projection_rhs
                    + b.certificate_columns
                    + b.accelerator_state;
            }
        }
        c
    }
    pub(crate) fn borrows_raw_weights(&self) -> bool {
        self.categorical.is_some() && !self.coefficients
    }
    pub(crate) fn supports_fused_projection(&self) -> bool {
        self.categorical.is_some() || self.coefficients
    }
    #[cfg(test)]
    pub(crate) fn is_ppml_categorical(&self) -> bool {
        self.categorical.is_some() && !self.coefficients
    }
    pub(crate) fn enable_checkpoint_lsmr(&mut self, deterministic: bool) {
        if let Some(p) = self.categorical.as_mut() {
            p.enable_checkpoint_lsmr(deterministic);
        }
    }
    #[cfg(test)]
    pub(crate) fn inject_checkpoint_lsmr_certificate_failure(&mut self) {
        self.categorical
            .as_mut()
            .expect("categorical checkpoint binding")
            .inject_checkpoint_lsmr_certificate_failure();
    }
    #[cfg(test)]
    pub(crate) fn inject_checkpoint_lsmr_fatal_failure(&mut self, code: ErrorCode) {
        self.categorical
            .as_mut()
            .expect("categorical checkpoint binding")
            .inject_checkpoint_lsmr_fatal_failure(code);
    }
    pub(crate) fn absorb(
        &mut self,
        raw_weights: BorrowedMapWeights<'_>,
        normalized_weights: &[f64],
        values: DenseMatrix,
        config: MapConfig,
        structural_constant_if_empty: bool,
        generation: MapGeometryGeneration,
    ) -> Result<CertifiedBorrowedMap> {
        if let Some(p) = self.categorical.as_mut() {
            return p.absorb(raw_weights, values, config);
        }
        let p = self.general.as_mut().ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "FE session has no projection operation",
            )
        })?;
        if self.coefficients {
            p.absorb_in_generation(
                normalized_weights,
                values,
                config,
                structural_constant_if_empty,
                Some(generation),
            )
        } else {
            p.absorb(
                normalized_weights,
                values,
                config,
                structural_constant_if_empty,
            )
        }
    }
}

impl Default for FeSession<'_> {
    fn default() -> Self {
        Self {
            general: None,
            categorical: None,
            coefficients: false,
            binding: None,
            cmg: None,
            lsmr: None,
            owned: None,
            normalized: Vec::new(),
            peak: crate::memory::SolverWorkspacePeak::ZERO,
            setup_seconds: 0.0,
            resident_bytes: 0,
            norm_handoff_peak: None,
            ols_chunked_norms: false,
            structural_constant: false,
            absorb_config: None,
        }
    }
}

impl<'a> FeSession<'a> {
    pub(crate) fn for_ols<P: crate::ols::prepared_fit_view::PreparedOlsFitView + ?Sized>(
        prepared: &'a P,
        weights: BorrowedMapWeights<'a>,
        config: crate::ols::PreparedOlsFitConfig,
    ) -> Result<Self> {
        let rhs = match config.execution.mode {
            crate::ols::PreparedOlsExecutionMode::Fast => prepared
                .fit_model()
                .columns
                .len()
                .checked_add(1)
                .ok_or_else(|| allocation("FE RHS count overflows"))?,
            crate::ols::PreparedOlsExecutionMode::Low => config
                .execution
                .poolsize
                .max(1)
                .min(prepared.fit_model().columns.len().max(1)),
        };
        Self::for_ols_rhs(prepared, weights, config, rhs)
    }

    pub(crate) fn for_ols_rhs<P: crate::ols::prepared_fit_view::PreparedOlsFitView + ?Sized>(
        prepared: &'a P,
        weights: BorrowedMapWeights<'a>,
        config: crate::ols::PreparedOlsFitConfig,
        rhs: usize,
    ) -> Result<Self> {
        let mut owner = Self {
            structural_constant: prepared.fit_has_absorbed_intercept(),
            ols_chunked_norms: config.execution.mode == crate::ols::PreparedOlsExecutionMode::Low,
            absorb_config: Some(config.absorb),
            ..Self::default()
        };
        if config.absorb.solver == super::FeSolverKind::Squarem {
            owner.prepare_owned(prepared, weights, rhs, config)?;
            return Ok(owner);
        }
        let borrowed = if config.absorb.solver == super::FeSolverKind::Cmg {
            Some(prepared.fit_borrowed_cmg_design()?)
        } else {
            prepared.fit_borrowed_intercept_map_design()?
        };
        if let Some(design) = borrowed {
            match config.absorb.solver {
                super::FeSolverKind::Cmg => {
                    owner.cmg = Some(super::CmgAbsorber::build(
                        design,
                        weights,
                        super::CmgConfig {
                            map: config.absorb.map,
                            threads: config.execution.threads,
                            batch_size: rhs,
                            retain_coefficients: config.absorb.retain_coefficients,
                            requested_solver: super::FeSolverKind::Cmg,
                            components: super::explicit_cmg_components(super::FeSolverKind::Cmg),
                            solver_memory_budget: config.absorb.solver_memory_budget,
                        },
                        prepared.fit_structural_intercept(),
                    )?);
                }
                super::FeSolverKind::Lsmr if !design.effects().is_empty() => {
                    owner.lsmr = Some(super::BorrowedLsmrAbsorber::new(
                        design,
                        weights,
                        config.absorb,
                    )?);
                }
                super::FeSolverKind::Auto if design.effects().len() == 2 => {
                    // A common structural binding, independent of output, VCE,
                    // worker budget or public storage choice.
                    let mut bytes = super::ReusableCategoricalMapAbsorber::owner_bytes(
                        &design,
                        rhs,
                        config.execution.threads.resolved(),
                    )?;
                    if config.absorb.retain_coefficients {
                        let levels: Vec<_> =
                            design.effects().iter().map(|e| e.n_levels()).collect();
                        let product = crate::memory::estimate_fe_solver_memory(
                            design.nobs(),
                            rhs,
                            &levels,
                            config.execution.threads.resolved(),
                            config.absorb.lsmr.local_size,
                        )?
                        .coefficient_batch_bytes;
                        bytes =
                            bytes
                                .checked_add(product.checked_mul(3).ok_or_else(|| {
                                    allocation("FE coefficient capture overflows")
                                })?)
                                .ok_or_else(|| allocation("FE graph capture overlap overflows"))?;
                    }
                    if config
                        .absorb
                        .solver_memory_budget
                        .is_some_and(|b| !b.fits(bytes))
                    {
                        owner.binding = Some(design);
                        return Ok(owner);
                    }
                    owner.admit(bytes, config.absorb)?;
                    let mut p = super::ReusableCategoricalMapAbsorber::new(
                        design,
                        rhs,
                        config.execution.threads.resolved(),
                    )?;
                    p.retain_coefficients = config.absorb.retain_coefficients;
                    owner.categorical = Some(p);
                }
                _ => owner.binding = Some(design),
            }
        } else {
            owner.prepare_owned(prepared, weights, rhs, config)?;
        }
        Ok(owner)
    }

    fn admit(&mut self, bytes: u128, config: super::AbsorbConfig) -> Result<()> {
        if config.solver_memory_budget.is_some_and(|b| !b.fits(bytes)) {
            return Err(allocation(
                "FE session exceeds the joint execution/solver budget",
            ));
        }
        self.peak = self
            .peak
            .max(crate::memory::SolverWorkspacePeak::from_bytes(bytes)?);
        Ok(())
    }

    fn prepare_owned<P: crate::ols::prepared_fit_view::PreparedOlsFitView + ?Sized>(
        &mut self,
        prepared: &P,
        weights: BorrowedMapWeights<'_>,
        rhs: usize,
        config: crate::ols::PreparedOlsFitConfig,
    ) -> Result<()> {
        let started = std::time::Instant::now();
        let memory = prepared.fit_owned_fe_design_memory()?;
        let vectors = if prepared.fit_model().columns.is_empty() {
            4
        } else {
            1
        };
        let weight_bytes = (prepared.fit_sample().n_active() as u128)
            .checked_mul(8 * vectors)
            .ok_or_else(|| allocation("FE caller/weight overlap overflows"))?;
        self.admit(
            memory
                .construction_bytes
                .checked_add(weight_bytes)
                .ok_or_else(|| allocation("FE construction overlap overflows"))?,
            config.absorb,
        )?;
        let design = prepared.fit_owned_fe_design()?;
        self.normalized
            .try_reserve_exact(design.nobs())
            .map_err(|_| allocation("unable to reserve FE normalized weights"))?;
        if self.normalized.capacity() != design.nobs() {
            return Err(allocation(
                "FE normalized weight capacity exceeded its admission",
            ));
        }
        match weights {
            BorrowedMapWeights::Unit => self.normalized.resize(design.nobs(), 1.),
            BorrowedMapWeights::Provided(values) => {
                let maximum = values.iter().copied().fold(0.0_f64, f64::max);
                if values.len() != design.nobs()
                    || values.iter().any(|w| !w.is_finite() || *w <= 0.)
                {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "FE weights are invalid",
                    ));
                }
                self.normalized.extend(values.iter().map(|w| w / maximum));
            }
        }
        let retained = memory
            .resident_bytes
            .checked_add(weight_bytes)
            .ok_or_else(|| allocation("FE resident overlap overflows"))?;
        self.resident_bytes = retained;
        let mut absorb = config.absorb;
        if let Some(mut budget) = absorb.solver_memory_budget {
            budget.execution_bytes = budget
                .execution_bytes
                .checked_add(retained)
                .ok_or_else(|| allocation("FE resident execution overflows"))?;
            absorb.solver_memory_budget = Some(budget);
        }
        self.absorb_config = Some(absorb);
        if config.absorb.solver == super::FeSolverKind::Auto {
            let base = ReusableMapAbsorber::base_workspace_bytes(&design, rhs)?;
            self.admit(
                retained
                    .checked_add(base)
                    .ok_or_else(|| allocation("FE workspace overlap overflows"))?,
                config.absorb,
            )?;
            let mut p = ReusableMapAbsorber::new(design, rhs)?;
            self.prepare_general_operations(
                &mut p,
                retained
                    .checked_add(base)
                    .ok_or_else(|| allocation("FE base overlap overflows"))?,
                config,
            )?;
            self.general = Some(p);
        } else {
            self.owned = Some(design);
        }
        self.setup_seconds = started.elapsed().as_secs_f64();
        Ok(())
    }

    /// Admission records every construction attempt, including a partially
    /// allocated optional payload that is dropped after a later refusal.
    fn optional<T>(
        &mut self,
        resident: u128,
        bytes: u128,
        config: super::AbsorbConfig,
        construct: impl FnOnce() -> Result<T>,
    ) -> Result<Option<T>> {
        let Some(total) = resident.checked_add(bytes) else {
            return Ok(None);
        };
        if config.solver_memory_budget.is_some_and(|b| !b.fits(total)) {
            return Ok(None);
        }
        self.admit(total, config)?;
        match construct() {
            Ok(value) => Ok(Some(value)),
            Err(e) if e.code == ErrorCode::Allocation => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn prepare_general_operations(
        &mut self,
        p: &mut ReusableMapAbsorber,
        base: u128,
        config: crate::ols::PreparedOlsFitConfig,
    ) -> Result<()> {
        // Numerical geometry cannot evict an affordable general projection.
        // Structure determines applicability; width/storage/workers are capacities.
        let mut resident = base;
        let geometry = ReusableMapAbsorber::certificate_geometry_bytes(p.design())?;
        if self
            .optional(resident, geometry, config.absorb, || {
                p.enable_certificate_geometry()
            })?
            .is_some()
        {
            resident += geometry;
        }
        resident = self.prepare_coefficient_operation(p, resident, config)?;
        if p.has_certificate_geometry() {
            let bytes = ReusableMapAbsorber::certificate_workspace_bytes(p.design(), p.max_rhs())?;
            if self
                .optional(resident, bytes, config.absorb, || {
                    p.enable_certificate_workspace()
                })?
                .is_some()
            {
                resident += bytes;
            }
        }
        if let Some(plan) = super::projection::ParallelProjectionPlan::for_design_and_threads(
            p.design(),
            p.max_rhs(),
            config.execution.threads.resolved(),
        )? {
            let bytes = plan.bytes()?;
            if self
                .optional(resident, bytes, config.absorb, || {
                    p.enable_parallel_projection(plan)
                })?
                .is_some()
            {
                resident += bytes;
            }
        }
        self.admit(resident, config.absorb)
    }

    fn prepare_coefficient_operation(
        &mut self,
        p: &mut ReusableMapAbsorber,
        mut resident: u128,
        config: crate::ols::PreparedOlsFitConfig,
    ) -> Result<u128> {
        if p.design().effects().len() != 2 || !p.has_certificate_geometry() {
            return Ok(resident);
        }
        let solver = ReusableMapAbsorber::coefficient_solver_bytes(
            p.design(),
            p.max_rhs(),
            config.execution.threads.resolved(),
        )?;
        if self
            .optional(resident, solver, config.absorb, || {
                p.enable_coefficient_solver(config.execution.threads.resolved())
            })?
            .is_none()
        {
            return Ok(resident);
        }
        resident += solver;
        self.coefficients = true;
        if let Some(coupling) =
            ReusableMapAbsorber::coefficient_coupling_bytes(p.design(), p.max_rhs())?
        {
            if self
                .optional(resident, coupling, config.absorb, || {
                    p.enable_coefficient_coupling()
                })?
                .is_some()
            {
                resident += coupling;
            }
        }
        Ok(resident)
    }

    fn attach_ols_norms(
        &mut self,
        result: CertifiedBorrowedMap,
        admission: super::AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        if self.general.is_none() && self.categorical.is_none() {
            return Ok(result);
        }
        // Low fits retain their established fixed-leaf norm reduction. A
        // whole-column FE norm is reusable only when that reduction is serial.
        let rows = result.residualized().nrows();
        if self.ols_chunked_norms
            && crate::parallel::StagePlan::current(rows as u128, rows).lanes > 1
        {
            return Ok(result);
        }
        let rhs = result.residualized().ncols();
        // Handoffs are sequential and use the same frozen RHS capacity. Do
        // not add each returned copy to the historical peak again.
        let bytes = match self.norm_handoff_peak {
            Some(bytes) => bytes,
            None => {
                let capacity = self.general.as_ref().map_or_else(
                    || self.categorical.as_ref().expect("norm owner").max_rhs(),
                    |p| p.max_rhs(),
                );
                let Some(bytes) = self
                    .peak
                    .bytes()
                    .and_then(|n| (n as u128).checked_add(capacity as u128 * 8))
                else {
                    return Ok(result);
                };
                self.norm_handoff_peak = Some(bytes);
                bytes
            }
        };
        if admission
            .solver_memory_budget
            .is_some_and(|b| !b.fits(bytes))
        {
            return Ok(result);
        }
        // The optional handoff coexists with every retained reusable bank.
        // A refusal leaves the certified matrix and caller's norm fallback intact.
        self.admit(bytes, admission)?;
        let mut copy = match crate::memory::Capacity::new(rhs as u128)
            .filled(0.0, "unable to reserve certified RHS norm handoff")
        {
            Ok(copy) => copy,
            Err(e) if e.code == ErrorCode::Allocation => return Ok(result),
            Err(e) => return Err(e),
        };
        let norms = if let Some(p) = &self.general {
            p.certified_rhs_norms(rhs)
        } else {
            self.categorical
                .as_ref()
                .expect("certified categorical norms")
                .certified_rhs_norms(rhs)
        };
        copy.copy_from_slice(norms);
        result.with_residual_norms(copy)
    }

    pub(crate) fn project_ols(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        mut config: super::AbsorbConfig,
        final_residual: bool,
        raw_final: bool,
    ) -> Result<CertifiedBorrowedMap> {
        let admission = config;
        if let Some(stored) = self.absorb_config {
            config.solver_memory_budget = stored.solver_memory_budget;
        }
        let mut result = if let Some(p) = self.cmg.as_mut() {
            if final_residual {
                let p = self.cmg.take().expect("CMG binding");
                if raw_final {
                    p.absorb_raw_final_residual(values, config.map.tolerance)
                } else {
                    p.absorb_final_residual(values, config.map.tolerance)
                }
            } else {
                p.absorb(values)
            }
        } else if let Some(p) = self.lsmr.as_mut() {
            p.absorb(
                weights,
                values,
                MapGeometryGeneration {
                    sample: 0,
                    topology: 0,
                    weights: 0,
                },
                config,
            )
        } else if let Some(p) = self.categorical.as_mut() {
            p.absorb(weights, values, config.map)
        } else if let Some(p) = self.general.as_mut() {
            let generation = MapGeometryGeneration {
                sample: 0,
                topology: 0,
                weights: 0,
            };
            if final_residual && self.coefficients {
                p.absorb_coefficient_residual(&self.normalized, values, config.map, generation)
            } else {
                p.absorb_in_generation(
                    &self.normalized,
                    values,
                    config.map,
                    false,
                    Some(generation),
                )
            }
        } else if let Some(design) = self.binding.as_ref() {
            project_intercepts(design, weights, values, config, self.structural_constant)
        } else {
            let mut values = values;
            let mut report = super::absorb_fixed_effects(
                self.owned.as_ref().expect("owned FE binding"),
                &self.normalized,
                &mut values,
                config,
            )?;
            report.solver_workspace_peak = report
                .solver_workspace_peak
                .with_retained(self.resident_bytes)?
                .max(self.peak);
            CertifiedBorrowedMap::from_certified_parts(values, report)
        }?;
        if !final_residual {
            result = self.attach_ols_norms(result, admission)?;
        }
        let norms = result.take_residual_norms();
        let (values, mut report, coefficients) = result.into_full_parts();
        report.requested_solver = config.solver;
        // Reusable operations allocate their banks at construction; the session
        // owns the complete bank admission, unlike the standalone MAP API.
        report.solver_workspace_peak = if self.general.is_some() {
            match report.solver_workspace_peak.bytes() {
                Some(_) => report.solver_workspace_peak.max(self.peak),
                None => self.peak,
            }
        } else {
            report.solver_workspace_peak.max(self.peak)
        };
        report.setup_seconds += std::mem::take(&mut self.setup_seconds);
        let result = match coefficients {
            Some(coefficients) => CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                values,
                report,
                coefficients,
            )?,
            None => CertifiedBorrowedMap::from_certified_parts(values, report)?,
        };
        if norms.is_empty() {
            Ok(result)
        } else {
            result.with_residual_norms(norms)
        }
    }
}

pub(crate) fn project_intercepts(
    design: &super::BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: super::AbsorbConfig,
    structural_constant: bool,
) -> Result<CertifiedBorrowedMap> {
    use super::FeSolverKind as S;
    match config.solver {
        S::Auto => super::project_borrowed_intercept_structural(
            design,
            weights,
            values,
            config.map,
            config.lsmr,
            config.solver_memory_budget,
            structural_constant,
            config.retain_coefficients,
        ),
        S::Lsmr if !design.effects().is_empty() => super::absorb_borrowed_intercept_lsmr(
            design,
            weights,
            values,
            config.lsmr,
            config.retain_coefficients,
        ),
        S::Cg => {
            let peak = crate::ols::prepared_fit_view::admit_prepared_intercept_cg(
                design,
                values.ncols(),
                config.solver_memory_budget,
                config.retain_coefficients,
            )?;
            // CG treats an empty explicit design as the structural constant,
            // which has no coefficients to retain.
            let options = super::BorrowedCgOptions {
                structural_constant: true,
                retain_coefficients: config.retain_coefficients && !design.effects().is_empty(),
                ..super::BorrowedCgOptions::default()
            };
            let mut r =
                super::absorb_borrowed_intercept_cg(design, weights, values, config.map, options)?;
            r.set_solver_workspace_peak(peak);
            Ok(r)
        }
        S::IronsTuck if !design.effects().is_empty() => {
            super::absorb_borrowed_intercept_irons_tuck_certified(
                design,
                weights,
                values,
                config.map,
                config.retain_coefficients,
            )
        }
        S::Cmg | S::Squarem => Err(FeregError::new(
            ErrorCode::Unsupported,
            "explicit FE backend requires its prepared adapter",
        )),
        S::Lsmr | S::IronsTuck | S::Map => {
            // A structural constant on an empty design has no coefficients to retain.
            let retain =
                config.retain_coefficients && !(structural_constant && design.effects().is_empty());
            super::absorb_borrowed_intercept_map(
                design,
                weights,
                values,
                config.map,
                structural_constant,
                retain,
            )
        }
    }
}
/// Recovery has its own reconstruction authority. Shared ownership does not
/// substitute a projection certificate for original-model reconstruction.
pub(crate) struct RecoveryRequest<'a> {
    pub(crate) topology: &'a super::FinalizedCategoricalTopology,
    pub(crate) effects: &'a [super::CategoricalAbsorbedEffectView<'a>],
    pub(crate) weights: BorrowedMapWeights<'a>,
    pub(crate) target: &'a [f64],
    pub(crate) requested_channels: &'a [bool],
    pub(crate) retained: Option<&'a super::FeCoefficientBatch>,
    pub(crate) known_connected: bool,
    pub(crate) materialize_absorbed_sum: bool,
    pub(crate) lsmr: super::LsmrConfig,
    pub(crate) map: MapConfig,
    pub(crate) memory_budget: Option<crate::memory::FeSolverMemoryBudget>,
    pub(crate) reconstruction_tolerance: Option<f64>,
}
impl FeSession<'_> {
    pub(crate) fn recover(
        request: RecoveryRequest<'_>,
    ) -> Result<super::RecoveredCategoricalEffects> {
        use super::topology_slope_map as recover;
        let r = request;
        let validation = r.topology.validation_workspace_bytes()?;
        let fits = |n| r.memory_budget.is_none_or(|b| b.fits(n));
        if !fits(validation) {
            return Err(allocation(
                "saved-FE topology validation exceeds the joint memory budget",
            ));
        }
        let channels = r.requested_channels.iter().filter(|&&v| v).count();
        let mut shapes = Vec::new();
        shapes
            .try_reserve_exact(r.effects.len())
            .map_err(|_| allocation("unable to reserve recovery shape"))?;
        for (effect, &source) in r.effects.iter().zip(r.topology.effect_to_topology()) {
            shapes.push((
                r.topology.registry().source(source)?.n_levels(),
                effect.has_intercept(),
                effect.slopes().len(),
            ));
        }
        let intercepts = r
            .effects
            .iter()
            .all(|e| e.has_intercept() && e.slopes().is_empty());
        let memory = if r.effects.is_empty() {
            None
        } else if intercepts {
            let levels = shapes.iter().map(|s| s.0).collect::<Vec<_>>();
            Some(crate::memory::estimate_intercept_recovery_memory(
                r.target.len(),
                &levels,
                channels,
                rayon::current_num_threads(),
                r.lsmr.local_size,
            )?)
        } else {
            Some(crate::memory::estimate_slope_recovery_memory(
                r.target.len(),
                &shapes,
                channels,
                r.topology.compacted_sources().len(),
                validation,
                rayon::current_num_threads(),
                r.lsmr.local_size,
            )?)
        };
        let mut peak = crate::memory::SolverWorkspacePeak::ZERO;
        if let Some(c) = r
            .retained
            .filter(|_| memory.is_none_or(|m| fits(m.retained_peak_bytes)))
        {
            include_recovery_peak(&mut peak, memory.map(|m| m.retained_peak_bytes))?;
            match recover::recover_finalized_categorical_effects_from_coefficients_selected_certified(
                r.topology, r.effects, r.weights, r.target, r.requested_channels, c, r.map.tolerance,
                r.known_connected, r.materialize_absorbed_sum) {
                Ok(v) => return Ok(v.with_solver_workspace_peak(peak)),
                Err(e) if numerical_recovery_miss(&e) => {},
                Err(e) => return Err(e),
            }
        }
        if r.effects.len() == 1 && !r.effects[0].slopes().is_empty() && r.effects[0].channels() <= 4
        {
            let extra = recover::single_slope_recovery_extra_bytes(
                r.target.len(),
                shapes[0].0,
                r.effects[0].channels(),
                rayon::current_num_threads(),
            )?;
            let bytes = memory
                .and_then(|m| m.retained_peak_bytes.checked_add(extra))
                .ok_or_else(|| allocation("direct recovery overlap overflows"))?;
            if fits(bytes) {
                include_recovery_peak(&mut peak, Some(bytes))?;
                match recover::recover_finalized_single_slope_direct_certified(
                    r.topology,
                    r.effects,
                    r.weights,
                    r.target,
                    r.requested_channels,
                    r.map.tolerance,
                    r.materialize_absorbed_sum,
                    false,
                ) {
                    Ok(v) => return Ok(v.with_solver_workspace_peak(peak)),
                    Err(e) if numerical_recovery_miss(&e) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        // A two-intercept FE span has a direct component graph reconstruction.
        // Every edge, requested channel and normalization is independently checked.
        if intercepts && r.effects.len() == 2 && memory.is_some_and(|m| fits(m.retained_peak_bytes))
        {
            include_recovery_peak(&mut peak, memory.map(|m| m.retained_peak_bytes))?;
            match recover::recover_finalized_two_effects_graph_certified(
                r.topology,
                r.effects,
                r.weights,
                r.target,
                r.requested_channels,
                r.map.tolerance,
                r.known_connected,
                r.materialize_absorbed_sum,
            ) {
                Ok(v) => return Ok(v.with_solver_workspace_peak(peak)),
                Err(e) if numerical_recovery_miss(&e) => {}
                Err(e) => return Err(e),
            }
        }
        // General reconstruction uses repository-owned fallible coefficient
        // banks. Upstream LSMR recovery cannot provide the allocation-failure
        // contract required by the shared OLS/PPML publication transaction.
        if memory.is_some_and(|m| !fits(m.map_peak_bytes)) {
            return Err(allocation(
                "saved-FE recovery does not fit the joint execution/recovery memory plan",
            ));
        }
        include_recovery_peak(&mut peak, memory.map(|m| m.map_peak_bytes))?;
        recover::recover_finalized_categorical_effects_map_selected_certified(
            r.topology,
            r.effects,
            r.weights,
            r.target,
            r.requested_channels,
            // Small FE moments alone do not certify a complete slope decomposition.
            // Preserve the internal recovery target used before this shared owner;
            // publication still independently checks the requested public tolerance.
            MapConfig {
                tolerance: r.map.tolerance.min(1e-12),
                ..r.map
            },
            r.materialize_absorbed_sum,
            r.reconstruction_tolerance,
        )
        .map(|v| v.with_solver_workspace_peak(peak))
    }
}
fn numerical_recovery_miss(e: &FeregError) -> bool {
    e.code == ErrorCode::NonConvergence
        || (e.code == ErrorCode::InvalidData
            && (e.message.contains("failed its FE certificate")
                || e.message.contains("exhausted its iteration budget")
                || e.message.contains("reconstruction")
                || e.message.contains("uncertified residual")))
}
fn include_recovery_peak(
    peak: &mut crate::memory::SolverWorkspacePeak,
    n: Option<u128>,
) -> Result<()> {
    *peak = peak.max(match n {
        Some(n) => crate::memory::SolverWorkspacePeak::from_bytes(n)?,
        None => crate::memory::SolverWorkspacePeak::UNMODELED,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn certified_norm_handoff_is_fresh_bounded_and_optional() {
        let design = FeDesign::new(4, Vec::new()).unwrap();
        let general = ReusableMapAbsorber::new(design, 2).unwrap();
        let mut session = FeSession {
            general: Some(general),
            normalized: vec![1.; 4],
            peak: crate::memory::SolverWorkspacePeak::from_bytes(1000).unwrap(),
            ..FeSession::default()
        };
        let config = super::super::AbsorbConfig::default();
        for multiplier in [1., 2., 0., 4.] {
            let input = DenseMatrix::from_columns(&[
                vec![
                    multiplier,
                    -2. * multiplier,
                    3. * multiplier,
                    -4. * multiplier,
                ],
                vec![0.; 4],
            ])
            .unwrap();
            let mut result = session
                .project_ols(
                    BorrowedMapWeights::Unit,
                    input.clone(),
                    config,
                    false,
                    false,
                )
                .unwrap();
            assert_eq!(result.residualized(), &input);
            assert_eq!(
                result.take_residual_norms(),
                vec![multiplier * 30.0_f64.sqrt(), 0.]
            );
            assert_eq!(session.peak.bytes(), Some(1016));
            assert_eq!(
                session
                    .general
                    .as_ref()
                    .unwrap()
                    .capacities()
                    .residual_norms,
                2
            );
        }
        let mut refused = config;
        refused.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 1000,
        });
        let input = DenseMatrix::from_columns(&[vec![1.; 4], vec![0.; 4]]).unwrap();
        let mut result = session
            .project_ols(
                BorrowedMapWeights::Unit,
                input.clone(),
                refused,
                false,
                false,
            )
            .unwrap();
        assert_eq!(result.residualized(), &input);
        assert!(result.take_residual_norms().is_empty());
        assert_eq!(session.peak.bytes(), Some(1016));
        let bad = DenseMatrix::from_columns(&[vec![f64::NAN; 4]]).unwrap();
        assert_eq!(
            session
                .project_ols(BorrowedMapWeights::Unit, bad, config, false, false)
                .unwrap_err()
                .code,
            ErrorCode::InvalidData
        );
        let input = DenseMatrix::from_columns(&[vec![3.; 4]]).unwrap();
        let mut result = session
            .project_ols(BorrowedMapWeights::Unit, input, config, false, false)
            .unwrap();
        assert_eq!(result.take_residual_norms(), vec![6.]);
        assert_eq!(session.peak.bytes(), Some(1016));
    }

    #[test]
    fn large_low_norm_handoff_preserves_the_callers_reduction() {
        let rows = 32_771;
        for threads in [1, 4, 16] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                let general =
                    ReusableMapAbsorber::new(FeDesign::new(rows, Vec::new()).unwrap(), 1).unwrap();
                let mut session = FeSession {
                    general: Some(general),
                    normalized: vec![1.; rows],
                    ols_chunked_norms: true,
                    ..FeSession::default()
                };
                let input = DenseMatrix::from_columns(&[vec![1.; rows]]).unwrap();
                let mut result = session
                    .project_ols(
                        BorrowedMapWeights::Unit,
                        input.clone(),
                        super::super::AbsorbConfig::default(),
                        false,
                        false,
                    )
                    .unwrap();
                assert_eq!(result.residualized(), &input);
                let norms = result.take_residual_norms();
                if threads == 1 {
                    assert_eq!(norms, vec![(rows as f64).sqrt()]);
                } else {
                    assert!(norms.is_empty());
                    assert_eq!(session.norm_handoff_peak, None);
                }
            });
        }
    }

    #[test]
    fn late_optional_allocation_refusal_preserves_attempt_peak_and_base_projection() {
        let design = FeDesign::new(4, Vec::new()).unwrap();
        let mut projector = ReusableMapAbsorber::new(design, 1).unwrap();
        let mut session = FeSession::default();
        let config = super::super::AbsorbConfig::default();
        session.admit(64, config).unwrap();
        let result = session
            .optional(64, 512, config, || {
                let first = crate::memory::Capacity::new(32).filled(0.0_f64, "first bank")?;
                let second = crate::memory::Capacity::new(usize::MAX as u128)
                    .filled(0.0_f64, "late bank refusal")?;
                Ok((first, second))
            })
            .unwrap();
        assert!(result.is_none());
        session.admit(64, config).unwrap();
        assert_eq!(session.peak.bytes(), Some(576));
        let input = DenseMatrix::from_columns(&[vec![1.0, -2.0, 3.0, -4.0]]).unwrap();
        let result = projector
            .absorb(&[1.; 4], input.clone(), MapConfig::default(), false)
            .unwrap();
        assert_eq!(result.residualized(), &input);
        assert!(result.report().certified());
        let mut refused = config;
        refused.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
            execution_bytes: 100,
            usable_bytes: 164,
        });
        assert!(session
            .optional(64, 1, refused, || -> Result<()> {
                panic!("veto must precede construction")
            })
            .unwrap()
            .is_none());
        assert_eq!(session.peak.bytes(), Some(576));
    }
}
