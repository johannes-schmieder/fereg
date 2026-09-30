//! Structural eligibility only: planning cannot observe N, workers, finite width or memory.
use super::Design;
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    spec::{Estimator, Options, Solver},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    NoEffects,
    Direct,
    SchurPcg,
    Lsmr,
    Cmg,
}
impl Backend {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NoEffects => "none",
            Self::Direct => "direct",
            Self::SchurPcg => "schur-pcg",
            Self::Lsmr => "lsmr",
            Self::Cmg => "cmg",
        }
    }
}
/// Internal test/development control; never a public Stata solver spelling.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlanOverride {
    #[default]
    Automatic,
    Force(Backend),
}
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    requested: Solver,
    selected: Backend,
    fallback: Option<Backend>,
    linear_budget: usize,
    tolerance: f64,
    eliminated_effect: Option<usize>,
    cmg_core: Option<[usize; 2]>,
}
impl Plan {
    pub fn new(design: &Design<'_>, estimator: Estimator, options: &Options) -> Result<Self> {
        Self::with_override(design, estimator, options, PlanOverride::Automatic)
    }
    #[doc(hidden)]
    pub fn with_override(
        design: &Design<'_>,
        estimator: Estimator,
        options: &Options,
        override_plan: PlanOverride,
    ) -> Result<Self> {
        options.validate()?;
        let shape = Shape::from_design(design)?;
        shape.validate_request(estimator, options.solver)?;
        let selected = match override_plan {
            PlanOverride::Automatic => Self::default_backend(shape.levels.len(), options.solver),
            PlanOverride::Force(backend) => {
                if options.solver != Solver::Auto {
                    return Err(invalid("backend overrides require an automatic request"));
                }
                shape.validate_backend(backend)?;
                if backend == Backend::Cmg {
                    shape.validate_request(estimator, Solver::Cmg)?;
                }
                backend
            }
        };
        let fallback = Self::default_fallback(selected);
        Ok(Self {
            requested: options.solver,
            selected,
            fallback,
            linear_budget: match estimator {
                Estimator::Ols => options.iterations,
                Estimator::Ppml => 16_000,
            },
            tolerance: options.tolerance,
            eliminated_effect: shape.largest_parameter_effect,
            cmg_core: shape.cmg_core,
        })
    }
    #[must_use]
    pub const fn requested(&self) -> Solver {
        self.requested
    }
    #[must_use]
    pub const fn selected(&self) -> Backend {
        self.selected
    }
    #[must_use]
    pub const fn fallback(&self) -> Option<Backend> {
        self.fallback
    }
    #[must_use]
    pub const fn linear_budget(&self) -> usize {
        self.linear_budget
    }
    #[must_use]
    pub const fn tolerance(&self) -> f64 {
        self.tolerance
    }
    #[must_use]
    pub const fn eliminated_effect(&self) -> Option<usize> {
        self.eliminated_effect
    }
    #[must_use]
    pub const fn cmg_core(&self) -> Option<[usize; 2]> {
        self.cmg_core
    }
    /// Allocation planning reads the same policy; a byte estimate cannot
    /// choose a backend or independently change a fallback.
    pub(crate) const fn default_backend(effects: usize, solver: Solver) -> Backend {
        match solver {
            Solver::Auto => match effects {
                0 => Backend::NoEffects,
                1 => Backend::Direct,
                _ => Backend::SchurPcg,
            },
            Solver::Lsmr => Backend::Lsmr,
            Solver::Cmg => Backend::Cmg,
        }
    }
    pub(crate) const fn default_fallback(backend: Backend) -> Option<Backend> {
        match backend {
            Backend::NoEffects | Backend::Lsmr => None,
            _ => Some(Backend::Lsmr),
        }
    }
}
struct Shape {
    levels: Vec<usize>,
    has_slopes: bool,
    largest_parameter_effect: Option<usize>,
    cmg_core: Option<[usize; 2]>,
    cmg_nuisance: usize,
}
impl Shape {
    fn from_design(design: &Design<'_>) -> Result<Self> {
        let mut levels = reserved(design.layout().effects.len())?;
        let mut order = reserved(design.layout().effects.len())?;
        let mut largest = None;
        let mut maximum = 0;
        for (index, effect) in design.layout().effects.iter().enumerate() {
            levels.push(effect.levels);
            order.push(index);
            let parameters = effect
                .levels
                .checked_mul(effect.channels)
                .ok_or_else(|| invalid("FE parameter dimensions overflow"))?;
            if largest.is_none() || parameters > maximum {
                largest = Some(index);
                maximum = parameters;
            }
        }
        order.sort_unstable_by(|&a, &b| levels[b].cmp(&levels[a]).then(a.cmp(&b)));
        let cmg_core = (order.len() >= 2).then(|| [order[0], order[1]]);
        let cmg_nuisance = order.iter().skip(2).try_fold(0usize, |total, &index| {
            total
                .checked_add(levels[index].saturating_sub(1))
                .ok_or_else(|| invalid("CMG nuisance dimension overflows"))
        })?;
        let has_slopes = design
            .topology()
            .effects()
            .iter()
            .any(|effect| !effect.slopes.is_empty());
        Ok(Self {
            levels,
            has_slopes,
            largest_parameter_effect: largest,
            cmg_core,
            cmg_nuisance,
        })
    }
    fn validate_request(&self, estimator: Estimator, solver: Solver) -> Result<()> {
        if estimator == Estimator::Ppml {
            match solver {
                Solver::Lsmr if self.levels.len()<2||self.has_slopes=>return Err(unsupported("PPML solver(lsmr) requires at least two categorical intercept effects and no slopes")),
                Solver::Cmg if !(2..=3).contains(&self.levels.len())||self.has_slopes=>return Err(unsupported("PPML solver(cmg) requires two or three categorical intercept effects and no slopes")),
                _=>(),
            }
        }
        if solver == Solver::Cmg {
            self.validate_backend(Backend::Cmg)?;
        }
        Ok(())
    }
    fn validate_backend(&self, backend: Backend) -> Result<()> {
        let supported = match backend {
            Backend::NoEffects => self.levels.is_empty(),
            Backend::Direct => self.levels.len() == 1,
            Backend::SchurPcg => self.levels.len() >= 2,
            Backend::Lsmr => true,
            Backend::Cmg => !self.has_slopes && self.cmg_nuisance <= 64,
        };
        if !supported {
            return Err(unsupported(
                "requested FE backend is unsupported for this model structure",
            ));
        }
        Ok(())
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn unsupported(message: &str) -> FeregError {
    FeregError::new(ErrorCode::Unsupported, message)
}
