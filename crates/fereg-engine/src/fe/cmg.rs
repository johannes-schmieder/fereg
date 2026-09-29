//! Small nuisance Schur complement in factored FWL form, using the shared QR.
use super::{CmgCore, CmgCoreCandidate, Design, StopReason};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    linalg::{ScaledDesign, WeightedQr},
    threads::{compensated_sum, ThreadTeam},
};

/// One immutable weight frame. Setup work remains observable even when setup
/// fails; retrying preparation cannot silently acquire a fresh Krylov budget.
pub struct CmgSession<'a, 'data> {
    design: &'a Design<'data>,
    core: CmgCore<'a, 'data>,
    nuisance_indices: Vec<usize>,
    core_columns: Option<DenseMatrix>,
    factor: Option<WeightedQr>,
    setup_progress: Vec<super::Attempt>,
    setup_started: bool,
}
impl<'a, 'data> CmgSession<'a, 'data> {
    pub fn new(design: &'a Design<'data>, effects: [usize; 2]) -> Result<Self> {
        if design
            .topology()
            .effects()
            .iter()
            .any(|effect| !effect.intercept || !effect.slopes.is_empty())
        {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "CMG requires categorical intercept effects",
            ));
        }
        let mut nuisance_indices = reserved(64)?;
        for (effect, layout) in design.layout().effects.iter().enumerate() {
            if effects.contains(&effect) {
                continue;
            }
            for level in 1..layout.levels {
                if nuisance_indices.len() == 64 {
                    return Err(FeregError::new(
                        ErrorCode::Unsupported,
                        "CMG supports at most 64 non-base nuisance columns",
                    ));
                }
                nuisance_indices.push(layout.offset + level);
            }
        }
        Ok(Self {
            core: CmgCore::new(design, effects)?,
            design,
            nuisance_indices,
            core_columns: None,
            factor: None,
            setup_progress: reserved(0)?,
            setup_started: false,
        })
    }
    #[must_use]
    pub fn setup_progress(&self) -> &[super::Attempt] {
        &self.setup_progress
    }
    #[must_use]
    pub fn nuisance_rank(&self) -> Option<usize> {
        self.factor.as_ref().map(WeightedQr::rank)
    }
    /// Each nuisance indicator is a separately reported auxiliary RHS with
    /// the same per-RHS budget. The factor is built at most once per frame.
    pub fn prepare(&mut self, budget: usize, tolerance: f64, team: &ThreadTeam) -> Result<()> {
        if self.setup_started {
            return Err(invalid(
                "CMG nuisance preparation has already been attempted",
            ));
        }
        if budget == 0 || !tolerance.is_finite() || tolerance <= 0. || tolerance >= 1. {
            return Err(invalid("invalid CMG nuisance controls"));
        }
        self.setup_started = true;
        let columns = self.nuisance_columns()?;
        let mut references = reserved(columns.ncols())?;
        for column in 0..columns.ncols() {
            references.push((0..self.design.nobs()).fold(0_f64, |norm, row| {
                norm.hypot(self.design.weights().at(row).sqrt() * columns[(row, column)])
            }));
        }
        let rank_threshold =
            8. * f64::EPSILON * self.design.nobs().max(columns.ncols()).max(1) as f64;
        // Setup projection error must be smaller than the original-column
        // rank guard. This is an accuracy target, never an acceptance relaxation.
        let projected =
            self.core
                .solve(&columns, budget, tolerance.min(rank_threshold / 16.), team)?;
        self.setup_progress = projected.progress;
        if self
            .setup_progress
            .iter()
            .any(|attempt| attempt.stop != StopReason::Converged)
        {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "CMG nuisance projection did not converge; inspect setup progress",
            ));
        }
        let residuals = self
            .design
            .reconstruct(&columns, &projected.coefficients, team)?;
        let mut scaled = ScaledDesign::new(residuals, self.design.weights())?;
        scaled.reference_scales(&references)?;
        let factor = WeightedQr::from_reference_scaled(scaled, rank_threshold)?;
        self.core_columns = Some(projected.coefficients);
        self.factor = Some(factor);
        Ok(())
    }
    /// Returns an unaccepted candidate. Original-input certification and any
    /// bounded fallback belong to the shared FE engine, as for other backends.
    pub fn solve(
        &mut self,
        rhs: &DenseMatrix,
        budget: usize,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<CmgCoreCandidate> {
        let factor = self
            .factor
            .as_ref()
            .ok_or_else(|| invalid("CMG nuisance preparation has not succeeded"))?;
        let mut candidate = self.core.solve(rhs, budget, tolerance, team)?;
        let mut residuals = self
            .design
            .reconstruct(rhs, &candidate.coefficients, team)?;
        for (column, attempt) in candidate.progress.iter().enumerate() {
            if attempt.stop != StopReason::Converged {
                for row in 0..residuals.nrows() {
                    residuals[(row, column)] = 0.;
                }
            }
        }
        let nuisance = factor.solve(residuals)?;
        let core_columns = self.core_columns.as_ref().expect("prepared CMG columns");
        for coefficient in 0..candidate.coefficients.nrows() {
            for column in 0..rhs.ncols() {
                candidate.coefficients[(coefficient, column)] -= compensated_sum(
                    (0..nuisance.nrows())
                        .map(|j| core_columns[(coefficient, j)] * nuisance[(j, column)]),
                );
            }
        }
        for (j, &coefficient) in self.nuisance_indices.iter().enumerate() {
            for column in 0..rhs.ncols() {
                candidate.coefficients[(coefficient, column)] = nuisance[(j, column)];
            }
        }
        if candidate
            .coefficients
            .as_slice()
            .iter()
            .any(|x| !x.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "CMG recovered coefficients are not representable",
            ));
        }
        Ok(candidate)
    }
    fn nuisance_columns(&self) -> Result<DenseMatrix> {
        let mut columns = DenseMatrix::zeros(self.design.nobs(), self.nuisance_indices.len())?;
        for (column, &coefficient) in self.nuisance_indices.iter().enumerate() {
            let effect = self
                .design
                .layout()
                .effects
                .partition_point(|layout| layout.offset <= coefficient)
                - 1;
            let level = coefficient - self.design.layout().effects[effect].offset;
            let members = self
                .design
                .topology()
                .effect_partition(effect)?
                .membership();
            for &row in members.members(level)? {
                columns[(row, column)] = 1.;
            }
        }
        Ok(columns)
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
