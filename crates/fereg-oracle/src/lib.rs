#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
//! Independent small-design oracles and seeded test inputs; never a runtime dependency.
pub mod compare;
pub mod designs;
pub mod explicit_dummy;
pub mod explicit_ols;
pub mod explicit_ppml;
