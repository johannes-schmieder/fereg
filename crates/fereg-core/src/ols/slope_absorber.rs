//! One-shot two-partition OLS ownership for the coefficient Schur projector.
//!
//! The design and normalized weights are immutable for this owner's lifetime.
//! Initial RHS blocks and the final residual share geometry, never certificates.
use super::{PreparedOlsExecutionMode, PreparedOlsFitConfig};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedMapWeights, CertifiedBorrowedMap, FeSolverKind, MapConfig, MapGeometryGeneration,
    ReusableMapAbsorber,
};
use crate::memory::SolverWorkspacePeak;
use crate::prepared_fit_view::PreparedOlsFitView;

pub(super) struct OlsSlopeAbsorber {
    projector: ReusableMapAbsorber,
    weights: Vec<f64>,
    peak: SolverWorkspacePeak,
    setup_seconds: f64,
}

fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "OLS slope owner exceeds admitted workspace",
    )
}

impl OlsSlopeAbsorber {
    pub(super) fn build<P: PreparedOlsFitView + ?Sized>(
        prepared: &P,
        weights: BorrowedMapWeights<'_>,
        config: PreparedOlsFitConfig,
        attempted_peak: &mut SolverWorkspacePeak,
    ) -> Result<Option<Box<[Self; 1]>>> {
        if config.absorb.solver != FeSolverKind::Auto
            || !config.absorb.certified_fallback
            || (config.absorb.retain_coefficients && !config.allow_final_fe_recovery)
            || !prepared.fit_has_two_effects_with_slopes()
        {
            return Ok(None);
        }
        let started = std::time::Instant::now();
        let memory = prepared.fit_owned_fe_design_memory()?;
        let fits = |bytes| {
            config
                .absorb
                .solver_memory_budget
                .map_or(true, |b| b.fits(bytes))
        };
        if !fits(memory.construction_bytes) {
            return Ok(None);
        }
        *attempted_peak =
            attempted_peak.max(SolverWorkspacePeak::from_bytes(memory.construction_bytes)?);
        let design = prepared.fit_owned_fe_design()?;
        let rhs = match config.execution.mode {
            PreparedOlsExecutionMode::Fast => prepared
                .fit_model()
                .columns
                .len()
                .checked_add(1)
                .ok_or_else(allocation)?,
            PreparedOlsExecutionMode::Low => config
                .execution
                .poolsize
                .max(1)
                .min(prepared.fit_model().columns.len().max(1)),
        };
        let threads = config.execution.threads.resolved();
        // The no-control caller keeps its outcome and final residual alongside
        // the projector; its execution parent has no resident-X allowance.
        let vectors = if prepared.fit_model().columns.is_empty() {
            4
        } else {
            1
        };
        let mut resident = memory
            .resident_bytes
            .checked_add(vectors * design.nobs() as u128 * 8)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() as u128))
            .ok_or_else(allocation)?;
        for bytes in [
            ReusableMapAbsorber::base_workspace_bytes(&design, rhs)?,
            ReusableMapAbsorber::certificate_geometry_bytes(&design)?,
            ReusableMapAbsorber::coefficient_solver_bytes(&design, rhs, threads)?,
        ] {
            resident = resident.checked_add(bytes).ok_or_else(allocation)?;
        }
        if !fits(resident) {
            return Ok(None);
        }
        let coupling = ReusableMapAbsorber::coefficient_coupling_bytes(&design, rhs)?
            .filter(|bytes| resident.checked_add(*bytes).is_some_and(fits));
        if let Some(bytes) = coupling {
            resident += bytes;
        }
        let certificate_bytes = ReusableMapAbsorber::coefficient_certificate_bytes(&design, rhs)?;
        let parallel_certificates =
            threads > 1 && resident.checked_add(certificate_bytes).is_some_and(fits);
        if parallel_certificates {
            resident += certificate_bytes;
        }
        *attempted_peak = attempted_peak.max(SolverWorkspacePeak::from_bytes(resident)?);
        let mut normalized = Vec::new();
        normalized
            .try_reserve_exact(design.nobs())
            .map_err(|_| allocation())?;
        if normalized.capacity() > design.nobs() {
            return Err(allocation());
        }
        match weights {
            BorrowedMapWeights::Unit => normalized.resize(design.nobs(), 1.),
            BorrowedMapWeights::Provided(values) => {
                let maximum = values.iter().copied().fold(0.0_f64, f64::max);
                if values.len() != design.nobs()
                    || values.iter().any(|w| !w.is_finite() || *w <= 0.)
                {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "OLS slope weights are invalid",
                    ));
                }
                normalized.extend(values.iter().map(|w| w / maximum));
            }
        }
        let mut projector = ReusableMapAbsorber::new(design, rhs)?;
        projector.enable_certificate_geometry()?;
        projector.enable_coefficient_solver(threads)?;
        if coupling.is_some() {
            projector.enable_coefficient_coupling()?;
        }
        if parallel_certificates {
            projector.enable_coefficient_certificates()?;
        }
        // Keep this optional owner out of the common OLS descriptor. Reserve
        // fallibly: Box::try_new is unavailable on the minimum Rust version.
        // With exact capacity one, conversion to the fixed array cannot grow.
        let mut owner = Vec::new();
        owner.try_reserve_exact(1).map_err(|_| allocation())?;
        if owner.capacity() != 1 {
            return Err(allocation());
        }
        owner.push(Self {
            projector,
            weights: normalized,
            peak: SolverWorkspacePeak::from_bytes(resident.max(memory.construction_bytes))?,
            setup_seconds: started.elapsed().as_secs_f64(),
        });
        Ok(Some(owner.into_boxed_slice().try_into().map_err(|_| allocation())?))
    }

    pub(super) fn absorb(
        &mut self,
        values: DenseMatrix,
        config: MapConfig,
        final_residual: bool,
    ) -> Result<CertifiedBorrowedMap> {
        // This private owner has no weight/design mutation API. The label is
        // local to it; it conveys no authority to another fit or projector.
        let generation = MapGeometryGeneration {
            sample: 0,
            topology: 0,
            weights: 0,
        };
        let result = if final_residual {
            self.projector
                .absorb_coefficient_residual(&self.weights, values, config, generation)
        } else {
            self.projector.absorb_in_generation(
                &self.weights,
                values,
                config,
                false,
                Some(generation),
            )
        }?;
        let (values, mut report) = result.into_parts();
        report.requested_solver = FeSolverKind::Auto;
        // The generic reusable owner deliberately reports no standalone bound;
        // this one-shot owner admits its entire lifetime and all optional banks.
        report.solver_workspace_peak = self.peak;
        report.setup_seconds += std::mem::take(&mut self.setup_seconds);
        CertifiedBorrowedMap::from_certified_parts(values, report)
    }
}
