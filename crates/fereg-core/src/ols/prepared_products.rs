//! One same-call owner for accepted OLS inference products.

use super::{
    CertifiedPreparedOlsFit, PreparedOlsBreadCoordinate, PreparedOlsFitStatistics,
    PreparedOlsReferenceWeightReport,
};

/// Retained heap ownership used by the common inference admission boundary.
#[doc(hidden)]
pub trait PreparedOlsInferenceBuffers {
    fn inference_owned_bytes(&self) -> u128;
}

impl PreparedOlsInferenceBuffers for () {
    fn inference_owned_bytes(&self) -> u128 {
        0
    }
}

impl PreparedOlsInferenceBuffers for crate::topology_dof::CategoricalTopologyDofReport {
    fn inference_owned_bytes(&self) -> u128 {
        self.owned_buffer_bytes()
    }
}

/// Accepted fit and its mathematically distinct covariance/certificate payload.
/// Construction remains private to the same-call certified pipelines.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedOlsInference<A, D, C, K, M = ()> {
    pub(super) fit: CertifiedPreparedOlsFit,
    pub(super) absorbed_dof: A,
    pub(super) weights: PreparedOlsReferenceWeightReport,
    pub(super) dof: D,
    pub(super) statistics: PreparedOlsFitStatistics,
    pub(super) covariance: C,
    pub(super) certificate: K,
    pub(super) metadata: M,
}

impl<A, D, C, K, M> CertifiedPreparedOlsInference<A, D, C, K, M> {
    pub(super) fn with_absorbed_dof<B>(
        self,
        absorbed_dof: B,
    ) -> CertifiedPreparedOlsInference<B, D, C, K, M> {
        CertifiedPreparedOlsInference {
            fit: self.fit,
            absorbed_dof,
            weights: self.weights,
            dof: self.dof,
            statistics: self.statistics,
            covariance: self.covariance,
            certificate: self.certificate,
            metadata: self.metadata,
        }
    }

    #[must_use]
    pub const fn fit(&self) -> &CertifiedPreparedOlsFit {
        &self.fit
    }
    #[must_use]
    pub const fn absorbed_dof(&self) -> &A {
        &self.absorbed_dof
    }
    #[must_use]
    pub const fn weights(&self) -> PreparedOlsReferenceWeightReport {
        self.weights
    }
    #[must_use]
    pub const fn statistics(&self) -> PreparedOlsFitStatistics {
        self.statistics
    }
    #[must_use]
    pub const fn covariance(&self) -> &C {
        &self.covariance
    }
    #[must_use]
    pub fn covariance_coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        self.fit.bread().coordinates()
    }
}

impl<A, D: Copy, C, K: Copy, M> CertifiedPreparedOlsInference<A, D, C, K, M> {
    #[must_use]
    pub const fn dof(&self) -> D {
        self.dof
    }
    #[must_use]
    pub const fn certificate(&self) -> K {
        self.certificate
    }
}

impl<
        A: PreparedOlsInferenceBuffers,
        D,
        C: PreparedOlsInferenceBuffers,
        K,
        M: PreparedOlsInferenceBuffers,
    > CertifiedPreparedOlsInference<A, D, C, K, M>
{
    /// Heap owners retained after inference, excluding borrowed preparation.
    #[must_use]
    pub fn owned_buffer_bytes(&self) -> u128 {
        self.fit.owned_buffer_bytes()
            + self.absorbed_dof.inference_owned_bytes()
            + self.covariance.inference_owned_bytes()
            + self.metadata.inference_owned_bytes()
    }
}
