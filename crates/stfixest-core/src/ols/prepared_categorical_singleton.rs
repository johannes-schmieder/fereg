//! Interaction-aware singleton pruning for prepared categorical absorbed terms.

use crate::error::Result;
use crate::fe::{
    prune_categorical_partition_singletons, CategoricalCodeView,
    CategoricalPartitionSingletonResult, SingletonMultiplicity,
};

use super::prepared_categorical_topology::{
    push_reserved, validate_source_table, PreparedTopologyReservation,
    SystemPreparedTopologyReservation,
};
use super::PreparedCategoricalEffectRequest;

/// Run the categorical-partition singleton fixed point over requested absorbed
/// terms. The returned registry is candidate-sample-only and may be discarded
/// after applying its active mask.
pub fn prune_prepared_categorical_singletons(
    source_table: &[CategoricalCodeView<'_>],
    requests: &[PreparedCategoricalEffectRequest],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep_singletons: bool,
) -> Result<CategoricalPartitionSingletonResult> {
    let mut reservation = SystemPreparedTopologyReservation;
    prune_with_reservation(
        source_table,
        requests,
        initially_active,
        multiplicity,
        keep_singletons,
        &mut reservation,
    )
}

fn prune_with_reservation<R: PreparedTopologyReservation>(
    source_table: &[CategoricalCodeView<'_>],
    requests: &[PreparedCategoricalEffectRequest],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep_singletons: bool,
    reservation: &mut R,
) -> Result<CategoricalPartitionSingletonResult> {
    validate_source_table(source_table, initially_active.len())?;
    let mut recipe_views = Vec::new();
    reservation.reserve_vec(
        &mut recipe_views,
        requests.len(),
        "unable to reserve prepared categorical singleton recipe views",
    )?;
    for request in requests {
        request.validate(source_table.len())?;
        push_reserved(
            &mut recipe_views,
            request.categorical_source_indices.as_slice(),
            "prepared categorical singleton recipe-view capacity exhausted",
        )?;
    }
    prune_categorical_partition_singletons(
        source_table,
        &recipe_views,
        initially_active,
        multiplicity,
        keep_singletons,
    )
}

#[cfg(test)]
pub(crate) fn prune_with_test_reservation<R: PreparedTopologyReservation>(
    source_table: &[CategoricalCodeView<'_>],
    requests: &[PreparedCategoricalEffectRequest],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep_singletons: bool,
    reservation: &mut R,
) -> Result<CategoricalPartitionSingletonResult> {
    prune_with_reservation(
        source_table,
        requests,
        initially_active,
        multiplicity,
        keep_singletons,
        reservation,
    )
}
