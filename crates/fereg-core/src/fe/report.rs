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
    /// Experimental implicit two-FE Schur PCG with a hybrid CMG preconditioner.
    CmgSchur,
    /// Conjugate gradients: explicit symmetric MAP, or guarded automatic two-FE Schur.
    Cg,
    /// Irons--Tuck vector extrapolation of symmetric MAP.
    IronsTuck,
    /// Squared extrapolation acceleration of symmetric MAP.
    Squarem,
    /// Exact global elimination of the largest categorical intercept effect
    /// with block-Schur PCG over the remaining effects (Auto only; reported,
    /// never requested).
    Elimination,
    /// Preconditioned conjugate gradients on the full or eliminated
    /// categorical intercept system (fedemean), with the reduction and
    /// preconditioner of [`crate::fe::AbsorbConfig::pcg`] (public
    /// `solver(cg, reduce() precondition())` other than CMG). Requested only;
    /// a projection reports `Cg` (full) or `Elimination` with its route.
    Pcg,
}

/// The reduction of an explicit [`FeSolverKind::Pcg`] request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PcgReduction {
    /// Eliminate the largest effect for two to five effects, else the full
    /// system (fedemean's automatic reduction).
    #[default]
    Auto,
    Full,
    Eliminate,
}

/// The preconditioner of an explicit [`FeSolverKind::Pcg`] request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PcgPreconditioner {
    /// The reduction's automatic choice: Jacobi escalating to block-Jacobi
    /// (elimination); Jacobi escalating to additive Schwarz when slow or
    /// stalled, within the memory limit (full system).
    #[default]
    Auto,
    /// No preconditioner (full system only).
    None,
    /// Jacobi throughout (no escalation).
    Jacobi,
    /// Block-Jacobi over the retained effects (elimination only).
    Block,
    /// Additive Schwarz over factor-pair subdomains (full system only).
    Schwarz,
}

/// An explicit [`FeSolverKind::Pcg`] request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PcgRequest {
    pub reduction: PcgReduction,
    pub preconditioner: PcgPreconditioner,
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
    /// No solve ran: the values are a linear combination of certified
    /// projections and their recomputed certificate passed at the scale of
    /// the combined terms (fast OLS final residuals without saved outputs).
    DerivedCertificate,
}

/// Outer problem of a projection, in the public `solver(..., reduce())` terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeReduction {
    /// Nothing was solved (no fixed effects, or a derived residual).
    None,
    /// Every effect's coefficients (MAP, LSMR, graph CG and direct solves).
    Full,
    /// The effect with the most levels eliminated exactly (Schur complement).
    Eliminate,
}

/// Method of a projection, in the public `solver(method, ...)` terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeMethod {
    /// Nothing was solved: the design has no fixed effects.
    None,
    /// An exact one-effect projection (group means). Callers that know the
    /// design refine a single-effect [`FeMethod::Map`] to this.
    Direct,
    /// Alternating projections.
    Map,
    /// (Preconditioned) conjugate gradients.
    Cg,
    /// Modified LSMR.
    Lsmr,
    /// A final residual formed from certified projections without a solve.
    Derived,
}

/// Acceleration of alternating projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeAcceleration {
    None,
    /// Conjugate gradients over the symmetric MAP operator.
    Cg,
    IronsTuck,
    Squarem,
}

/// Local solve of one retained effect's block-Jacobi block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeLocalSolve {
    /// Exact grounded Cholesky factorization per connected component.
    Dense,
    /// One fixed CMG cycle.
    Cmg,
    /// The block's inverse diagonal.
    Jacobi,
}

/// Most design effects a block-Jacobi report describes (the elimination's
/// range: the eliminated effect plus four retained blocks).
pub const FE_ROUTE_MAX_EFFECTS: usize = 5;

/// The subdomain local solves of an additive Schwarz preconditioner,
/// counted by kind (all zero when the producer does not report them).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FeSchwarzSolves {
    /// One level on a subdomain's smaller side: nothing to solve.
    pub trivial: u32,
    /// Dense grounded Cholesky factorization.
    pub dense: u32,
    /// Exact envelope (skyline) Cholesky factorization.
    pub skyline: u32,
    /// Approximate Cholesky factorization.
    pub approximate: u32,
}

impl FeSchwarzSolves {
    /// Every subdomain counted (saturating).
    #[must_use]
    pub const fn subdomains(self) -> u32 {
        self.trivial
            .saturating_add(self.dense)
            .saturating_add(self.skyline)
            .saturating_add(self.approximate)
    }

    /// The larger count of each kind.
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        Self {
            trivial: self.trivial.max(other.trivial),
            dense: self.dense.max(other.dense),
            skyline: self.skyline.max(other.skyline),
            approximate: self.approximate.max(other.approximate),
        }
    }
}

