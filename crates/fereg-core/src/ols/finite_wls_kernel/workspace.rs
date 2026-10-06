//! Numerical banks shared by stateless OLS and retained finite operations.
#[derive(Debug)]
pub(crate) struct FiniteWorkspace<Slot = ()> {
    pub(crate) response: Vec<f64>,
    pub(crate) gram: Vec<f64>,
    pub(crate) factor: Vec<f64>,
    pub(crate) inverse_lower: Vec<f64>,
    pub(crate) rhs: Vec<f64>,
    pub(crate) moment_partials: Vec<f64>,
    pub(crate) row: Vec<f64>,
    pub(crate) solve: Vec<f64>,
    pub(crate) permutation: Vec<usize>,
    pub(crate) scores: Vec<f64>,
    pub(crate) coefficients: Vec<f64>,
    pub(crate) bread: Vec<f64>,
    pub(crate) residuals: Vec<f64>,
    pub(crate) reported_coefficients: Vec<Option<f64>>,
    pub(crate) kept: Vec<usize>,
    pub(crate) candidate_positions: Vec<usize>,
    pub(crate) candidate_ids: Vec<usize>,
    pub(crate) candidate_scales: Vec<f64>,
    pub(crate) reference_mean_corrections: Vec<f64>,
    pub(crate) basis_slots: Vec<Slot>,
}
impl<Slot> Default for FiniteWorkspace<Slot> {
    fn default() -> Self {
        Self {
            response: Vec::new(),
            gram: Vec::new(),
            factor: Vec::new(),
            inverse_lower: Vec::new(),
            rhs: Vec::new(),
            moment_partials: Vec::new(),
            row: Vec::new(),
            solve: Vec::new(),
            permutation: Vec::new(),
            scores: Vec::new(),
            coefficients: Vec::new(),
            bread: Vec::new(),
            residuals: Vec::new(),
            reported_coefficients: Vec::new(),
            kept: Vec::new(),
            candidate_positions: Vec::new(),
            candidate_ids: Vec::new(),
            candidate_scales: Vec::new(),
            reference_mean_corrections: Vec::new(),
            basis_slots: Vec::new(),
        }
    }
}
