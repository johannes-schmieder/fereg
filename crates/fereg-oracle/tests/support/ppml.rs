#![allow(dead_code)]
//! Native prepared-PPML request construction from oracle designs. Metadata
//! only: no oracle computation reads these production types.
use super::inputs;
use fereg_core::ppml::{
    LoadedPpmlInput, PpmlConfig, PpmlMemoryConfig, PpmlSolverFamily, PpmlVceRequest,
    PreparedPpmlAdjustmentKind, PreparedPpmlOutputRequest, PreparedPpmlRequestV1,
};
use fereg_core::{
    LoadedNumericColumn, OlsClusterTermV1, OlsInputBinding, OlsInputRole, PreparedOlsFitConfig,
    SeparationConfig, SourceEntry, SourceId,
};
use fereg_oracle::designs::Design;

/// Native request for `design` with optional user-scale exposure.
pub fn request(
    design: &Design,
    exposure: Option<&[f64]>,
    keep_singletons: bool,
    separation: SeparationConfig,
) -> (PreparedPpmlRequestV1, LoadedPpmlInput) {
    let (mut ols, mut loaded) = inputs(design, keep_singletons);
    let mut clusters = Vec::new();
    let mut position = u32::try_from(ols.bindings.len() + 1).expect("binding position");
    for (dimension, codes) in design.clusters.iter().enumerate() {
        let id =
            SourceId::new(u32::try_from(ols.model.sources.len() + 1).expect("id")).expect("id");
        ols.model.sources.push(SourceEntry {
            id,
            canonical_name: format!("cluster{dimension}"),
        });
        ols.bindings.push(OlsInputBinding {
            stata_var_position: position,
            role: OlsInputRole::NumericSource(id),
        });
        position += 1;
        loaded
            .numeric_sources
            .push((id, column(codes.iter().map(|&c| c as f64).collect())));
        clusters.push(OlsClusterTermV1 {
            label: format!("cluster{dimension}"),
            sources: vec![id],
        });
    }
    let request = PreparedPpmlRequestV1 {
        bias: None,
        ols,
        vce: if clusters.is_empty() {
            PpmlVceRequest::Robust
        } else {
            PpmlVceRequest::Clustered
        },
        clusters,
        ppml: PpmlConfig::default(),
        linear: PreparedOlsFitConfig::default(),
        memory: PpmlMemoryConfig::default(),
        memory_preflight_probe: None,
        allocation_snapshot_probe: None,
        phase_allocation_probe: None,
        linear_phase_allocation_probe: None,
        action_allocation_snapshot_probe: None,
        action_allocation_start: None,
        action_allocation_previous_baseline_bytes: None,
        separation,
        output: PreparedPpmlOutputRequest {
            residual: None,
            xb: false,
            absorbed: false,
            eta: false,
            mean: true,
            requested_observation_columns: 1,
            publication_matrix_cells: 4_096,
        },
        solver: PpmlSolverFamily::Auto,
        inject_fe_recovery_allocation_failure: false,
        inject_topology_sharing: false,
        inject_separation_cg: false,
    };
    let loaded = LoadedPpmlInput {
        bias_units: None,
        ols: loaded,
        adjustment: exposure.map(|values| column(values.to_vec())),
        adjustment_kind: if exposure.is_some() {
            PreparedPpmlAdjustmentKind::Exposure
        } else {
            PreparedPpmlAdjustmentKind::None
        },
    };
    (request, loaded)
}

fn column(values: Vec<f64>) -> LoadedNumericColumn {
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    }
}
