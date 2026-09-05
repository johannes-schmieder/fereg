use crate::fe::FeCertificate;

/// Fixed-effect absorption implementation requested or actually used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeSolverKind {
    /// Choose from the certified MAP and modified-LSMR paths using design shape.
    Auto,
    /// Symmetric method of alternating projections.
    Map,
    /// Matrix-free modified LSMR with a configurable preconditioner.
    Lsmr,
    /// Combinatorial-multigrid-preconditioned graph PCG for sparse AKM designs.
    Cmg,
    /// Conjugate-gradient acceleration of symmetric MAP.
    Cg,
    /// Irons--Tuck vector extrapolation of symmetric MAP.
    IronsTuck,
    /// Squared extrapolation acceleration of symmetric MAP.
    Squarem,
}

/// Preconditioner requested or used by modified LSMR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LsmrPreconditioner {
    /// Diagonal for small/simple designs, additive Schwarz otherwise.
    Auto,
    /// Identity preconditioner.
    Off,
    /// Jacobi/diagonal preconditioner.
    Diagonal,
    /// One-level additive Schwarz over factor-pair subdomains.
    Schwarz,
}

/// Why a fixed-effect absorption run stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolveStopReason {
    /// The design contains no fixed-effect terms.
    NoFixedEffects,
    /// The final true fixed-effect orthogonality certificate passed.
    Certified,
    /// The iterative method exhausted its budget before certification.
    MaximumIterations,
    /// The numerical recurrence stopped but the recomputed certificate failed.
    Uncertified,
}

/// Diagnostics from one fixed-effect absorption run.
#[derive(Debug, Clone, PartialEq)]
pub struct SolveReport {
    /// Solver requested by the caller (`Auto` is retained here).
    pub requested_solver: FeSolverKind,
    /// Solver that produced the returned residuals.
    pub used_solver: FeSolverKind,
    /// Iteration count reported by the selected solver.
    pub iterations: usize,
    /// Independently recertified, tighter PPML checkpoint batches after the first.
    pub checkpoint_retries: usize,
    /// Stop reason after the independently recomputed certificate.
    pub stop_reason: SolveStopReason,
    /// True fixed-effect moment certificate on the returned residuals.
    pub certificate: FeCertificate,
    /// Recurrence-based relative normal-equation residual, when available.
    pub normal_equation_residual: f64,
    /// Solver/preconditioner setup wall time in seconds.
    pub setup_seconds: f64,
    /// Iterative solve wall time in seconds.
    pub solve_seconds: f64,
    /// Independently recomputed certificate wall time in seconds.
    pub certification_seconds: f64,
    /// Whether `Auto` abandoned its first choice and certified the other path.
    pub fallback_used: bool,
    /// Requested LSMR preconditioner, or `None` for non-LSMR routes.
    pub requested_preconditioner: Option<LsmrPreconditioner>,
    /// Effective LSMR preconditioner, or `None` for non-LSMR routes.
    pub used_preconditioner: Option<LsmrPreconditioner>,
    /// Bounded automatic-routing pilot sweeps included in setup cost.
    pub pilot_iterations: usize,
    /// Observed pilot contraction ratio; zero means no pilot was run.
    pub pilot_contraction: f64,
    /// Deterministic, diagnostic-only warnings from routing or preconditioning.
    pub warnings: Vec<String>,
}

impl SolveReport {
    /// Whether the returned values satisfy the final scientific certificate.
    #[must_use]
    pub fn certified(&self) -> bool {
        self.certificate.passed
            && matches!(
                self.stop_reason,
                SolveStopReason::Certified | SolveStopReason::NoFixedEffects
            )
    }
}
