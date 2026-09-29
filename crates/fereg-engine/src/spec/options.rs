//! Retained request options from SPEC §1; legacy memory and solver routes are absent.
use super::protocol::parse_key_values;
use crate::error::{ErrorCode, FeregError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estimator {
    Ols,
    Ppml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Solver {
    #[default]
    Auto,
    Lsmr,
    Cmg,
}

impl Solver {
    pub fn parse(value: &str) -> Result<Self> {
        if value.eq_ignore_ascii_case("auto") {
            Ok(Self::Auto)
        } else if value.eq_ignore_ascii_case("lsmr") {
            Ok(Self::Lsmr)
        } else if value.eq_ignore_ascii_case("cmg") {
            Ok(Self::Cmg)
        } else {
            Err(invalid("solver must be auto, lsmr, or cmg"))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub solver: Solver,
    pub threads: usize,
    pub tolerance: f64,
    /// OLS linear budget or PPML outer budget; PPML frames have their own fixed budget.
    pub iterations: usize,
}

impl Options {
    #[must_use]
    pub const fn for_estimator(estimator: Estimator) -> Self {
        Self {
            solver: Solver::Auto,
            threads: 0,
            tolerance: 1e-8,
            iterations: match estimator {
                Estimator::Ols => 16_000,
                Estimator::Ppml => 10_000,
            },
        }
    }

    pub fn parse(args: &[String], estimator: Estimator) -> Result<Self> {
        let mut options = Self::for_estimator(estimator);
        for (key, value) in parse_key_values(args, &["solver", "threads", "tolerance", "iterate"])?
        {
            match key.as_str() {
                "solver" => options.solver = Solver::parse(&value)?,
                "threads" => {
                    options.threads = value
                        .parse()
                        .map_err(|_| invalid("threads must be a nonnegative integer"))?
                }
                "tolerance" => {
                    options.tolerance = value.parse().map_err(|_| invalid("invalid tolerance"))?
                }
                "iterate" => {
                    options.iterations = value
                        .parse()
                        .map_err(|_| invalid("iterate must be a positive integer"))?
                }
                _ => unreachable!("closed protocol vocabulary"),
            }
        }
        options.validate()?;
        Ok(options)
    }

    pub fn validate(&self) -> Result<()> {
        if !self.tolerance.is_finite() || self.tolerance <= 0.0 || self.tolerance >= 1.0 {
            return Err(invalid(
                "tolerance must be finite and strictly between zero and one",
            ));
        }
        if self.iterations == 0 {
            return Err(invalid("iterate must be a positive integer"));
        }
        Ok(())
    }
}

fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_solvers_and_estimator_budgets_are_explicit() {
        for estimator in [Estimator::Ols, Estimator::Ppml] {
            for (name, solver) in [
                ("auto", Solver::Auto),
                ("lsmr", Solver::Lsmr),
                ("cmg", Solver::Cmg),
            ] {
                let options = Options::parse(&[format!("solver={name}")], estimator).unwrap();
                assert_eq!(options.solver, solver);
                assert_eq!(
                    options.iterations,
                    if estimator == Estimator::Ols {
                        16_000
                    } else {
                        10_000
                    }
                );
            }
        }
        for argument in [
            "solver=cg",
            "solver=within",
            "memory=low",
            "deterministic=0",
            "threads=-1",
            "threads=1.5",
            "iterate=0",
            "tolerance=NaN",
            "tolerance=inf",
            "tolerance=0",
            "tolerance=1",
        ] {
            assert!(
                Options::parse(&[argument.into()], Estimator::Ols).is_err(),
                "{argument}"
            );
        }
    }
}
