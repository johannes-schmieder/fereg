//! Numerical products share ownership; canonical and frozen proofs retain
//! their distinct authority. Immutable coordinate identities live in the basis.
use super::*;
use crate::ols::{
    PreparedOlsBreadCertificate, PreparedOlsRankCertificate, PreparedOlsScoreCertificate,
};

#[derive(Debug)]
pub(crate) struct CanonicalLinearProof {
    pub(crate) rank: PreparedOlsRankCertificate,
    pub(crate) score: PreparedOlsScoreCertificate,
    pub(crate) bread: PreparedOlsBreadCertificate,
    pub(crate) residual_absorption: SolveReport,
}

// Inline proofs preserve the admitted footprint and allocation-free rollback.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum LinearFrameProof {
    Canonical(CanonicalLinearProof),
    Frozen(FrozenLinearSolveCertificate),
}

#[derive(Debug, Default)]
pub(crate) struct LinearPredictors {
    pub(super) finite: Vec<f64>,
    pub(super) absorbed: Vec<f64>,
}

#[derive(Debug)]
pub(crate) struct CertifiedLinearFrame {
    pub(crate) coefficients: Vec<f64>,
    pub(crate) reported_coefficients: Vec<Option<f64>>,
    pub(crate) bread: Vec<f64>,
    pub(crate) within_residuals: Vec<f64>,
    pub(crate) response_absorption_report: SolveReport,
    pub(crate) proof: LinearFrameProof,
    pub(crate) generations: LinearSessionGenerations,
    pub(crate) decomposition: Option<LinearPredictors>,
}

impl CertifiedLinearFrame {
    pub(crate) fn canonical_proof(&self) -> Option<&CanonicalLinearProof> {
        match &self.proof {
            LinearFrameProof::Canonical(proof) => Some(proof),
            _ => None,
        }
    }
    pub(crate) fn frozen_certificate(&self) -> Option<FrozenLinearSolveCertificate> {
        match self.proof {
            LinearFrameProof::Frozen(proof) => Some(proof),
            _ => None,
        }
    }
    pub(crate) fn finite_rank(&self) -> usize {
        self.coefficients.len()
    }
    pub(crate) fn residuals(&self) -> &[f64] {
        &self.within_residuals
    }
    pub(crate) fn bread(&self) -> &[f64] {
        &self.bread
    }
    pub(crate) fn absorption_report(&self) -> &SolveReport {
        &self.response_absorption_report
    }
    #[cfg(test)]
    pub(crate) fn rank_certificate(&self) -> &PreparedOlsRankCertificate {
        &self
            .canonical_proof()
            .expect("canonical rank authority")
            .rank
    }
    #[cfg(test)]
    pub(crate) fn score_certificate(&self) -> &PreparedOlsScoreCertificate {
        &self
            .canonical_proof()
            .expect("canonical score authority")
            .score
    }
    #[cfg(test)]
    pub(crate) fn residual_absorption_report(&self) -> &SolveReport {
        &self
            .canonical_proof()
            .expect("canonical residual authority")
            .residual_absorption
    }
    pub(super) fn predictors(&self) -> &LinearPredictors {
        self.decomposition
            .as_ref()
            .expect("canonical decomposition was validated")
    }
    pub(crate) fn certified(&self) -> bool {
        self.response_absorption_report.certified()
            && match &self.proof {
                LinearFrameProof::Canonical(p) => {
                    p.rank.passed
                        && p.score.passed
                        && p.bread.passed
                        && p.residual_absorption.certified()
                }
                LinearFrameProof::Frozen(p) => p.passed,
            }
    }
}
