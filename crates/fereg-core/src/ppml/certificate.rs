#[derive(Debug, Clone, PartialEq)]
pub struct PpmlCertificate {
    pub finite_absolute_infinity_norm: f64,
    pub fixed_effect_absolute_infinity_norm: f64,
    pub relative_infinity_norm: f64,
    pub relative_two_norm: f64,
    pub passed: bool,
}