/// Preconditioner of a projection's iterative method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FePreconditioner {
    None,
    Jacobi,
    /// Block-Jacobi over the retained effects; one entry per design effect
    /// (`None` for the eliminated effect).
    Block([Option<FeLocalSolve>; FE_ROUTE_MAX_EFFECTS]),
    /// Additive Schwarz over factor-pair subdomains, with their local solves
    /// where the producer reports them.
    Schwarz(FeSchwarzSolves),
    /// Combinatorial multigrid.
    Cmg,
}

/// The route a certified projection actually took: reduction x method x
/// preconditioner (or MAP acceleration), and how often a solve escalated to
/// a stronger preconditioner. Reporting only; it never steers a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeRoute {
    pub reduction: FeReduction,
    pub method: FeMethod,
    pub acceleration: FeAcceleration,
    pub preconditioner: FePreconditioner,
    /// Solves (column batches or LSMR windows) that escalated.
    pub escalations: u32,
}

impl FeRoute {
    /// A route without acceleration or escalation.
    #[must_use]
    pub const fn new(
        reduction: FeReduction,
        method: FeMethod,
        preconditioner: FePreconditioner,
    ) -> Self {
        Self {
            reduction,
            method,
            acceleration: FeAcceleration::None,
            preconditioner,
            escalations: 0,
        }
    }

    /// Exact elimination of one effect and CG on the reduced system,
    /// preconditioned by its Jacobi diagonal or by a CMG hierarchy.
    #[must_use]
    pub const fn eliminated_cg(cmg: bool) -> Self {
        let preconditioner = if cmg {
            FePreconditioner::Cmg
        } else {
            FePreconditioner::Jacobi
        };
        Self::new(FeReduction::Eliminate, FeMethod::Cg, preconditioner)
    }

    /// The route an untagged report's labels describe.
    #[must_use]
    pub fn from_labels(
        used: FeSolverKind,
        preconditioner: Option<LsmrPreconditioner>,
        stop: SolveStopReason,
    ) -> Self {
        use FeMethod as M;
        use FePreconditioner as P;
        use FeReduction as R;
        let map = |acceleration| Self {
            acceleration,
            ..Self::new(R::Full, M::Map, P::None)
        };
        match (stop, used) {
            (SolveStopReason::NoFixedEffects, _) | (_, FeSolverKind::Auto) => {
                Self::new(R::None, M::None, P::None)
            }
            (SolveStopReason::DerivedCertificate, _) => Self::new(R::None, M::Derived, P::None),
            (_, FeSolverKind::Map) => map(FeAcceleration::None),
            (_, FeSolverKind::Cg) => map(FeAcceleration::Cg),
            (_, FeSolverKind::IronsTuck) => map(FeAcceleration::IronsTuck),
            (_, FeSolverKind::Squarem) => map(FeAcceleration::Squarem),
            (_, FeSolverKind::Lsmr) => Self::new(
                R::Full,
                M::Lsmr,
                match preconditioner {
                    Some(LsmrPreconditioner::Diagonal) => P::Jacobi,
                    Some(LsmrPreconditioner::Schwarz) => P::Schwarz(FeSchwarzSolves::default()),
                    _ => P::None,
                },
            ),
            (_, FeSolverKind::Cmg) => Self::new(R::Full, M::Cg, P::Cmg),
            (_, FeSolverKind::CmgSchur) => Self::eliminated_cg(true),
            (_, FeSolverKind::Elimination) => Self::eliminated_cg(false),
            // A request label only; its producers tag their route.
            (_, FeSolverKind::Pcg) => Self::new(R::Full, M::Cg, P::Jacobi),
        }
    }

    /// Combine the routes of two parts of one projection (Low-memory RHS
    /// blocks) that share a solver label: block-Jacobi or Schwarz wins over
    /// Jacobi, local solves combine and escalations add.
    #[must_use]
    pub fn merged(self, other: Self) -> Self {
        let preconditioner = match (self.preconditioner, other.preconditioner) {
            (FePreconditioner::Block(a), FePreconditioner::Block(b)) => {
                let mut locals = a;
                for (local, other) in locals.iter_mut().zip(b) {
                    *local = local.or(other);
                }
                FePreconditioner::Block(locals)
            }
            (FePreconditioner::Block(a), _) | (_, FePreconditioner::Block(a)) => {
                FePreconditioner::Block(a)
            }
            (FePreconditioner::Schwarz(a), FePreconditioner::Schwarz(b)) => {
                FePreconditioner::Schwarz(a.max(b))
            }
            (FePreconditioner::Schwarz(a), _) | (_, FePreconditioner::Schwarz(a)) => {
                FePreconditioner::Schwarz(a)
            }
            (FePreconditioner::Cmg, _) | (_, FePreconditioner::Cmg) => FePreconditioner::Cmg,
            (a, _) => a,
        };
        Self {
            preconditioner,
            escalations: self.escalations.saturating_add(other.escalations),
            ..self
        }
    }
}

