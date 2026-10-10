use super::*;
// Independent witnesses for the retained compatibility envelopes.
const LOW_SCRATCH_IO_BUFFER_CEILING_BYTES: u128 = 1 << 20;
const RETAINED_MOMENT_TILE_ELEMENTS: usize = 1024;

static PREFLIGHT_EVENTS: std::sync::Mutex<Vec<PpmlMemoryPreflightEvent>> =
    std::sync::Mutex::new(Vec::new());

fn record_preflight(event: PpmlMemoryPreflightEvent) -> Result<()> {
    PREFLIGHT_EVENTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(event);
    Ok(())
}

fn dimensions() -> PpmlMemoryDimensions {
    PpmlMemoryDimensions {
        metadata: Default::default(),
        observations: 100_000,
        original_observations: 100_000,
        candidate_observations: 100_000,
        loaded_numeric_columns: 6,
        retained_base_adjustment_capacity_elements: 0,
        retained_raw_preseparation_probability_weight_elements: 0,
        finite_columns: 20,
        coefficient_slots: 20,
        reported_coefficient_slots: 20,
        fe_dimensions: 2,
        fe_total_levels: 30_000,
        fe_max_levels: 20_000,
        fe_total_channels: 2,
        fe_max_channels: 1,
        fe_max_level_members: 20,
        fe_coefficient_directions: 30_000,
        fe_max_effect_directions: 20_000,
        cluster_dimensions: 2,
        cluster_subset_groups: 35_000,
        separation_workspace_vectors: 8,
        separation_singleton_enabled: true,
        separation_method_count: 4,
        separation_generation_capacity: 100_000,
        separation_rebase_enabled: true,
        separation_exact_fe_enabled: true,
        separation_mu_enabled: false,
        simplex_tableau_cells: 1_000_000,
        requested_output_columns: 0,
        requested_mean_output: false,
        requested_saved_fe_channels: 0,
        publication_matrix_cells: 2_000,
        threads: 4,
        allocation_audit_enabled: false,
    }
}

/// Zero-coefficient workspace, as the prepared initializer hands over its
/// owned predictor and coefficient vectors.
fn zero_start_workspace(plan: PpmlMemoryPlan, eta: &[f64]) -> Result<PpmlIterationWorkspace> {
    PpmlIterationWorkspace::from_initial_state(
        plan,
        eta.to_vec(),
        vec![0.0; plan.dimensions.coefficient_slots],
    )
}

fn plan_with_diagnostic_capacity(capacity: usize) -> PpmlMemoryPlan {
    plan_ppml_memory(
        dimensions(),
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            require_fast_route: false,
            available_bytes: None,
            embedding_resident_bytes: 0,
            poolsize: 4,
            diagnostic_capacity: capacity,
        },
    )
    .expect("memory plan")
}

#[test]
fn objective_partial_bank_is_charged_beside_all_iteration_vectors() {
    for threads in [1, 4, 8] {
        let shape = PpmlMemoryDimensions {
            threads,
            ..dimensions()
        };
        let estimate = estimate_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
        let cells = objective_partial_cells(shape.observations, threads);
        let expected = (10 * shape.observations as u128 + cells as u128) * 8;
        assert_eq!(estimate.fast.iteration_buffer_bytes, expected);
        assert_eq!(estimate.low.iteration_buffer_bytes, expected);
        let plan = plan_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
        let workspace = zero_start_workspace(plan, &vec![0.; shape.observations]).unwrap();
        assert_eq!(workspace.objective_partials.len(), cells);
        assert_eq!(workspace.objective_partials.capacity(), cells);
        assert_eq!(
            workspace.capacity_report().observation_capacity_sum,
            8 * shape.observations + cells
        );
        workspace.certify_steady_state_capacity().unwrap();
    }
}

#[test]
fn raw_owner_capacity_supplements_but_never_duplicates_embedding() {
    let shape = dimensions();
    let before = estimate_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
    for embedding in [0, 113, 10_007, 10_008] {
        let mut shape = shape;
        shape.metadata.input_owner_bytes = 10_007;
        let after = estimate_ppml_memory(
            shape,
            PpmlMemoryConfig {
                embedding_resident_bytes: embedding,
                ..PpmlMemoryConfig::default()
            },
        )
        .unwrap();
        assert_eq!(
            after.fast.input_owner_supplement_bytes,
            10_007_u128.saturating_sub(embedding)
        );
        assert_eq!(
            after.fast.iteration_peak_bytes - before.fast.iteration_peak_bytes,
            embedding.max(10_007)
        );
        assert_eq!(
            after.fast.separation_peak_bytes - before.fast.separation_peak_bytes,
            embedding.max(10_007)
        );
    }
}

