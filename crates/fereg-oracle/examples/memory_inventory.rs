//! Scalar-only characterization for parent/candidate inventory comparisons.
//! Run with `cargo run -p fereg-oracle --example memory_inventory --profile ci`.
//! Complete fields and errors are printed; no fit workspace is allocated.
#![forbid(unsafe_code)]

use fereg_core::options::MemoryMode;
use fereg_core::ppml::{
    estimate_ppml_memory, plan_ppml_memory, PpmlMemoryConfig, PpmlMemoryDimensions,
};
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
fn emit(id: usize, shape: PpmlMemoryDimensions) {
    let config = PpmlMemoryConfig {
        poolsize: 4,
        ..PpmlMemoryConfig::default()
    };
    println!("{id}|inventory|{:?}", estimate_ppml_memory(shape, config));
    for mode in [MemoryMode::Fast, MemoryMode::Low, MemoryMode::Auto] {
        for budget in [None, Some(1), Some(768 * 1024 * 1024)] {
            let plan = plan_ppml_memory(
                shape,
                PpmlMemoryConfig {
                    requested: mode,
                    available_bytes: budget,
                    ..config
                },
            );
            println!(
                "{id}|{mode:?}|{budget:?}|{:?}",
                plan.map(|p| (p.selected, p.selected_bytes(), p.usable_bytes))
            );
        }
    }
}

fn main() {
    let mut id = 0;
    for n in [1, 17, 16383, 65536, 100000] {
        for k in [0, 1, 5, 16, 65] {
            for fe in [0, 1, 2, 3] {
                for threads in [1, 4, 16] {
                    let mut d = dimensions();
                    d.observations = n;
                    d.candidate_observations = n + 7;
                    d.original_observations = n + 11;
                    d.finite_columns = k;
                    d.coefficient_slots = k + 2;
                    d.reported_coefficient_slots = k + 4;
                    d.fe_dimensions = fe;
                    d.fe_total_levels = fe * n.min(40);
                    d.fe_max_levels = if fe == 0 { 0 } else { n.min(40) };
                    d.fe_total_channels = fe * if id % 3 == 0 { 2 } else { 1 };
                    d.fe_max_channels = if fe == 0 { 0 } else { d.fe_total_channels / fe };
                    d.fe_coefficient_directions = d.fe_total_levels * d.fe_max_channels;
                    d.fe_max_effect_directions = d.fe_max_levels * d.fe_max_channels;
                    d.fe_max_level_members = if fe == 0 { 0 } else { n };
                    d.threads = threads;
                    d.separation_generation_capacity = n;
                    d.separation_rebase_enabled = id % 2 == 0;
                    d.separation_mu_enabled = id % 4 == 0;
                    d.separation_exact_fe_enabled = id % 5 != 0;
                    d.separation_singleton_enabled = id % 3 != 0;
                    d.separation_workspace_vectors = if id % 2 == 0 { 8 } else { 0 };
                    d.simplex_tableau_cells = if id % 3 == 0 { n * (k + 1) } else { 0 };
                    d.retained_base_adjustment_capacity_elements = if id % 2 == 0 {
                        d.candidate_observations
                    } else {
                        0
                    };
                    d.retained_raw_preseparation_probability_weight_elements =
                        if id % 3 == 0 { n + 1 } else { 0 };
                    d.requested_mean_output = id % 2 == 0;
                    d.requested_saved_fe_channels =
                        if id % 3 == 0 { d.fe_total_channels } else { 0 };
                    d.requested_output_columns =
                        d.requested_saved_fe_channels + usize::from(d.requested_mean_output) + 2;
                    d.allocation_audit_enabled = id % 7 == 0;
                    d.metadata.bias_workspace_bytes = (id % 3) as u128 * 65536;
                    emit(id, d);
                    id += 1;
                }
            }
        }
    }
    for edge in 0..12 {
        emit(id, edge_shape(edge));
        id += 1;
    }
}

fn edge_shape(edge: usize) -> PpmlMemoryDimensions {
    let mut d = dimensions();
    match edge {
        0 => {
            d.loaded_numeric_columns = usize::MAX;
            d.observations = usize::MAX;
            d.candidate_observations = usize::MAX;
            d.original_observations = usize::MAX;
        }
        1 => {
            d.finite_columns = usize::MAX;
            d.coefficient_slots = usize::MAX;
            d.reported_coefficient_slots = usize::MAX;
        }
        2 => d.metadata.bias_workspace_bytes = u128::MAX,
        3 => d.metadata.input_owner_bytes = u128::MAX,
        4 => d.metadata.prepared_nested_bytes = u128::MAX,
        5 => d.metadata.preparation_transient_bytes = u128::MAX,
        6 => d.metadata.result_bytes = u128::MAX,
        7 => d.metadata.generation_fixed_bytes = u128::MAX,
        8 => d.metadata.generation_bytes_per_row = u128::MAX,
        9 => d.observations = 0,
        10 => d.threads = 0,
        11 => {
            d.finite_columns = usize::MAX;
            d.coefficient_slots = usize::MAX;
            d.reported_coefficient_slots = usize::MAX;
            d.metadata.result_bytes = u128::MAX;
        }
        _ => unreachable!(),
    }
    d
}