/// A producer's description of its route, kept with the solver label it
/// describes: once later code relabels the report, the labels decide again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeRouteTag {
    pub used_solver: FeSolverKind,
    pub route: FeRoute,
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
    /// Additional PPML checkpoint attempts after a rejected Schur or LSMR candidate.
    pub checkpoint_retries: usize,
    /// Modeled workspace over all attempts, excluding the caller execution base.
    pub solver_workspace_peak: crate::memory::SolverWorkspacePeak,
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
    /// Bounded automatic-routing pilot sweeps included in reported work.
    pub pilot_iterations: usize,
    /// Observed pilot contraction ratio; zero means no contraction was measured.
    pub pilot_contraction: f64,
    /// Deterministic, diagnostic-only warnings from routing or preconditioning.
    pub warnings: Vec<String>,
    /// The producer's route description, when the solver label alone does not
    /// identify it; read the route through [`SolveReport::route`].
    pub route_tag: Option<FeRouteTag>,
}

impl SolveReport {
    /// The route that produced the returned residuals: the producer's tag
    /// while the solver label still matches it, else the labels' route.
    #[must_use]
    pub fn route(&self) -> FeRoute {
        match self.route_tag {
            // A derived or empty projection ran no solve, whatever the report
            // it was formed from describes.
            Some(tag)
                if tag.used_solver == self.used_solver
                    && !matches!(
                        self.stop_reason,
                        SolveStopReason::NoFixedEffects | SolveStopReason::DerivedCertificate
                    ) =>
            {
                tag.route
            }
            _ => FeRoute::from_labels(self.used_solver, self.used_preconditioner, self.stop_reason),
        }
    }

    /// Describe this report's route for its current solver label.
    pub fn set_route(&mut self, route: FeRoute) {
        self.route_tag = Some(FeRouteTag {
            used_solver: self.used_solver,
            route,
        });
    }

    /// Fold the route of another part of the same projection into this one
    /// (see [`FeRoute::merged`]); `source_won` means this report has just
    /// adopted the source's solver label. Untagged parts keep their labels'
    /// route.
    pub(crate) fn merge_route(&mut self, source: &Self, source_won: bool) {
        if self.route_tag.is_none() && source.route_tag.is_none() {
            return;
        }
        let combined = if source_won {
            source.route()
        } else if self.used_solver == source.used_solver {
            self.route().merged(source.route())
        } else {
            let mut route = self.route();
            route.escalations = route.escalations.saturating_add(source.route().escalations);
            route
        };
        self.set_route(combined);
    }

    /// Report for one solver route with no preconditioner, pilot, fallback,
    /// checkpoint retry or modeled workspace, zero iterations and zero timing.
    /// Routes override the fields they record with struct-update syntax.
    #[must_use]
    pub(crate) fn single_route(
        solver: FeSolverKind,
        stop_reason: SolveStopReason,
        certificate: FeCertificate,
    ) -> Self {
        Self {
            requested_solver: solver,
            used_solver: solver,
            iterations: 0,
            checkpoint_retries: 0,
            solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
            stop_reason,
            certificate,
            normal_equation_residual: 0.0,
            setup_seconds: 0.0,
            solve_seconds: 0.0,
            certification_seconds: 0.0,
            fallback_used: false,
            requested_preconditioner: None,
            used_preconditioner: None,
            pilot_iterations: 0,
            pilot_contraction: 0.0,
            warnings: Vec::new(),
            route_tag: None,
        }
    }

