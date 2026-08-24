//! Small dependency-free dense linear-algebra kernels.

mod solve;

pub use solve::{dot, norm2, solve_linear_system};
