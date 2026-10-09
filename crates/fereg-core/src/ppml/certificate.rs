#[derive(Debug, Clone, PartialEq)]
pub struct PpmlCertificate {
    pub finite_absolute_infinity_norm: f64,
    pub fixed_effect_absolute_infinity_norm: f64,
    pub relative_infinity_norm: f64,
    pub relative_two_norm: f64,
    pub passed: bool,
}

/// Relative size of one score moment against its basis norm and reference
/// scale. Dividing by the larger scale first keeps the product of two finite
/// norms from overflowing, which would turn a material moment into a false
/// zero. A zero scale accepts only a zero moment; nonfinite scales fail.
pub(super) fn relative_moment(moment: f64, basis_norm: f64, reference: f64) -> f64 {
    if basis_norm > 0.0 && reference > 0.0 {
        if basis_norm >= reference {
            (moment.abs() / basis_norm) / reference
        } else {
            (moment.abs() / reference) / basis_norm
        }
    } else if moment == 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}

#[cfg(test)]
mod tests {
    use super::relative_moment;

    #[test]
    fn relative_moment_survives_norm_products_beyond_f64() {
        // 1e200 * 1e200 overflows; the old product form reported zero.
        let relative = relative_moment(1e300, 1e200, 1e200);
        assert!(relative.is_finite() && relative > 0.0);
        assert_eq!(relative, (1e300 / 1e200) / 1e200);
        assert_eq!(relative_moment(0.0, 0.0, 1.0), 0.0);
        assert_eq!(relative_moment(1.0, 0.0, 1.0), f64::INFINITY);
        assert_eq!(relative_moment(1.0, f64::NAN, 1.0), f64::INFINITY);
        assert_eq!(relative_moment(-3.0, 2.0, 0.5), (3.0 / 2.0) / 0.5);
    }
}
