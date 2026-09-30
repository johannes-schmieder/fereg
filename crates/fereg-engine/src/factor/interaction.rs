//! Lexicographic tuple encoding without an N-by-arity tuple bank.
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy)]
pub struct CategoricalCodeView<'a> {
    pub codes: &'a [u32],
    pub n_levels: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedInteraction {
    pub codes: Vec<u32>,
    pub n_levels: usize,
}

pub fn factorize_code_interaction(
    components: &[CategoricalCodeView<'_>],
) -> Result<EncodedInteraction> {
    factorize_code_interaction_polled(components, None)
}
pub(crate) fn factorize_code_interaction_polled(
    components: &[CategoricalCodeView<'_>],
    poll: Poll<'_>,
) -> Result<EncodedInteraction> {
    execution::check(poll)?;
    let n = validate_components(components, poll)?;
    let compare = |a: usize, b: usize| {
        components
            .iter()
            .map(|component| component.codes[a].cmp(&component.codes[b]))
            .find(|order| *order != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    };
    let mut order = reserved(n)?;
    for row in 0..n {
        execution::check_chunk(poll, row)?;
        order.push(row);
    }
    execution::sort_unstable_by(&mut order, |&a, &b| compare(a, b), poll)?;
    let mut codes = reserved(n)?;
    codes.resize(n, 0);
    let mut n_levels = 0;
    let mut previous = None;
    for (position, row) in order.into_iter().enumerate() {
        execution::check_chunk(poll, position)?;
        if previous.is_none_or(|old| compare(old, row) != Ordering::Equal) {
            n_levels += 1;
        }
        codes[row] = u32::try_from(n_levels).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "too many observed categorical tuples",
            )
        })? - 1;
        previous = Some(row);
    }
    execution::check(poll)?;
    Ok(EncodedInteraction { codes, n_levels })
}
fn validate_components(components: &[CategoricalCodeView<'_>], poll: Poll<'_>) -> Result<usize> {
    let first = components
        .first()
        .ok_or_else(|| invalid("categorical interaction requires at least one component source"))?;
    let nobs = first.codes.len();
    for (component_index, component) in components.iter().copied().enumerate() {
        execution::check(poll)?;
        if component.codes.len() != nobs {
            return Err(invalid(format!(
                "categorical interaction component {} has {} rows; expected {nobs}",
                component_index + 1,
                component.codes.len(),
            )));
        }
        if nobs == 0 {
            if component.n_levels != 0 {
                return Err(invalid(format!(
                    "empty categorical interaction component {} declares {} levels",
                    component_index + 1,
                    component.n_levels,
                )));
            }
            continue;
        }
        if component.n_levels == 0 {
            return Err(invalid(format!(
                "nonempty categorical interaction component {} declares zero levels",
                component_index + 1,
            )));
        }
        if component.n_levels > u32::MAX as usize {
            return Err(invalid(format!(
                "categorical interaction component {} level count exceeds u32::MAX",
                component_index + 1,
            )));
        }
        for (row, code) in component.codes.iter().copied().enumerate() {
            execution::check_chunk(poll, row)?;
            if code as usize >= component.n_levels {
                return Err(invalid_data(format!(
                    "categorical interaction component {} code at row {} is outside 0..{}",
                    component_index + 1,
                    row + 1,
                    component.n_levels,
                )));
            }
        }
    }
    Ok(nobs)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}
fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

impl<'a> CategoricalCodeView<'a> {
    #[must_use]
    pub const fn new(codes: &'a [u32], n_levels: usize) -> Self {
        Self { codes, n_levels }
    }
}
impl EncodedInteraction {
    #[must_use]
    pub fn codes(&self) -> &[u32] {
        &self.codes
    }
    #[must_use]
    pub const fn n_levels(&self) -> usize {
        self.n_levels
    }
    #[must_use]
    pub fn into_codes(self) -> Vec<u32> {
        self.codes
    }
}
