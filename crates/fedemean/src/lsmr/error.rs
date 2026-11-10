//! Runtime failures of the modified LSMR engine.
//!
//! [`SolveError`] covers runtime failures during a solve, including
//! operator/preconditioner application and iterative-solver input
//! validation.
//
// Copyright (c) 2026 within contributors
// Copyright (c) 2026 appliedAI Institute for Europe gGmbH
// MIT License; see the notice in `crates/fedemean/src/lsmr.rs`.

/// Runtime failure while executing a solve.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SolveError {
    /// The caller cancelled; all work started by this call has completed.
    Cancelled,
    /// A required numeric scratch buffer could not be reserved during a solve.
    Allocation {
        /// Context identifying the failed reservation.
        context: &'static str,
    },
    /// Solver input was invalid before any iteration was attempted.
    InvalidInput {
        /// Context string identifying the validation site.
        context: &'static str,
        /// Validation failure details.
        message: String,
    },
}

impl std::fmt::Display for SolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "solve cancelled"),
            Self::Allocation { context } => {
                write!(f, "unable to reserve solver scratch at {context}")
            }
            Self::InvalidInput { context, message } => {
                write!(f, "invalid solver input at {context}: {message}")
            }
        }
    }
}

impl std::error::Error for SolveError {}
