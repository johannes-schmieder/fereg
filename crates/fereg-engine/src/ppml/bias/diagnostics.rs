//! Fixed public schema; the sole call-level plan supplies the workspace bound.
use super::{Channel, Correction, Method};
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiagnosticTable {
    pub values: [f64; 21],
}
impl DiagnosticTable {
    pub const ROWS: [&'static str; 21] = [
        "method",
        "units",
        "probes",
        "seed",
        "stream",
        "groups",
        "solves",
        "max_score",
        "max_reconstruction",
        "seconds",
        "workspace_bytes",
        "max_increment",
        "min_mu",
        "max_mu",
        "min_unit_rows",
        "max_unit_rows",
        "route",
        "valid",
        "tolerance",
        "min_level_information",
        "max_fe_leverage_lb",
    ];
    pub const COLUMN: &'static str = "value";
    pub const VALIDITY: &'static str = "interior-retained-sample";
}
impl Correction {
    /// The bound must cover the complete correction lifetime in the call-level
    /// memory plan, including shared FE/WLS storage and requested original-axis
    /// publication. This serialization neither estimates nor admits memory.
    pub fn diagnostic_table(&self, workspace_bound: u128) -> Result<DiagnosticTable> {
        let (method, probes, seed, stream) = match self.request.method {
            // Exact unit actions are work, not randomized probes. Keeping the
            // request fields zero preserves the existing Stata transaction.
            Method::Exact => (1., 0., 0., 0.),
            Method::Randomized {
                probes,
                seed,
                stream,
            } => (2., f64::from(probes), f64::from(seed), f64::from(stream)),
        };
        let mut maximum_increment = 0_f64;
        for coordinate in 0..self.coefficients.dimension() {
            maximum_increment =
                maximum_increment.max(self.coefficients.value(coordinate, Channel::Total)?.abs());
        }
        Ok(DiagnosticTable {
            values: [
                method,
                self.units as f64,
                probes,
                seed,
                stream,
                self.groups as f64,
                self.diagnostics.solves as f64,
                self.diagnostics.maximum_score,
                self.diagnostics.maximum_reconstruction,
                self.elapsed_seconds,
                workspace_bound as f64,
                maximum_increment,
                self.minimum_mean,
                self.maximum_mean,
                self.minimum_unit_rows as f64,
                self.maximum_unit_rows as f64,
                // Every retained backend uses the shared sparse information
                // operator. Its backend work remains in the ordinary reports;
                // this is not a new size-selected cached-information route.
                1.,
                1.,
                self.certificate_tolerance,
                self.diagnostics.minimum_level_information,
                self.diagnostics.maximum_fe_leverage_lower_bound,
            ],
        })
    }
}