#[test]
fn generation_supplement_is_admitted_for_both_simultaneous_owners() {
    let mut shape = dimensions();
    // Force the categorical capacity bound to dominate the old FE heuristic.
    shape.metadata.generation_fixed_bytes = 123_457;
    shape.metadata.generation_bytes_per_row = 1_024;
    let before = estimate_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
    shape.metadata.generation_fixed_bytes += 97;
    let after = estimate_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
    for (before, after) in [(before.fast, after.fast), (before.low, after.low)] {
        assert!(before.prepared_generation_supplement_bytes > 0);
        assert_eq!(
            after.prepared_generation_bytes - before.prepared_generation_bytes,
            97
        );
        assert_eq!(after.iteration_peak_bytes - before.iteration_peak_bytes, 97);
        assert_eq!(
            after.separation_replacement_generation_bytes
                - before.separation_replacement_generation_bytes,
            97
        );
        assert_eq!(
            after.separation_rebase_bytes - before.separation_rebase_bytes,
            97
        );
        assert_eq!(
            after.separation_peak_bytes - before.separation_peak_bytes,
            194
        );
    }
    let config = PpmlMemoryConfig {
        requested: MemoryMode::Fast,
        ..PpmlMemoryConfig::default()
    };
    let peak = estimate_ppml_memory(shape, config).unwrap().fast.peak_bytes;
    // This fixture remains below the fixed-headroom crossover.
    assert!(peak < 4 * MINIMUM_HEADROOM_BYTES);
    plan_ppml_memory(
        shape,
        PpmlMemoryConfig {
            available_bytes: Some(peak + MINIMUM_HEADROOM_BYTES),
            ..config
        },
    )
    .unwrap();
    assert_eq!(
        plan_ppml_memory(
            shape,
            PpmlMemoryConfig {
                available_bytes: Some(peak + MINIMUM_HEADROOM_BYTES - 1),
                ..config
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::Allocation
    );
    shape.metadata.generation_bytes_per_row = u128::MAX;
    assert_eq!(
        estimate_ppml_memory(shape, config).unwrap_err().code,
        ErrorCode::Allocation
    );
}

#[test]
fn nested_metadata_is_admitted_at_every_overlapping_lifetime() {
    let plain_dimensions = dimensions();
    let plain = estimate_ppml_memory(plain_dimensions, PpmlMemoryConfig::default()).unwrap();
    let mut shape = plain_dimensions;
    shape.metadata = super::super::PpmlMetadataMemory {
        prepared_nested_bytes: 70_003,
        preparation_transient_bytes: 90_007,
        result_bytes: 110_017,
        ..Default::default()
    };
    let charged = estimate_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
    for (before, after) in [(plain.fast, charged.fast), (plain.low, charged.low)] {
        assert_eq!(
            after.prepared_model_metadata_bytes - before.prepared_model_metadata_bytes,
            70_003
        );
        assert_eq!(
            after.separation_replacement_generation_bytes
                - before.separation_replacement_generation_bytes,
            160_010
        );
        assert_eq!(
            after.result_metadata_bytes - before.result_metadata_bytes,
            110_017
        );
        for (old, new) in [
            (before.iteration_peak_bytes, after.iteration_peak_bytes),
            (
                before.strict_final_peak_bytes,
                after.strict_final_peak_bytes,
            ),
            (before.inference_peak_bytes, after.inference_peak_bytes),
            (before.fe_recovery_peak_bytes, after.fe_recovery_peak_bytes),
            (before.publication_peak_bytes, after.publication_peak_bytes),
        ] {
            assert_eq!(
                new - old,
                180_020,
                "parents coexist; they are not sequential maxima"
            );
        }
    }
    for (requested, peak) in [
        (MemoryMode::Fast, charged.fast.peak_bytes),
        (MemoryMode::Low, charged.low.peak_bytes),
    ] {
        let config = PpmlMemoryConfig {
            requested,
            available_bytes: Some(peak + MINIMUM_HEADROOM_BYTES),
            ..PpmlMemoryConfig::default()
        };
        let plan = plan_ppml_memory(shape, config).unwrap();
        assert_eq!(plan.usable_bytes, Some(peak));
        assert_eq!(
            plan_ppml_memory(
                shape,
                PpmlMemoryConfig {
                    available_bytes: Some(peak + MINIMUM_HEADROOM_BYTES - 1),
                    ..config
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::Allocation
        );
        let mut changed = shape;
        changed.metadata.prepared_nested_bytes -= 1;
        assert!(
            plan.with_final_dimensions(changed).is_err(),
            "rebase cannot discard unreleased metadata capacity"
        );
    }
    shape.metadata.result_bytes = u128::MAX;
    assert_eq!(
        estimate_ppml_memory(shape, PpmlMemoryConfig::default())
            .unwrap_err()
            .code,
        ErrorCode::Allocation
    );
}

#[test]
fn map_preflight_covers_singleton_rich_owned_topology_construction() {
    let mut dimensions = dimensions();
    dimensions.observations = 10_000;
    dimensions.original_observations = 10_000;
    dimensions.candidate_observations = 10_000;
    dimensions.separation_generation_capacity = 2;
    dimensions.cluster_subset_groups = 10_000;
    dimensions.fe_dimensions = 1;
    dimensions.fe_total_levels = 10_000;
    dimensions.fe_max_levels = 10_000;
    dimensions.fe_total_channels = 1;
    dimensions.fe_max_channels = 1;
    dimensions.fe_max_level_members = 1;
    dimensions.fe_coefficient_directions = 10_000;
    dimensions.fe_max_effect_directions = 10_000;
    let estimate = estimate_ppml_memory(dimensions, PpmlMemoryConfig::default()).unwrap();
    let mut exact = crate::memory::OwnedFeDesignMemoryEstimate::default();
    exact.add_effect(10_000, 10_000, 0).unwrap();
    let old_envelope = 10_000_u128 * (16 + 16);
    assert!(exact.construction_bytes > old_envelope);
    for inventory in [estimate.fast, estimate.low] {
        assert!(inventory.fe_solver_topology_bytes >= exact.construction_bytes);
    }
    for (requested, inventory) in [
        (MemoryMode::Fast, estimate.fast),
        (MemoryMode::Low, estimate.low),
    ] {
        let config = PpmlMemoryConfig {
            requested,
            available_bytes: Some(inventory.peak_bytes + MINIMUM_HEADROOM_BYTES),
            ..PpmlMemoryConfig::default()
        };
        let admitted = plan_ppml_memory(dimensions, config).unwrap();
        assert_eq!(admitted.usable_bytes, Some(inventory.peak_bytes));
        assert_eq!(
            plan_ppml_memory(
                dimensions,
                PpmlMemoryConfig {
                    available_bytes: config.available_bytes.map(|bytes| bytes - 1),
                    ..config
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::Allocation
        );
    }
}

#[test]
fn all_positive_report_capacity_reduces_both_route_peaks_by_exact_storage() {
    let general = dimensions();
    let mut positive = general;
    positive.separation_generation_capacity = 2;
    let config = PpmlMemoryConfig::default();
    let wide = estimate_ppml_memory(general, config).expect("general plan");
    let narrow = estimate_ppml_memory(positive, config).expect("positive plan");
    let saving = (general.observations - 2) as u128
        * (4 * size_of::<SeparationMethodReport>() + size_of::<u64>()) as u128;
    for (before, after) in [(wide.fast, narrow.fast), (wide.low, narrow.low)] {
        assert_eq!(before.peak_bytes - after.peak_bytes, saving);
        assert_eq!(
            after.separation_method_report_record_bytes,
            (8 * size_of::<SeparationMethodReport>()) as u128
        );
        assert_eq!(after.separation_generation_hash_bytes, 16);
    }
    let mut plan = plan_ppml_memory(positive, config).expect("positive plan");
    let mut enlarged = positive;
    enlarged.separation_generation_capacity = 3;
    assert!(plan.with_final_dimensions(enlarged).is_err());
    plan = plan.with_final_dimensions(positive).expect("same envelope");
    assert_eq!(plan.preflight_dimensions.separation_generation_capacity, 2);
}

#[test]
fn low_route_replaces_resident_n_by_k_with_pool_and_scratch() {
    let estimate = estimate_ppml_memory(
        dimensions(),
        PpmlMemoryConfig {
            poolsize: 4,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("estimate");
    assert!(
        estimate.low.retained_within_resident_bytes < estimate.fast.retained_within_resident_bytes
    );
    assert_eq!(
        estimate.fast.retained_within_resident_bytes,
        100_000_u128 * (20 + 1) * size_of::<f64>() as u128,
        "fast retains the pre-compaction N-by-(K+1) strided capacity",
    );
    assert_eq!(
        estimate.low.retained_within_resident_bytes,
        100_000_u128 * 4 * size_of::<f64>() as u128,
        "low retains only its bounded resident pool",
    );
    assert_eq!(
        estimate.fast.fe_workspace_bytes,
        (21_u128 * 14
            + 1
            + 21
            + 2 * 20
            + 20_000 * 21
            + 20_000
            + 100_000_u128.div_ceil(PARALLEL_DOT_CHUNK_ROWS as u128) * 21
            + 30_000
            + 2 * 10_000 // component roots and current-weight scales
            + 2 * 20_000 * 21
            + 4 * 10_000 * 21
            + 5 * 21)
            * size_of::<f64>() as u128,
        "fast MAP capacity covers combined projection, parallel group, and two-FE level-PCG workspaces",
    );
    assert_eq!(
        estimate.low.fe_workspace_bytes,
        (4_u128 * 14
            + 1
            + 4
            + 2 * 20
            + 100_000 * 4
            + 20_000 * 4
            + 20_000
            + 100_000_u128.div_ceil(PARALLEL_DOT_CHUNK_ROWS as u128) * 4
            + 30_000
            + 2 * 10_000 // component roots and current-weight scales
            + 2 * 20_000 * 4
            + 4 * 10_000 * 4
            + 5 * 4)
            * size_of::<f64>() as u128,
        "low admits pool-width categorical projection and level-PCG workspaces",
    );
    assert_eq!(estimate.fast.retained_within_io_buffer_bytes, 0);
    assert_eq!(
        estimate.low.retained_within_io_buffer_bytes, LOW_SCRATCH_IO_BUFFER_CEILING_BYTES,
        "low retains its bounded row-major scratch I/O tile",
    );
    assert_eq!(estimate.fast.retained_within_scratch_bytes, 0);
    assert_eq!(
        estimate.low.retained_within_scratch_bytes,
        100_000_u128 * 20 * 8
    );
    assert!(estimate.low.peak_bytes < estimate.fast.peak_bytes);
}

#[test]
fn low_categorical_workspace_caps_oversized_pools_before_admission() {
    let mut dimensions = dimensions();
    dimensions.finite_columns = 3;
    dimensions.coefficient_slots = 3;
    dimensions.reported_coefficient_slots = 3;
    dimensions.threads = 1;
    let small = estimate_ppml_memory(
        dimensions,
        PpmlMemoryConfig {
            poolsize: 3,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("bounded low pool");
    let large = estimate_ppml_memory(
        dimensions,
        PpmlMemoryConfig {
            poolsize: 10_000,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("oversized requested low pool");
    assert_eq!(small.low.fe_workspace_bytes, large.low.fe_workspace_bytes);
    assert_eq!(small.low.peak_bytes, large.low.peak_bytes);
    let rhs = 3_u128;
    let generic = rhs * 14 + 1 + rhs + 2 * 20 + 100_000 * rhs;
    let graph = 30_000 + 2 * 10_000 + 2 * 20_000 * rhs + 4 * 10_000 * rhs + 5 * rhs;
    assert_eq!(large.low.fe_workspace_bytes, (generic + graph) * 8);
}

#[test]
fn narrow_fast_two_fe_route_charges_level_pcg_state_exactly() {
    let mut narrow = dimensions();
    narrow.finite_columns = 3;
    narrow.coefficient_slots = 3;
    narrow.reported_coefficient_slots = 3;
    narrow.threads = 1;
    let estimate = estimate_ppml_memory(narrow, PpmlMemoryConfig::default())
        .expect("narrow categorical estimate");
    let rhs = 4_u128;
    let generic_map_cells = rhs * 14 + 1 + rhs + 2 * 20;
    let accelerator_cells =
        30_000_u128 + 2 * 10_000 + 2 * 20_000 * rhs + 4 * 10_000 * rhs + 5 * rhs;
    assert_eq!(
        estimate.fast.fe_workspace_bytes,
        (generic_map_cells + accelerator_cells) * size_of::<f64>() as u128,
    );
}

#[test]
fn sparse_two_fe_fast_route_charges_level_pcg_state_exactly() {
    let mut sparse = dimensions();
    sparse.observations = 256;
    sparse.separation_generation_capacity = 256;
    sparse.original_observations = 256;
    sparse.candidate_observations = 256;
    sparse.finite_columns = 1;
    sparse.coefficient_slots = 1;
    sparse.reported_coefficient_slots = 1;
    sparse.fe_total_levels = 144;
    sparse.fe_max_levels = 80;
    sparse.fe_max_level_members = 4;
    sparse.fe_coefficient_directions = 144;
    sparse.fe_max_effect_directions = 80;
    sparse.threads = 1;
    let estimate = estimate_ppml_memory(sparse, PpmlMemoryConfig::default())
        .expect("sparse two-FE categorical estimate");
    let rhs = 2_u128;
    let generic_map_cells = rhs * 14 + 1 + rhs + 2 * 4;
    let graph_cells = 144 + 2 * 64 + 2 * 80 * rhs + 4 * 64 * rhs + 5 * rhs;
    assert_eq!(
        estimate.fast.fe_workspace_bytes,
        (generic_map_cells + graph_cells) * size_of::<f64>() as u128,
    );
}

#[test]
fn low_scratch_io_buffer_covers_empty_bounded_and_wide_rows_exactly() {
    let mut empty = dimensions();
    empty.observations = 1;
    empty.separation_generation_capacity = 1;
    empty.candidate_observations = 1;
    empty.original_observations = 1;
    empty.loaded_numeric_columns = 0;
    empty.finite_columns = 0;
    empty.coefficient_slots = 0;
    empty.reported_coefficient_slots = 0;
    empty.fe_max_level_members = 1;
    let empty_inventory = estimate_ppml_memory(empty, PpmlMemoryConfig::default())
        .expect("rank-zero low scratch estimate")
        .low;
    assert_eq!(empty_inventory.retained_within_io_buffer_bytes, 8);

    let bounded_inventory = estimate_ppml_memory(dimensions(), PpmlMemoryConfig::default())
        .expect("bounded low scratch estimate")
        .low;
    assert_eq!(
        bounded_inventory.retained_within_io_buffer_bytes,
        LOW_SCRATCH_IO_BUFFER_CEILING_BYTES,
    );

    let mut wide = empty;
    wide.finite_columns = 200_000;
    wide.coefficient_slots = 200_000;
    wide.reported_coefficient_slots = 200_000;
    let wide_inventory = estimate_ppml_memory(wide, PpmlMemoryConfig::default())
        .expect("wide-row low scratch estimate")
        .low;
    assert_eq!(
        wide_inventory.retained_within_io_buffer_bytes,
        200_000_u128 * size_of::<f64>() as u128,
        "one complete scratch row dominates the one-MiB tile ceiling",
    );
}

#[test]
fn preflight_probe_observes_rejection_before_workspace_allocation() {
    let base = PpmlMemoryConfig {
        poolsize: 4,
        ..PpmlMemoryConfig::default()
    };
    let estimate = estimate_ppml_memory(dimensions(), base).expect("estimate");
    assert!(estimate.low.peak_bytes < estimate.fast.peak_bytes);
    let available = (estimate.low.peak_bytes + MINIMUM_HEADROOM_BYTES)
        .max((estimate.low.peak_bytes * 5).div_ceil(4));
    assert!(available - MINIMUM_HEADROOM_BYTES.max(available / 5) < estimate.fast.peak_bytes);
    PREFLIGHT_EVENTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();

    let error = plan_ppml_memory_with_probe(
        dimensions(),
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            available_bytes: Some(available),
            ..base
        },
        Some(record_preflight),
    )
    .expect_err("fast route must reject at preflight");

    assert_eq!(error.code, ErrorCode::Allocation);
    let events = PREFLIGHT_EVENTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], PpmlMemoryPreflightEvent::Begin);
    let PpmlMemoryPreflightEvent::Decision(observation) = events[1] else {
        panic!("preflight probe omitted decision")
    };
    assert_eq!(observation.requested, MemoryMode::Fast);
    assert_eq!(observation.selected, None);
    assert!(!observation.workspace_allocation_started);
    assert_eq!(observation.fast_peak_bytes, estimate.fast.peak_bytes);
    assert_eq!(observation.low_peak_bytes, estimate.low.peak_bytes);
}

#[test]
fn generation_zero_plan_rebases_only_to_a_bounded_final_generation() {
    let config = PpmlMemoryConfig {
        requested: MemoryMode::Auto,
        require_fast_route: false,
        available_bytes: None,
        embedding_resident_bytes: 0,
        poolsize: 4,
        diagnostic_capacity: 32,
    };
    let preliminary = plan_ppml_memory(dimensions(), config).expect("preliminary plan");
    let mut final_dimensions = dimensions();
    final_dimensions.observations = 80_000;
    final_dimensions.separation_generation_capacity = 80_000;
    final_dimensions.finite_columns = 16;
    final_dimensions.coefficient_slots = 16;
    final_dimensions.reported_coefficient_slots = 18;
    final_dimensions.fe_dimensions = 1;
    final_dimensions.fe_total_levels = 24_000;
    final_dimensions.fe_max_levels = 16_000;
    final_dimensions.fe_total_channels = 1;
    final_dimensions.fe_max_channels = 1;
    final_dimensions.fe_max_level_members = 16;
    final_dimensions.fe_coefficient_directions = 24_000;
    final_dimensions.fe_max_effect_directions = 16_000;
    final_dimensions.cluster_subset_groups = 28_000;
    final_dimensions.publication_matrix_cells = 1_500;
    let rebased = preliminary
        .with_final_dimensions(final_dimensions)
        .expect("bounded final rebase");
    assert_eq!(rebased.preflight_dimensions, dimensions());
    assert_eq!(rebased.dimensions, final_dimensions);
    assert_eq!(rebased.selected, preliminary.selected);
    assert_eq!(rebased.estimate, preliminary.estimate);

    let mut growing = final_dimensions;
    growing.observations = dimensions().observations + 1;
    growing.separation_generation_capacity = dimensions().observations + 1;
    let error = preliminary
        .with_final_dimensions(growing)
        .expect_err("final generation cannot exceed generation zero");
    assert_eq!(error.code, ErrorCode::InvalidArgument);

    let mut changed_policy = final_dimensions;
    changed_policy.separation_workspace_vectors -= 1;
    assert_eq!(
        preliminary
            .with_final_dimensions(changed_policy)
            .expect_err("execution-policy dimensions remain exact")
            .code,
        ErrorCode::InvalidArgument,
    );
}

#[test]
fn simplex_preflight_charges_joint_live_design_and_kernel_ceiling() {
    let mut without_simplex = dimensions();
    // Isolate the simplex branch from the sequential IR/Mu vector bank.
    // When both are enabled the planner correctly takes their maximum.
    without_simplex.separation_workspace_vectors = 0;
    without_simplex.simplex_tableau_cells = 0;
    without_simplex.separation_singleton_enabled = false;
    without_simplex.separation_method_count = 0;
    without_simplex.separation_rebase_enabled = false;
    without_simplex.separation_exact_fe_enabled = false;
    let base = estimate_ppml_memory(without_simplex, PpmlMemoryConfig::default())
        .expect("non-simplex memory estimate");
    let mut with_simplex = without_simplex;
    with_simplex.simplex_tableau_cells = 12_345;
    with_simplex.separation_method_count = 1;
    with_simplex.separation_rebase_enabled = true;
    let bounded = estimate_ppml_memory(with_simplex, PpmlMemoryConfig::default())
        .expect("bounded full-model simplex estimate");
    let one_ceiling = 12_345_u128 * size_of::<f64>() as u128;

    for (plain, simplex) in [(base.fast, bounded.fast), (base.low, bounded.low)] {
        assert_eq!(simplex.simplex_combined_bytes, one_ceiling);
        let linear_validation = simplex.coefficient_buffer_bytes
            + simplex.retained_within_resident_bytes
            + simplex.retained_within_io_buffer_bytes
            + simplex.fe_solver_topology_bytes
            + simplex.linear_vector_bytes
            + simplex.fe_workspace_bytes
            + simplex.linear_workspace_bytes
            + simplex.linear_result_metadata_bytes;
        let proposal = with_simplex.observations as u128
            * (size_of::<usize>() as u128 + size_of::<bool>() as u128);
        let certificates = 2_u128
            * with_simplex.finite_columns as u128
            * with_simplex.finite_columns as u128
            * size_of::<f64>() as u128;
        assert_eq!(
            simplex.separation_workspace_bytes,
            one_ceiling.max(linear_validation + certificates) + proposal,
            "simplex storage and shared-linear validation are sequential method alternatives",
        );
        assert_eq!(plain.separation_workspace_bytes, 0);
        assert_eq!(plain.simplex_combined_bytes, 0);
    }

    let minimum_bounded_peak = bounded.fast.peak_bytes.min(bounded.low.peak_bytes);
    let usable_just_below_bound = minimum_bounded_peak - 1;
    assert!(
        minimum_bounded_peak - one_ceiling <= usable_just_below_bound,
        "a stale zero-simplex model would have admitted this fit",
    );
    let error = plan_ppml_memory(
        with_simplex,
        PpmlMemoryConfig {
            requested: MemoryMode::Auto,
            available_bytes: Some(MINIMUM_HEADROOM_BYTES + usable_just_below_bound),
            ..PpmlMemoryConfig::default()
        },
    )
    .expect_err("joint simplex peak must fail before allocation");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn retained_generation_zero_active_pweights_are_charged_exactly() {
    let mut unweighted_dimensions = dimensions();
    unweighted_dimensions.candidate_observations = unweighted_dimensions.observations + 10_000;
    unweighted_dimensions.original_observations =
        unweighted_dimensions.candidate_observations + 10_000;
    let unweighted = estimate_ppml_memory(unweighted_dimensions, PpmlMemoryConfig::default())
        .expect("unweighted estimate");
    let mut probability_dimensions = unweighted_dimensions;
    probability_dimensions.retained_raw_preseparation_probability_weight_elements =
        probability_dimensions.observations;
    let probability = estimate_ppml_memory(probability_dimensions, PpmlMemoryConfig::default())
        .expect("probability-weight estimate");
    let expected = probability_dimensions.observations as u128 * size_of::<f64>() as u128;
    assert!(
        probability_dimensions.retained_raw_preseparation_probability_weight_elements
            < probability_dimensions.candidate_observations,
        "missing/domain-excluded candidate rows must not create retained raw-weight storage",
    );
    assert_eq!(
        probability
            .fast
            .retained_raw_preseparation_probability_weight_bytes,
        expected,
    );
    assert_eq!(
        probability
            .low
            .retained_raw_preseparation_probability_weight_bytes,
        expected,
    );
    assert_eq!(
        probability.fast.iteration_peak_bytes - unweighted.fast.iteration_peak_bytes,
        expected,
    );

    let preliminary = plan_ppml_memory(probability_dimensions, PpmlMemoryConfig::default())
        .expect("generation-zero pweight plan");
    let mut final_dimensions = probability_dimensions;
    final_dimensions.observations /= 2;
    final_dimensions.separation_generation_capacity = final_dimensions.observations;
    let rebased = preliminary
        .with_final_dimensions(final_dimensions)
        .expect("smaller final pweight generation");
    assert_eq!(
        rebased
            .preflight_dimensions
            .retained_raw_preseparation_probability_weight_elements,
        probability_dimensions.observations,
    );
    assert_eq!(
        rebased
            .selected_inventory()
            .retained_raw_preseparation_probability_weight_bytes,
        expected,
        "the retained owner vector remains generation-zero-active-sized after separation",
    );

    final_dimensions.retained_raw_preseparation_probability_weight_elements = 0;
    assert_eq!(
        preliminary
            .with_final_dimensions(final_dimensions)
            .expect_err("final generation cannot erase retained owner storage")
            .code,
        ErrorCode::InvalidArgument,
    );
}

#[test]
fn embedding_resident_baseline_is_charged_to_every_phase_peak() {
    let base = estimate_ppml_memory(dimensions(), PpmlMemoryConfig::default())
        .expect("core-only estimate");
    let embedding_resident_bytes = 1_048_913_u128;
    let with_embedding = estimate_ppml_memory(
        dimensions(),
        PpmlMemoryConfig {
            embedding_resident_bytes,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("embedding-aware estimate");
    for (plain, embedded) in [
        (base.fast, with_embedding.fast),
        (base.low, with_embedding.low),
    ] {
        assert_eq!(embedded.embedding_resident_bytes, embedding_resident_bytes);
        assert_eq!(
            embedded.preparation_peak_bytes - plain.preparation_peak_bytes,
            embedding_resident_bytes,
        );
        assert_eq!(
            embedded.separation_peak_bytes - plain.separation_peak_bytes,
            embedding_resident_bytes,
        );
        assert_eq!(
            embedded.iteration_peak_bytes - plain.iteration_peak_bytes,
            embedding_resident_bytes,
        );
        assert_eq!(
            embedded.publication_peak_bytes - plain.publication_peak_bytes,
            embedding_resident_bytes,
        );
    }

    let plan = plan_ppml_memory(
        dimensions(),
        PpmlMemoryConfig {
            embedding_resident_bytes,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("embedding-aware plan");
    assert_eq!(plan.embedding_resident_bytes, embedding_resident_bytes);
    assert_eq!(
        plan.selected_inventory().embedding_resident_bytes,
        embedding_resident_bytes,
    );
}

#[test]
fn post_solve_phases_charge_exact_residual_and_record_transient() {
    let estimate =
        estimate_ppml_memory(dimensions(), PpmlMemoryConfig::default()).expect("memory estimate");
    let expected_transient = dimensions().observations as u128 * size_of::<f64>() as u128
        + FINITE_SOLVE_FIXED_RECORD_COUNT * FINITE_SOLVE_FIXED_RECORD_BYTES
        + OBJECTIVE_LINE_SEARCH_RECORD_BYTES;

    for inventory in [estimate.fast, estimate.low] {
        assert_eq!(inventory.post_solve_transient_bytes, expected_transient,);
        assert_eq!(
            inventory.post_solve_peak_bytes,
            inventory.iteration_peak_bytes + expected_transient,
        );
        assert!(inventory.peak_bytes >= inventory.post_solve_peak_bytes);
    }

    let plan = plan_ppml_memory(dimensions(), PpmlMemoryConfig::default()).expect("memory plan");
    let inventory = plan.selected_inventory();
    assert_eq!(
        plan.modeled_phase_peak_bytes(PpmlPhase::ObjectiveLineSearch),
        inventory.post_solve_peak_bytes,
    );
    assert_eq!(
        inventory.objective_line_search_peak_bytes, inventory.post_solve_peak_bytes,
        "normal production line search has no equivalence-audit lifetime",
    );
    assert_eq!(
        plan.modeled_phase_peak_bytes(PpmlPhase::WeightResponseRefresh),
        inventory.iteration_peak_bytes,
    );
    for phase in [PpmlPhase::FeProjection, PpmlPhase::FiniteSolve] {
        assert_eq!(
            plan.modeled_phase_peak_bytes(phase),
            inventory.strict_final_peak_bytes,
            "nested linear subphases inherit the strict canonical lifetime",
        );
    }
}

#[test]
fn allocation_audit_alone_charges_one_equivalence_refresh_vector() {
    let normal_dimensions = dimensions();
    let normal = estimate_ppml_memory(normal_dimensions, PpmlMemoryConfig::default())
        .expect("normal memory estimate");
    let mut audited_dimensions = normal_dimensions;
    audited_dimensions.allocation_audit_enabled = true;
    let audited = estimate_ppml_memory(audited_dimensions, PpmlMemoryConfig::default())
        .expect("allocation-audit memory estimate");
    let expected = normal_dimensions.observations as u128 * size_of::<f64>() as u128;

    for (plain, qualified) in [(normal.fast, audited.fast), (normal.low, audited.low)] {
        assert_eq!(plain.audit_equivalence_refresh_transient_bytes, 0);
        assert_eq!(
            plain.weight_response_refresh_peak_bytes,
            plain.iteration_peak_bytes
        );
        assert_eq!(
            qualified.audit_equivalence_refresh_transient_bytes,
            expected
        );
        assert_eq!(qualified.iteration_peak_bytes, plain.iteration_peak_bytes);
        assert_eq!(qualified.post_solve_peak_bytes, plain.post_solve_peak_bytes);
        assert_eq!(
            plain.objective_line_search_peak_bytes,
            plain.post_solve_peak_bytes
        );
        assert_eq!(
            qualified.objective_line_search_peak_bytes,
            plain.objective_line_search_peak_bytes + expected,
        );
        assert_eq!(
            qualified.strict_final_peak_bytes,
            plain.strict_final_peak_bytes + expected,
            "the same equivalence response remains live through nested strict recertification",
        );
        assert_eq!(
            qualified.weight_response_refresh_peak_bytes,
            qualified.iteration_peak_bytes + expected,
        );
    }

    let normal_plan = plan_ppml_memory(normal_dimensions, PpmlMemoryConfig::default())
        .expect("normal memory plan");
    let audited_plan = plan_ppml_memory(audited_dimensions, PpmlMemoryConfig::default())
        .expect("allocation-audit memory plan");
    assert_eq!(
        normal_plan.modeled_phase_peak_bytes(PpmlPhase::WeightResponseRefresh),
        normal_plan.selected_inventory().iteration_peak_bytes,
        "normal production fits retain the original steady refresh model",
    );
    assert_eq!(
        audited_plan.modeled_phase_peak_bytes(PpmlPhase::WeightResponseRefresh),
        audited_plan
            .selected_inventory()
            .weight_response_refresh_peak_bytes,
    );
    assert_eq!(
        audited_plan.modeled_phase_peak_bytes(PpmlPhase::FeProjection)
            - normal_plan.modeled_phase_peak_bytes(PpmlPhase::FeProjection),
        expected,
        "nested strict projection sees the same one-vector qualification lifetime",
    );
}

#[test]
fn strict_final_solve_charges_four_vectors_and_exact_candidate_phase_maximum() {
    let expected_phase_metadata = |dimensions: PpmlMemoryDimensions| {
        let k = dimensions.finite_columns as u128;
        let slots = dimensions.coefficient_slots as u128;
        let installed = k * 5 * size_of::<usize>() as u128
            + slots
                * (size_of::<PreparedOlsFittedCoefficient>()
                    + size_of::<PreparedOlsBreadCoordinate>()
                    + size_of::<FrozenLinearBasisSlot>()) as u128;
        let assembly = slots
            * (size_of::<PreparedOlsFittedCoefficient>() + size_of::<PreparedOlsBreadCoordinate>())
                as u128
            + k * (size_of::<Option<f64>>()
                + size_of::<PreparedOlsFitStatus>()
                + 2 * size_of::<f64>()
                + 5 * size_of::<usize>()) as u128;
        (
            installed,
            assembly,
            k * size_of::<f64>() as u128 + installed.max(assembly),
        )
    };
    let ordinary = dimensions();
    let mut slot_heavy = ordinary;
    slot_heavy.coefficient_slots *= 2;
    slot_heavy.reported_coefficient_slots *= 2;
    let (ordinary_installed, ordinary_assembly, _) = expected_phase_metadata(ordinary);
    let (heavy_installed, heavy_assembly, _) = expected_phase_metadata(slot_heavy);
    assert!(ordinary_assembly > ordinary_installed);
    assert!(heavy_installed > heavy_assembly);

    for memory_dimensions in [ordinary, slot_heavy] {
        let estimate = estimate_ppml_memory(memory_dimensions, PpmlMemoryConfig::default())
            .expect("memory estimate");
        let expected_vectors = memory_dimensions.observations as u128
            * STRICT_FINAL_OVERLAP_VECTOR_COUNT
            * size_of::<f64>() as u128;
        let (_, _, expected_metadata) = expected_phase_metadata(memory_dimensions);
        let expected_transient = expected_vectors + expected_metadata;
        for inventory in [estimate.fast, estimate.low] {
            assert_eq!(
                inventory.strict_final_metadata_transient_bytes,
                expected_metadata,
            );
            assert_eq!(inventory.strict_final_transient_bytes, expected_transient);
            assert_eq!(
                inventory.strict_final_peak_bytes,
                inventory.iteration_peak_bytes + expected_transient,
            );
            assert!(inventory.peak_bytes >= inventory.strict_final_peak_bytes);
        }
    }

    let plan = plan_ppml_memory(ordinary, PpmlMemoryConfig::default()).expect("memory plan");
    assert_eq!(
        plan.modeled_phase_peak_bytes(PpmlPhase::StrictFinalSolve),
        plan.selected_inventory().strict_final_peak_bytes,
    );
}

#[test]
fn fitted_start_mu_peak_retains_fit_and_detector_workspace() {
    let mut mu_dimensions = dimensions();
    mu_dimensions.separation_mu_enabled = true;
    mu_dimensions.separation_method_count = 5;
    let inventory = estimate_ppml_memory(mu_dimensions, PpmlMemoryConfig::default())
        .expect("memory estimate")
        .fast;
    let expected_retained =
        mu_dimensions.observations as u128 * size_of::<f64>() as u128 + MU_EXPLORATION_RECORD_BYTES;
    assert_eq!(inventory.mu_exploration_retained_bytes, expected_retained,);
    let common = inventory.embedding_resident_bytes
        + inventory.canonical_numeric_bytes
        + inventory.retained_base_adjustment_bytes
        + inventory.retained_raw_preseparation_probability_weight_bytes
        + inventory.topology_bytes
        + inventory.cluster_topology_bytes
        + inventory.row_ledger_bytes
        + inventory.row_axis_arc_header_bytes
        + inventory.prepared_model_metadata_bytes
        + inventory.prepared_generation_supplement_bytes
        + inventory.input_owner_supplement_bytes
        + inventory.separation_method_report_row_bytes
        + inventory.separation_method_report_level_bytes
        + inventory.separation_method_report_record_bytes
        + inventory.separation_generation_hash_bytes
        + inventory.diagnostic_bytes;
    let linear_validation = inventory.coefficient_buffer_bytes
        + inventory.retained_within_resident_bytes
        + inventory.retained_within_io_buffer_bytes
        + inventory.fe_solver_topology_bytes
        + inventory.linear_vector_bytes
        + inventory.fe_workspace_bytes
        + inventory.linear_workspace_bytes
        + inventory.linear_result_metadata_bytes;
    let detector_vectors = mu_dimensions.observations as u128
        * mu_dimensions.separation_workspace_vectors as u128
        * size_of::<f64>() as u128;
    let proposal = mu_dimensions.observations as u128
        * (size_of::<usize>() as u128 + size_of::<bool>() as u128);
    let certificates = 2_u128
        * mu_dimensions.finite_columns as u128
        * mu_dimensions.finite_columns as u128
        * size_of::<f64>() as u128;
    let mu_detection =
        common + linear_validation + detector_vectors + proposal + certificates + expected_retained;
    let mu_transaction = common
        + inventory.separation_rebase_bytes
        + inventory.separation_rebase_arc_header_bytes
        + expected_retained;
    let mu_cascade = common + inventory.separation_cascade_workspace_bytes + expected_retained;
    assert_eq!(
        inventory.separation_mu_peak_bytes,
        inventory
            .iteration_peak_bytes
            .max(inventory.separation_peak_bytes)
            .max(mu_detection)
            .max(mu_cascade)
            .max(mu_transaction),
    );
    assert_eq!(
        plan_ppml_memory(mu_dimensions, PpmlMemoryConfig::default())
            .expect("memory plan")
            .modeled_phase_peak_bytes(PpmlPhase::SeparationMu),
        inventory.separation_mu_peak_bytes,
    );
    assert!(inventory.separation_mu_peak_bytes >= inventory.separation_peak_bytes);
}

#[test]
fn row_axis_model_separates_steady_ledgers_from_atomic_rebase_overlap() {
    let mut row_dimensions = dimensions();
    row_dimensions.observations = 80_000;
    row_dimensions.separation_generation_capacity = 80_000;
    row_dimensions.candidate_observations = 90_000;
    row_dimensions.original_observations = 100_000;
    let inventory = estimate_ppml_memory(row_dimensions, PpmlMemoryConfig::default())
        .expect("row-axis estimate")
        .fast;
    let word = size_of::<usize>() as u128;
    assert_eq!(size_of::<DropReason>(), 1);
    assert_eq!(size_of::<Option<SeparationDropReason>>(), 1);
    assert_eq!(size_of::<bool>(), 1);
    let expected_activity = 100_000_u128;
    assert_eq!(
        inventory.steady_row_map_bytes,
        (90_000_u128 + 2 * 80_000_u128) * word,
        "the owner candidate, coordinator retained, and prepared-generation maps are distinct",
    );
    assert_eq!(inventory.prepared_drop_reason_bytes, 2 * 100_000_u128);
    assert_eq!(inventory.separation_activity_bytes, expected_activity);
    assert_eq!(inventory.separation_reason_bytes, 100_000_u128);
    assert_eq!(
        inventory.row_ledger_bytes,
        inventory.steady_row_map_bytes
            + inventory.prepared_drop_reason_bytes
            + inventory.separation_activity_bytes
            + inventory.separation_reason_bytes,
        "row-ledger aggregate is the sum of concrete map, enum, and bit-vector storage",
    );
    assert_eq!(
        inventory.row_axis_arc_header_bytes,
        3 * (2 * word + size_of::<Vec<usize>>() as u128),
        "candidate rows, current retained rows, and initial drop reasons own three Arc control blocks after rebase",
    );
    assert_eq!(inventory.final_drop_reason_bytes, 100_000_u128);
    assert_eq!(
        inventory.separation_report_row_bytes,
        2 * 80_000_u128 * word,
    );
    assert_eq!(inventory.separation_report_reason_bytes, 80_000_u128);
    assert_eq!(
        inventory.separation_method_report_row_bytes,
        80_000_u128 * word
    );
    let expected_replacement = inventory.canonical_numeric_bytes
        + inventory.topology_bytes
        + inventory.cluster_topology_bytes
        + inventory.prepared_model_metadata_bytes
        + 100_000_u128
        + 80_000_u128 * word;
    assert_eq!(
        inventory.separation_replacement_generation_bytes, expected_replacement,
        "the complete replacement generation overlaps the still-owned current generation",
    );
    assert_eq!(
        inventory.separation_rebase_proposal_bytes, 0,
        "the exact-capacity primary moves directly into the commit",
    );
    assert_eq!(
        inventory.separation_rebase_direction_bytes, 0,
        "the independently certified direction is released before cascade/rebuild",
    );
    let staged_ledgers =
        2 * 100_000_u128 + 80_000_u128 * word + inventory.separation_rebase_proposal_bytes;
    let staged_candidate = 100_000_u128;
    let completed_or_build = expected_replacement
        .max(expected_replacement - 80_000_u128 * size_of::<f64>() as u128 + staged_candidate);
    let expected_rebase =
        staged_ledgers + completed_or_build + inventory.separation_rebase_direction_bytes;
    assert_eq!(
        inventory.separation_rebase_bytes, expected_rebase,
        "atomic rebase owns staged ledgers and one full replacement beside the old generation",
    );
    assert_eq!(
        inventory.separation_rebase_staging_bytes,
        expected_rebase - expected_replacement,
    );
    assert_eq!(
        inventory.separation_rebase_arc_header_bytes,
        2 * word + size_of::<Vec<usize>>() as u128,
        "only the new retained-row Arc<Vec> owns a fresh control block",
    );
    let separation_base = inventory.embedding_resident_bytes
        + inventory.canonical_numeric_bytes
        + inventory.retained_base_adjustment_bytes
        + inventory.retained_raw_preseparation_probability_weight_bytes
        + inventory.topology_bytes
        + inventory.cluster_topology_bytes
        + inventory.row_ledger_bytes
        + inventory.row_axis_arc_header_bytes
        + inventory.prepared_model_metadata_bytes
        + inventory.prepared_generation_supplement_bytes
        + inventory.input_owner_supplement_bytes
        + inventory.separation_method_report_row_bytes
        + inventory.separation_method_report_level_bytes
        + inventory.separation_method_report_record_bytes
        + inventory.separation_generation_hash_bytes
        + inventory.diagnostic_bytes;
    let detector_peak = separation_base
        + inventory
            .separation_workspace_bytes
            .max(inventory.separation_singleton_workspace_bytes);
    let cascade_peak = separation_base + inventory.separation_cascade_workspace_bytes;
    let transaction_peak = separation_base
        + inventory.separation_rebase_bytes
        + inventory.separation_rebase_arc_header_bytes;
    assert_eq!(
        inventory.separation_peak_bytes,
        detector_peak.max(cascade_peak).max(transaction_peak),
        "detector, cascade, and accepted old-plus-new rebase are sequential phase alternatives",
    );

    let mut malformed = row_dimensions;
    malformed.original_observations = malformed.observations - 1;
    assert_eq!(
        estimate_ppml_memory(malformed, PpmlMemoryConfig::default())
            .expect_err("short original axis")
            .code,
        ErrorCode::InvalidArgument,
    );

    let mut short_candidate = row_dimensions;
    short_candidate.candidate_observations = short_candidate.observations - 1;
    assert_eq!(
        estimate_ppml_memory(short_candidate, PpmlMemoryConfig::default())
            .expect_err("candidate axis shorter than prepared sample")
            .code,
        ErrorCode::InvalidArgument,
    );

    let mut candidate_beyond_original = row_dimensions;
    candidate_beyond_original.candidate_observations =
        candidate_beyond_original.original_observations + 1;
    assert_eq!(
        estimate_ppml_memory(candidate_beyond_original, PpmlMemoryConfig::default())
            .expect_err("candidate axis longer than original sample")
            .code,
        ErrorCode::InvalidArgument,
    );

    let plan =
        plan_ppml_memory(row_dimensions, PpmlMemoryConfig::default()).expect("row-axis plan");
    let mut mismatched_final = row_dimensions;
    mismatched_final.observations -= 1;
    mismatched_final.original_observations -= 1;
    assert_eq!(
        plan.with_final_dimensions(mismatched_final)
            .expect_err("original axis must be stable")
            .code,
        ErrorCode::InvalidArgument,
    );

    let mut mismatched_candidate = row_dimensions;
    mismatched_candidate.observations -= 1;
    mismatched_candidate.candidate_observations -= 1;
    assert_eq!(
        plan.with_final_dimensions(mismatched_candidate)
            .expect_err("candidate axis must be stable after route selection")
            .code,
        ErrorCode::InvalidArgument,
    );
}

#[test]
fn probability_rebase_build_retains_only_source_positions_until_exact_restore() {
    let mut probability = dimensions();
    probability.observations = 80_000;
    probability.separation_generation_capacity = 80_000;
    probability.candidate_observations = 90_000;
    probability.original_observations = 100_000;
    probability.retained_raw_preseparation_probability_weight_elements = 80_000;
    probability.fe_max_level_members = probability
        .fe_max_level_members
        .min(probability.observations);
    let inventory = estimate_ppml_memory(probability, PpmlMemoryConfig::default())
        .expect("probability rebase estimate")
        .fast;
    let n = probability.observations as u128;
    let original = probability.original_observations as u128;
    let word = size_of::<usize>() as u128;
    let staged_ledgers = 2 * original + n * word + inventory.separation_rebase_proposal_bytes;
    let staged_candidate = original;
    let retained_positions = n * word;
    let staged_values = n * size_of::<f64>() as u128;
    let stage_only = staged_candidate + retained_positions + staged_values;
    let build = inventory.separation_replacement_generation_bytes - n * size_of::<f64>() as u128
        + staged_candidate
        + retained_positions;
    let expected = staged_ledgers
        + stage_only
            .max(build)
            .max(inventory.separation_replacement_generation_bytes)
        + inventory.separation_rebase_direction_bytes;
    assert_eq!(inventory.separation_rebase_bytes, expected);
    assert_eq!(
        inventory.separation_rebase_staging_bytes,
        expected - inventory.separation_replacement_generation_bytes,
    );
    assert!(build > inventory.separation_replacement_generation_bytes);
}

#[test]
fn preparation_staging_uses_exact_candidate_capacity() {
    let mut compact = dimensions();
    compact.observations = 40_000;
    compact.separation_generation_capacity = 40_000;
    compact.candidate_observations = 60_000;
    compact.retained_base_adjustment_capacity_elements = 60_000;
    compact.original_observations = 100_000;
    compact.fe_max_level_members = compact.fe_max_level_members.min(compact.observations);
    let compact_inventory = estimate_ppml_memory(compact, PpmlMemoryConfig::default())
        .expect("compact candidate estimate")
        .fast;

    let mut wider_candidate = compact;
    wider_candidate.candidate_observations = 90_000;
    wider_candidate.retained_base_adjustment_capacity_elements = 90_000;
    let wider_inventory = estimate_ppml_memory(wider_candidate, PpmlMemoryConfig::default())
        .expect("wider candidate estimate")
        .fast;
    let candidate_delta =
        (wider_candidate.candidate_observations - compact.candidate_observations) as u128;
    let word = size_of::<usize>() as u128;
    assert_eq!(
        wider_inventory.steady_row_map_bytes - compact_inventory.steady_row_map_bytes,
        candidate_delta * word,
    );
    assert_eq!(
        wider_inventory.separation_rebase_bytes - compact_inventory.separation_rebase_bytes,
        0,
        "atomic replacement generations are bounded by active/original axes, not if/in capacity",
    );
    assert_eq!(
        wider_inventory.retained_base_adjustment_bytes
            - compact_inventory.retained_base_adjustment_bytes,
        candidate_delta * size_of::<f64>() as u128,
        "the owner-side offset/logged exposure retains immutable candidate capacity",
    );

    let staging_columns = compact.loaded_numeric_columns as u128 + 3;
    let compact_staging =
        compact.candidate_observations as u128 * staging_columns * size_of::<f64>() as u128;
    let wider_staging =
        wider_candidate.candidate_observations as u128 * staging_columns * size_of::<f64>() as u128;
    let compact_common = compact_inventory.embedding_resident_bytes
        + compact_inventory.canonical_numeric_bytes
        + compact_inventory.retained_base_adjustment_bytes
        + compact_inventory.retained_raw_preseparation_probability_weight_bytes
        + compact_inventory.topology_bytes
        + compact_inventory.cluster_topology_bytes
        + compact_inventory.row_ledger_bytes
        + compact_inventory.row_axis_arc_header_bytes
        + compact_inventory.prepared_model_metadata_bytes
        + compact_inventory.separation_method_report_row_bytes
        + compact_inventory.separation_method_report_level_bytes
        + compact_inventory.diagnostic_bytes;
    let wider_common = wider_inventory.embedding_resident_bytes
        + wider_inventory.canonical_numeric_bytes
        + wider_inventory.retained_base_adjustment_bytes
        + wider_inventory.retained_raw_preseparation_probability_weight_bytes
        + wider_inventory.topology_bytes
        + wider_inventory.cluster_topology_bytes
        + wider_inventory.row_ledger_bytes
        + wider_inventory.row_axis_arc_header_bytes
        + wider_inventory.prepared_model_metadata_bytes
        + wider_inventory.separation_method_report_row_bytes
        + wider_inventory.separation_method_report_level_bytes
        + wider_inventory.diagnostic_bytes;
    // Preparation can be dominated by the preallocated iteration bank,
    // so assert the planner's maximum from its exact candidate-capacity
    // staging term rather than assuming which side wins.
    assert_eq!(
        wider_staging - compact_staging,
        candidate_delta * staging_columns * size_of::<f64>() as u128,
    );
    assert_eq!(
        compact_inventory.preparation_peak_bytes,
        (compact_common + compact_staging).max(compact_inventory.iteration_peak_bytes),
    );
    assert_eq!(
        wider_inventory.preparation_peak_bytes,
        (wider_common + wider_staging).max(wider_inventory.iteration_peak_bytes),
    );
}

#[test]
fn singleton_queue_and_level_counters_are_admitted_for_both_routes() {
    let mut dims = dimensions();
    dims.separation_singleton_enabled = true;
    let expected = dims.observations as u128 * (size_of::<usize>() + 2 * size_of::<bool>()) as u128
        + dims.fe_total_levels as u128 * size_of::<u128>() as u128
        + (dims.fe_dimensions as u128 + 1) * size_of::<usize>() as u128
        + dims.fe_dimensions as u128 * size_of::<(&[u32], &LevelIndex)>() as u128
        + dims.loaded_numeric_columns as u128 * size_of::<CategoricalCodeView<'_>>() as u128;
    let estimate = estimate_ppml_memory(dims, PpmlMemoryConfig::default()).unwrap();
    assert_eq!(estimate.fast.separation_singleton_workspace_bytes, expected);
    assert_eq!(estimate.low.separation_singleton_workspace_bytes, expected);
    dims.separation_singleton_enabled = false;
    let disabled = estimate_ppml_memory(dims, PpmlMemoryConfig::default()).unwrap();
    assert_eq!(disabled.fast.separation_singleton_workspace_bytes, 0);
    assert_eq!(disabled.low.separation_singleton_workspace_bytes, 0);
}

#[test]
fn prepared_and_linear_metadata_use_registered_concrete_type_widths() {
    assert_eq!(size_of::<PpmlCoefficientStatus>(), 1);
    assert_eq!(size_of::<CoefficientSlot>(), 4);
    assert_eq!(size_of::<PreparedOlsCoefficient>(), 24);
    assert_eq!(size_of::<PreparedOlsColumn>(), 48);
    assert_eq!(size_of::<PreparedOlsBreadCoordinate>(), 16);
    assert_eq!(size_of::<FrozenLinearBasisSlot>(), 24);
    assert_eq!(size_of::<PreparedOlsFitStatus>(), 1);
    assert_eq!(size_of::<PreparedOlsFittedCoefficient>(), 40);
    assert_eq!(size_of::<Option<f64>>(), 16);

    let mut metadata_dimensions = dimensions();
    metadata_dimensions.finite_columns = 1;
    metadata_dimensions.coefficient_slots = 1;
    metadata_dimensions.reported_coefficient_slots = 1;
    let inventory = estimate_ppml_memory(metadata_dimensions, PpmlMemoryConfig::default())
        .expect("metadata estimate")
        .fast;
    assert_eq!(
        inventory.prepared_model_metadata_bytes,
        1 + 24 + 4 + 48,
        "omission, coefficient, report-slot, and column metadata match the traced K=1 fit",
    );
    assert_eq!(
        inventory.linear_result_metadata_bytes,
        4 * 8 + 8 + 40 + 16 + 24 + 4 * 16,
        "rank indices, accepted fit, bread, basis, and bounded reported workspace match the traced K=1 fit",
    );
    assert_eq!(
        inventory.linear_vector_bytes,
        metadata_dimensions.observations as u128 * 7 * size_of::<f64>() as u128
            + 11 * size_of::<f64>() as u128
            + 4 * size_of::<usize>() as u128,
        "observation vectors, eleven retained rank vectors, and four reusable index banks are all resident",
    );
}

#[test]
fn finite_session_charges_inverse_lower_and_bounded_route_moment_storage() {
    let memory_dimensions = dimensions();
    let estimate = estimate_ppml_memory(memory_dimensions, PpmlMemoryConfig::default())
        .expect("finite-session estimate");
    let k = memory_dimensions.finite_columns as u128;
    let requested = (memory_dimensions.threads as u128).min(k).max(1);
    let tile_cells = (memory_dimensions.observations as u128 * k)
        .min(k.max(RETAINED_MOMENT_TILE_ELEMENTS as u128));
    let fast_expected = LINEAR_SQUARE_WORKSPACE_COUNT * k * k * size_of::<f64>() as u128;
    let low_expected =
        (LINEAR_SQUARE_WORKSPACE_COUNT * k * k + requested * tile_cells) * size_of::<f64>() as u128;
    assert_eq!(LINEAR_SQUARE_WORKSPACE_COUNT, 7);
    assert_eq!(estimate.fast.linear_workspace_bytes, fast_expected);
    assert_eq!(estimate.low.linear_workspace_bytes, low_expected);
    assert!(estimate.fast.iteration_peak_bytes >= estimate.fast.linear_workspace_bytes);
    assert!(estimate.low.strict_final_peak_bytes >= estimate.low.linear_workspace_bytes);

    let mut uneven = memory_dimensions;
    uneven.finite_columns = 5;
    uneven.coefficient_slots = 5;
    uneven.reported_coefficient_slots = 5;
    uneven.threads = 4;
    let uneven_low = estimate_ppml_memory(uneven, PpmlMemoryConfig::default())
        .expect("uneven retained partition estimate")
        .low;
    assert_eq!(
        uneven_low.linear_workspace_bytes,
        (LINEAR_SQUARE_WORKSPACE_COUNT * 25 + 4 * RETAINED_MOMENT_TILE_ELEMENTS as u128)
            * size_of::<f64>() as u128,
        "K=5 must also admit four rank-4 partitions over its retained stored width",
    );
}

#[test]
fn model_scales_session_vectors_slots_and_reusable_map_basis_separately() {
    let mut base_dimensions = dimensions();
    // This test isolates generic reusable-basis scaling. Keep its RHS one
    // beyond the PPML-only categorical accelerator admission boundary.
    base_dimensions.finite_columns = FUSED_PARALLEL_MAX_RHS;
    base_dimensions.coefficient_slots = FUSED_PARALLEL_MAX_RHS;
    base_dimensions.reported_coefficient_slots = FUSED_PARALLEL_MAX_RHS;
    let base = estimate_ppml_memory(base_dimensions, PpmlMemoryConfig::default())
        .expect("base estimate")
        .fast;

    let mut more_rows = base_dimensions;
    more_rows.observations += 1_000;
    more_rows.candidate_observations += 1_000;
    more_rows.original_observations += 1_000;
    let rows = estimate_ppml_memory(more_rows, PpmlMemoryConfig::default())
        .expect("row estimate")
        .fast;
    assert_eq!(
        rows.linear_vector_bytes - base.linear_vector_bytes,
        1_000_u128 * 7 * size_of::<f64>() as u128,
    );

    let mut more_slots = base_dimensions;
    more_slots.coefficient_slots += 1_000;
    more_slots.reported_coefficient_slots += 1_000;
    let slots = estimate_ppml_memory(more_slots, PpmlMemoryConfig::default())
        .expect("slot estimate")
        .fast;
    assert_eq!(
        slots.coefficient_buffer_bytes - base.coefficient_buffer_bytes,
        1_000_u128 * COEFFICIENT_VECTOR_COUNT * size_of::<f64>() as u128,
    );
    assert_eq!(
        slots.retained_within_resident_bytes, base.retained_within_resident_bytes,
        "reporting slots must never create an observation-by-slot design",
    );

    let mut larger_level = base_dimensions;
    larger_level.fe_max_channels = 2;
    larger_level.fe_total_channels = 3;
    larger_level.fe_coefficient_directions += larger_level.fe_max_levels;
    larger_level.fe_max_effect_directions *= 2;
    larger_level.fe_max_level_members += 1_000;
    let larger = estimate_ppml_memory(larger_level, PpmlMemoryConfig::default())
        .expect("largest-level estimate")
        .fast;
    assert!(larger.fe_workspace_bytes > 0);
    // A slope span releases categorical graph banks; the independent
    // workspace model need not grow across different mathematics.
    assert!(larger.fe_solver_topology_bytes > base.fe_solver_topology_bytes);
}

#[test]
fn auto_falls_back_to_low_and_explicit_fast_fails_before_workspace() {
    let base = PpmlMemoryConfig {
        requested: MemoryMode::Auto,
        require_fast_route: false,
        available_bytes: None,
        embedding_resident_bytes: 0,
        poolsize: 4,
        diagnostic_capacity: 8,
    };
    let estimate = estimate_ppml_memory(dimensions(), base).expect("estimate");
    let headroom = MINIMUM_HEADROOM_BYTES;
    let available = estimate.low.peak_bytes + headroom;
    assert!(available.saturating_sub(headroom) < estimate.fast.peak_bytes);
    let auto = plan_ppml_memory(
        dimensions(),
        PpmlMemoryConfig {
            available_bytes: Some(available),
            ..base
        },
    )
    .expect("low fallback");
    assert_eq!(auto.selected, PpmlMemoryRoute::Low);

    let error = plan_ppml_memory(
        dimensions(),
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            available_bytes: Some(available),
            ..base
        },
    )
    .expect_err("fast must fail preflight");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("before workspace allocation"));
}

#[test]
fn no_route_fit_fails_closed() {
    let error = plan_ppml_memory(
        dimensions(),
        PpmlMemoryConfig {
            available_bytes: Some(MINIMUM_HEADROOM_BYTES),
            ..PpmlMemoryConfig::default()
        },
    )
    .expect_err("no usable memory");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn detector_graph_admission_and_irls_workspaces_have_separate_lifetimes() {
    let mut plan = plan_ppml_memory(dimensions(), PpmlMemoryConfig::default()).unwrap();
    let base = plan.selected_bytes();
    plan.usable_bytes = Some(base + 4096);
    assert!(plan.admit_separation_projection(4096));
    assert_eq!(plan.selected_bytes(), base + 4096);
    let old = plan;
    assert!(!plan.admit_separation_projection(4097));
    assert!(!plan.admit_separation_projection(u128::MAX));
    assert_eq!(plan, old);
    plan = plan.with_solver_workspace(4096).unwrap();
    assert_eq!(plan.selected_bytes(), base + 4096);
    assert!(plan.with_solver_workspace(4097).is_err());
    assert_eq!(
        plan.modeled_phase_peak_bytes(PpmlPhase::SeparationIr),
        plan.selected_inventory().separation_peak_bytes + 4096
    );
    assert_eq!(
        plan.modeled_phase_peak_bytes(PpmlPhase::StrictFinalSolve),
        plan.selected_inventory().strict_final_peak_bytes + 4096
    );
}

#[test]
fn optional_reusable_certificate_accounts_for_projection_and_phase_lifetime() {
    let mut base = plan_ppml_memory(dimensions(), PpmlMemoryConfig::default())
        .unwrap()
        .with_solver_workspace(1024)
        .unwrap();
    assert!(base.admit_reusable_projection(2048));
    let mut plan = base;
    plan.usable_bytes = Some(base.selected_bytes() + 4095);
    let before = plan;
    assert!(!plan.admit_reusable_certificate(4096));
    assert_eq!(plan, before);
    plan.usable_bytes = Some(base.selected_bytes() + 4096);
    assert!(plan.admit_reusable_certificate(4096));
    assert_eq!(plan.selected_bytes(), base.selected_bytes() + 4096);
    assert!(plan.with_solver_workspace(1025).is_err());
    assert!(!plan.admit_reusable_projection(2049));
    assert!(!plan.admit_reusable_certificate(u128::MAX));
    for phase in [
        PpmlPhase::Preparation,
        PpmlPhase::WeightResponseRefresh,
        PpmlPhase::FeProjection,
        PpmlPhase::FiniteSolve,
        PpmlPhase::ObjectiveLineSearch,
        PpmlPhase::StrictFinalSolve,
    ] {
        assert_eq!(
            plan.modeled_phase_peak_bytes(phase),
            base.modeled_phase_peak_bytes(phase) + 4096
        );
    }
    for phase in [
        PpmlPhase::SeparationFe,
        PpmlPhase::SeparationMu,
        PpmlPhase::Inference,
        PpmlPhase::FeRecovery,
        PpmlPhase::Publication,
    ] {
        assert_eq!(
            plan.modeled_phase_peak_bytes(phase),
            base.modeled_phase_peak_bytes(phase)
        );
    }
    assert!(base.admit_reusable_certificate(4096));
    let mut telemetry =
        PpmlMemoryTelemetry::new(base, dimensions().threads, dimensions().threads).unwrap();
    telemetry.observe_linear_route(FeSolverKind::Map);
    telemetry.observe_scratch(0);
    telemetry.validate().unwrap();
    telemetry.plan.reusable_certificate_bytes += 1;
    // Re-admission verifies a real budget, rather than trusting prior telemetry.
    telemetry.plan.usable_bytes = Some(1);
    assert!(telemetry.validate().is_err());
}

#[test]
fn optional_reusable_slope_accounts_for_geometry_and_phase_lifetime() {
    let mut base = plan_ppml_memory(dimensions(), PpmlMemoryConfig::default())
        .unwrap()
        .with_solver_workspace(1024)
        .unwrap();
    assert!(base.admit_reusable_projection(1024));
    assert!(base.admit_reusable_certificate(1024));
    let mut plan = base;
    plan.usable_bytes = Some(base.selected_bytes() + 4095);
    let before = plan;
    assert!(!plan.admit_reusable_slope(4096));
    assert_eq!(plan, before);
    plan.usable_bytes = Some(base.selected_bytes() + 4096);
    assert!(plan.admit_reusable_slope(4096));
    assert_eq!(plan.selected_bytes(), base.selected_bytes() + 4096);
    // Refusing a certificate result bank preserves the admitted solver.
    let solver_only = plan;
    assert!(!plan.admit_reusable_slope(4097));
    assert_eq!(plan, solver_only);
    assert!(plan.with_solver_workspace(1025).is_err());
    assert!(!plan.admit_reusable_projection(1025));
    assert!(!plan.admit_reusable_slope(u128::MAX));
    for phase in [
        PpmlPhase::Preparation,
        PpmlPhase::WeightResponseRefresh,
        PpmlPhase::FeProjection,
        PpmlPhase::FiniteSolve,
        PpmlPhase::ObjectiveLineSearch,
        PpmlPhase::StrictFinalSolve,
    ] {
        assert_eq!(
            plan.modeled_phase_peak_bytes(phase),
            base.modeled_phase_peak_bytes(phase) + 4096
        );
    }
    for phase in [
        PpmlPhase::SeparationFe,
        PpmlPhase::SeparationMu,
        PpmlPhase::Inference,
        PpmlPhase::FeRecovery,
        PpmlPhase::Publication,
    ] {
        assert_eq!(
            plan.modeled_phase_peak_bytes(phase),
            base.modeled_phase_peak_bytes(phase)
        );
    }
    assert!(base.admit_reusable_slope(4096));
    let mut telemetry =
        PpmlMemoryTelemetry::new(base, dimensions().threads, dimensions().threads).unwrap();
    telemetry.observe_linear_route(FeSolverKind::Map);
    telemetry.observe_scratch(0);
    telemetry.validate().unwrap();
    telemetry.plan.reusable_slope_bytes += 1;
    // Re-admission verifies a real budget, rather than trusting prior telemetry.
    telemetry.plan.usable_bytes = Some(1);
    assert!(telemetry.validate().is_err());
}

#[test]
fn optional_reusable_projection_respects_exact_budget_and_phase_lifetime() {
    let base = plan_ppml_memory(dimensions(), PpmlMemoryConfig::default())
        .unwrap()
        .with_solver_workspace(1024)
        .unwrap();
    let mut plan = base;
    plan.usable_bytes = Some(base.selected_bytes() + 4095);
    let before = plan;
    assert!(!plan.admit_reusable_projection(4096));
    assert_eq!(plan, before);
    plan.usable_bytes = Some(base.selected_bytes() + 4096);
    assert!(plan.admit_reusable_projection(4096));
    assert_eq!(plan.selected_bytes(), base.selected_bytes() + 4096);
    assert!(plan.with_solver_workspace(1025).is_err());
    for phase in [
        PpmlPhase::Preparation,
        PpmlPhase::WeightResponseRefresh,
        PpmlPhase::FeProjection,
        PpmlPhase::FiniteSolve,
        PpmlPhase::ObjectiveLineSearch,
        PpmlPhase::StrictFinalSolve,
    ] {
        assert_eq!(
            plan.modeled_phase_peak_bytes(phase),
            base.modeled_phase_peak_bytes(phase) + 4096
        );
    }
    for phase in [
        PpmlPhase::SeparationFe,
        PpmlPhase::SeparationMu,
        PpmlPhase::Inference,
        PpmlPhase::FeRecovery,
        PpmlPhase::Publication,
    ] {
        assert_eq!(
            plan.modeled_phase_peak_bytes(phase),
            base.modeled_phase_peak_bytes(phase)
        );
    }
    let before = plan;
    assert!(!plan.admit_reusable_projection(u128::MAX));
    assert_eq!(plan, before);
    let mut unlimited = base;
    assert!(unlimited.admit_reusable_projection(4096));
    let mut telemetry =
        PpmlMemoryTelemetry::new(unlimited, dimensions().threads, dimensions().threads).unwrap();
    telemetry.observe_linear_route(FeSolverKind::Map);
    telemetry.observe_scratch(0);
    telemetry.validate().unwrap();
}

#[test]
fn public_solver_workspace_is_bound_to_fit_and_nested_phase_peaks() {
    let base =
        plan_ppml_memory(dimensions(), PpmlMemoryConfig::default()).expect("base PPML memory plan");
    let execution = base.selected_inventory().peak_bytes;
    let solver_workspace = 4_096_u128;
    let bound = base
        .with_solver_workspace(solver_workspace)
        .expect("solver workspace binding");
    assert_eq!(bound.selected_bytes(), execution + solver_workspace);
    for phase in [
        PpmlPhase::WeightResponseRefresh,
        PpmlPhase::FeProjection,
        PpmlPhase::FiniteSolve,
        PpmlPhase::ObjectiveLineSearch,
        PpmlPhase::StrictFinalSolve,
    ] {
        assert_eq!(
            bound.modeled_phase_peak_bytes(phase),
            base.modeled_phase_peak_bytes(phase) + solver_workspace,
        );
    }
    assert_eq!(
        bound.modeled_phase_peak_bytes(PpmlPhase::Inference),
        base.modeled_phase_peak_bytes(PpmlPhase::Inference),
        "the ordinary-frame solver is released before inference",
    );
}

#[test]
fn public_solver_workspace_fails_closed_one_byte_above_usable_memory() {
    let mut plan =
        plan_ppml_memory(dimensions(), PpmlMemoryConfig::default()).expect("base PPML memory plan");
    let solver_workspace = 4_096_u128;
    plan.usable_bytes = Some(
        plan.selected_inventory()
            .peak_bytes
            .checked_add(solver_workspace - 1)
            .expect("test memory boundary"),
    );
    let error = plan
        .with_solver_workspace(solver_workspace)
        .expect_err("solver workspace above the usable cap must fail");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn output_requests_increase_only_final_stage_peaks() {
    let empty =
        estimate_ppml_memory(dimensions(), PpmlMemoryConfig::default()).expect("empty outputs");
    let mut requested_dimensions = dimensions();
    requested_dimensions.requested_output_columns = 5;
    let requested = estimate_ppml_memory(requested_dimensions, PpmlMemoryConfig::default())
        .expect("requested outputs");
    assert_eq!(
        requested.fast.saved_output_bytes - empty.fast.saved_output_bytes,
        100_000_u128 * 5 * 8
    );
    assert_eq!(
        requested.fast.iteration_peak_bytes,
        empty.fast.iteration_peak_bytes
    );
    assert!(requested.fast.publication_peak_bytes > empty.fast.publication_peak_bytes);
}

#[test]
fn publication_reserves_descriptors_for_every_declared_channel() {
    let mut shape = dimensions();
    shape.requested_saved_fe_channels = 0;
    for saved in [0, 1, shape.fe_total_channels] {
        shape.requested_saved_fe_channels = saved;
        shape.requested_output_columns = saved;
        let estimate = estimate_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
        for inventory in [estimate.fast, estimate.low] {
            assert_eq!(
                inventory.publication_bytes,
                shape.publication_matrix_cells as u128 * size_of::<f64>() as u128
                    + shape.fe_total_channels as u128 * size_of::<&[f64]>() as u128
            );
        }
    }
}

#[test]
fn publication_phase_models_independent_psd_and_wald_validation_scratch() {
    let mut validator_dimensions = dimensions();
    validator_dimensions.observations = 1_000;
    validator_dimensions.original_observations = 1_000;
    validator_dimensions.candidate_observations = 1_000;
    validator_dimensions.separation_generation_capacity = 1_000;
    validator_dimensions.finite_columns = 17;
    validator_dimensions.coefficient_slots = 21;
    validator_dimensions.reported_coefficient_slots = 21;
    validator_dimensions.publication_matrix_cells = 0;
    validator_dimensions.requested_output_columns = 0;
    let estimate = estimate_ppml_memory(validator_dimensions, PpmlMemoryConfig::default())
        .expect("publication-validator estimate");
    let rank = validator_dimensions.finite_columns as u128;
    let expected_psd =
        (PUBLICATION_PSD_SQUARE_WORKSPACE_COUNT * rank * rank + rank) * size_of::<f64>() as u128;
    let expected_wald = (2 * rank * rank + 2 * rank) * size_of::<f64>() as u128
        + rank * size_of::<usize>() as u128
        + validator_dimensions.reported_coefficient_slots as u128;
    let expected = expected_psd
        .max(expected_wald)
        .max(validator_dimensions.original_observations as u128)
        .max(2 * validator_dimensions.candidate_observations as u128)
        .max(
            (validator_dimensions.reported_coefficient_slots
                + validator_dimensions.requested_saved_fe_channels) as u128,
        );
    assert_eq!(
        estimate.fast.publication_validator_workspace_bytes,
        expected
    );
    assert_eq!(estimate.low.publication_validator_workspace_bytes, expected);

    let fitted_common = |inventory: PpmlMemoryInventory| {
        inventory.canonical_numeric_bytes
            + inventory.result_metadata_bytes
            + inventory.retained_base_adjustment_bytes
            + inventory.retained_raw_preseparation_probability_weight_bytes
            + inventory.topology_bytes
            + inventory.cluster_topology_bytes
            + inventory.row_ledger_bytes
            + inventory.row_axis_arc_header_bytes
            + inventory.prepared_model_metadata_bytes
            + inventory.separation_method_report_row_bytes
            + inventory.separation_method_report_level_bytes
            + inventory.separation_method_report_record_bytes
            + inventory.separation_generation_hash_bytes
            + inventory.diagnostic_bytes
            + inventory.iteration_buffer_bytes
            + inventory.coefficient_buffer_bytes
            + inventory.retained_within_resident_bytes
            + inventory.retained_within_io_buffer_bytes
            + inventory.linear_result_metadata_bytes
    };
    assert_eq!(
        estimate.fast.publication_peak_bytes,
        fitted_common(estimate.fast)
            + estimate.fast.final_drop_reason_bytes
            + estimate.fast.separation_report_row_bytes
            + estimate.fast.separation_report_reason_bytes
            + estimate.fast.saved_output_bytes
            + estimate.fast.publication_bytes
            + estimate.fast.publication_result_bytes
            + estimate.fast.inference_result_bytes
            + expected,
    );
    assert_eq!(
        estimate.fast.publication_result_bytes,
        validator_dimensions.observations as u128
            * (PUBLICATION_RESULT_NUMERIC_VECTOR_COUNT * size_of::<f64>() as u128
                + PUBLICATION_RESULT_ROW_MAP_COUNT * size_of::<usize>() as u128)
            + (2 * size_of::<usize>() + size_of::<Vec<usize>>()) as u128,
    );
    let plan = plan_ppml_memory(
        validator_dimensions,
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("publication-validator plan");
    assert_eq!(
        plan.modeled_phase_peak_bytes(PpmlPhase::Publication),
        estimate.fast.publication_peak_bytes,
    );

    let mut slot_only_dimensions = validator_dimensions;
    slot_only_dimensions.finite_columns = 0;
    slot_only_dimensions.coefficient_slots = 0;
    slot_only_dimensions.reported_coefficient_slots = 10_000;
    let slot_only = estimate_ppml_memory(slot_only_dimensions, PpmlMemoryConfig::default())
        .expect("slot-state estimate");
    assert_eq!(
        slot_only.fast.publication_validator_workspace_bytes,
        (slot_only_dimensions.reported_coefficient_slots
            + slot_only_dimensions.requested_saved_fe_channels) as u128,
        "slot and saved-channel masks coexist even at rank zero",
    );
}

#[test]
fn publication_validation_uses_original_and_candidate_axes_after_drops() {
    let mut shape = dimensions();
    shape.observations = 8;
    shape.fe_max_level_members = 8;
    shape.separation_generation_capacity = 8;
    shape.original_observations = 1_000_000;
    shape.candidate_observations = 100_000;
    let original = estimate_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
    assert_eq!(
        original.fast.publication_validator_workspace_bytes,
        1_000_000
    );
    assert_eq!(
        original.low.publication_validator_workspace_bytes,
        1_000_000
    );
    shape.candidate_observations = 750_000;
    let replay = estimate_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
    assert_eq!(replay.fast.publication_validator_workspace_bytes, 1_500_000);
    assert_eq!(replay.low.publication_validator_workspace_bytes, 1_500_000);
    for (before, after) in [(original.fast, replay.fast), (original.low, replay.low)] {
        assert!(after.publication_peak_bytes >= before.publication_peak_bytes + 500_000);
    }
}

#[test]
fn reported_inference_axis_is_admitted_through_recovery_and_publication() {
    let mut shape = dimensions();
    shape.reported_coefficient_slots = 20_000;
    let estimate = estimate_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
    let covariance = 20_000_u128 * 20_000 * size_of::<f64>() as u128;
    for inventory in [estimate.fast, estimate.low] {
        assert!(inventory.inference_result_bytes > covariance);
        assert!(inventory.inference_peak_bytes >= inventory.inference_result_bytes);
        assert!(inventory.fe_recovery_peak_bytes >= inventory.inference_result_bytes);
        assert!(inventory.publication_peak_bytes >= inventory.inference_result_bytes);
    }
    // Use the headroom rule's large-budget branch. This observes the
    // public admission boundary rather than asserting only a formula.
    let available = estimate.fast.peak_bytes + (estimate.fast.peak_bytes - 1) / 4;
    let config = PpmlMemoryConfig {
        requested: MemoryMode::Fast,
        available_bytes: Some(available),
        ..PpmlMemoryConfig::default()
    };
    let admitted = plan_ppml_memory(shape, config).expect("full inference axis fits");
    let exact = available;
    assert_eq!(admitted.usable_bytes, Some(estimate.fast.peak_bytes));
    assert_eq!(
        plan_ppml_memory(
            shape,
            PpmlMemoryConfig {
                available_bytes: Some(exact),
                ..config
            }
        )
        .unwrap()
        .usable_bytes,
        Some(estimate.fast.peak_bytes),
    );
    assert_eq!(
        plan_ppml_memory(
            shape,
            PpmlMemoryConfig {
                available_bytes: Some(exact - 1),
                ..config
            }
        )
        .expect_err("one byte short must fail before covariance allocation")
        .code,
        ErrorCode::Allocation,
    );
}

#[test]
fn partial_fe_save_models_complete_recovery_and_no_save_pays_nothing() {
    let mut recovery_dimensions = dimensions();
    recovery_dimensions.observations = 1_000;
    recovery_dimensions.separation_generation_capacity = 1_000;
    recovery_dimensions.loaded_numeric_columns = 0;
    recovery_dimensions.finite_columns = 0;
    recovery_dimensions.coefficient_slots = 0;
    recovery_dimensions.reported_coefficient_slots = 0;
    recovery_dimensions.fe_dimensions = 1;
    recovery_dimensions.fe_total_levels = 1_000;
    recovery_dimensions.fe_max_levels = 1_000;
    recovery_dimensions.fe_total_channels = 100;
    recovery_dimensions.fe_max_channels = 100;
    recovery_dimensions.fe_max_level_members = 1;
    recovery_dimensions.fe_coefficient_directions = 100_000;
    recovery_dimensions.fe_max_effect_directions = 100_000;
    recovery_dimensions.cluster_dimensions = 0;
    recovery_dimensions.cluster_subset_groups = 0;
    recovery_dimensions.separation_workspace_vectors = 0;
    recovery_dimensions.simplex_tableau_cells = 0;
    recovery_dimensions.requested_output_columns = 1;
    recovery_dimensions.requested_saved_fe_channels = 0;
    recovery_dimensions.publication_matrix_cells = 0;
    recovery_dimensions.threads = 1;

    let no_save = estimate_ppml_memory(recovery_dimensions, PpmlMemoryConfig::default())
        .expect("no-save estimate");
    let mut partial_dimensions = recovery_dimensions;
    partial_dimensions.requested_saved_fe_channels = 1;
    let partial = estimate_ppml_memory(partial_dimensions, PpmlMemoryConfig::default())
        .expect("partial-save estimate");
    let complete_recovery_bytes = 1_000_u128 * (100 + 1) * size_of::<f64>() as u128;
    let required = partial.fast.fe_recovery_workspace_bytes;
    assert_eq!(no_save.fast.fe_recovery_workspace_bytes, 0);
    assert!(
        required >= complete_recovery_bytes + (1_000 + 100_000) * 8,
        "the residual and coefficient bank coexist with the complete output"
    );
    assert_eq!(
        partial.fast.fe_recovery_peak_bytes - no_save.fast.fe_recovery_peak_bytes,
        required,
        "one requested channel still recovers the total and all declared channels",
    );
    assert_eq!(
        partial.low.fe_recovery_peak_bytes - no_save.low.fe_recovery_peak_bytes,
        required,
    );
    let mut all_dimensions = partial_dimensions;
    all_dimensions.requested_saved_fe_channels = 100;
    all_dimensions.requested_output_columns = 100;
    let all = estimate_ppml_memory(all_dimensions, PpmlMemoryConfig::default()).unwrap();
    assert_eq!(
        all.fast.fe_recovery_workspace_bytes, required,
        "partial and full saves certify the same complete-channel decomposition"
    );
    let plan = plan_ppml_memory(
        partial_dimensions,
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("partial saved-FE recovery preflight");
    assert_eq!(
        plan.modeled_phase_peak_bytes(PpmlPhase::FeRecovery),
        partial.fast.fe_recovery_peak_bytes,
    );
    assert!(plan.selected_bytes() >= partial.fast.fe_recovery_peak_bytes);

    let mut impossible = partial_dimensions;
    impossible.requested_saved_fe_channels = 101;
    assert_eq!(
        estimate_ppml_memory(impossible, PpmlMemoryConfig::default())
            .expect_err("saved channels exceed declarations")
            .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn saved_intercept_slope_preflight_has_one_transient_total_and_no_public_duplicate() {
    let mut saved = dimensions();
    saved.observations = 1_000;
    saved.separation_generation_capacity = 1_000;
    saved.original_observations = 1_000;
    saved.candidate_observations = 1_000;
    saved.loaded_numeric_columns = 0;
    saved.finite_columns = 0;
    saved.coefficient_slots = 0;
    saved.reported_coefficient_slots = 0;
    saved.fe_dimensions = 1;
    saved.fe_total_levels = 100;
    saved.fe_max_levels = 100;
    saved.fe_total_channels = 2;
    saved.fe_max_channels = 2;
    saved.fe_max_level_members = 10;
    saved.fe_coefficient_directions = 200;
    saved.fe_max_effect_directions = 200;
    saved.cluster_dimensions = 0;
    saved.cluster_subset_groups = 0;
    saved.separation_workspace_vectors = 0;
    saved.simplex_tableau_cells = 0;
    // Saved intercept, saved slope, and requested mu. Mu reuses the one
    // mean retained for composite objective replay.
    saved.requested_output_columns = 3;
    saved.requested_mean_output = true;
    saved.requested_saved_fe_channels = 2;
    saved.publication_matrix_cells = 0;
    saved.threads = 1;

    let mut no_save = saved;
    no_save.requested_output_columns = 1;
    no_save.requested_saved_fe_channels = 0;
    let no_save_estimate =
        estimate_ppml_memory(no_save, PpmlMemoryConfig::default()).expect("mean-only estimate");
    let estimate = estimate_ppml_memory(saved, PpmlMemoryConfig::default())
        .expect("saved intercept/slope estimate");
    let vector = saved.observations as u128 * size_of::<f64>() as u128;
    assert_eq!(estimate.fast.saved_output_bytes, 2 * vector);
    assert_eq!(estimate.low.saved_output_bytes, 2 * vector);
    assert_eq!(
        estimate.fast.fe_recovery_peak_bytes - no_save_estimate.fast.fe_recovery_peak_bytes,
        estimate.fast.fe_recovery_workspace_bytes,
        "recovery retains numeric and coefficient work beside total + intercept + slope",
    );
    assert!(estimate.fast.fe_recovery_workspace_bytes > 4 * vector + 200 * 8);
    assert_eq!(
        estimate.fast.publication_peak_bytes - no_save_estimate.fast.publication_peak_bytes,
        2 * vector,
        "publication retains only the two requested channels, not a duplicate total",
    );

    let exact_available = estimate
        .fast
        .peak_bytes
        .checked_add(MINIMUM_HEADROOM_BYTES)
        .expect("preflight boundary");
    let accepted = plan_ppml_memory(
        saved,
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            available_bytes: Some(exact_available),
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("exact saved-output preflight boundary");
    assert_eq!(accepted.usable_bytes, Some(estimate.fast.peak_bytes));
    assert_eq!(
        plan_ppml_memory(
            saved,
            PpmlMemoryConfig {
                requested: MemoryMode::Fast,
                available_bytes: Some(exact_available - 1),
                ..PpmlMemoryConfig::default()
            },
        )
        .expect_err("one byte below the saved-output boundary must fail")
        .code,
        ErrorCode::Allocation,
    );
}

#[test]
fn reusable_workspace_has_stable_data_sized_capacities() {
    let mut small_dimensions = dimensions();
    small_dimensions.observations = 4;
    small_dimensions.separation_generation_capacity = 4;
    small_dimensions.fe_max_level_members = 4;
    small_dimensions.finite_columns = 2;
    small_dimensions.coefficient_slots = 2;
    small_dimensions.reported_coefficient_slots = 2;
    let plan = plan_ppml_memory(
        small_dimensions,
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            require_fast_route: false,
            available_bytes: None,
            embedding_resident_bytes: 0,
            poolsize: 2,
            diagnostic_capacity: 3,
        },
    )
    .expect("plan");
    let mut workspace = zero_start_workspace(plan, &[0.0; 4]).expect("workspace");
    copy_equal_len(
        &mut workspace.coefficients,
        &[0.0, 0.0],
        "PPML coefficients",
    )
    .expect("coefficients");
    let initial = workspace.capacity_report();
    for _ in 1..=50 {
        workspace
            .refresh_working_state(
                &[0.0, 1.0, 2.0, 3.0],
                &[1.0; 4],
                &[0.0; 4],
                PoissonBounds::default(),
            )
            .expect("working state");
        workspace
            .set_candidate_eta(&[0.1, 0.2, 0.3, 0.4])
            .expect("candidate eta");
        copy_equal_len(
            &mut workspace.candidate_coefficients,
            &[0.5, -0.25],
            "PPML candidate coefficients",
        )
        .expect("candidate coefficients");
        workspace
            .fill_line_search_trial(0.5, PoissonBounds::default())
            .expect("trial");
        workspace.accept_trial().expect("accept");
        assert_eq!(workspace.capacity_report(), initial);
    }
    workspace
        .prepare_final_hessian_response(&[0.0; 4])
        .expect("final-Hessian response reuses the working-response buffer");
    assert_eq!(workspace.capacity_report(), initial);
    assert_eq!(
        plan.selected_inventory().strict_final_peak_bytes,
        plan.selected_inventory().iteration_peak_bytes
            + plan.selected_inventory().strict_final_transient_bytes,
        "final-Hessian workspace remains reusable while canonical recertification overlaps four row vectors",
    );
}

#[test]
fn stable_line_search_accepts_in_place_without_capacity_growth() {
    let mut small_dimensions = dimensions();
    small_dimensions.observations = 4;
    small_dimensions.separation_generation_capacity = 4;
    small_dimensions.fe_max_level_members = 4;
    small_dimensions.finite_columns = 2;
    small_dimensions.coefficient_slots = 3;
    small_dimensions.reported_coefficient_slots = 3;
    let plan = plan_ppml_memory(
        small_dimensions,
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            require_fast_route: false,
            available_bytes: None,
            embedding_resident_bytes: 0,
            poolsize: 2,
            diagnostic_capacity: 0,
        },
    )
    .expect("plan");
    let outcome = [0.0, 1.0, 2.0, 3.0];
    let weights = [1.0; 4];
    let mut workspace = zero_start_workspace(plan, &[0.0; 4]).expect("workspace");
    workspace
        .refresh_working_state(&outcome, &weights, &[0.0; 4], PoissonBounds::default())
        .expect("working state");
    let objective =
        poisson_objective_from_eta(&outcome, workspace.eta(), workspace.mean(), &weights)
            .expect("objective");
    workspace
        .set_candidate_eta(&[-0.5, 0.0, 2.0_f64.ln(), 3.0_f64.ln()])
        .expect("candidate eta");
    copy_equal_len(
        &mut workspace.candidate_coefficients,
        &[0.25, -0.5, 1.0],
        "PPML candidate coefficients",
    )
    .expect("candidate coefficients");
    let capacities = workspace.capacity_report();
    let accepted = workspace
        .accept_candidate(&outcome, &weights, objective, PpmlConfig::default())
        .expect("accepted candidate");
    assert_eq!(accepted.step, 1.0);
    assert!(accepted.objective.deviance < objective.deviance);
    assert_eq!(workspace.coefficients(), &[0.25, -0.5, 1.0]);
    assert_eq!(workspace.capacity_report(), capacities);
}

#[test]
fn transformed_line_search_failure_preserves_accepted_state() {
    let mut shape = dimensions();
    shape.observations = 2;
    shape.separation_generation_capacity = 2;
    shape.fe_max_level_members = 2;
    shape.finite_columns = 1;
    shape.coefficient_slots = 1;
    shape.reported_coefficient_slots = 1;
    let plan = plan_ppml_memory(shape, PpmlMemoryConfig::default()).unwrap();
    let mut workspace = zero_start_workspace(plan, &[0.; 2]).unwrap();
    let bounds = PoissonBounds::default();
    workspace
        .refresh_working_state(&[1., 2.], &[1.; 2], &[0.; 2], bounds)
        .unwrap();
    workspace.set_candidate_eta(&[0.25, -0.5]).unwrap();
    copy_equal_len(
        &mut workspace.candidate_coefficients,
        &[0.75],
        "PPML candidate coefficients",
    )
    .unwrap();
    let before = (
        workspace.eta().to_vec(),
        workspace.mean().to_vec(),
        workspace.coefficients().to_vec(),
        workspace.capacity_report(),
    );
    for overflow in [false, true] {
        let result = workspace.fill_line_search_trial_with(0.5, bounds, |eta| {
            eta[0] = if overflow { 1000. } else { 0.1 };
            if overflow {
                Ok(())
            } else {
                Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "injected profile refusal",
                ))
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::NonConvergence);
        assert_eq!(workspace.eta(), before.0);
        assert_eq!(workspace.mean(), before.1);
        assert_eq!(workspace.coefficients(), before.2);
        assert_eq!(workspace.capacity_report(), before.3);
    }
}

#[test]
fn line_search_backtracks_a_finite_candidate_whose_mean_overflows() {
    let mut small_dimensions = dimensions();
    small_dimensions.observations = 2;
    small_dimensions.separation_generation_capacity = 2;
    small_dimensions.fe_max_level_members = 2;
    small_dimensions.finite_columns = 1;
    small_dimensions.coefficient_slots = 1;
    small_dimensions.reported_coefficient_slots = 1;
    let plan = plan_ppml_memory(
        small_dimensions,
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            require_fast_route: false,
            available_bytes: None,
            embedding_resident_bytes: 0,
            poolsize: 1,
            diagnostic_capacity: 0,
        },
    )
    .expect("plan");
    let target = 10.0_f64.exp();
    let outcome = [target, target];
    let weights = [1.0; 2];
    let mut workspace = zero_start_workspace(plan, &[0.0; 2]).expect("workspace");
    workspace
        .refresh_working_state(&outcome, &weights, &[0.0; 2], PoissonBounds::default())
        .expect("initial working state");
    let objective =
        poisson_objective_from_eta(&outcome, workspace.eta(), workspace.mean(), &weights)
            .expect("initial objective");
    workspace
        .set_candidate_eta(&[1_000.0; 2])
        .expect("finite extreme candidate");
    copy_equal_len(
        &mut workspace.candidate_coefficients,
        &[1_000.0],
        "PPML candidate coefficients",
    )
    .expect("finite extreme coefficient");

    let accepted = workspace
        .accept_candidate(&outcome, &weights, objective, PpmlConfig::default())
        .expect("a smaller representable improving step must be accepted");
    assert!(accepted.step < 1.0);
    assert!(accepted.step_halvings > 0);
    assert!(accepted.objective.deviance < objective.deviance);
    assert!(workspace
        .mean()
        .iter()
        .all(|mean| mean.is_finite() && *mean > 0.0));
}

#[test]
fn line_search_propagates_raw_domain_errors_instead_of_halving_them() {
    let mut small_dimensions = dimensions();
    small_dimensions.observations = 2;
    small_dimensions.separation_generation_capacity = 2;
    small_dimensions.fe_max_level_members = 2;
    small_dimensions.finite_columns = 1;
    small_dimensions.coefficient_slots = 1;
    small_dimensions.reported_coefficient_slots = 1;
    let plan = plan_ppml_memory(small_dimensions, PpmlMemoryConfig::default()).expect("plan");
    let mut workspace = zero_start_workspace(plan, &[0.0; 2]).expect("workspace");
    let valid_outcome = [1.0; 2];
    let weights = [1.0; 2];
    workspace
        .refresh_working_state(
            &valid_outcome,
            &weights,
            &[0.0; 2],
            PoissonBounds::default(),
        )
        .expect("initial working state");
    let objective =
        poisson_objective_from_eta(&valid_outcome, workspace.eta(), workspace.mean(), &weights)
            .expect("initial objective");
    workspace
        .set_candidate_eta(&[0.0; 2])
        .expect("candidate eta");
    copy_equal_len(
        &mut workspace.candidate_coefficients,
        &[0.0],
        "PPML candidate coefficients",
    )
    .expect("candidate coefficient");

    let error = workspace
        .accept_candidate(&[-1.0, 1.0], &weights, objective, PpmlConfig::default())
        .expect_err("invalid raw outcome must not be hidden as failed step-halving");
    assert_eq!(error.code, ErrorCode::InvalidData);
    assert_eq!(error.return_code(), 459);
}

#[test]
fn telemetry_never_infers_unobserved_allocator_or_timer_values() {
    let plan = plan_with_diagnostic_capacity(2);
    let workspace =
        zero_start_workspace(plan, &vec![0.0; plan.dimensions.observations]).expect("workspace");
    let mut telemetry = PpmlMemoryTelemetry::new(plan, 4, 4).expect("telemetry");
    assert_eq!(telemetry.timings.get(PpmlPhase::Preparation), None);
    assert_eq!(telemetry.allocations.peak_delta_bytes(), None);
    assert_eq!(telemetry.linear_route, None);
    assert_eq!(telemetry.scratch_peak_bytes, None);
    telemetry.record_phase(PpmlPhase::Preparation, Duration::ZERO);
    assert_eq!(
        telemetry.timings.get(PpmlPhase::Preparation),
        Some(Duration::ZERO)
    );
    telemetry.observe_allocator(PpmlAllocationTelemetry {
        start_bytes: Some(100),
        current_bytes: Some(100),
        peak_bytes: Some(150),
        previous_baseline_bytes: Some(100),
        allocation_events_start: Some(10),
        allocation_events_end: Some(12),
    });
    telemetry
        .certify_workspace(&workspace)
        .expect("capacity proof");
    telemetry.observe_linear_route(FeSolverKind::Map);
    telemetry.observe_scratch(0);
    telemetry
        .observe_diagnostic_records(2)
        .expect("bounded diagnostic observation");
    assert_eq!(telemetry.allocations.peak_delta_bytes(), Some(50));
    assert_eq!(telemetry.linear_route, Some(FeSolverKind::Map));
    assert_eq!(telemetry.scratch_peak_bytes, Some(0));
    assert_eq!(telemetry.diagnostic_records_retained, 2);
    assert!(telemetry.ppml_workspace_allocation_free);
    assert!(!telemetry.whole_iteration_allocation_free);
}

#[test]
fn certified_memory_telemetry_rejects_mutated_modeled_and_observed_claims() {
    let plan = plan_with_diagnostic_capacity(2);
    let workspace = PpmlIterationWorkspace::from_initial_state(
        plan,
        vec![0.0; plan.dimensions.observations],
        vec![0.0; plan.dimensions.coefficient_slots],
    )
    .expect("prepared-style workspace");
    let mut valid = PpmlMemoryTelemetry::new(plan, 4, 4).expect("telemetry");
    valid.observe_linear_route(FeSolverKind::Map);
    valid.observe_scratch(0);
    valid
        .observe_diagnostic_records(2)
        .expect("bounded diagnostics");
    valid
        .certify_workspace(&workspace)
        .expect("workspace certificate");
    let modeled_preparation = plan.modeled_phase_peak_bytes(PpmlPhase::Preparation);
    valid.observe_phase_peak(
        PpmlPhase::Preparation,
        u64::try_from(modeled_preparation).expect("small modeled peak"),
    );
    valid.validate().expect("valid telemetry");

    let mut mutated = valid.clone();
    mutated.plan.estimate.fast.peak_bytes += 1;
    assert_eq!(
        mutated.validate().expect_err("mutated plan").code,
        ErrorCode::InvalidArgument
    );

    let mut mutated = valid.clone();
    mutated.fallback_mask = 1_u32 << 31;
    mutated.fallback_count = 1;
    assert_eq!(
        mutated.validate().expect_err("unknown fallback").code,
        ErrorCode::InvalidArgument
    );

    let mut mutated = valid.clone();
    mutated.allocations.start_bytes = Some(100);
    assert_eq!(
        mutated.validate().expect_err("partial allocator").code,
        ErrorCode::InvalidArgument
    );

    let mut mutated = valid.clone();
    mutated.diagnostic_records_retained = 3;
    assert_eq!(
        mutated.validate().expect_err("unbounded diagnostics").code,
        ErrorCode::InvalidArgument
    );

    let mut mutated = valid.clone();
    mutated.linear_route = Some(FeSolverKind::Auto);
    assert_eq!(
        mutated.validate().expect_err("automatic route").code,
        ErrorCode::InvalidArgument
    );

    let mut mutated = valid.clone();
    mutated.scratch_peak_bytes = None;
    assert_eq!(
        mutated.validate().expect_err("missing scratch").code,
        ErrorCode::InvalidArgument
    );

    let mut mutated = valid.clone();
    mutated.ppml_workspace_allocation_free = false;
    assert_eq!(
        mutated.validate().expect_err("workspace claim").code,
        ErrorCode::InvalidArgument
    );

    let mut mutated = valid.clone();
    mutated.whole_iteration_allocation_free = true;
    assert_eq!(
        mutated.validate().expect_err("whole-iteration claim").code,
        ErrorCode::InvalidArgument
    );

    let mut mutated = valid;
    mutated.observed_phase_peak_bytes[PpmlPhase::Preparation as usize] =
        Some(u64::try_from(modeled_preparation + 1).expect("small modeled peak"));
    assert_eq!(
        mutated.validate().expect_err("under-modeled phase").code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn steady_state_certificate_requires_zero_events_and_all_capacity_proofs() {
    let capacity = PpmlWorkspaceCapacityReport {
        observation_capacity_sum: 800,
        coefficient_capacity_sum: 30,
        diagnostic_capacity: 128,
    };
    let linear_capacity = PpmlLinearWorkspaceCapacityReport {
        element_capacity_sum: 10_000,
        retained_resident_elements: 2_000,
        retained_scratch_bytes: 1_600_000,
        fe_topology_resident_bytes: 4_096,
    };
    let observation = PpmlSteadyStateAllocationObservation {
        measured_iterations: 16,
        allocator_events_start: 41,
        allocator_events_end: 41,
        allocator_current_start: 10_000,
        allocator_current_end: 10_000,
        allocator_peak_start: 10_000,
        allocator_peak_end: 10_000,
        workspace_start: capacity,
        workspace_end: capacity,
        linear_workspace_start: linear_capacity,
        linear_workspace_end: linear_capacity,
        linear_workspace_capacity_stable: true,
        scratch_bytes_start: 1_600_000,
        scratch_bytes_end: 1_600_000,
        linear_route: FeSolverKind::Map,
        linear_session_allocation_free: true,
        fresh_warm_equivalent: true,
        steady_iteration_median_nanoseconds: 250_000,
    };
    let passed = certify_ppml_steady_state_allocation(observation).expect("certificate");
    assert!(passed.passed);

    let allocated = certify_ppml_steady_state_allocation(PpmlSteadyStateAllocationObservation {
        allocator_events_end: 42,
        allocator_peak_end: 10_008,
        ..observation
    })
    .expect("failed certificate remains observational");
    assert!(!allocated.passed);
    assert_eq!(allocated.allocation_events, 1);
    assert_eq!(allocated.allocator_peak_growth_bytes, 8);

    let unqualified_route =
        certify_ppml_steady_state_allocation(PpmlSteadyStateAllocationObservation {
            linear_route: FeSolverKind::Lsmr,
            ..observation
        })
        .expect("explicit non-MAP route remains observational");
    assert!(!unqualified_route.route_allocation_qualified);
    assert!(!unqualified_route.passed);

    assert_eq!(
        certify_ppml_steady_state_allocation(PpmlSteadyStateAllocationObservation {
            measured_iterations: 15,
            ..observation
        })
        .expect_err("short audit window")
        .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn malformed_and_overflowing_dimensions_fail_before_estimation() {
    let mut malformed = dimensions();
    malformed.cluster_dimensions = 6;
    assert_eq!(
        estimate_ppml_memory(malformed, PpmlMemoryConfig::default())
            .expect_err("six-way clustering")
            .code,
        ErrorCode::InvalidArgument
    );

    let mut missing_fe_shape = dimensions();
    missing_fe_shape.fe_max_level_members = 0;
    assert_eq!(
        estimate_ppml_memory(missing_fe_shape, PpmlMemoryConfig::default())
            .expect_err("missing largest FE level")
            .code,
        ErrorCode::InvalidArgument
    );

    let mut overflowing = dimensions();
    overflowing.observations = usize::MAX;
    overflowing.separation_generation_capacity = usize::MAX;
    overflowing.original_observations = usize::MAX;
    overflowing.candidate_observations = usize::MAX;
    overflowing.loaded_numeric_columns = usize::MAX;
    assert_eq!(
        estimate_ppml_memory(overflowing, PpmlMemoryConfig::default())
            .expect_err("overflow")
            .code,
        ErrorCode::Allocation
    );
}

#[test]
fn rank_zero_candidate_keeps_the_possible_positive_rank_workspace_ceiling() {
    let mut rank_zero_candidate = dimensions();
    rank_zero_candidate.observations = 100;
    rank_zero_candidate.separation_generation_capacity = 100;
    rank_zero_candidate.original_observations = 100;
    rank_zero_candidate.candidate_observations = 100;
    rank_zero_candidate.finite_columns = 1;
    rank_zero_candidate.coefficient_slots = 1;
    rank_zero_candidate.reported_coefficient_slots = 1;
    rank_zero_candidate.cluster_dimensions = 1;
    rank_zero_candidate.cluster_subset_groups = 2;
    rank_zero_candidate.threads = 1;
    let estimate = estimate_ppml_memory(rank_zero_candidate, PpmlMemoryConfig::default())
        .expect("rank-zero cluster inventory");
    // Admission precedes rank discovery. One candidate column can retain
    // rank one, whose ordering lifetime owns four scalar matrix banks and
    // three row-order banks, plus scores, a subset report and one reduction.
    let rank_zero_ordering = 100_u128 * size_of::<usize>() as u128;
    let expected = 32
        + 3 * rank_zero_ordering
        + 16
        + size_of::<crate::sandwich::ClusterSubsetReduction>() as u128
        + 8;
    assert!(expected > rank_zero_ordering);
    assert_eq!(estimate.fast.inference_workspace_bytes, expected);
    assert_eq!(estimate.low.inference_workspace_bytes, expected);
}

#[test]
fn phase_names_and_wire_order_are_frozen() {
    assert_eq!(
        PPML_PHASES.map(PpmlPhase::name),
        [
            "preparation",
            "separation_fe",
            "separation_simplex",
            "separation_ir",
            "separation_mu",
            "weight_response_refresh",
            "fe_projection",
            "finite_solve",
            "objective_line_search",
            "strict_final_solve",
            "inference",
            "fe_recovery",
            "publication",
        ]
    );
}
