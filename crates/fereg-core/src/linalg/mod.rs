//! Small dependency-free dense linear-algebra kernels.

mod solve;

pub use solve::{dot, norm2, solve_linear_system};

/// One Neumaier compensated-sum update without a finiteness branch. A
/// nonfinite sum or correction stays nonfinite under later updates, so one
/// check after a loop reaches the same decision as checking every update.
#[inline(always)]
pub(crate) fn neumaier_add(value: f64, sum: &mut f64, correction: &mut f64) {
    let updated = *sum + value;
    let adjustment = if sum.abs() >= value.abs() {
        (*sum - updated) + value
    } else {
        (value - updated) + *sum
    };
    *correction += adjustment;
    *sum = updated;
}
