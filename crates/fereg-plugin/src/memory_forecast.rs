//! Expected use is distinct from the conservative admission envelope.
//! Sequential finite and FE workspaces are not simultaneously resident.
use fereg_core::error::{ErrorCode, FeregError, Result};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Forecast {
    pub native_peak: u128,
    pub stata_growth: u128,
}
impl Forecast {
    /// Private calibration candidate, never a certificate or admission input.
    pub(crate) fn trace(
        self,
        baseline: crate::memory_report::Baseline,
        native_before: Option<u64>,
    ) -> Result<()> {
        if std::env::var_os("FEREG_ACTION_MEMORY_AUDIT").is_some() {
            let combined = match (baseline.before_allocated, native_before) {
                (Some(stata), Some(native)) => Some(self.combined(stata, native)?),
                _ => None,
            };
            eprintln!("FEREG_MEMORY_FORECAST|version=2|calibrated=0|native_peak={}|stata_growth={}|combined={combined:?}", self.native_peak, self.stata_growth);
        }
        Ok(())
    }

    pub(crate) fn combined(self, stata_before: u128, native_before: u64) -> Result<u128> {
        sum(&[
            stata_before,
            u128::from(native_before),
            self.native_peak,
            self.stata_growth,
        ])
    }
    /// Observation outputs already exist in the pre-plugin Stata baseline.
    /// Publication may copy e(sample) and each numeric result matrix. Descriptor
    /// and stripe storage receives a separate small, explicitly estimated bank.
    pub(crate) fn publication_growth(original_rows: usize, matrix_cells: u128) -> Result<u128> {
        sum(&[original_rows as u128, mul(matrix_cells, 8)?, 64 << 10])
    }
}

pub(crate) fn ols_projection_parent(
    prepared_owner: u128,
    metadata_owner: u128,
    rows: usize,
    generated_columns: usize,
    poolsize: usize,
    fast: bool,
) -> Result<u128> {
    let columns = if fast {
        generated_columns
    } else {
        generated_columns.min(poolsize)
    };
    // Raw [y X] is owned by the caller of absorption. The normalized weight
    // lane is separate; later finite QR/cache banks must not be added here.
    let row_bytes = mul(sum(&[columns as u128, 2])?, 8)?;
    sum(&[
        prepared_owner,
        metadata_owner,
        mul(rows as u128, row_bytes)?,
    ])
}

pub(crate) fn sum(values: &[u128]) -> Result<u128> {
    values
        .iter()
        .try_fold(0_u128, |a, b| a.checked_add(*b).ok_or_else(overflow))
}
fn mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b).ok_or_else(overflow)
}
fn overflow() -> FeregError {
    FeregError::new(ErrorCode::Allocation, "memory forecast overflows")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sequential_finite_buffers_are_not_projection_parents() {
        // Three solver matrices are incremental. The caller owns one RHS bank
        // and a weight lane, independently of the later two finite banks.
        let n = 4_361_733_usize;
        let rhs = 19_u128;
        let solver = n as u128 * rhs * 8 * 3;
        let parent = ols_projection_parent(1234, 567, n, 18, 10, true).unwrap();
        assert_eq!(parent + solver, 1234 + 567 + n as u128 * (20 + 57) * 8);
        assert_eq!(
            ols_projection_parent(1234, 567, n, 18, 10, false).unwrap(),
            1234 + 567 + n as u128 * 12 * 8
        );
    }
    #[test]
    fn baseline_is_added_once_and_does_not_change_available_memory() {
        let f = Forecast {
            native_peak: 200,
            stata_growth: 30,
        };
        assert_eq!(f.combined(1000, 40).unwrap(), 1270);
        assert!(f.combined(u128::MAX, 0).is_err());
        assert!(ols_projection_parent(u128::MAX, 1, 1, 1, 1, true).is_err());
    }
}
