//! Collision-free coding for categorical fixed-effect interactions.
//!
//! The factorizer stores one row ordering and one output `u32` code per
//! observation. It never materializes concatenated strings, hashed tuple keys,
//! or an observation-by-arity tuple table. Codes are assigned in lexicographic
//! component-code order, so the result is deterministic under row permutation
//! whenever each component's compact coding is deterministic.

use std::cmp::Ordering;

use crate::error::{ErrorCode, Result, StFixestError};

/// Borrowed compact categorical codes for one interaction component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoricalCodeView<'a> {
    codes: &'a [u32],
    n_levels: usize,
}

impl<'a> CategoricalCodeView<'a> {
    #[must_use]
    pub const fn new(codes: &'a [u32], n_levels: usize) -> Self {
        Self { codes, n_levels }
    }

    #[must_use]
    pub const fn codes(self) -> &'a [u32] {
        self.codes
    }

    #[must_use]
    pub const fn n_levels(self) -> usize {
        self.n_levels
    }
}

/// Compact codes for one categorical interaction partition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedInteraction {
    codes: Vec<u32>,
    n_levels: usize,
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

/// Factorize an all-categorical interaction without lossy hashing.
///
/// The returned codes are contiguous in `0..n_levels` and ordered
/// lexicographically by the supplied compact component codes. The algorithm is
/// `O(A N log N)` for arity `A` and observations `N`, with one `usize` ordering
/// and one `u32` output vector. Component storage is borrowed.
pub fn factorize_code_interaction(
    components: &[CategoricalCodeView<'_>],
) -> Result<EncodedInteraction> {
    let mut reservation = SystemInteractionReservation;
    factorize_code_interaction_with_reservation(components, &mut reservation)
}

trait InteractionReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemInteractionReservation;

impl InteractionReservation for SystemInteractionReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

fn factorize_code_interaction_with_reservation<R: InteractionReservation>(
    components: &[CategoricalCodeView<'_>],
    reservation: &mut R,
) -> Result<EncodedInteraction> {
    let nobs = validate_components(components)?;
    if nobs == 0 {
        return Ok(EncodedInteraction {
            codes: Vec::new(),
            n_levels: 0,
        });
    }

    let mut order = Vec::new();
    reservation.reserve_vec(
        &mut order,
        nobs,
        "unable to reserve categorical-interaction row ordering",
    )?;
    for row in 0..nobs {
        push_reserved(
            &mut order,
            row,
            "categorical-interaction row-order capacity exhausted",
        )?;
    }
    order.sort_unstable_by(|left, right| compare_rows(components, *left, *right));

    let mut codes = Vec::new();
    reservation.reserve_vec(
        &mut codes,
        nobs,
        "unable to reserve categorical-interaction codes",
    )?;
    if codes.capacity() < nobs {
        return Err(allocation(
            "categorical-interaction code capacity is smaller than the observation count",
        ));
    }
    codes.resize(nobs, 0);

    let mut level_index = 0_usize;
    let mut previous_row = order[0];
    codes[previous_row] = 0;
    for &row in &order[1..] {
        if !rows_equal(components, previous_row, row) {
            level_index = level_index
                .checked_add(1)
                .ok_or_else(|| allocation("categorical-interaction level count overflows"))?;
            if level_index > u32::MAX as usize {
                return Err(invalid_data(
                    "categorical interaction has more than u32::MAX + 1 observed levels",
                ));
            }
        }
        codes[row] = u32::try_from(level_index).map_err(|_| {
            invalid_data("categorical interaction level code cannot be represented as u32")
        })?;
        previous_row = row;
    }
    let n_levels = level_index
        .checked_add(1)
        .ok_or_else(|| allocation("categorical-interaction level count overflows"))?;

    Ok(EncodedInteraction { codes, n_levels })
}

fn validate_components(components: &[CategoricalCodeView<'_>]) -> Result<usize> {
    let first = components.first().ok_or_else(|| {
        invalid("categorical interaction requires at least one component source")
    })?;
    let nobs = first.codes.len();
    for (component_index, component) in components.iter().copied().enumerate() {
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

fn compare_rows(
    components: &[CategoricalCodeView<'_>],
    left: usize,
    right: usize,
) -> Ordering {
    for component in components {
        match component.codes[left].cmp(&component.codes[right]) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    // A row-index tie-break makes the comparator total while leaving equal
    // tuples in one adjacent run. The assigned interaction code depends only on
    // the tuple, not on this tie-break.
    left.cmp(&right)
}

fn rows_equal(
    components: &[CategoricalCodeView<'_>],
    left: usize,
    right: usize,
) -> bool {
    components
        .iter()
        .all(|component| component.codes[left] == component.codes[right])
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn invalid(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidData, message)
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        fail_at: Option<usize>,
        skip_label: Option<&'static str>,
    }

    impl InteractionReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected categorical-interaction allocation failure",
                ));
            }
            if self.skip_label == Some(label) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test interaction reservation failed"))
        }
    }

    #[test]
    fn two_way_codes_are_compact_collision_free_and_lexicographic() {
        let firm = [2, 0, 2, 1, 0, 2, 1];
        let year = [1, 2, 1, 0, 2, 0, 0];
        let encoded = factorize_code_interaction(&[
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 3),
        ])
        .expect("factorization");
        assert_eq!(encoded.n_levels(), 4);
        // Tuples in lexical order are (0,2), (1,0), (2,0), (2,1).
        assert_eq!(encoded.codes(), &[3, 0, 3, 1, 0, 2, 1]);
    }

    #[test]
    fn three_way_and_repeated_components_are_exact() {
        let a = [0, 0, 1, 1, 1, 0];
        let b = [0, 1, 0, 1, 1, 0];
        let c = [1, 1, 0, 0, 1, 1];
        let encoded = factorize_code_interaction(&[
            CategoricalCodeView::new(&a, 2),
            CategoricalCodeView::new(&b, 2),
            CategoricalCodeView::new(&c, 2),
        ])
        .expect("three-way factorization");
        assert_eq!(encoded.n_levels(), 5);
        assert_eq!(encoded.codes(), &[0, 1, 2, 3, 4, 0]);

        let repeated = factorize_code_interaction(&[
            CategoricalCodeView::new(&a, 2),
            CategoricalCodeView::new(&a, 2),
        ])
        .expect("repeated component");
        assert_eq!(repeated.n_levels(), 2);
        assert_eq!(repeated.codes(), a);
    }

    #[test]
    fn row_permutation_preserves_tuple_to_code_mapping() {
        let left = [0, 1, 0, 2, 1, 2, 0];
        let right = [2, 0, 1, 1, 0, 1, 2];
        let original = factorize_code_interaction(&[
            CategoricalCodeView::new(&left, 3),
            CategoricalCodeView::new(&right, 3),
        ])
        .expect("original");
        let permutation = [5, 2, 6, 1, 4, 0, 3];
        let permuted_left = permutation.map(|row| left[row]);
        let permuted_right = permutation.map(|row| right[row]);
        let permuted = factorize_code_interaction(&[
            CategoricalCodeView::new(&permuted_left, 3),
            CategoricalCodeView::new(&permuted_right, 3),
        ])
        .expect("permuted");
        assert_eq!(original.n_levels(), permuted.n_levels());
        for (new_row, old_row) in permutation.into_iter().enumerate() {
            assert_eq!(permuted.codes()[new_row], original.codes()[old_row]);
        }
    }

    #[test]
    fn unused_component_levels_are_recompacted() {
        let left = [4, 4, 1, 1];
        let right = [3, 3, 2, 2];
        let encoded = factorize_code_interaction(&[
            CategoricalCodeView::new(&left, 5),
            CategoricalCodeView::new(&right, 4),
        ])
        .expect("factorization with holes");
        assert_eq!(encoded.n_levels(), 2);
        assert_eq!(encoded.codes(), &[1, 1, 0, 0]);
    }

    #[test]
    fn empty_input_has_no_levels() {
        let encoded = factorize_code_interaction(&[
            CategoricalCodeView::new(&[], 0),
            CategoricalCodeView::new(&[], 0),
        ])
        .expect("empty interaction");
        assert!(encoded.codes().is_empty());
        assert_eq!(encoded.n_levels(), 0);
    }

    #[test]
    fn malformed_components_fail_before_allocation() {
        let valid = [0, 1];
        let short = [0];
        let invalid_code = [0, 2];
        let cases = [
            factorize_code_interaction(&[]).expect_err("zero arity"),
            factorize_code_interaction(&[
                CategoricalCodeView::new(&valid, 2),
                CategoricalCodeView::new(&short, 1),
            ])
            .expect_err("row mismatch"),
            factorize_code_interaction(&[CategoricalCodeView::new(&valid, 0)])
                .expect_err("zero levels"),
            factorize_code_interaction(&[CategoricalCodeView::new(&invalid_code, 2)])
                .expect_err("invalid code"),
            factorize_code_interaction(&[CategoricalCodeView::new(&[], 1)])
                .expect_err("empty with levels"),
        ];
        assert_eq!(cases[0].code, ErrorCode::InvalidArgument);
        assert_eq!(cases[1].code, ErrorCode::InvalidArgument);
        assert_eq!(cases[2].code, ErrorCode::InvalidArgument);
        assert_eq!(cases[3].code, ErrorCode::InvalidData);
        assert_eq!(cases[4].code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn every_workspace_reservation_and_capacity_guard_fails_closed() {
        let left = [0, 0, 1, 1];
        let right = [0, 1, 0, 1];
        let components = [
            CategoricalCodeView::new(&left, 2),
            CategoricalCodeView::new(&right, 2),
        ];
        let mut successful = InjectedReservation::default();
        factorize_code_interaction_with_reservation(&components, &mut successful)
            .expect("successful reservation audit");
        assert_eq!(successful.calls, 2);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = factorize_code_interaction_with_reservation(&components, &mut injected)
                .expect_err("injected allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for label in [
            "unable to reserve categorical-interaction row ordering",
            "unable to reserve categorical-interaction codes",
        ] {
            let mut undersized = InjectedReservation {
                skip_label: Some(label),
                ..InjectedReservation::default()
            };
            let error = factorize_code_interaction_with_reservation(&components, &mut undersized)
                .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "{label}");
        }
    }

    #[test]
    fn moderate_large_case_retains_exact_partition() {
        const N: usize = 50_000;
        let left = (0..N)
            .map(|row| u32::try_from((row * 17) % 997).expect("bounded"))
            .collect::<Vec<_>>();
        let right = (0..N)
            .map(|row| u32::try_from((row * 31) % 101).expect("bounded"))
            .collect::<Vec<_>>();
        let encoded = factorize_code_interaction(&[
            CategoricalCodeView::new(&left, 997),
            CategoricalCodeView::new(&right, 101),
        ])
        .expect("large factorization");
        assert_eq!(encoded.codes().len(), N);
        for first in (0..N).step_by(137) {
            for second in (0..N).step_by(251) {
                assert_eq!(
                    encoded.codes()[first] == encoded.codes()[second],
                    left[first] == left[second] && right[first] == right[second]
                );
            }
        }
    }
}
