use fereg_core::ppml::{
    estimate_ppml_memory, plan_ppml_memory, PpmlMemoryConfig, PpmlMemoryDimensions, PpmlMemoryRoute,
};
use fereg_core::{ErrorCode, MemoryMode};

const OBSERVATIONS: usize = 10_000_000;
const DEFAULT_POOLSIZE: usize = 10;
// With the full retained-session, report, rollback, and accepted-rebase
// lifetimes modeled, this limit leaves room for Low while still rejecting
// Fast after the embedded input floor is included.
const LOW_FAST_LIMIT_BYTES: u128 = 33_100_000_000;
const NO_ROUTE_LIMIT_BYTES: u128 = 1_500_000_000;

// The complete-input loader keeps outcome plus every source as an f64 vector
// and a bit-packed missingness vector, together with the candidate bit vector.
// For the frozen k50/two-FE recipe that is 53 numeric and 54 bit vectors.
const K50_EMBEDDING_RESIDENT_FLOOR_BYTES: u128 = 4_307_500_000;

fn governing_k50_dimensions() -> PpmlMemoryDimensions {
    PpmlMemoryDimensions {
        observations: OBSERVATIONS,
        original_observations: OBSERVATIONS,
        candidate_observations: OBSERVATIONS,
        loaded_numeric_columns: 52,
        retained_base_adjustment_capacity_elements: 0,
        retained_raw_preseparation_probability_weight_elements: 0,
        finite_columns: 51,
        coefficient_slots: 51,
        reported_coefficient_slots: 51,
        fe_dimensions: 2,
        fe_total_levels: 20_000_000,
        fe_max_levels: OBSERVATIONS,
        fe_total_channels: 2,
        fe_max_channels: 1,
        fe_max_level_members: OBSERVATIONS,
        fe_coefficient_directions: 20_000_000,
        fe_max_effect_directions: OBSERVATIONS,
        cluster_dimensions: 0,
        cluster_subset_groups: 0,
        separation_workspace_vectors: 8,
        separation_singleton_enabled: true,
        separation_method_count: 4,
        separation_rebase_enabled: true,
        separation_exact_fe_enabled: true,
        separation_mu_enabled: false,
        simplex_tableau_cells: 1_000_000,
        requested_output_columns: 0,
        requested_mean_output: false,
        requested_saved_fe_channels: 0,
        publication_matrix_cells: 3_241,
        threads: 1,
        allocation_audit_enabled: false,
    }
}

fn planner_config(
    requested: MemoryMode,
    available_bytes: u128,
    embedding_resident_bytes: u128,
) -> PpmlMemoryConfig {
    PpmlMemoryConfig {
        requested,
        require_fast_route: false,
        available_bytes: Some(available_bytes),
        embedding_resident_bytes,
        poolsize: DEFAULT_POOLSIZE,
        diagnostic_capacity: 128,
    }
}

