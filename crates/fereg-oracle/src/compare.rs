//! Observable-result comparisons, independent of backend certificates (SPEC §20).
#[derive(Debug, Clone, Copy)]
pub struct Tolerance {
    pub relative: f64,
    pub absolute: f64,
}

pub const COEFFICIENT: Tolerance = Tolerance {
    relative: 1e-7,
    absolute: 1e-10,
};
pub const INFERENCE: Tolerance = Tolerance {
    relative: 1e-6,
    absolute: 1e-10,
};
pub const PREDICTION: Tolerance = Tolerance {
    relative: 1e-7,
    absolute: 1e-9,
};

impl Tolerance {
    pub fn check(self, actual: f64, expected: f64) -> Result<(), String> {
        let bound = self.absolute + self.relative * expected.abs();
        if actual.is_finite() && expected.is_finite() && (actual - expected).abs() <= bound {
            Ok(())
        } else {
            Err(format!(
                "actual={actual:.17e}, expected={expected:.17e}, bound={bound:.3e}"
            ))
        }
    }

    pub fn check_slice(self, actual: &[f64], expected: &[f64]) -> Result<(), String> {
        if actual.len() != expected.len() {
            return Err(format!(
                "lengths differ: {} and {}",
                actual.len(),
                expected.len()
            ));
        }
        for (i, (&a, &b)) in actual.iter().zip(expected).enumerate() {
            self.check(a, b)
                .map_err(|error| format!("slot {i}: {error}"))?;
        }
        Ok(())
    }
}
