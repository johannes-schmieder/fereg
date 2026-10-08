//! Collision-free coding for categorical fixed-effect interactions.
//!
//! The factorizer stores output `u32` codes and either a bounded dense mapping
//! or a row ordering. It never materializes concatenated strings, hashed tuple
//! keys, or an observation-by-arity tuple table. Codes are assigned in lexicographic
//! component-code order, so the result is deterministic under row permutation
//! whenever each component's compact coding is deterministic.

use std::cmp::Ordering;

use crate::error::{allocation, invalid_argument as invalid, invalid_data, Result};
use crate::memory::push_reserved;

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
    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.codes)
    }

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
/// lexicographically by the supplied compact component codes. A bounded mapping
/// costs `O(A N + P)` for arity A, rows N and Cartesian levels P; larger products
/// use `O(A N log N)` comparison sorting. Component storage is borrowed.
pub fn factorize_code_interaction(
    components: &[CategoricalCodeView<'_>],
) -> Result<EncodedInteraction> {
    let mut reservation = SystemInteractionReservation;
    factorize_code_interaction_with_reservation(components, &mut reservation)
}

pub(crate) trait InteractionReservation {
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

    if let Some(encoded) =
        try_factorize_dense_code_interaction(nobs, components.iter().copied(), reservation)?
    {
        return Ok(encoded);
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

/// Callers validate component alignment and code ranges before this helper.
/// The table replaces the row order; the output temporarily holds tuple slots.
pub(crate) fn try_factorize_dense_code_interaction<'a, I, R>(
    nobs: usize,
    components: I,
    reservation: &mut R,
) -> Result<Option<EncodedInteraction>>
where
    I: Iterator<Item = CategoricalCodeView<'a>> + Clone,
    R: InteractionReservation,
{
    let Some(order_bytes) = nobs.checked_mul(std::mem::size_of::<usize>()) else {
        return Ok(None);
    };
    let Some(slots) = components
        .clone()
        .try_fold(1usize, |product, component| {
            product.checked_mul(component.n_levels)
        })
        .filter(|slots| {
            *slots > 0
                && *slots <= u32::MAX as usize
                && slots
                    .checked_mul(std::mem::size_of::<u32>())
                    .is_some_and(|bytes| bytes <= order_bytes)
        })
    else {
        return Ok(None);
    };
    let mut canonical_by_slot = Vec::new();
    reservation.reserve_vec(
        &mut canonical_by_slot,
        slots,
        "unable to reserve categorical-interaction dense mapping",
    )?;
    if canonical_by_slot.capacity() < slots {
        return Err(allocation(
            "categorical-interaction mapping capacity is insufficient",
        ));
    }
    canonical_by_slot.resize(slots, u32::MAX);
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
    for (row, code) in codes.iter_mut().enumerate() {
        // Validated code bounds and the admitted product bound every prefix.
        let slot = components.clone().fold(0usize, |prefix, component| {
            prefix * component.n_levels + component.codes[row] as usize
        });
        *code = slot as u32;
        canonical_by_slot[slot] = 0;
    }
    let mut n_levels = 0usize;
    for canonical in &mut canonical_by_slot {
        if *canonical != u32::MAX {
            *canonical = n_levels as u32;
            n_levels += 1;
        }
    }
    for code in &mut codes {
        *code = canonical_by_slot[*code as usize];
    }
    Ok(Some(EncodedInteraction { codes, n_levels }))
}

fn validate_components(components: &[CategoricalCodeView<'_>]) -> Result<usize> {
    let first = components
        .first()
        .ok_or_else(|| invalid("categorical interaction requires at least one component source"))?;
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

fn compare_rows(components: &[CategoricalCodeView<'_>], left: usize, right: usize) -> Ordering {
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

fn rows_equal(components: &[CategoricalCodeView<'_>], left: usize, right: usize) -> bool {
    components
        .iter()
        .all(|component| component.codes[left] == component.codes[right])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

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
        for levels in [2, 3] {
            let components = [
                CategoricalCodeView::new(&left, levels),
                CategoricalCodeView::new(&right, levels),
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
                if levels == 2 {
                    "unable to reserve categorical-interaction dense mapping"
                } else {
                    "unable to reserve categorical-interaction row ordering"
                },
                "unable to reserve categorical-interaction codes",
            ] {
                let mut undersized = InjectedReservation {
                    skip_label: Some(label),
                    ..InjectedReservation::default()
                };
                let error =
                    factorize_code_interaction_with_reservation(&components, &mut undersized)
                        .expect_err("capacity guard");
                assert_eq!(error.code, ErrorCode::Allocation, "{label}");
            }
        }
    }

    #[test]
    fn dense_table_admission_uses_the_displaced_order_bytes() {
        let left = [0, 1, 0];
        let right = [0, 2, 2];
        for nobs in [2, 3] {
            let components = [
                CategoricalCodeView::new(&left[..nobs], 2),
                CategoricalCodeView::new(&right[..nobs], 3),
            ];
            let mut reservation = InjectedReservation::default();
            let encoded = try_factorize_dense_code_interaction(
                nobs,
                components.iter().copied(),
                &mut reservation,
            )
            .expect("table planning");
            if nobs == 2 {
                assert!(encoded.is_none());
                assert_eq!(reservation.calls, 0);
            } else {
                assert_eq!(
                    encoded.expect("24 bytes fit 24-byte order budget").codes(),
                    &[0, 2, 1]
                );
                assert_eq!(reservation.calls, 2);
            }
        }
        let huge = [0, u32::MAX - 1, 1];
        let components = [CategoricalCodeView::new(&huge, u32::MAX as usize); 3];
        let mut reservation = InjectedReservation::default();
        assert!(try_factorize_dense_code_interaction(
            3,
            components.iter().copied(),
            &mut reservation
        )
        .expect("overflow declines")
        .is_none());
        assert_eq!(reservation.calls, 0);
        let encoded = factorize_code_interaction(&components).expect("overflow keeps sorting");
        assert_eq!(encoded.codes(), &[0, 2, 1]);
    }

    #[test]
    fn table_and_sort_routes_match_an_independent_tuple_map() {
        use std::collections::BTreeMap;
        for arity in 1..=6 {
            for nobs in [1, 7, 32, 257, 4096] {
                for extra_levels in [0, 19] {
                    let columns: Vec<Vec<u32>> = (0..arity)
                        .map(|column| {
                            (0..nobs)
                                .map(|row| {
                                    ((row * 17 + row / 7 + column * 3) % (column + 2)) as u32
                                })
                                .collect()
                        })
                        .collect();
                    let components: Vec<_> = columns
                        .iter()
                        .enumerate()
                        .map(|(index, codes)| {
                            CategoricalCodeView::new(codes, index + 2 + extra_levels)
                        })
                        .collect();
                    let tuples: Vec<Vec<u32>> = (0..nobs)
                        .map(|row| columns.iter().map(|codes| codes[row]).collect())
                        .collect();
                    let mut canonical: BTreeMap<Vec<u32>, u32> =
                        tuples.iter().cloned().map(|tuple| (tuple, 0)).collect();
                    for (code, (_, value)) in canonical.iter_mut().enumerate() {
                        *value = code as u32;
                    }
                    let expected: Vec<u32> = tuples.iter().map(|tuple| canonical[tuple]).collect();
                    let actual = factorize_code_interaction(&components).expect("factorization");
                    assert_eq!(actual.n_levels(), canonical.len());
                    assert_eq!(
                        actual.codes(),
                        expected,
                        "arity={arity}, rows={nobs}, extra={extra_levels}"
                    );
                }
            }
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