    /// Whether the returned values satisfy the final scientific certificate.
    #[must_use]
    pub fn certified(&self) -> bool {
        self.certificate.passed
            && matches!(
                self.stop_reason,
                SolveStopReason::Certified
                    | SolveStopReason::NoFixedEffects
                    | SolveStopReason::DerivedCertificate
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(used: FeSolverKind, preconditioner: Option<LsmrPreconditioner>) -> SolveReport {
        SolveReport {
            used_preconditioner: preconditioner,
            ..SolveReport::single_route(
                used,
                SolveStopReason::Certified,
                FeCertificate {
                    absolute_infinity_norm: 0.0,
                    relative_infinity_norm: 0.0,
                    relative_two_norm: 0.0,
                    passed: true,
                },
            )
        }
    }

    #[test]
    fn solver_labels_describe_their_routes() {
        use FeMethod as M;
        use FePreconditioner as P;
        use FeReduction as R;
        for (used, preconditioner, expected) in [
            (
                FeSolverKind::Map,
                None,
                (R::Full, M::Map, P::None, FeAcceleration::None),
            ),
            (
                FeSolverKind::Cg,
                None,
                (R::Full, M::Map, P::None, FeAcceleration::Cg),
            ),
            (
                FeSolverKind::IronsTuck,
                None,
                (R::Full, M::Map, P::None, FeAcceleration::IronsTuck),
            ),
            (
                FeSolverKind::Lsmr,
                Some(LsmrPreconditioner::Schwarz),
                (
                    R::Full,
                    M::Lsmr,
                    P::Schwarz(FeSchwarzSolves::default()),
                    FeAcceleration::None,
                ),
            ),
            (
                FeSolverKind::Lsmr,
                Some(LsmrPreconditioner::Off),
                (R::Full, M::Lsmr, P::None, FeAcceleration::None),
            ),
            (
                FeSolverKind::Cmg,
                None,
                (R::Full, M::Cg, P::Cmg, FeAcceleration::None),
            ),
            (
                FeSolverKind::CmgSchur,
                None,
                (R::Eliminate, M::Cg, P::Cmg, FeAcceleration::None),
            ),
            (
                FeSolverKind::Elimination,
                None,
                (R::Eliminate, M::Cg, P::Jacobi, FeAcceleration::None),
            ),
        ] {
            let route = report(used, preconditioner).route();
            assert_eq!(
                (
                    route.reduction,
                    route.method,
                    route.preconditioner,
                    route.acceleration
                ),
                expected,
                "{used:?}"
            );
            assert_eq!(route.escalations, 0);
        }
        let mut none = report(FeSolverKind::Map, None);
        none.stop_reason = SolveStopReason::NoFixedEffects;
        assert_eq!(none.route().method, FeMethod::None);
        none.stop_reason = SolveStopReason::DerivedCertificate;
        assert_eq!(none.route().method, FeMethod::Derived);
        // A derived residual formed from a tagged absorption ran no solve.
        none.set_route(FeRoute::new(
            FeReduction::Eliminate,
            FeMethod::Cg,
            FePreconditioner::Jacobi,
        ));
        assert_eq!(none.route().method, FeMethod::Derived);
    }

    #[test]
    fn a_route_tag_holds_only_while_its_solver_label_does() {
        let schur = FeRoute::new(
            FeReduction::Eliminate,
            FeMethod::Cg,
            FePreconditioner::Jacobi,
        );
        let mut tagged = report(FeSolverKind::Map, None);
        tagged.set_route(schur);
        assert_eq!(tagged.route(), schur);
        tagged.used_solver = FeSolverKind::Lsmr;
        tagged.used_preconditioner = Some(LsmrPreconditioner::Diagonal);
        assert_eq!(
            tagged.route(),
            FeRoute::new(FeReduction::Full, FeMethod::Lsmr, FePreconditioner::Jacobi)
        );
    }

    #[test]
    fn merged_parts_keep_the_strongest_preconditioner_and_add_escalations() {
        let block = |effect: usize, local| {
            let mut locals = [None; FE_ROUTE_MAX_EFFECTS];
            locals[effect] = Some(local);
            FePreconditioner::Block(locals)
        };
        let mut first = report(FeSolverKind::Elimination, None);
        let mut escalated = FeRoute::new(
            FeReduction::Eliminate,
            FeMethod::Cg,
            block(1, FeLocalSolve::Cmg),
        );
        escalated.escalations = 2;
        first.set_route(escalated);
        let mut second = report(FeSolverKind::Elimination, None);
        let mut hinted = FeRoute::new(
            FeReduction::Eliminate,
            FeMethod::Cg,
            block(2, FeLocalSolve::Dense),
        );
        hinted.escalations = 1;
        second.set_route(hinted);
        first.merge_route(&second, false);
        let route = first.route();
        assert_eq!(route.escalations, 3);
        assert_eq!(
            route.preconditioner,
            FePreconditioner::Block([
                None,
                Some(FeLocalSolve::Cmg),
                Some(FeLocalSolve::Dense),
                None,
                None
            ])
        );
        // An untagged Jacobi part does not hide the other part's blocks.
        let mut plain = report(FeSolverKind::Elimination, None);
        plain.merge_route(&first, false);
        assert_eq!(plain.route().preconditioner, route.preconditioner);
        // Two untagged parts keep their labels' route and stay untagged.
        let mut a = report(FeSolverKind::Map, None);
        a.merge_route(&report(FeSolverKind::Map, None), false);
        assert!(a.route_tag.is_none());
    }
}
