//! Frozen outer convergence policy. A request for strict checks is not convergence.
use crate::{
    error::{ErrorCode, FeregError, Result},
    spec::Options,
};
#[derive(Debug, Clone, Copy)]
pub struct Controls {
    tolerance: f64,
    max_iterations: usize,
}
impl Controls {
    pub fn new(options: &Options) -> Result<Self> {
        options.validate()?;
        if 0.1 * options.tolerance == 0. {
            return Err(invalid(
                "PPML tolerance cannot represent a positive strict target",
            ));
        }
        Ok(Self {
            tolerance: options.tolerance,
            max_iterations: options.iterations,
        })
    }
    #[must_use]
    pub fn tolerance(self) -> f64 {
        self.tolerance
    }
    #[must_use]
    pub fn publication_tolerance(self) -> f64 {
        self.tolerance.min(1e-8)
    }
    #[must_use]
    pub fn strict_tolerance(self) -> f64 {
        (0.1 * self.tolerance).min(1e-12)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Scores {
    pub finite: f64,
    pub fixed_effects: f64,
}
impl Scores {
    fn maximum(self) -> Result<f64> {
        if [self.finite, self.fixed_effects]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
        {
            return Err(invalid("PPML score evidence is invalid"));
        }
        Ok(self.finite.max(self.fixed_effects))
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision {
    Continue,
    StrictChecksRequired,
}
#[derive(Debug)]
pub struct Convergence {
    controls: Controls,
    accepted: usize,
    consecutive: usize,
    strict_pending: bool,
    relative_change: Option<f64>,
    scores: Option<f64>,
    inner_tolerance: f64,
}
impl Convergence {
    #[must_use]
    pub fn new(controls: Controls) -> Self {
        Self {
            controls,
            accepted: 0,
            consecutive: 0,
            strict_pending: false,
            relative_change: None,
            scores: None,
            inner_tolerance: 1e-6,
        }
    }
    /// Scores must be independently recomputed at the newly accepted state.
    pub fn observe(
        &mut self,
        old_deviance: f64,
        new_deviance: f64,
        scores: Scores,
    ) -> Result<Decision> {
        if self.strict_pending {
            return Err(invalid("strict PPML checks are pending"));
        }
        if self.accepted >= self.controls.max_iterations {
            return Err(exhausted());
        }
        if [old_deviance, new_deviance]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
        {
            return Err(invalid("invalid accepted PPML deviance"));
        }
        let score = scores.maximum()?;
        let relative =
            (old_deviance - new_deviance).abs() / old_deviance.min(new_deviance).max(0.1);
        if !relative.is_finite() {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML deviance change is not representable",
            ));
        }
        self.accepted += 1;
        self.relative_change = Some(relative);
        self.scores = Some(score);
        self.consecutive =
            if relative <= self.controls.tolerance && score <= self.controls.tolerance {
                self.consecutive + 1
            } else {
                0
            };
        self.strict_pending = self.consecutive >= 2;
        if self.strict_pending {
            Ok(Decision::StrictChecksRequired)
        } else if self.accepted == self.controls.max_iterations {
            Err(exhausted())
        } else {
            Ok(Decision::Continue)
        }
    }
    pub fn reject_strict(&mut self) -> Result<()> {
        if !self.strict_pending {
            return Err(invalid("no strict PPML checkpoint to reject"));
        }
        self.strict_pending = false;
        self.consecutive = 0;
        self.inner_tolerance = self.controls.strict_tolerance();
        if self.accepted >= self.controls.max_iterations {
            Err(exhausted())
        } else {
            Ok(())
        }
    }
    /// Harvested monotone eight-stage policy, tightened by accepted objective/score progress.
    pub fn next_inner_tolerance(&mut self) -> f64 {
        let floor = self.controls.strict_tolerance();
        if self.strict_pending {
            self.inner_tolerance = floor;
            return floor;
        }
        let iteration = self.accepted.saturating_add(1);
        let staged = if iteration >= 8 {
            floor
        } else {
            let fraction = iteration.saturating_sub(1) as f64 / 7.;
            ((1. - fraction) * 1e-6_f64.ln() + fraction * floor.ln()).exp()
        };
        let progress = match (self.relative_change, self.scores) {
            (Some(change), Some(score))
                if change <= self.controls.tolerance && score <= self.controls.tolerance =>
            {
                floor
            }
            (Some(change), Some(score)) => (0.1 * change.max(score)).clamp(floor, 1e-6),
            _ => 1e-6,
        };
        self.inner_tolerance = self.inner_tolerance.min(staged).min(progress).max(floor);
        self.inner_tolerance
    }
    #[must_use]
    pub fn accepted_steps(&self) -> usize {
        self.accepted
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn exhausted() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "PPML outer iteration budget exhausted",
    )
}