#[test]
fn governing_k50_shape_selects_low_and_rejects_fast_at_constrained_memory() {
    let dimensions = governing_k50_dimensions();
    let base = estimate_ppml_memory(dimensions, PpmlMemoryConfig::default())
        .expect("frozen governing estimate");
    // Fast retains the complete two-FE PCG state for all K+1 projected RHS:
    // L + 2 * L_left * (K+1) + 4 * L_right * (K+1) f64 cells,
    // plus five recurrence scalars per RHS.
    assert_eq!(base.fast.peak_bytes, 47_400_233_650);
    assert_eq!(base.fast.separation_peak_bytes, 47_170_268_279);
    assert_eq!(base.low.peak_bytes, 21_430_045_694);
    assert_eq!(base.low.separation_peak_bytes, 21_430_045_694);
    assert_eq!(base.fast.peak_bytes - base.low.peak_bytes, 25_970_187_956);

    let numeric_vector_bytes = 53_u128 * OBSERVATIONS as u128 * 8;
    let bit_vector_bytes = 54_u128 * OBSERVATIONS as u128 / 8;
    assert_eq!(
        numeric_vector_bytes + bit_vector_bytes,
        K50_EMBEDDING_RESIDENT_FLOOR_BYTES
    );

    let embedded = estimate_ppml_memory(
        dimensions,
        planner_config(
            MemoryMode::Auto,
            LOW_FAST_LIMIT_BYTES,
            K50_EMBEDDING_RESIDENT_FLOOR_BYTES,
        ),
    )
    .expect("embedded governing estimate");
    assert_eq!(embedded.fast.peak_bytes, 51_707_733_650);
    assert_eq!(embedded.low.peak_bytes, 25_737_545_694);

    let auto = plan_ppml_memory(
        dimensions,
        planner_config(
            MemoryMode::Auto,
            LOW_FAST_LIMIT_BYTES,
            K50_EMBEDDING_RESIDENT_FLOOR_BYTES,
        ),
    )
    .expect("auto must fall back to the low route");
    assert_eq!(auto.selected, PpmlMemoryRoute::Low);
    assert_eq!(auto.safety_headroom_bytes, 6_620_000_000);
    assert_eq!(auto.usable_bytes, Some(26_480_000_000));
    assert_eq!(
        auto.usable_bytes.expect("imposed usable memory") - embedded.low.peak_bytes,
        742_454_306
    );
    assert_eq!(
        embedded.fast.peak_bytes - auto.usable_bytes.expect("imposed usable memory"),
        25_227_733_650
    );

    let low = plan_ppml_memory(
        dimensions,
        planner_config(
            MemoryMode::Low,
            LOW_FAST_LIMIT_BYTES,
            K50_EMBEDDING_RESIDENT_FLOOR_BYTES,
        ),
    )
    .expect("explicit low route must fit");
    assert_eq!(low.selected, PpmlMemoryRoute::Low);

    let fast_error = plan_ppml_memory(
        dimensions,
        planner_config(
            MemoryMode::Fast,
            LOW_FAST_LIMIT_BYTES,
            K50_EMBEDDING_RESIDENT_FLOOR_BYTES,
        ),
    )
    .expect_err("explicit fast route must fail preflight");
    assert_eq!(fast_error.code, ErrorCode::Allocation);
    assert!(fast_error.message.contains("before workspace allocation"));

    let no_route_error = plan_ppml_memory(
        dimensions,
        planner_config(
            MemoryMode::Auto,
            NO_ROUTE_LIMIT_BYTES,
            K50_EMBEDDING_RESIDENT_FLOOR_BYTES,
        ),
    )
    .expect_err("the no-route fixture must fail preflight");
    assert_eq!(no_route_error.code, ErrorCode::Allocation);
}

#[test]
fn previous_k10_four_gigabyte_shape_still_rejects_every_route() {
    let dimensions = PpmlMemoryDimensions {
        loaded_numeric_columns: 12,
        finite_columns: 11,
        coefficient_slots: 11,
        reported_coefficient_slots: 11,
        publication_matrix_cells: 561,
        ..governing_k50_dimensions()
    };
    let base = estimate_ppml_memory(dimensions, PpmlMemoryConfig::default())
        .expect("previous constrained estimate");
    assert_eq!(base.fast.peak_bytes, 21_800_051_330);
    assert_eq!(base.low.peak_bytes, 16_521_099_386);
    assert_eq!(base.fast.peak_bytes - base.low.peak_bytes, 5_278_951_944);

    // Outcome plus twelve sources and their missingness masks, plus candidate.
    let old_embedding_resident_floor_bytes =
        13_u128 * OBSERVATIONS as u128 * 8 + 14_u128 * OBSERVATIONS as u128 / 8;
    assert_eq!(old_embedding_resident_floor_bytes, 1_057_500_000);

    for requested in [MemoryMode::Low, MemoryMode::Fast, MemoryMode::Auto] {
        let error = plan_ppml_memory(
            dimensions,
            planner_config(requested, 4_000_000_000, old_embedding_resident_floor_bytes),
        )
        .expect_err("the previous four-gigabyte fixture must reject every route");
        assert_eq!(error.code, ErrorCode::Allocation);
    }
}
