/// Variance estimator requested for an OLS fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VceKind {
    Unadjusted,
    Robust,
    Clustered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSubsetReport {
    /// Zero-based cluster dimensions in this inclusion-exclusion subset.
    pub dimensions: Vec<usize>,
    pub groups: usize,
    pub sign: i8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VceReport {
    pub kind: VceKind,
    pub covariance: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub cluster_subsets: Vec<ClusterSubsetReport>,
}
