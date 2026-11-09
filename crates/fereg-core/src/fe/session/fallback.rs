//! The certified fallback of an explicit request whose route does not serve
//! the design before any work (SPEC §1): the Auto request that replaces it.

use super::FeSession;
use crate::error::{ErrorCode, Result};
use crate::fe::BorrowedMapWeights;

impl<'a> FeSession<'a> {
    /// The session of an OLS fit. An explicit CMG request whose route does
    /// not support the design (absorbed slopes, CMG-Schur outside two
    /// intercept effects, more nuisance columns than the CMG adapter takes)
    /// is served by the Auto routes, reported as the certified fallback with
    /// the reason `unsupported` (SPEC §1).
    pub(crate) fn for_ols_rhs<P: crate::ols::prepared_fit_view::PreparedOlsFitView + ?Sized>(
        prepared: &'a P,
        weights: BorrowedMapWeights<'a>,
        config: crate::ols::PreparedOlsFitConfig,
        rhs: usize,
    ) -> Result<Self> {
        let requested = config.absorb.solver;
        match Self::for_ols_request(prepared, weights, config, rhs) {
            Err(error)
                if error.code == ErrorCode::Unsupported
                    && config.absorb.certified_fallback
                    && matches!(
                        requested,
                        crate::fe::FeSolverKind::Cmg | crate::fe::FeSolverKind::CmgSchur
                    ) =>
            {
                let auto = crate::ols::PreparedOlsFitConfig {
                    absorb: auto_fallback_config(config.absorb),
                    ..config
                };
                let mut owner = Self::for_ols_request(prepared, weights, auto, rhs)?;
                owner.substituted = Some((requested, error.message));
                Ok(owner)
            }
            other => other,
        }
    }
}

/// The Auto request that serves an explicit request its route does not
/// support: the same tolerances, budgets and memory, every route choice
/// automatic.
pub(super) fn auto_fallback_config(config: crate::fe::AbsorbConfig) -> crate::fe::AbsorbConfig {
    let defaults = crate::fe::AbsorbConfig::default();
    crate::fe::AbsorbConfig {
        solver: crate::fe::FeSolverKind::Auto,
        routing: None,
        it_coefficients: false,
        cmg_schur_degree: defaults.cmg_schur_degree,
        cmg_schur_samples: 0,
        cmg_schur_seed: 0,
        lsmr: crate::fe::LsmrConfig {
            preconditioner: crate::fe::LsmrPreconditioner::Auto,
            ..config.lsmr
        },
        pcg: crate::fe::PcgRequest::default(),
        ..config
    }
}
