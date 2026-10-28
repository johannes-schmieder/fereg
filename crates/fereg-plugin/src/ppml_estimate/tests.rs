#[test]
#[cfg(feature = "ffi")]
fn action_ledger_keeps_ppml_embedding_and_all_generation_stages() {
    use fereg_core::ppml::{plan_ppml_memory, PpmlMemoryDimensions};
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

    let _guard = crate::instrumentation::lock_action_observation();
    for requested in [MemoryMode::Fast, MemoryMode::Low] {
        let mut plan = plan_ppml_memory(
            dimensions(),
            PpmlMemoryConfig {
                requested,
                available_bytes: None,
                embedding_resident_bytes: 123456,
                poolsize: 4,
                diagnostic_capacity: 128,
                require_fast_route: false,
            },
        )
        .unwrap();
        plan.solver_workspace_bytes = 765432;
        let before = crate::ffi::allocation_usage();
        let ledger = ppml_action_memory_ledger(plan, None, None).unwrap();
        assert_eq!(crate::ffi::allocation_usage().events, before.events);
        let summary = ledger.summary();
        assert_eq!(summary.missing_stages, 0);
        assert!(
            summary.accounted_peak >= plan.modeled_phase_peak_bytes(PpmlPhase::StrictFinalSolve)
        );
        assert!(summary.accounted_peak >= plan.modeled_phase_peak_bytes(PpmlPhase::Publication));
        assert_eq!(summary.complete_peak, None);
        assert_eq!(summary.ratio(u64::MAX), None);
    }
}

use super::*;
use crate::model_metadata::{CLUSTER_COMPONENT_ROW_WIDTH, CLUSTER_TERM_ROW_WIDTH};
use fereg_core::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledModelSpecification, CompiledTermGraph,
    ComponentKind, ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind,
    SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use fereg_core::protocol::{ADO_VERSION, MODEL_WIRE_VERSION};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
#[cfg(feature = "ffi")]
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn embedding_snapshot_excludes_exact_hint_capacity_or_discards_it() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CURRENT: AtomicU64 = AtomicU64::new(0);
    fn probe() -> fereg_core::ppml::PpmlAllocatorSnapshot {
        fereg_core::ppml::PpmlAllocatorSnapshot {
            current_bytes: CURRENT.load(Ordering::Relaxed),
            ..Default::default()
        }
    }
    let make = || {
        PreparedOlsLoadHint::prepare(
            [(
                SourceId::new(1).unwrap(),
                &[2., 1., 2.][..],
                &[false; 3][..],
            )]
            .into_iter(),
            3,
            u128::MAX,
        )
        .unwrap()
    };
    let mut hint = Some(make());
    let capacity = u64::try_from(hint.as_ref().unwrap().owned_bytes()).unwrap();
    CURRENT.store(1_024 + capacity, Ordering::Relaxed);
    assert_eq!(embedding_without_load_hint(probe, 24, &mut hint), 1_000);
    assert!(hint.is_some());
    CURRENT.store(24 + capacity - 1, Ordering::Relaxed);
    assert_eq!(
        embedding_without_load_hint(probe, 24, &mut hint),
        capacity - 1
    );
    assert!(hint.is_none());
    assert_eq!(embedding_without_load_hint(probe, u64::MAX, &mut hint), 0);
}

#[cfg(feature = "ffi")]
static EARLY_PREFLIGHT_CURRENT_BYTES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "ffi")]
static EARLY_PREFLIGHT_PEAK_BYTES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "ffi")]
static EARLY_PREFLIGHT_EVENTS: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "ffi")]
fn record_early_preflight_allocator(
    event: fereg_core::ppml::PpmlMemoryPreflightEvent,
) -> Result<()> {
    if matches!(
        event,
        fereg_core::ppml::PpmlMemoryPreflightEvent::Decision(_)
    ) {
        let snapshot = crate::ffi::allocation_usage();
        EARLY_PREFLIGHT_CURRENT_BYTES.store(snapshot.current_bytes, Ordering::Relaxed);
        EARLY_PREFLIGHT_PEAK_BYTES.store(snapshot.peak_bytes, Ordering::Relaxed);
        EARLY_PREFLIGHT_EVENTS.store(snapshot.events, Ordering::Relaxed);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
enum StoreEvent {
    Numeric(u32, usize, f64),
    Matrix(String, usize, usize, f64),
}

struct FakeSpi {
    local_bundle: Option<Vec<u8>>,
    macro_reads: usize,
    nobs: usize,
    nvars: usize,
    numeric: BTreeMap<(u32, usize), (f64, bool)>,
    matrices: BTreeMap<String, Vec<Vec<f64>>>,
    reads: usize,
    numeric_rows_enabled: bool,
    row_reads: usize,
    first_numeric_width: usize,
    stores: Vec<StoreEvent>,
    record_stores: bool,
    store_calls: usize,
    fail_store_at: Option<usize>,
    interrupt_store_at: Option<usize>,
    interrupt_at: Option<usize>,
    interrupt_checks: usize,
    interrupt_after_display: bool,
    interrupt_when_marker_exists: Option<std::path::PathBuf>,
    displays: Vec<String>,
    string_variables: BTreeSet<u32>,
}

impl FakeSpi {
    fn receipt_complete(&self) -> f64 {
        self.matrices["RREC"][0][PPML_RESULT_RECEIPT_COMPLETE_COLUMN - 1]
    }

    fn output(&self, variable: u32, observation: usize) -> Option<f64> {
        self.numeric
            .get(&(variable, observation))
            .map(|(value, _)| *value)
    }
}

impl SpiApi for FakeSpi {
    fn supports_numeric_rows(&self) -> bool {
        self.numeric_rows_enabled
    }

    fn read_numeric_rows(
        &mut self,
        variables: &[u32],
        first: usize,
        output: &mut [f64],
    ) -> Result<f64> {
        if self.row_reads == 0 {
            self.first_numeric_width = variables.len();
        }
        self.row_reads += 1;
        for (row, values) in output.chunks_exact_mut(variables.len()).enumerate() {
            for (value, variable) in values.iter_mut().zip(variables) {
                let (number, missing) = self.read_numeric(*variable, first + row)?;
                *value = if missing { 1e100 } else { number };
            }
        }
        Ok(1e100)
    }

    fn read_estimate_bundle(&mut self, buffer: &mut [u8]) -> Result<()> {
        self.macro_reads += 1;
        let value = self
            .local_bundle
            .as_ref()
            .ok_or_else(|| invalid("fake local request macro is unavailable"))?;
        let copied = value.len().min(buffer.len() - 1);
        buffer[..copied].copy_from_slice(&value[..copied]);
        buffer[copied] = 0;
        Ok(())
    }

    fn nobs(&mut self) -> Result<usize> {
        Ok(self.nobs)
    }

    fn nvars(&mut self) -> Result<usize> {
        Ok(self.nvars)
    }

    fn variable_is_string(&mut self, variable: u32) -> Result<bool> {
        Ok(self.string_variables.contains(&variable))
    }

    fn variable_is_strl(&mut self, _variable: u32) -> Result<bool> {
        Ok(false)
    }

    fn read_numeric(&mut self, variable: u32, observation: usize) -> Result<(f64, bool)> {
        self.reads += 1;
        self.numeric
            .get(&(variable, observation))
            .copied()
            .ok_or_else(|| invalid("fake PPML numeric input is missing"))
    }

    fn read_string(&mut self, _variable: u32, _observation: usize) -> Result<Vec<u8>> {
        Err(unsupported("fake PPML fixture has no string variables"))
    }

    fn matrix_dimensions(&mut self, name: &str) -> Result<(usize, usize)> {
        let matrix = self
            .matrices
            .get(name)
            .ok_or_else(|| invalid("fake PPML matrix is missing"))?;
        Ok((matrix.len(), matrix.first().map(Vec::len).unwrap_or(0)))
    }

    fn matrix_element(&mut self, name: &str, row: usize, column: usize) -> Result<f64> {
        self.matrices
            .get(name)
            .and_then(|matrix| matrix.get(row - 1))
            .and_then(|row| row.get(column - 1))
            .copied()
            .ok_or_else(|| invalid("fake PPML matrix coordinate is missing"))
    }

    fn store_numeric(&mut self, variable: u32, observation: usize, value: f64) -> Result<()> {
        self.store_calls += 1;
        if self.interrupt_store_at == Some(self.store_calls) {
            return Err(FeregError::new(
                ErrorCode::Interrupted,
                "injected publication interruption",
            ));
        }
        if self.fail_store_at == Some(self.store_calls) {
            return Err(internal("injected PPML numeric-store failure"));
        }
        self.numeric.insert((variable, observation), (value, false));
        if self.record_stores {
            self.stores
                .push(StoreEvent::Numeric(variable, observation, value));
        }
        Ok(())
    }

    fn store_matrix(
        &mut self,
        name: &PreparedMatrixStoreName,
        row: usize,
        column: usize,
        value: f64,
    ) -> Result<()> {
        self.store_calls += 1;
        if self.interrupt_store_at == Some(self.store_calls) {
            return Err(FeregError::new(
                ErrorCode::Interrupted,
                "injected publication interruption",
            ));
        }
        if self.fail_store_at == Some(self.store_calls) {
            return Err(internal("injected PPML matrix-store failure"));
        }
        self.matrices
            .get_mut(name.as_str())
            .and_then(|matrix| matrix.get_mut(row - 1))
            .and_then(|row| row.get_mut(column - 1))
            .map(|slot| *slot = value)
            .ok_or_else(|| invalid("fake PPML matrix store is out of range"))?;
        if self.record_stores {
            self.stores.push(StoreEvent::Matrix(
                name.as_str().to_owned(),
                row,
                column,
                value,
            ));
        }
        Ok(())
    }

    fn display(&mut self, message: &str) -> Result<()> {
        if let Some(path) = &self.interrupt_when_marker_exists {
            assert!(
                !path.exists(),
                "the controller must not interrupt SPI display"
            );
        }
        self.displays.push(message.to_owned());
        if self.interrupt_after_display {
            self.interrupt_at = Some(self.interrupt_checks.saturating_add(1));
        }
        Ok(())
    }

    fn check_interrupted(&mut self) -> Result<()> {
        self.interrupt_checks += 1;
        if self.interrupt_at == Some(self.interrupt_checks)
            || self
                .interrupt_when_marker_exists
                .as_ref()
                .is_some_and(|path| path.exists())
        {
            Err(FeregError::new(
                ErrorCode::Interrupted,
                "injected hidden PPML interruption",
            ))
        } else {
            Ok(())
        }
    }
}

#[test]
fn memory_baseline_and_matrix_are_validated_before_any_store() {
    for replacement in [
        "stata_alloc_before=-1",
        "stata_used_before=9007199254740993",
        "output_memory_matrix=RB",
    ] {
        let (mut args, mut spi) = fixture();
        let key = replacement.split_once('=').unwrap().0;
        *args
            .iter_mut()
            .find(|arg| arg.starts_with(&format!("{key}=")))
            .unwrap() = replacement.to_owned();
        assert!(estimate_ppml(&args, &mut spi).is_err(), "{replacement}");
        assert_eq!(spi.store_calls, 0);
        assert_eq!(spi.receipt_complete(), 0.0);
    }
}

#[test]
fn partial_memory_report_is_published_without_claiming_combined_coverage() {
    let (args, mut spi) = fixture();
    estimate_ppml(&args, &mut spi).unwrap();
    let report = &spi.matrices["RMEMORY"];
    assert_eq!(report[0], vec![1.0, 2.0]);
    assert_eq!(report[1], vec![1.0, 1.0]);
    assert_eq!(report[3], vec![0.0, 0.0]);
    assert_eq!(report[7], vec![0.0, 0.0]);
    assert_eq!(report[8], vec![0.0, 0.0]);
    assert!(report[10][1] > 0.0);
    assert_eq!(spi.receipt_complete(), 1.0);
}

fn fixture() -> (Vec<String>, FakeSpi) {
    let source = SourceId::new(1).expect("source");
    let graph = CompiledTermGraph {
        sources: vec![SourceEntry {
            id: source,
            canonical_name: "x".to_owned(),
        }],
        terms: vec![ExpandedTerm {
            id: TermId::new(1).expect("term"),
            order: TermOrder::new(1).expect("order"),
            coefficient_slot: CoefficientSlot::new(1).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "c.x".to_owned(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source,
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }],
    };
    let wire = ModelWireV2::from_graph(&graph).expect("wire");
    let mut matrices = BTreeMap::from([
        ("S".to_owned(), wire.sources.rows),
        ("T".to_owned(), wire.terms.rows),
        ("C".to_owned(), wire.components.rows),
        ("A".to_owned(), wire.absorb_terms.rows),
        ("AC".to_owned(), wire.absorb_components.rows),
        ("CT".to_owned(), zero_matrix(1, CLUSTER_TERM_ROW_WIDTH)),
        ("CC".to_owned(), zero_matrix(1, CLUSTER_COMPONENT_ROW_WIDTH)),
        (
            "BIND".to_owned(),
            vec![
                vec![1.0, 1.0, 0.0],
                vec![2.0, 2.0, 0.0],
                vec![3.0, 3.0, 1.0],
            ],
        ),
        ("RB".to_owned(), zero_matrix(1, 2)),
        ("RV".to_owned(), zero_matrix(2, 2)),
        (
            "RSTATUS".to_owned(),
            zero_matrix(2, COEFFICIENT_STATUS_COLUMNS),
        ),
        ("RFIT".to_owned(), zero_matrix(FIT_ROWS, 1)),
        ("RDOF".to_owned(), zero_matrix(DOF_ROWS, 1)),
        ("RFE".to_owned(), zero_matrix(1, FE_COLUMNS)),
        ("RCLUSTER".to_owned(), zero_matrix(1, CLUSTER_COLUMNS)),
        ("RSUBSET".to_owned(), zero_matrix(1, SUBSET_COLUMNS)),
        (
            "RSEP".to_owned(),
            zero_matrix(
                separation_method_capacity(SeparationMethods::default()),
                SEPARATION_COLUMNS,
            ),
        ),
        ("RINF".to_owned(), zero_matrix(INFERENCE_ROWS, 1)),
        (
            "RDROP".to_owned(),
            zero_matrix(DROP_COUNT_ROWS, DROP_COUNT_COLUMNS),
        ),
        (
            "RSEQ".to_owned(),
            zero_matrix(SEPARATION_SEQUENCE_ROWS, SEPARATION_SEQUENCE_COLUMNS),
        ),
        ("RCERT".to_owned(), zero_matrix(CERTIFICATE_ROWS, 1)),
        (
            "RSOLVER".to_owned(),
            zero_matrix(SOLVER_ROWS, SOLVER_COLUMNS),
        ),
        ("RMEMORY".to_owned(), zero_matrix(MEMORY_ROWS, 2)),
        ("RRUNTIME".to_owned(), zero_matrix(RUNTIME_ROWS, 1)),
        (
            "RREC".to_owned(),
            zero_matrix(1, PPML_RESULT_RECEIPT_COLUMNS),
        ),
    ]);
    let mut numeric = BTreeMap::new();
    for row in 0..10 {
        let x = row as f64 / 4.0;
        let outcome = (0.2 + 0.15 * x).exp();
        let observation = row + 1;
        numeric.insert((1, observation), (1.0, false));
        numeric.insert((2, observation), (outcome, false));
        numeric.insert((3, observation), (x, false));
        for variable in 4..=10 {
            numeric.insert((variable, observation), (-999.0, false));
        }
    }
    let args = vec![
        format!("ado={ADO_VERSION}"),
        format!("protocol_major={}", ACTION_PROTOCOL_VERSION.major),
        format!("protocol_minor={}", ACTION_PROTOCOL_VERSION.minor),
        "stata=18.0".to_owned(),
        format!("confirm={CONFIRMATION}"),
        format!("plugin={PLUGIN_VERSION}"),
        format!("request_major={}", PPML_REQUEST_WIRE_VERSION.major),
        format!("request_minor={}", PPML_REQUEST_WIRE_VERSION.minor),
        format!("result_major={}", PPML_RESULT_WIRE_VERSION.major),
        format!("result_minor={}", PPML_RESULT_WIRE_VERSION.minor),
        format!("model_major={}", MODEL_WIRE_VERSION.major),
        format!("model_minor={}", MODEL_WIRE_VERSION.minor),
        "source_matrix=S".to_owned(),
        "source_count=1".to_owned(),
        "term_matrix=T".to_owned(),
        "term_count=1".to_owned(),
        "component_matrix=C".to_owned(),
        "component_count=1".to_owned(),
        format!("source_names={}", registry(&wire.source_names)),
        format!("canonical_tokens={}", registry(&wire.canonical_tokens)),
        format!(
            "raw_component_operators={}",
            registry(&wire.raw_component_operators),
        ),
        format!(
            "raw_time_series_operators={}",
            registry(&wire.raw_time_series_operators),
        ),
        "absorb_term_matrix=A".to_owned(),
        "absorb_term_count=0".to_owned(),
        "absorb_component_matrix=AC".to_owned(),
        "absorb_component_count=0".to_owned(),
        "absorb_labels=.".to_owned(),
        "absorb_saved_names=.".to_owned(),
        "cluster_term_matrix=CT".to_owned(),
        "cluster_term_count=0".to_owned(),
        "cluster_component_matrix=CC".to_owned(),
        "cluster_component_count=0".to_owned(),
        "cluster_labels=.".to_owned(),
        "binding_matrix=BIND".to_owned(),
        "binding_count=3".to_owned(),
        "include_intercept=1".to_owned(),
        "candidate_all=1".to_owned(),
        "input_complete=1".to_owned(),
        "keep_singletons=0".to_owned(),
        "adjustment=none".to_owned(),
        "adjustment_position=0".to_owned(),
        "residual=response".to_owned(),
        "vce=robust".to_owned(),
        "memory=auto".to_owned(),
        "poolsize=10".to_owned(),
        "threads=0".to_owned(),
        "deterministic=0".to_owned(),
        "solver=auto".to_owned(),
        "tolerance=1e-8".to_owned(),
        "iterate=10000".to_owned(),
        "separation=default".to_owned(),
        "separation_tolerance=1e-7".to_owned(),
        "separation_max_iterations=100".to_owned(),
        "simplex_max_iterations=10000".to_owned(),
        "simplex_max_tableau_cells=8000000".to_owned(),
        "output_sample=4".to_owned(),
        "output_residual=5".to_owned(),
        "output_xb=9".to_owned(),
        "output_absorbed=10".to_owned(),
        "output_eta=6".to_owned(),
        "output_mean=7".to_owned(),
        "output_drop_reason=8".to_owned(),
        "output_saved_count=0".to_owned(),
        "output_saved_channels=.".to_owned(),
        "output_b_matrix=RB".to_owned(),
        "output_v_matrix=RV".to_owned(),
        "output_status_matrix=RSTATUS".to_owned(),
        "output_fit_matrix=RFIT".to_owned(),
        "output_dof_matrix=RDOF".to_owned(),
        "output_fe_matrix=RFE".to_owned(),
        "output_cluster_matrix=RCLUSTER".to_owned(),
        "output_subset_matrix=RSUBSET".to_owned(),
        "output_separation_matrix=RSEP".to_owned(),
        "output_inference_matrix=RINF".to_owned(),
        "output_drop_counts_matrix=RDROP".to_owned(),
        "output_separation_sequence_matrix=RSEQ".to_owned(),
        "output_certificate_matrix=RCERT".to_owned(),
        "output_solver_matrix=RSOLVER".to_owned(),
        "output_runtime_matrix=RRUNTIME".to_owned(),
        "output_memory_matrix=RMEMORY".to_owned(),
        "stata_used_entry=.".to_owned(),
        "stata_alloc_entry=.".to_owned(),
        "stata_used_before=.".to_owned(),
        "stata_alloc_before=.".to_owned(),
        "output_receipt_matrix=RREC".to_owned(),
    ];
    (
        args,
        FakeSpi {
            local_bundle: None,
            macro_reads: 0,
            nobs: 10,
            nvars: 10,
            numeric,
            matrices: std::mem::take(&mut matrices),
            reads: 0,
            numeric_rows_enabled: false,
            row_reads: 0,
            first_numeric_width: 0,
            // Keep the post-snapshot RUNTIME/COMPLETE tail allocation-free
            // in the mock just as it is through the production SPI bridge.
            stores: Vec::with_capacity(1_024),
            record_stores: true,
            store_calls: 0,
            fail_store_at: None,
            interrupt_store_at: None,
            interrupt_at: None,
            interrupt_checks: 0,
            interrupt_after_display: false,
            interrupt_when_marker_exists: None,
            displays: Vec::new(),
            string_variables: BTreeSet::new(),
        },
    )
}

#[test]
fn string_numeric_inputs_fail_before_any_read_or_store() {
    // Positions 1-3 bind the candidate marker, the outcome and continuous x.
    for position in [1, 2, 3] {
        let (args, mut spi) = fixture();
        spi.string_variables.insert(position);
        let error = estimate_ppml(&args, &mut spi).expect_err("string numeric input");
        assert_eq!(error.code, ErrorCode::Unsupported, "position {position}");
        assert_eq!(spi.reads, 0);
        assert_eq!(spi.store_calls, 0);
    }
}

fn categorical_absorb_cluster_weight_fixture() -> (Vec<String>, FakeSpi) {
    let (mut args, mut spi) = fixture();
    let x = SourceId::new(1).expect("x source");
    let h = SourceId::new(2).expect("h source");
    let g = SourceId::new(3).expect("g source");
    let cluster_only = SourceId::new(4).expect("cluster-only source");
    let specification = CompiledModelSpecification {
        model: CompiledTermGraph {
            sources: vec![
                SourceEntry {
                    id: x,
                    canonical_name: "x".to_owned(),
                },
                SourceEntry {
                    id: h,
                    canonical_name: "h".to_owned(),
                },
                SourceEntry {
                    id: g,
                    canonical_name: "g".to_owned(),
                },
                SourceEntry {
                    id: cluster_only,
                    canonical_name: "cluster_only".to_owned(),
                },
            ],
            terms: vec![
                continuous_test_term(1, x),
                categorical_test_term(2, h, 0.0, true),
                categorical_test_term(3, h, 1.0, false),
            ],
        },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("absorb term"),
            intercept: true,
            categorical_sources: vec![g],
            slope_sources: Vec::new(),
            label: "g".to_owned(),
            saved_name: "fe_g".to_owned(),
        }],
    };
    let wire = ModelWireV2::from_specification(&specification).expect("categorical PPML wire");
    let absorb_component_count = wire.absorb_components.rows.len();
    for (name, matrix) in [
        ("S", wire.sources.rows),
        ("T", wire.terms.rows),
        ("C", wire.components.rows),
        ("A", wire.absorb_terms.rows),
        ("AC", wire.absorb_components.rows),
    ] {
        spi.matrices.insert(name.to_owned(), matrix);
    }
    spi.matrices.insert("CT".to_owned(), vec![vec![1.0, 1.0]]);
    spi.matrices
        .insert("CC".to_owned(), vec![vec![1.0, 1.0, 4.0]]);
    spi.matrices.insert(
        "BIND".to_owned(),
        vec![
            vec![1.0, 1.0, 0.0],
            vec![2.0, 2.0, 0.0],
            vec![3.0, 3.0, 1.0],
            vec![4.0, 3.0, 2.0],
            vec![5.0, 3.0, 3.0],
            vec![6.0, 5.0, 0.0],
            vec![15.0, 3.0, 4.0],
        ],
    );
    spi.matrices.insert("RB".to_owned(), zero_matrix(1, 4));
    spi.matrices.insert("RV".to_owned(), zero_matrix(4, 4));
    spi.matrices.insert(
        "RSTATUS".to_owned(),
        zero_matrix(4, COEFFICIENT_STATUS_COLUMNS),
    );
    spi.matrices
        .insert("RFE".to_owned(), zero_matrix(1, FE_COLUMNS));
    spi.matrices
        .insert("RCLUSTER".to_owned(), zero_matrix(1, CLUSTER_COLUMNS));
    spi.matrices
        .insert("RSUBSET".to_owned(), zero_matrix(1, SUBSET_COLUMNS));

    spi.nobs = 32;
    spi.nvars = 15;
    spi.numeric.clear();
    for row in 0..32 {
        let observation = row + 1;
        let group = row / 4;
        let within = row % 4;
        let x_value = [-1.5_f64, -0.5, 0.5, 1.5][within];
        let h_value = [0.0_f64, 1.0, 0.0, 1.0][within];
        let group_effect = (group as f64 - 3.5) * 0.04;
        let outcome = (0.3 + group_effect + 0.12 * x_value + 0.2 * h_value).exp();
        let frequency_weight = if row % 3 == 0 { 2.0 } else { 1.0 };
        for (variable, value) in [
            (1, 1.0),
            (2, outcome),
            (3, x_value),
            (4, h_value),
            (5, group as f64 + 1.0),
            (6, frequency_weight),
            (15, group as f64 + 1.0),
        ] {
            spi.numeric.insert((variable, observation), (value, false));
        }
        for variable in 7..=14 {
            spi.numeric.insert((variable, observation), (-999.0, false));
        }
    }

    for (key, replacement) in [
        ("source_count", "source_count=4".to_owned()),
        (
            "source_names",
            format!("source_names={}", registry(&wire.source_names)),
        ),
        ("term_count", "term_count=3".to_owned()),
        (
            "canonical_tokens",
            format!("canonical_tokens={}", registry(&wire.canonical_tokens)),
        ),
        ("component_count", "component_count=3".to_owned()),
        (
            "raw_component_operators",
            format!(
                "raw_component_operators={}",
                registry(&wire.raw_component_operators)
            ),
        ),
        (
            "raw_time_series_operators",
            format!(
                "raw_time_series_operators={}",
                registry(&wire.raw_time_series_operators)
            ),
        ),
        ("absorb_term_count", "absorb_term_count=1".to_owned()),
        (
            "absorb_component_count",
            format!("absorb_component_count={absorb_component_count}"),
        ),
        (
            "absorb_labels",
            format!("absorb_labels={}", registry(&wire.absorb_labels)),
        ),
        (
            "absorb_saved_names",
            format!("absorb_saved_names={}", registry(&wire.absorb_saved_names)),
        ),
        ("cluster_term_count", "cluster_term_count=1".to_owned()),
        (
            "cluster_component_count",
            "cluster_component_count=1".to_owned(),
        ),
        ("cluster_labels", "cluster_labels=1:g".to_owned()),
        ("binding_count", "binding_count=7".to_owned()),
        ("vce", "vce=cluster".to_owned()),
        ("output_sample", "output_sample=7".to_owned()),
        ("output_residual", "output_residual=8".to_owned()),
        ("output_eta", "output_eta=9".to_owned()),
        ("output_mean", "output_mean=10".to_owned()),
        ("output_drop_reason", "output_drop_reason=11".to_owned()),
        ("output_xb", "output_xb=12".to_owned()),
        ("output_absorbed", "output_absorbed=13".to_owned()),
        ("output_saved_count", "output_saved_count=1".to_owned()),
        (
            "output_saved_channels",
            "output_saved_channels=2:14".to_owned(),
        ),
    ] {
        replace_argument(&mut args, key, replacement);
    }
    (args, spi)
}

fn bias_fixture(saved: bool) -> (Vec<String>, FakeSpi) {
    let (mut args, mut spi) = categorical_absorb_cluster_weight_fixture();
    let x = SourceId::new(1).unwrap();
    let h = SourceId::new(2).unwrap();
    let g = SourceId::new(3).unwrap();
    let cluster = SourceId::new(4).unwrap();
    let wire = ModelWireV2::from_specification(&CompiledModelSpecification {
        model: CompiledTermGraph {
            sources: [(x, "x"), (h, "h"), (g, "g"), (cluster, "cluster_only")]
                .into_iter()
                .map(|(id, name)| SourceEntry {
                    id,
                    canonical_name: name.to_owned(),
                })
                .collect(),
            terms: vec![continuous_test_term(1, x)],
        },
        absorbed_terms: [(g, "g", "fe_g"), (h, "h", "")]
            .into_iter()
            .enumerate()
            .map(|(j, (source, label, saved_name))| AbsorbedTerm {
                id: AbsorbTermId::new(j as u32 + 1).unwrap(),
                intercept: true,
                categorical_sources: vec![source],
                slope_sources: Vec::new(),
                label: label.to_owned(),
                saved_name: saved_name.to_owned(),
            })
            .collect(),
    })
    .unwrap();
    for (key, value) in [
        ("term_count", "term_count=1".to_owned()),
        ("component_count", "component_count=1".to_owned()),
        ("absorb_term_count", "absorb_term_count=2".to_owned()),
        (
            "absorb_component_count",
            format!(
                "absorb_component_count={}",
                wire.absorb_components.rows.len()
            ),
        ),
        (
            "canonical_tokens",
            format!("canonical_tokens={}", registry(&wire.canonical_tokens)),
        ),
        (
            "raw_component_operators",
            format!(
                "raw_component_operators={}",
                registry(&wire.raw_component_operators)
            ),
        ),
        (
            "raw_time_series_operators",
            format!(
                "raw_time_series_operators={}",
                registry(&wire.raw_time_series_operators)
            ),
        ),
        (
            "absorb_labels",
            format!("absorb_labels={}", registry(&wire.absorb_labels)),
        ),
        (
            "absorb_saved_names",
            format!("absorb_saved_names={}", registry(&wire.absorb_saved_names)),
        ),
        ("output_saved_count", "output_saved_count=2".to_owned()),
        (
            "output_saved_channels",
            "output_saved_channels=2:14|1:0".to_owned(),
        ),
    ] {
        replace_argument(&mut args, key, value);
    }
    for (name, values) in [
        ("T", wire.terms.rows),
        ("C", wire.components.rows),
        ("A", wire.absorb_terms.rows),
        ("AC", wire.absorb_components.rows),
    ] {
        spi.matrices.insert(name.to_owned(), values);
    }
    for (name, rows, cols) in [
        ("RB", 1, 2),
        ("RV", 2, 2),
        ("RSTATUS", 2, COEFFICIENT_STATUS_COLUMNS),
        ("RFE", 2, FE_COLUMNS),
        ("RBIAS", bias::CHANNELS, 2),
        ("RBDIAG", bias::DIAGNOSTICS, 1),
        ("RBSUPPORT", 2, bias::SUPPORT_COLUMNS),
    ] {
        spi.matrices
            .insert(name.to_owned(), zero_matrix(rows, cols));
    }
    spi.nvars = if saved { 34 } else { 16 };
    for row in 1..=spi.nobs {
        spi.numeric
            .insert((16, row), (((row - 1) / 2 + 1) as f64, false));
        spi.numeric
            .insert((2, row), (1. + ((row * 7 + row / 4) % 9) as f64, false));
        if saved {
            for position in 17..=34 {
                spi.numeric.insert((position, row), (-999., false));
            }
        }
    }
    args.extend(
        [
            "bias_method=exact",
            "bias_unit=16",
            "bias_probes=0",
            "bias_seed=0",
            "bias_stream=0",
            "output_bias_matrix=RBIAS",
            "output_bias_diagnostics=RBDIAG",
            "output_bias_support=RBSUPPORT",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    args.push(format!("output_bias_count={}", if saved { 18 } else { 0 }));
    args.push(format!(
        "output_bias_channels={}",
        if saved {
            (17..=34)
                .map(|p| format!("2:{p}"))
                .collect::<Vec<_>>()
                .join("|")
        } else {
            ".".to_owned()
        }
    ));
    (args, spi)
}

#[test]
fn bias_publication_and_every_store_failure_are_atomic() {
    let (args, mut spi) = bias_fixture(true);
    estimate_ppml(&args, &mut spi).unwrap();
    assert_eq!(spi.matrices["RBDIAG"][0][0], 1.);
    assert_eq!(spi.matrices["RBDIAG"][1][0], 16.);
    assert_eq!(spi.matrices["RBSUPPORT"][0][1], 2.);
    assert_eq!(
        spi.matrices["RREC"][0][PPML_RESULT_RECEIPT_COMPLETE_COLUMN - 1],
        1.
    );
    assert!(spi.matrices["RBIAS"][1][0].abs() > 1e-7);
    let (args, mut missing) = bias_fixture(true);
    missing.numeric.insert((16, 2), (0., true));
    let error = estimate_ppml(&args, &mut missing).unwrap_err();
    assert!(error.message.contains("missing on a retained"));
    assert_eq!(missing.store_calls, 0);
    for ordinal in 1..=spi.store_calls {
        let (args, mut failed) = bias_fixture(true);
        failed.fail_store_at = Some(ordinal);
        assert!(
            estimate_ppml(&args, &mut failed).is_err(),
            "store {ordinal}"
        );
        assert_eq!(failed.receipt_complete(), 0., "store {ordinal}");
    }
}

#[test]
fn bias_support_failure_is_before_any_publication_and_destinations_are_guarded() {
    for saved in [false, true] {
        let (args, mut spi) = bias_fixture(saved);
        for row in 1..=spi.nobs {
            spi.numeric
                .insert((16, row), (((row - 1) / 4 + 71) as f64, false));
        }
        let error = estimate_ppml(&args, &mut spi).unwrap_err();
        assert!(error.message.contains("FE1: 8/8"), "{error}");
        assert_eq!(spi.store_calls, 0);
    }
    let (mut args, mut spi) = bias_fixture(false);
    args.retain(|s| !s.starts_with("output_bias_support="));
    assert!(estimate_ppml(&args, &mut spi).is_err());
    assert_eq!(spi.reads, 0);
    let (mut args, mut spi) = bias_fixture(false);
    replace_argument(
        &mut args,
        "output_bias_support",
        "output_bias_support=RBDIAG".to_owned(),
    );
    assert!(estimate_ppml(&args, &mut spi).is_err());
    assert_eq!(spi.reads, 0);
}

#[cfg(feature = "ffi")]
fn allocation_failure_phase_fixture(phase: PpmlPhase) -> (Vec<String>, FakeSpi) {
    let (mut args, mut spi) = categorical_absorb_cluster_weight_fixture();
    replace_argument(&mut args, "threads", "threads=1".to_owned());
    replace_argument(&mut args, "deterministic", "deterministic=1".to_owned());
    let (argument, methods) = match phase {
        PpmlPhase::SeparationFe => (
            "separation=fe",
            SeparationMethods {
                fixed_effects: true,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
        ),
        PpmlPhase::SeparationSimplex => (
            "separation=simplex,ir",
            SeparationMethods {
                fixed_effects: false,
                simplex: true,
                iterated_rectifier: true,
                mu: false,
            },
        ),
        PpmlPhase::SeparationIr => (
            "separation=ir",
            SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: true,
                mu: false,
            },
        ),
        PpmlPhase::SeparationMu => (
            "separation=mu",
            SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: true,
            },
        ),
        _ => (
            "separation=none",
            SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
        ),
    };
    replace_argument(&mut args, "separation", argument.to_owned());
    spi.matrices.insert(
        "RSEP".to_owned(),
        zero_matrix(separation_method_capacity(methods), SEPARATION_COLUMNS),
    );

    if matches!(
        phase,
        PpmlPhase::SeparationSimplex | PpmlPhase::SeparationIr | PpmlPhase::SeparationMu
    ) {
        // Retain a positive outcome in every absorbed group while giving
        // each numerical detector a genuine Poisson boundary to inspect.
        for observation in 1..=spi.nobs {
            if observation % 4 == 1 {
                spi.numeric.insert((2, observation), (0.0, false));
            }
        }
    }
    (args, spi)
}

#[cfg(feature = "ffi")]
fn allocation_audit_n100k_no_fe_k0_fixture() -> (Vec<String>, FakeSpi) {
    const OBSERVATIONS: usize = 100_000;
    let (mut args, mut spi) = fixture();
    let wire = ModelWireV2::from_graph(&CompiledTermGraph {
        sources: Vec::new(),
        terms: Vec::new(),
    })
    .expect("empty structural-constant wire");
    for (name, matrix) in [
        ("S", wire.sources.rows),
        ("T", wire.terms.rows),
        ("C", wire.components.rows),
        ("A", wire.absorb_terms.rows),
        ("AC", wire.absorb_components.rows),
    ] {
        spi.matrices.insert(name.to_owned(), matrix);
    }
    spi.matrices.insert(
        "BIND".to_owned(),
        vec![vec![1.0, 1.0, 0.0], vec![2.0, 2.0, 0.0]],
    );
    spi.matrices.insert("RB".to_owned(), zero_matrix(1, 1));
    spi.matrices.insert("RV".to_owned(), zero_matrix(1, 1));
    spi.matrices.insert(
        "RSTATUS".to_owned(),
        zero_matrix(1, COEFFICIENT_STATUS_COLUMNS),
    );
    spi.matrices.insert(
        "RSEP".to_owned(),
        zero_matrix(
            separation_method_capacity(SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            }),
            SEPARATION_COLUMNS,
        ),
    );
    spi.nobs = OBSERVATIONS;
    spi.nvars = 3;
    spi.numeric.clear();
    for observation in 1..=OBSERVATIONS {
        let outcome = 1.0 + (observation % 5) as f64;
        spi.numeric.insert((1, observation), (1.0, false));
        spi.numeric.insert((2, observation), (outcome, false));
        spi.numeric.insert((3, observation), (-999.0, false));
    }
    // The production SPI stores do not allocate Rust-side event records.
    // Reserve the fake log before the action baseline so the qualification
    // window measures the estimator rather than test harness growth.
    spi.stores = Vec::with_capacity(OBSERVATIONS + 1_024);

    for (key, replacement) in [
        ("source_count", "source_count=0".to_owned()),
        ("source_names", "source_names=.".to_owned()),
        ("term_count", "term_count=0".to_owned()),
        ("canonical_tokens", "canonical_tokens=.".to_owned()),
        ("component_count", "component_count=0".to_owned()),
        (
            "raw_component_operators",
            "raw_component_operators=.".to_owned(),
        ),
        (
            "raw_time_series_operators",
            "raw_time_series_operators=.".to_owned(),
        ),
        ("binding_count", "binding_count=2".to_owned()),
        ("residual", "residual=none".to_owned()),
        ("separation", "separation=none".to_owned()),
        ("threads", "threads=1".to_owned()),
        ("deterministic", "deterministic=1".to_owned()),
        ("output_sample", "output_sample=3".to_owned()),
        ("output_residual", "output_residual=0".to_owned()),
        ("output_xb", "output_xb=0".to_owned()),
        ("output_absorbed", "output_absorbed=0".to_owned()),
        ("output_eta", "output_eta=0".to_owned()),
        ("output_mean", "output_mean=0".to_owned()),
        ("output_drop_reason", "output_drop_reason=0".to_owned()),
    ] {
        replace_argument(&mut args, key, replacement);
    }
    args.push("allocation_audit=1".to_owned());
    (args, spi)
}

fn continuous_test_term(id: u32, source: SourceId) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(id).expect("term"),
        order: TermOrder::new(id).expect("order"),
        coefficient_slot: CoefficientSlot::new(id).expect("slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: "c.x".to_owned(),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component"),
            source,
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.0,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: "c".to_owned(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn categorical_test_term(id: u32, source: SourceId, level: f64, base: bool) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(id).expect("term"),
        order: TermOrder::new(id).expect("order"),
        coefficient_slot: CoefficientSlot::new(id).expect("slot"),
        raw_kind: RawTermKind::Factor,
        whole_column_omitted: base,
        canonical_token: format!("{level}.h"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component"),
            source,
            kind: ComponentKind::Categorical,
            level_state: LevelState::Present,
            level_value: level,
            base: if base {
                MetadataFlag::True
            } else {
                MetadataFlag::False
            },
            omitted: if base {
                MetadataFlag::True
            } else {
                MetadataFlag::False
            },
            raw_operator: level.to_string(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

#[test]
fn ppml_local_bundle_preserves_science_and_rejects_incomplete_input_before_reads() {
    let (args, mut ordinary) = fixture();
    estimate_ppml(&args, &mut ordinary).unwrap();
    let (args, mut local) = fixture();
    let mut payload = format!("{}:", args.len());
    for value in &args {
        payload.push_str(&format!("{}:{value}", value.len()));
    }
    let descriptor = [format!("bundle_local={}", payload.len())];
    local.local_bundle = Some(payload.as_bytes().to_vec());
    estimate_ppml(&descriptor, &mut local).unwrap();
    assert_eq!((ordinary.macro_reads, local.macro_reads), (0, 1));
    for name in [
        "RB", "RV", "RSTATUS", "RFIT", "RDOF", "RFE", "RCLUSTER", "RSUBSET", "RSEP", "RINF",
        "RDROP", "RSEQ", "RCERT", "RREC",
    ] {
        assert_eq!(ordinary.matrices[name], local.matrices[name], "{name}");
    }
    assert_eq!(ordinary.reads, local.reads);
    assert_eq!(ordinary.stores.last(), local.stores.last());
    assert_eq!(ordinary.store_calls, local.store_calls);
    let (_, mut truncated) = fixture();
    truncated.local_bundle = Some(payload.as_bytes()[..payload.len() - 1].to_vec());
    assert!(estimate_ppml(&descriptor, &mut truncated).is_err());
    assert_eq!(
        (
            truncated.macro_reads,
            truncated.reads,
            truncated.store_calls
        ),
        (1, 0, 0)
    );
    assert_eq!(truncated.receipt_complete(), 0.0);
}

#[test]
fn hidden_ppml_action_commits_a_distinct_receipt_last() {
    let (args, mut spi) = fixture();
    estimate_ppml(&args, &mut spi).expect("hidden PPML estimate");
    assert_eq!(spi.receipt_complete(), 1.0);
    assert_eq!(spi.matrices["RREC"][0][0], fereg_core::PPML_RESULT_MAGIC);
    assert_eq!(spi.matrices["RREC"][0][1], 1.0);
    assert_eq!(
        spi.matrices["RREC"][0][2],
        f64::from(PPML_RESULT_WIRE_VERSION.minor)
    );
    assert_eq!(spi.matrices["RREC"][0][4], 2.0);
    assert_eq!(spi.matrices["RREC"][0][6], 10.0);
    assert_eq!(spi.matrices["RREC"][0][8], 10.0);
    assert_eq!(spi.matrices["RREC"][0][19], INFERENCE_ROWS as f64);
    assert_eq!(spi.matrices["RREC"][0][20], DROP_COUNT_ROWS as f64);
    assert_eq!(spi.matrices["RREC"][0][21], SEPARATION_SEQUENCE_ROWS as f64);
    assert!(spi.matrices["RREC"][0][22] >= 1.0);
    for limb in 24..32 {
        let value = spi.matrices["RREC"][0][limb];
        assert_eq!(value.fract(), 0.0);
        assert!((0.0..=f64::from(u32::MAX)).contains(&value));
    }
    assert_eq!(spi.matrices["RINF"][0][0], 2.0);
    assert_eq!(spi.matrices["RINF"][1][0], 2.0);
    assert_eq!(spi.matrices["RINF"][15][0], 1.0);
    assert_eq!(spi.matrices["RDROP"][0], vec![0.0, 10.0]);
    for reason in 0..DROP_COUNT_ROWS {
        assert_eq!(spi.matrices["RDROP"][reason][0], reason as f64);
    }
    assert_eq!(spi.matrices["RSEQ"][0][0], 1.0);
    assert_eq!(spi.matrices["RSEQ"][0][1], 1.0);
    assert_eq!(spi.matrices["RSEQ"][4][1], 0.0);
    assert_eq!(spi.matrices["RRUNTIME"][40][0], 0.0);
    assert_eq!(spi.matrices["RRUNTIME"][76][0], 1.0);
    assert_eq!(spi.matrices["RRUNTIME"][77][0], 1.0);
    let allocator_rows = [
        spi.matrices["RRUNTIME"][79][0],
        spi.matrices["RRUNTIME"][80][0],
        spi.matrices["RRUNTIME"][81][0],
    ];
    if allocator_rows
        .iter()
        .all(|value| crate::ppml_result::is_stata_publishable(*value))
    {
        assert!(allocator_rows[1] >= allocator_rows[0]);
        assert_eq!(allocator_rows[2], allocator_rows[1] - allocator_rows[0]);
    } else {
        assert!(allocator_rows
            .iter()
            .all(|value| !crate::ppml_result::is_stata_publishable(*value)));
    }
    assert!(spi.matrices["RRUNTIME"][82..95]
        .iter()
        .all(|row| crate::ppml_result::is_stata_publishable(row[0])));
    assert!(spi.matrices["RRUNTIME"][95..108]
        .iter()
        .all(|row| !crate::ppml_result::is_stata_publishable(row[0])));
    assert_eq!(
        spi.stores.last(),
        Some(&StoreEvent::Matrix(
            "RREC".to_owned(),
            1,
            PPML_RESULT_RECEIPT_COMPLETE_COLUMN,
            1.0,
        )),
    );
    assert_eq!(
        spi.stores.get(spi.stores.len() - 2),
        Some(&StoreEvent::Matrix(
            "RRUNTIME".to_owned(),
            RUNTIME_ROWS,
            1,
            spi.matrices["RRUNTIME"][RUNTIME_ROWS - 1][0],
        )),
    );
    for observation in 1..=10 {
        assert_eq!(spi.output(4, observation), Some(1.0));
        assert!(spi
            .output(5, observation)
            .is_some_and(|value| value.abs() < 1e-8));
        assert!(spi.output(7, observation).is_some_and(|value| value > 0.0));
        assert_eq!(spi.output(8, observation), Some(0.0));
        assert!(spi
            .output(9, observation)
            .zip(spi.output(6, observation))
            .is_some_and(|(xb, eta)| (xb - eta).abs() < 1e-12));
        assert!(spi
            .output(10, observation)
            .is_some_and(|value| value.abs() < 1e-12));
    }
}

#[test]
fn default_request_binds_separation_and_fit_to_one_linear_policy() {
    let (args, mut spi) = fixture();
    let specification = EstimatePpmlSpec::parse(&args).expect("default PPML specification");
    let action = specification
        .decode_and_validate(&mut spi)
        .expect("default PPML request");
    let (absorb, fixed_effect_collinearity_tolerance) = action.config.shared_linear_policy();
    assert_eq!(action.linear.absorb, absorb);
    assert_eq!(
        action.linear.fixed_effect_collinearity_tolerance,
        fixed_effect_collinearity_tolerance,
    );
    assert_eq!(action.separation.methods, SeparationMethods::default());
}

#[test]
fn ppml_topology_injection_is_explicit_and_does_not_accept_routing_presets() {
    let (args, mut spi) = fixture();
    let action = EstimatePpmlSpec::parse(&args)
        .unwrap()
        .decode_and_validate(&mut spi)
        .unwrap();
    assert!(
        !prepared_ppml_request(&action, None)
            .unwrap()
            .inject_topology_sharing
    );
    let mut experiment = args.clone();
    experiment.push("inject=ppml_topology".to_owned());
    let action = EstimatePpmlSpec::parse(&experiment)
        .unwrap()
        .decode_and_validate(&mut spi)
        .unwrap();
    assert!(
        prepared_ppml_request(&action, None)
            .unwrap()
            .inject_topology_sharing
    );
    for unsupported in [
        "routing_h4_k0",
        "routing_cells",
        "ppml_topology_bad",
        "routing_incremental_many",
        "routing_incremental_two",
        "routing_many_geometry_v1",
        "routing_two_gates_v2",
    ] {
        let mut rejected = args.clone();
        rejected.push(format!("inject={unsupported}"));
        assert!(EstimatePpmlSpec::parse(&rejected).is_err());
    }
}

#[test]
fn ppml_separation_auto_injection_is_explicit_and_exclusive() {
    let (args, mut spi) = fixture();
    let action = EstimatePpmlSpec::parse(&args)
        .unwrap()
        .decode_and_validate(&mut spi)
        .unwrap();
    let request = prepared_ppml_request(&action, None).unwrap();
    assert!(!request.inject_separation_auto);
    let mut experiment = args.clone();
    experiment.push("inject=ppml_separation_auto".to_owned());
    let action = EstimatePpmlSpec::parse(&experiment)
        .unwrap()
        .decode_and_validate(&mut spi)
        .unwrap();
    let request = prepared_ppml_request(&action, None).unwrap();
    assert!(request.inject_separation_auto);
    assert!(!request.inject_topology_sharing);
    assert!(!request.inject_fe_recovery_allocation_failure);
    for unsupported in [
        "ppml_separation_auto_v1",
        "ppml_separation",
        "PPML_SEPARATION_AUTO",
    ] {
        let mut rejected = args.clone();
        rejected.push(format!("inject={unsupported}"));
        assert!(EstimatePpmlSpec::parse(&rejected).is_err());
    }
}

#[test]
fn public_solver_names_decode_to_typed_families() {
    for (name, expected) in [
        ("auto", PpmlSolverFamily::Auto),
        ("map", PpmlSolverFamily::Map),
        ("lsmr", PpmlSolverFamily::Lsmr),
        ("cmg", PpmlSolverFamily::Cmg),
    ] {
        let (mut args, _spi) = fixture();
        replace_argument(&mut args, "solver", format!("solver={name}"));
        let specification = EstimatePpmlSpec::parse(&args).expect("solver specification");
        assert_eq!(
            specification.decode_solver().expect("public solver"),
            expected
        );
    }
}

#[test]
fn unknown_and_retired_solver_spellings_fail_closed() {
    let (mut args, _spi) = fixture();
    replace_argument(&mut args, "solver", "solver=cg".to_owned());
    let error = EstimatePpmlSpec::parse(&args)
        .expect("complete request")
        .decode_solver()
        .expect_err("unknown public solver must fail closed");
    assert_eq!(error.code, ErrorCode::Syntax);

    let (mut args, _spi) = fixture();
    args.push("fe_solver=lsmr".to_owned());
    let error =
        EstimatePpmlSpec::parse(&args).expect_err("retired private solver key must be rejected");
    assert!(error
        .message
        .contains("unknown estimate-ppml field fe_solver"));
}

#[test]
fn ols_cmgschur_options_fail_before_ppml_reads_and_stores() {
    for field in [
        "solver=cmgschur",
        "cmgdegree=5",
        "cmgsamples=0",
        "cmgsamples=1",
        "cmgseed=0",
        "cmgseed=1",
    ] {
        let (mut args, mut spi) = fixture();
        if field.starts_with("solver=") {
            replace_argument(&mut args, "solver", field.to_owned());
        } else {
            args.push(field.to_owned());
        }
        let error = estimate_ppml(&args, &mut spi).unwrap_err();
        assert_eq!(error.code, ErrorCode::Syntax);
        assert_eq!((spi.reads, spi.store_calls), (0, 0));
        assert_eq!(spi.receipt_complete(), 0.0);
    }
}

fn with_certified_fixture(
    fixture: (Vec<String>, FakeSpi),
    assertion: impl FnOnce(EstimatePpmlAction, fereg_core::ppml::CertifiedPpmlResult, FakeSpi),
) {
    with_certified_fixture_probe(fixture, None, assertion);
}

fn with_certified_fixture_probe(
    (args, mut spi): (Vec<String>, FakeSpi),
    phase_allocation_probe: Option<fereg_core::ppml::PpmlPhaseAllocatorProbe>,
    assertion: impl FnOnce(EstimatePpmlAction, fereg_core::ppml::CertifiedPpmlResult, FakeSpi),
) {
    let specification = EstimatePpmlSpec::parse(&args).expect("PPML fixture specification");
    let action = specification
        .decode_and_validate(&mut spi)
        .expect("PPML fixture action");
    let loaded = load_ols_input(
        &action.request,
        action.candidate_all,
        action.input_complete,
        &mut spi,
    )
    .expect("PPML fixture input");
    let adjustment = load_adjustment(action.adjustment, loaded.candidate.view(), &mut spi)
        .expect("PPML fixture adjustment");
    let result = fit_prepared_ppml(
        PreparedPpmlRequestV1 {
            bias: None,
            ols: fallible_clone_ols_request(&action.request).expect("request copy"),
            clusters: fallible_clone_clusters(&action.clusters).expect("cluster copy"),
            vce: action.vce,
            ppml: action.config,
            linear: action.linear,
            memory: action.memory,
            memory_preflight_probe: None,
            allocation_snapshot_probe: None,
            phase_allocation_probe,
            linear_phase_allocation_probe: None,
            action_allocation_snapshot_probe: None,
            action_allocation_start: None,
            action_allocation_previous_baseline_bytes: None,
            separation: action.separation,
            output: PreparedPpmlOutputRequest {
                residual: action.residual,
                xb: action.outputs.xb.is_some(),
                absorbed: action.outputs.absorbed.is_some(),
                eta: action.outputs.eta.is_some(),
                mean: action.outputs.mean.is_some(),
                requested_observation_columns: action.requested_observation_columns,
                publication_matrix_cells: action.publication_matrix_cells,
            },
            solver: action.solver,
            inject_fe_recovery_allocation_failure: false,
            inject_topology_sharing: false,
            inject_separation_auto: false,
        },
        LoadedPpmlInput {
            bias_units: None,
            ols: loaded,
            adjustment,
            adjustment_kind: action.adjustment.kind(),
        },
    )
    .expect("PPML fixture fit");
    assertion(action, result, spi);
}

fn with_unpublished_shadow(
    assertion: impl FnOnce(&EstimatePpmlAction, &mut PpmlShadowResult<'_>, &mut FakeSpi),
) {
    with_certified_fixture(fixture(), |action, result, mut spi| {
        let mut shadow = build_certified_ppml_shadow(
            &result,
            CertifiedPpmlShadowContext {
                residual: action.residual,
                xb_requested: action.outputs.xb.is_some(),
                separation_methods: action.separation.methods,
                drop_singletons: action.separation.drop_singletons,
                elapsed_seconds: 0.0,
            },
        )
        .expect("PPML fixture shadow");
        assertion(&action, &mut shadow, &mut spi);
    });
}

#[test]
fn structural_checkpoint_publication_accepts_both_storage_modes() {
    with_unpublished_shadow(|_, shadow, _| {
        let (args, mut spi) = categorical_absorb_cluster_weight_fixture();
        let mut action = EstimatePpmlSpec::parse(&args)
            .unwrap()
            .decode_and_validate(&mut spi)
            .unwrap();
        let term = action.request.absorbed_terms[0].clone();
        action.request.absorbed_terms = vec![term; 3];
        for request in [PpmlSolverFamily::Auto, PpmlSolverFamily::Map] {
            action.solver = request;
            for row in 0..SOLVER_ROWS {
                shadow.solver[row * SOLVER_COLUMNS..row * SOLVER_COLUMNS + 6].copy_from_slice(&[
                    f64::from(request.code()),
                    2.,
                    1.,
                    2.,
                    1.,
                    3.,
                ]);
            }
            for selected in [1., 2.] {
                shadow.runtime[5] = selected;
                for memory in [MemoryMode::Auto, MemoryMode::Fast, MemoryMode::Low] {
                    action.memory.requested = memory;
                    validate_solver_evidence(&action, shadow).unwrap();
                }
            }
            shadow.solver[1] = 3.;
            assert!(validate_solver_evidence(&action, shadow).is_err());
            shadow.solver[1] = 2.;
        }
        action.request.absorbed_terms.pop();
        assert!(validate_solver_evidence(&action, shadow).is_err());
    });
}

#[cfg(feature = "ffi")]
static CORE_PUBLICATION_END_CURRENT: AtomicU64 = AtomicU64::new(u64::MAX);

#[cfg(feature = "ffi")]
fn capture_core_publication_end(
    phase: PpmlPhase,
    event: fereg_core::ppml::PpmlPhaseAllocatorEvent,
) -> u64 {
    if phase == PpmlPhase::Publication && event == fereg_core::ppml::PpmlPhaseAllocatorEvent::End {
        CORE_PUBLICATION_END_CURRENT.store(
            crate::ffi::allocation_usage().current_bytes,
            Ordering::Relaxed,
        );
    }
    // This probe checks retained ownership, not a phase-peak estimate.
    0
}

#[test]
#[cfg(feature = "ffi")]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn core_publication_observation_has_no_obsolete_fit_or_generation_owner() {
    let _guard = crate::instrumentation::lock_action_observation();
    let (separated_args, mut separated_spi) =
        allocation_failure_phase_fixture(PpmlPhase::SeparationFe);
    for row in 1..=4 {
        separated_spi.numeric.insert((2, row), (0.0, false));
    }
    let (mut low_args, low_spi) = categorical_absorb_cluster_weight_fixture();
    replace_argument(&mut low_args, "memory", "memory=low".to_owned());
    for fixture in [
        fixture(),
        categorical_absorb_cluster_weight_fixture(),
        (separated_args, separated_spi),
        (low_args, low_spi),
    ] {
        CORE_PUBLICATION_END_CURRENT.store(u64::MAX, Ordering::Relaxed);
        with_certified_fixture_probe(
            fixture,
            Some(capture_core_publication_end),
            |_action, _result, _spi| {
                assert_eq!(
                    CORE_PUBLICATION_END_CURRENT.load(Ordering::Relaxed),
                    crate::ffi::allocation_usage().current_bytes,
                    "core return released obsolete state after the publication observation",
                );
            },
        );
    }
}

#[test]
#[cfg(feature = "ffi")]
fn final_sample_validation_allocates_no_observation_mask() {
    let _guard = crate::instrumentation::lock_action_observation();
    let n = 100_000;
    // Construct the shared row owner before measuring validation, as the
    // estimator does. Publication audits separately charge its control block.
    let rows: std::sync::Arc<Vec<usize>> = std::sync::Arc::new((0..n).step_by(2).collect());
    let prepared = std::sync::Arc::new(rows.as_ref().clone());
    let reasons = (0..n)
        .map(|row| {
            if row % 2 == 0 {
                fereg_core::DropReason::Retained
            } else {
                fereg_core::DropReason::Singleton
            }
        })
        .collect();
    let outcome = vec![1.0; rows.len()];
    let weights = vec![1.0; rows.len()];
    let before = crate::ffi::allocation_usage();
    let sample =
        fereg_core::ppml::PpmlFinalSample::new(n, n, prepared, rows, reasons, outcome, weights)
            .unwrap();
    let after = crate::ffi::allocation_usage();
    assert_eq!(after.events, before.events);
    assert_eq!(after.current_bytes, before.current_bytes);
    assert_eq!(sample.original_rows().len(), n / 2);
}

#[test]
#[cfg(feature = "ffi")]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn publication_owners_match_independent_deallocation_and_exact_admission() {
    let _guard = crate::instrumentation::lock_action_observation();
    let (separated_args, mut separated_spi) =
        allocation_failure_phase_fixture(PpmlPhase::SeparationFe);
    for observation in 1..=4 {
        separated_spi.numeric.insert((2, observation), (0.0, false));
    }
    let (mut low_args, low_spi) = categorical_absorb_cluster_weight_fixture();
    replace_argument(&mut low_args, "memory", "memory=low".to_owned());
    for fixture in [
        fixture(),
        categorical_absorb_cluster_weight_fixture(),
        (separated_args, separated_spi),
        (low_args, low_spi),
    ] {
        with_certified_fixture(fixture, |mut action, result, spi| {
            // Force nontrivial nested and outer capacity at the surviving
            // embedding boundary; these are not observation-sized scans.
            action.clusters.reserve(11);
            for cluster in &mut action.clusters {
                cluster.label.reserve(29);
                cluster.sources.reserve(7);
            }
            action.outputs.saved_channels.reserve(13);
            action.break_marker_token.reserve(31);
            action.request.model.sources.reserve(5);
            for source in &mut action.request.model.sources {
                source.canonical_name.reserve(17);
            }
            let before_count = crate::ffi::allocation_usage();
            let result_bytes = result.owned_buffer_bytes();
            let action_bytes = action.metadata_capacity_bytes();
            let model = action.publication_memory(&result).unwrap();
            let peak = model.peak_bytes().unwrap();
            model.admit(Some(peak)).unwrap();
            assert_eq!(crate::ffi::allocation_usage().events, before_count.events);
            assert_eq!(
                model.admit(Some(peak - 1)).unwrap_err().code,
                ErrorCode::Allocation
            );
            assert_eq!(spi.store_calls, 0);
            assert_eq!(spi.receipt_complete(), 0.0);

            let start = crate::instrumentation::begin_ppml_observation();
            let mut shadow = build_certified_ppml_shadow(
                &result,
                CertifiedPpmlShadowContext {
                    residual: action.residual,
                    xb_requested: action.outputs.xb.is_some(),
                    separation_methods: action.separation.methods,
                    drop_singletons: action.separation.drop_singletons,
                    elapsed_seconds: 0.0,
                },
            )
            .unwrap();
            model.validate_capacity(&shadow).unwrap();
            let after_shadow = crate::ffi::allocation_usage();
            assert!(
                u128::from(after_shadow.peak_bytes - start.allocation_current_bytes)
                    <= model.shadow_bytes
            );
            let shadow_bytes = shadow.capacity_bytes();
            assert_eq!(
                u128::from(after_shadow.current_bytes - start.allocation_current_bytes),
                shadow_bytes
            );
            // Spare descriptor capacity must be charged and rejected, even
            // though the lengths and every scientific output still agree.
            shadow
                .saved_channels
                .reserve(model.shadow_bytes as usize + 1);
            assert_eq!(
                model.validate_capacity(&shadow).unwrap_err().code,
                ErrorCode::Allocation
            );
            drop(shadow);
            let before = crate::ffi::allocation_usage();
            drop(result);
            let after = crate::ffi::allocation_usage();
            assert_eq!(
                u128::from(before.current_bytes - after.current_bytes),
                result_bytes
            );
            drop(action);
            assert_eq!(
                u128::from(after.current_bytes - crate::ffi::allocation_usage().current_bytes),
                action_bytes
            );
        });
    }
}

#[test]
fn bounded_evidence_and_receipt_tampering_fail_before_any_store() {
    with_unpublished_shadow(|action, shadow, spi| {
        validate_shadow(action, shadow, spi).expect("untampered shadow");

        let inference = shadow.inference[0];
        shadow.inference[0] = 3.0;
        assert_eq!(
            validate_shadow(action, shadow, spi)
                .expect_err("inference tamper")
                .code,
            ErrorCode::Internal
        );
        shadow.inference[0] = inference;

        shadow.drop_counts[1] += 1.0;
        assert_eq!(
            validate_shadow(action, shadow, spi)
                .expect_err("drop-count tamper")
                .code,
            ErrorCode::Internal
        );
        shadow.drop_counts[1] -= 1.0;

        let row = SEPARATION_SEQUENCE_COLUMNS;
        for column in 0..SEPARATION_SEQUENCE_COLUMNS {
            shadow.separation_sequence.swap(column, row + column);
        }
        assert_eq!(
            validate_shadow(action, shadow, spi)
                .expect_err("sequence order tamper")
                .code,
            ErrorCode::Internal
        );
        for column in 0..SEPARATION_SEQUENCE_COLUMNS {
            shadow.separation_sequence.swap(column, row + column);
        }

        shadow.receipt[22] += 1.0;
        assert_eq!(
            validate_shadow(action, shadow, spi)
                .expect_err("execution-count receipt tamper")
                .code,
            ErrorCode::Internal
        );
        shadow.receipt[22] -= 1.0;

        for (limb, label) in [
            (24, "initial generation hash"),
            (27, "final generation hash"),
            (29, "execution trace digest"),
            (31, "generation-hash digest"),
        ] {
            shadow.receipt[limb] += 1.0;
            assert_eq!(
                validate_shadow(action, shadow, spi).expect_err(label).code,
                ErrorCode::Internal,
                "{label}",
            );
            shadow.receipt[limb] -= 1.0;
        }
        validate_shadow(action, shadow, spi).expect("restored shadow");
        assert_eq!(spi.store_calls, 0);
        assert!(spi.stores.is_empty());
        assert_eq!(spi.receipt_complete(), 0.0);
    });
}

#[cfg(feature = "ffi")]
#[test]
fn malformed_registry_cardinality_rejects_before_reservation() {
    let _guard = crate::instrumentation::lock_action_observation();
    let (start, _) = crate::ffi::begin_ols_allocation_observation();
    let error = crate::model_metadata::decode_registry(
        "1:x",
        usize::MAX / 32,
        crate::model_metadata::RegistryFraming::Delimited,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(crate::ffi::allocation_usage().peak_bytes - start.current_bytes < 1024);
}

#[cfg(feature = "ffi")]
#[test]
fn no_route_rejects_before_canonical_numeric_allocation_and_stores_nothing() {
    const CHILD_ENV: &str = "FEREG_PPML_EARLY_PREFLIGHT_TEST_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(
            std::env::current_exe().expect("current plugin test executable"),
        )
        .env(CHILD_ENV, "1")
        .env_remove("FEREG_MEMORY_LIMIT_BYTES")
        .arg(
            "ppml_estimate::tests::no_route_rejects_before_canonical_numeric_allocation_and_stores_nothing",
        )
        .arg("--exact")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .status()
        .expect("launch isolated early-preflight allocator test");
        assert!(status.success(), "isolated early-preflight test failed");
        return;
    }

    {
        let _guard = crate::instrumentation::lock_action_observation();
        let (args, mut spi) = allocation_audit_n100k_no_fe_k0_fixture();
        let specification = EstimatePpmlSpec::parse(&args).expect("N=100,000 specification");
        let action = specification
            .decode_and_validate(&mut spi)
            .expect("N=100,000 decoded action");
        let loaded = load_ols_input(
            &action.request,
            action.candidate_all,
            action.input_complete,
            &mut spi,
        )
        .expect("N=100,000 loaded input");
        let adjustment = load_adjustment(action.adjustment, loaded.candidate.view(), &mut spi)
            .expect("N=100,000 adjustment");
        let mut memory = action.memory;
        memory.available_bytes = Some(1);
        memory.embedding_resident_bytes = 0;
        let prepared_request = PreparedPpmlRequestV1 {
            bias: None,
            ols: fallible_clone_ols_request(&action.request).expect("request copy"),
            clusters: fallible_clone_clusters(&action.clusters).expect("cluster copy"),
            vce: action.vce,
            ppml: action.config,
            linear: action.linear,
            memory,
            memory_preflight_probe: Some(record_early_preflight_allocator),
            allocation_snapshot_probe: None,
            phase_allocation_probe: None,
            linear_phase_allocation_probe: None,
            action_allocation_snapshot_probe: None,
            action_allocation_start: None,
            action_allocation_previous_baseline_bytes: None,
            separation: action.separation,
            output: PreparedPpmlOutputRequest {
                residual: action.residual,
                xb: action.outputs.xb.is_some(),
                absorbed: action.outputs.absorbed.is_some(),
                eta: action.outputs.eta.is_some(),
                mean: action.outputs.mean.is_some(),
                requested_observation_columns: action.requested_observation_columns,
                publication_matrix_cells: action.publication_matrix_cells,
            },
            solver: action.solver,
            inject_fe_recovery_allocation_failure: false,
            inject_topology_sharing: false,
            inject_separation_auto: false,
        };
        let prepared_input = LoadedPpmlInput {
            bias_units: None,
            ols: loaded,
            adjustment,
            adjustment_kind: action.adjustment.kind(),
        };

        EARLY_PREFLIGHT_CURRENT_BYTES.store(0, Ordering::Relaxed);
        EARLY_PREFLIGHT_PEAK_BYTES.store(0, Ordering::Relaxed);
        EARLY_PREFLIGHT_EVENTS.store(0, Ordering::Relaxed);
        let (start, _) = crate::ffi::begin_allocation_observation();
        let error = fit_prepared_ppml(prepared_request, prepared_input)
            .expect_err("one-byte budget must reject every PPML route");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(error.return_code(), 909);
        assert_eq!(
            EARLY_PREFLIGHT_EVENTS.load(Ordering::Relaxed),
            start.events,
            "classification and route decision must allocate no canonical or temporary buffer",
        );
        assert_eq!(
            EARLY_PREFLIGHT_CURRENT_BYTES.load(Ordering::Relaxed),
            start.current_bytes,
            "no memory may remain allocated at the no-route decision",
        );
        assert_eq!(
            EARLY_PREFLIGHT_PEAK_BYTES.load(Ordering::Relaxed),
            start.peak_bytes,
            "no observation-sized canonical allocation may precede rejection",
        );
    }

    std::env::set_var("FEREG_MEMORY_LIMIT_BYTES", "1");
    // Exercise the complete plugin transaction under the same imposed
    // one-byte limit. The direct core observation above proves timing;
    // this second arm proves that the ordinary error path cannot publish
    // a prefix or completion receipt.
    let (args, mut spi) = allocation_audit_n100k_no_fe_k0_fixture();
    let error = estimate_ppml(&args, &mut spi)
        .expect_err("the plugin transaction must propagate no-route rejection");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(error.return_code(), 909);
    assert_eq!(spi.store_calls, 0);
    assert!(spi.stores.is_empty());
    assert_eq!(spi.receipt_complete(), 0.0);
}

#[cfg(feature = "ffi")]
#[test]
fn allocation_audit_is_opt_in_and_publishes_real_phase_evidence() {
    const CHILD_ENV: &str = "FEREG_PPML_ALLOCATION_AUDIT_TEST_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        // The tracking allocator is process-global by design. Run this
        // qualification assertion in a one-test child so allocations by
        // unrelated parallel unit tests cannot be misattributed to a
        // PPML phase and spuriously exceed its preflight model.
        let status = std::process::Command::new(
            std::env::current_exe().expect("current plugin test executable"),
        )
        .env(CHILD_ENV, "1")
        .arg("ppml_estimate::tests::allocation_audit_is_opt_in_and_publishes_real_phase_evidence")
        .arg("--exact")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .status()
        .expect("launch isolated PPML allocation-audit test");
        assert!(status.success(), "isolated PPML allocation audit failed");
        return;
    }
    let (args, mut normal) = fixture();
    estimate_ppml(&args, &mut normal).expect("normal PPML estimate");
    let normal_outer = normal.matrices["RRUNTIME"][1][0];
    assert_eq!(normal.matrices["RRUNTIME"][40][0], 0.0);

    let (mut audited_args, mut audited) = fixture();
    audited_args.push("allocation_audit=1".to_owned());
    estimate_ppml(&audited_args, &mut audited).expect("audited PPML estimate");
    let runtime = &audited.matrices["RRUNTIME"];
    assert_eq!(runtime[9][0], 0.0, "automatic thread request stays public");
    assert_eq!(runtime[10][0], ThreadConfig::available() as f64);
    assert_eq!(runtime[40][0], 1.0);
    assert!(runtime[41][0] >= 16.0);
    assert!(runtime[1][0] > normal_outer);
    assert_eq!(
        runtime[44][0], 0.0,
        "warm iterations must not allocate identity scratch"
    );
    assert_eq!(
        runtime[72][0], 1.0,
        "linear session reuse must be certified"
    );
    assert_eq!(runtime[75][0], 1.0, "real allocator certificate must pass");
    assert!(runtime[41..76]
        .iter()
        .all(|row| crate::ppml_result::is_stata_publishable(row[0])));
    assert!(runtime[95..108]
        .iter()
        .all(|row| crate::ppml_result::is_stata_publishable(row[0])));
    assert!(runtime[80][0] >= runtime[79][0]);
    assert_eq!(runtime[81][0], runtime[80][0] - runtime[79][0]);
    assert_eq!(audited.receipt_complete(), 1.0);
}

#[cfg(feature = "ffi")]
#[test]
fn allocation_failure_is_atomic_in_every_fit_lifetime_phase() {
    const CHILD_ENV: &str = "FEREG_PPML_PHASE_ALLOCATION_FAILURE_TEST_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(
            std::env::current_exe().expect("current plugin test executable"),
        )
        .env(CHILD_ENV, "1")
        .arg("ppml_estimate::tests::allocation_failure_is_atomic_in_every_fit_lifetime_phase")
        .arg("--exact")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .status()
        .expect("launch isolated PPML allocation-failure test");
        assert!(
            status.success(),
            "isolated PPML allocation-failure test failed"
        );
        return;
    }

    for phase in fereg_core::ppml::PPML_PHASES {
        let (args, mut spi) = allocation_failure_phase_fixture(phase);
        crate::ffi::inject_ppml_allocation_failure(phase);
        let result = estimate_ppml(&args, &mut spi);
        let fired = crate::ffi::ppml_allocation_failure_fired();
        let successful_events = crate::ffi::test_ppml_phase_allocation_events(phase);
        eprintln!(
            "PPML allocation-failure phase {}: fired={fired}, successful_events={successful_events}",
            phase.name(),
        );
        if fired {
            let error = result.expect_err("injected PPML allocation must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation, "phase {}", phase.name());
            assert_eq!(spi.store_calls, 0, "phase {}", phase.name());
            assert!(spi.stores.is_empty(), "phase {}", phase.name());
            assert_eq!(spi.receipt_complete(), 0.0, "phase {}", phase.name());
        } else {
            result.unwrap_or_else(|error| {
                panic!(
                    "allocation-free phase {} did not complete normally: {error}",
                    phase.name(),
                )
            });
            assert_eq!(
                successful_events,
                0,
                "phase {} allocated but did not trigger its armed failure",
                phase.name(),
            );
            assert_eq!(spi.receipt_complete(), 1.0, "phase {}", phase.name());
        }
    }
}

#[cfg(feature = "ffi")]
#[test]
fn allocation_audit_n100k_no_fe_k0_accounts_embedding_preparation() {
    const CHILD_ENV: &str = "FEREG_PPML_N100K_ALLOCATION_AUDIT_TEST_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(
            std::env::current_exe().expect("current plugin test executable"),
        )
        .env(CHILD_ENV, "1")
        .arg("ppml_estimate::tests::allocation_audit_n100k_no_fe_k0_accounts_embedding_preparation")
        .arg("--exact")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .status()
        .expect("launch isolated N=100,000 PPML allocation-audit test");
        assert!(status.success(), "isolated N=100,000 audit failed");
        return;
    }

    let (args, mut spi) = allocation_audit_n100k_no_fe_k0_fixture();
    let estimation = estimate_ppml(&args, &mut spi);
    let embedding = TEST_LAST_EMBEDDING_RESIDENT_BYTES.load(std::sync::atomic::Ordering::Relaxed);
    if let Err(error) = &estimation {
        eprintln!("n100k_no_fe_k0 embedding={embedding} error={error}");
    }
    estimation.expect("N=100,000 structural-constant audit");
    let modeled_preparation = spi.matrices["RRUNTIME"][82][0];
    let observed_preparation = spi.matrices["RRUNTIME"][95][0];
    assert!(embedding > 0);
    assert!(crate::ppml_result::is_stata_publishable(
        modeled_preparation
    ));
    assert!(crate::ppml_result::is_stata_publishable(
        observed_preparation
    ));
    assert!(observed_preparation <= modeled_preparation);
    assert_eq!(spi.receipt_complete(), 1.0);
    assert_eq!(
        spi.stores.last(),
        Some(&StoreEvent::Matrix(
            "RREC".to_owned(),
            1,
            PPML_RESULT_RECEIPT_COMPLETE_COLUMN,
            PPML_RESULT_RECEIPT_COMPLETE_VALUE,
        )),
    );
    eprintln!(
        "n100k_no_fe_k0 embedding={embedding} modeled_preparation={modeled_preparation} observed_preparation={observed_preparation}",
    );
}

#[cfg(feature = "ffi")]
#[test]
fn allocation_audit_preflights_worker_team_switches_one_four_one() {
    const CHILD_ENV: &str = "FEREG_PPML_THREAD_SWITCH_ALLOCATION_AUDIT_TEST_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(
            std::env::current_exe().expect("current plugin test executable"),
        )
        .env(CHILD_ENV, "1")
        .arg("ppml_estimate::tests::allocation_audit_preflights_worker_team_switches_one_four_one")
        .arg("--exact")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .status()
        .expect("launch isolated PPML worker-team switch audit");
        assert!(status.success(), "isolated worker-team switch audit failed");
        return;
    }

    for (action_index, requested_threads) in [1_usize, 4, 1].into_iter().enumerate() {
        let (mut args, mut spi) = allocation_audit_n100k_no_fe_k0_fixture();
        replace_argument(&mut args, "threads", format!("threads={requested_threads}"));
        estimate_ppml(&args, &mut spi).unwrap_or_else(|error| {
            panic!(
                "worker-team switch action {action_index} ({requested_threads} threads) failed: {error}",
            )
        });
        let runtime = &spi.matrices["RRUNTIME"];
        assert_eq!(runtime[9][0], requested_threads as f64);
        assert_eq!(runtime[10][0], requested_threads as f64);
        assert_eq!(
            runtime[40][0],
            1.0,
            "action {action_index} allocation-audit rows 41-78: {:?}",
            &runtime[40..78],
        );
        for phase in 0..fereg_core::ppml::PPML_PHASE_COUNT {
            let modeled = runtime[82 + phase][0];
            let observed = runtime[95 + phase][0];
            assert!(crate::ppml_result::is_stata_publishable(modeled));
            assert!(crate::ppml_result::is_stata_publishable(observed));
            assert!(
                observed <= modeled,
                "action {action_index} phase {phase}: observed={observed}, modeled={modeled}",
            );
        }
        let start = runtime[17][0];
        let current = runtime[18][0];
        let peak = runtime[19][0];
        let peak_delta = runtime[20][0];
        let growth = runtime[21][0];
        let ratio = runtime[22][0];
        let modeled_selected = runtime[13][0];
        for value in [start, current, peak, peak_delta, growth, ratio] {
            assert!(crate::ppml_result::is_stata_publishable(value));
        }
        assert!(peak >= start.max(current));
        assert_eq!(peak_delta, peak - start);
        assert_eq!(growth, current - start);
        assert!(peak_delta <= modeled_selected);
        assert!(ratio <= 1.0);
        assert_eq!(runtime[79][0] + runtime[81][0], runtime[80][0]);
        assert_eq!(spi.receipt_complete(), 1.0);
    }
}

#[test]
fn hidden_ppml_action_carries_factor_absorb_cluster_weight_and_saved_fe() {
    let (args, mut spi) = categorical_absorb_cluster_weight_fixture();
    estimate_ppml(&args, &mut spi).expect("categorical hidden PPML estimate");
    assert_eq!(spi.receipt_complete(), 1.0);
    assert_eq!(spi.matrices["RREC"][0][3], 3.0);
    assert_eq!(spi.matrices["RREC"][0][4], 4.0);
    assert_eq!(spi.matrices["RREC"][0][9], 1.0);
    assert_eq!(spi.matrices["RREC"][0][10], 1.0);
    assert_eq!(spi.matrices["RREC"][0][11], 1.0);
    assert_eq!(spi.matrices["RREC"][0][15], 0.0);
    assert_eq!(spi.matrices["RREC"][0][17], 1.0);
    assert_eq!(spi.matrices["RSTATUS"][0][1], 1.0);
    assert_eq!(spi.matrices["RSTATUS"][1][1], 2.0);
    assert_eq!(spi.matrices["RSTATUS"][2][1], 1.0);
    assert_eq!(spi.matrices["RSTATUS"][3][1], 7.0);
    assert_eq!(&spi.matrices["RFE"][0][..3], &[1.0, 1.0, 3.0]);
    assert_eq!(&spi.matrices["RCLUSTER"][0], &[1.0, 1.0, 8.0, 0.0]);
    assert_eq!(&spi.matrices["RSUBSET"][0], &[1.0, 8.0, 1.0, 1.0]);
    assert_eq!(spi.matrices["RFIT"][20][0], 1.0);
    assert_eq!(spi.matrices["RFIT"][21][0], 32.0);
    assert_eq!(spi.matrices["RFIT"][22][0], 32.0);
    assert_eq!(spi.matrices["RFIT"][23][0], 1.0);
    for observation in 1..=32 {
        assert_eq!(spi.output(7, observation), Some(1.0));
        assert_eq!(spi.output(11, observation), Some(0.0));
        let eta = spi.output(9, observation).expect("eta");
        let xb = spi.output(12, observation).expect("xb");
        let absorbed = spi.output(13, observation).expect("absorbed");
        let saved = spi.output(14, observation).expect("saved FE");
        assert!((eta - xb - absorbed).abs() < 1e-10);
        assert!((saved - absorbed).abs() < 1e-10);
    }
    assert_eq!(
        spi.stores.last(),
        Some(&StoreEvent::Matrix(
            "RREC".to_owned(),
            1,
            PPML_RESULT_RECEIPT_COMPLETE_COLUMN,
            1.0,
        )),
    );
}

#[cfg(feature = "ffi")]
#[test]
fn numeric_loader_hint_is_adopted_by_ppml_generation_zero() {
    const CHILD_ENV: &str = "FEREG_PPML_LOAD_HINT_ADOPTION_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD_ENV, "1")
            .arg("ppml_estimate::tests::numeric_loader_hint_is_adopted_by_ppml_generation_zero")
            .args(["--exact", "--nocapture", "--test-threads=1"])
            .status()
            .expect("isolated PPML load-hint adoption check");
        assert!(status.success());
        return;
    }
    static PREPARATION_ENDS: AtomicU64 = AtomicU64::new(0);
    static GENERATION_EVENTS: AtomicU64 = AtomicU64::new(u64::MAX);
    fn probe(phase: PpmlPhase, event: fereg_core::ppml::PpmlPhaseAllocatorEvent) -> u64 {
        let peak = crate::ffi::ppml_phase_allocation_probe(phase, event);
        if phase == PpmlPhase::Preparation
            && event == fereg_core::ppml::PpmlPhaseAllocatorEvent::End
            && PREPARATION_ENDS.fetch_add(1, Ordering::Relaxed) == 1
        {
            // Input selection is the first Preparation window; initial
            // generation construction is the second, before IRLS workspaces.
            GENERATION_EVENTS.store(
                crate::ffi::test_ppml_phase_allocation_events(phase),
                Ordering::Relaxed,
            );
        }
        peak
    }
    let (mut args, mut spi) = categorical_absorb_cluster_weight_fixture();
    let initial = spi.nobs;
    for row in initial + 1..=16_384 {
        for variable in 1..=u32::try_from(spi.nvars).unwrap() {
            let value = spi.numeric[&(variable, (row - 1) % initial + 1)];
            spi.numeric.insert((variable, row), value);
        }
    }
    spi.nobs = 16_384;
    spi.numeric_rows_enabled = true;
    replace_argument(&mut args, "threads", "threads=4".to_owned());
    let mut action = EstimatePpmlSpec::parse(&args)
        .unwrap()
        .decode_and_validate(&mut spi)
        .unwrap();
    let mut memory = crate::input_memory::InputMemory::new(None, 0).unwrap();
    let mut hint = None;
    let loaded = crate::estimate::load_ols_input_with_action_team(
        &action.request,
        action.candidate_all,
        action.input_complete,
        &mut memory,
        &mut spi,
        action.linear.execution.threads,
        Some(&mut hint),
    )
    .unwrap();
    assert!(
        hint.is_some(),
        "the production numeric loader must produce the capsule"
    );
    assert_eq!(spi.first_numeric_width, 2);
    let sources = PreparedOlsLoadHint::source_ids(&action.request)
        .unwrap()
        .len() as u64;
    let input = LoadedPpmlInput {
        bias_units: None,
        ols: loaded,
        adjustment: None,
        adjustment_kind: action.adjustment.kind(),
    };
    // Isolate the allocation witness from worker startup and work scheduling.
    // The same generation-zero adoption follows a real four-worker load.
    action.linear.execution.threads = ThreadConfig::from_requested(1, false).unwrap();
    let request = prepared_ppml_request(&action, Some(probe)).unwrap();
    let run = |hint| {
        PREPARATION_ENDS.store(0, Ordering::Relaxed);
        GENERATION_EVENTS.store(u64::MAX, Ordering::Relaxed);
        // This direct-core allocation-count witness has no plugin action to
        // establish its allocator baseline. Exclude the existing fake SPI,
        // fixture owners and prior comparison result, then count this call's
        // cloned owners and workspaces. Complete action memory coverage is
        // checked separately by the saved-FE phase-memory witness below.
        let _ = crate::ffi::begin_allocation_observation();
        let result = fit_prepared_ppml_interruptible(
            request.clone(),
            (input.clone(), hint),
            &mut |_| Ok(()),
        )
        .unwrap();
        let events = GENERATION_EVENTS.load(Ordering::Relaxed);
        assert_ne!(events, u64::MAX, "generation-zero preparation was observed");
        (result, events)
    };
    drop(run(None));
    let (baseline, baseline_events) = run(None);
    let (adopted, adopted_events) = run(hint);
    // Each cached source avoids at least its codes and levels allocations.
    // Both routes allocate the same fresh source descriptor Vec; the consumed
    // hint descriptor Vec was allocated before this phase, during loading.
    // Discarding/recomputing a capsule cannot satisfy this positive witness.
    assert!(
        adopted_events + 2 * sources <= baseline_events,
        "generation-zero allocations: baseline={baseline_events}, hinted={adopted_events}"
    );
    assert_eq!(adopted.coefficients(), baseline.coefficients());
    let exact_inference = |mut report: fereg_core::ppml::PpmlInferenceResult| {
        // Include exact omitted-coordinate NaN payloads as well as every
        // remaining inference field. Structural equality alone rejects NaN.
        let bits = [
            std::mem::take(&mut report.vce.covariance),
            std::mem::take(&mut report.vce.standard_errors),
            std::mem::take(&mut report.vce.z_statistics),
        ]
        .map(|values| values.into_iter().map(f64::to_bits).collect::<Vec<_>>());
        (report, bits)
    };
    assert_eq!(
        exact_inference(adopted.inference().clone()),
        exact_inference(baseline.inference().clone()),
    );
    assert_eq!(adopted.sample(), baseline.sample());
    assert_eq!(adopted.certificate(), baseline.certificate());
    assert_eq!(adopted.memory().plan, baseline.memory().plan);
}

#[cfg(feature = "ffi")]
#[test]
fn saved_fe_memory_covers_measured_final_phases_across_repeated_calls() {
    const CHILD_ENV: &str = "FEREG_PPML_SAVED_FE_MEMORY_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD_ENV, "1")
            .arg("ppml_estimate::tests::saved_fe_memory_covers_measured_final_phases_across_repeated_calls")
            .args(["--exact", "--nocapture", "--test-threads=1"])
            .status().expect("isolated saved-FE allocator check");
        assert!(status.success());
        return;
    }
    // Repeat an independently certified factor/FW/cluster fixture at an
    // axis large enough to overlap owned factor preparation and enter parallel
    // recovery. Allocate the fake SPI
    // cells and event log before the measured action, as the real SPI
    // stores do not allocate Rust observation storage.
    for (mode, threads) in [("fast", 1), ("low", 4), ("fast", 1)] {
        let (mut args, mut spi) = categorical_absorb_cluster_weight_fixture();
        let initial = spi.nobs;
        let rows = 16_384;
        for row in initial + 1..=rows {
            for variable in 1..=u32::try_from(spi.nvars).unwrap() {
                let original = spi.numeric[&(variable, (row - 1) % initial + 1)];
                spi.numeric.insert((variable, row), original);
            }
        }
        spi.nobs = rows;
        spi.numeric_rows_enabled = true;
        spi.stores = Vec::with_capacity(rows * 8 + 1_024);
        replace_argument(&mut args, "memory", format!("memory={mode}"));
        replace_argument(&mut args, "threads", format!("threads={threads}"));
        args.push("allocation_audit=1".to_owned());
        estimate_ppml(&args, &mut spi).expect("saved-FE phase memory fit");
        if threads > 1 {
            assert!(spi.row_reads > 0);
            // The ordinary factor h and FE g are prepared before the remaining
            // outcome, regressor, frequency weight and cluster-only source.
            assert_eq!(spi.first_numeric_width, 2);
        }
        let runtime = &spi.matrices["RRUNTIME"];
        for phase in [
            PpmlPhase::Preparation,
            PpmlPhase::Inference,
            PpmlPhase::FeRecovery,
            PpmlPhase::Publication,
        ] {
            let modeled = runtime[82 + phase as usize][0];
            let observed = runtime[95 + phase as usize][0];
            assert!(crate::ppml_result::is_stata_publishable(modeled));
            assert!(crate::ppml_result::is_stata_publishable(observed));
            assert!(
                observed <= modeled,
                "{mode}/{threads} {}: observed={observed}, modeled={modeled}",
                phase.name()
            );
            eprintln!(
                "saved_fe_memory {mode}/{threads} {} observed={observed} modeled={modeled}",
                phase.name()
            );
        }
        assert_eq!(spi.receipt_complete(), 1.0);
    }
}

#[test]
fn active_factor_level_that_vanishes_retains_a_zero_numerical_recipe() {
    let (mut args, mut spi) = categorical_absorb_cluster_weight_fixture();
    // The original wire declares h==1 as an active factor slot.  Leave
    // that level on exactly one candidate observation, then make its
    // outcome missing.  The final prepared sample contains only h==0,
    // so core must preserve the active original recipe as a zero column
    // classified by the existing numerical rank machinery.
    for observation in 1..=spi.nobs {
        spi.numeric.insert((4, observation), (0.0, false));
    }
    spi.numeric.insert((4, 1), (1.0, false));
    spi.numeric.insert((2, 1), (0.0, true));
    replace_argument(&mut args, "input_complete", "input_complete=0".to_owned());

    estimate_ppml(&args, &mut spi).expect("rebased categorical hidden PPML estimate");
    assert_eq!(spi.receipt_complete(), 1.0);
    assert_eq!(spi.matrices["RSTATUS"][1][1], 2.0);
    assert_eq!(spi.matrices["RSTATUS"][2], [3.0, 6.0, 0.0, 1.0]);
    assert_eq!(spi.matrices["RREC"][0][8], 31.0);
}

#[test]
fn saved_fe_recovery_uses_the_accepted_hessian_for_nonexact_counts() {
    let (args, mut spi) = categorical_absorb_cluster_weight_fixture();
    // Unlike the exact exponential fixture, these positive integer counts
    // do not lie exactly on the requested log-linear surface.  The final
    // strict update therefore moves eta/mu and exercises the requirement
    // that saved-FE recovery, inference bread, and the published state all
    // use the same accepted q*mu Hessian frame.
    for observation in 1..=spi.nobs {
        let count = 1.0 + ((observation * 7 + observation / 4) % 5) as f64;
        spi.numeric.insert((2, observation), (count, false));
    }

    estimate_ppml(&args, &mut spi).expect("nonexact-count saved-FE estimate");
    assert_eq!(spi.receipt_complete(), 1.0);
    assert_eq!(spi.matrices["RREC"][0][17], 1.0);
    for observation in 1..=spi.nobs {
        let absorbed = spi.output(13, observation).expect("absorbed predictor");
        let saved = spi.output(14, observation).expect("saved fixed effect");
        assert!((saved - absorbed).abs() < 1e-9);
    }
}

#[test]
fn malformed_wire_and_output_conflicts_fail_before_observation_reads() {
    for (key, value, expected) in [
        ("request_major", "99", ErrorCode::ProtocolMismatch),
        ("result_minor", "99", ErrorCode::ProtocolMismatch),
        ("output_sample", "2", ErrorCode::InvalidArgument),
        ("residual", "none", ErrorCode::Syntax),
        ("residual", "bogus", ErrorCode::Syntax),
        ("solver", "cg", ErrorCode::Syntax),
    ] {
        let (mut args, mut spi) = fixture();
        replace_argument(&mut args, key, format!("{key}={value}"));
        let error = estimate_ppml(&args, &mut spi).expect_err("must reject");
        assert_eq!(error.code, expected, "{key}");
        assert_eq!(spi.reads, 0, "{key}");
        assert_eq!(spi.store_calls, 0, "{key}");
        assert_eq!(spi.receipt_complete(), 0.0, "{key}");
    }
}

#[test]
fn finite_valid_unrepresentable_result_fails_r430_atomically() {
    let (args, mut spi) = fixture();
    // Each input is finite, nonnegative, and below Stata's missing-value
    // threshold, but their fitted aggregate cannot be represented on the
    // result wire.  Numerical publication failure must never be relabeled
    // malformed data or leak a partial transaction.
    for observation in 1..=spi.nobs {
        spi.numeric.insert((2, observation), (5.5e307, false));
    }
    let error =
        estimate_ppml(&args, &mut spi).expect_err("unrepresentable finite PPML result must fail");
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert_eq!(spi.store_calls, 0);
    assert_eq!(spi.receipt_complete(), 0.0);
}

#[test]
fn stale_receipt_interrupt_and_panic_leave_complete_zero() {
    let (args, mut stale) = fixture();
    stale.matrices.get_mut("RREC").expect("receipt")[0][0] = 1.0;
    assert!(estimate_ppml(&args, &mut stale).is_err());
    assert_eq!(stale.reads, 0);
    assert_eq!(stale.store_calls, 0);
    assert_eq!(stale.receipt_complete(), 0.0);

    let (args, mut interrupted) = fixture();
    interrupted.interrupt_at = Some(1);
    let error = estimate_ppml(&args, &mut interrupted).expect_err("interrupted");
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert_eq!(interrupted.store_calls, 0);
    assert_eq!(interrupted.receipt_complete(), 0.0);

    let (mut args, mut panicked) = fixture();
    args.push("inject=panic".to_owned());
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _ = estimate_ppml(&args, &mut panicked);
    }));
    assert!(outcome.is_err());
    assert_eq!(panicked.reads, 0);
    assert_eq!(panicked.store_calls, 0);
    assert_eq!(panicked.receipt_complete(), 0.0);
}

#[test]
fn native_break_marker_is_emitted_after_core_entry_and_interrupts_before_stores() {
    let (mut args, mut spi) = fixture();
    args.push("inject=breaknative".to_owned());
    let marker_token = format!(
        "{:032x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    );
    let marker_path = crate::developer_injection::native_break_marker_path("feppml", &marker_token);
    args.push(format!("break_marker_token={marker_token}"));
    spi.interrupt_when_marker_exists = Some(marker_path.clone());

    let error = estimate_ppml(&args, &mut spi)
        .expect_err("native Break injection must stop inside the core");
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert_eq!(
        spi.displays,
        [crate::developer_injection::PPML_NATIVE_BREAK_MARKER]
    );
    assert_eq!(
        std::fs::read_to_string(&marker_path).expect("native marker file"),
        crate::developer_injection::PPML_NATIVE_BREAK_MARKER
    );
    std::fs::remove_file(&marker_path).expect("remove native marker file");
    assert!(
        spi.reads > 0,
        "the marker must follow complete native input loading"
    );
    assert_eq!(spi.store_calls, 0);
    assert!(spi.stores.is_empty());
    assert_eq!(spi.receipt_complete(), 0.0);
}

#[test]
fn native_break_rejects_missing_or_path_like_marker_tokens_before_reads() {
    let (mut missing_args, mut missing_spi) = fixture();
    missing_args.push("inject=breaknative".to_owned());
    let missing = estimate_ppml(&missing_args, &mut missing_spi)
        .expect_err("breaknative without a token must fail closed");
    assert_eq!(missing.code, ErrorCode::Syntax);
    assert_eq!(missing_spi.reads, 0);

    let (mut path_args, mut path_spi) = fixture();
    path_args.push("inject=breaknative".to_owned());
    path_args.push("break_marker_token=/tmp/arbitrary.marker".to_owned());
    let path = estimate_ppml(&path_args, &mut path_spi)
        .expect_err("a path must never cross the hidden token wire");
    assert_eq!(path.code, ErrorCode::Syntax);
    assert_eq!(path_spi.reads, 0);
}

#[test]
fn licensed_allocation_hook_reaches_fallible_fe_recovery_and_is_action_local() {
    let (mut args, mut injected) = categorical_absorb_cluster_weight_fixture();
    args.push("inject=allocation".to_owned());
    let error = estimate_ppml(&args, &mut injected)
        .expect_err("FE-recovery reservation injection must fail");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(error.return_code(), 909);
    assert!(error.message.contains("complete FE recovery mask"));
    assert_eq!(injected.store_calls, 0);
    assert!(injected.stores.is_empty());
    assert_eq!(injected.receipt_complete(), 0.0);

    let (args, mut following) = categorical_absorb_cluster_weight_fixture();
    estimate_ppml(&args, &mut following)
        .expect("allocation injection must not leak into the next action");
    assert_eq!(following.receipt_complete(), 1.0);
}

#[test]
#[cfg(feature = "ffi")]
fn publication_through_complete_has_no_native_allocation() {
    let _guard = crate::instrumentation::lock_action_observation();
    with_unpublished_shadow(|action, shadow, spi| {
        spi.record_stores = false;
        let memory_report = MemoryReport::partial(
            Baseline::default(),
            None,
            crate::action_memory::Ledger::new(crate::action_memory::Estimator::Ppml).summary(),
            0,
        )
        .unwrap();
        let before = crate::ffi::allocation_usage();
        precommit(action, shadow, spi).unwrap();
        let validated = crate::ffi::allocation_usage();
        store_shadow_prefix(action, shadow, spi).unwrap();
        let stored = crate::ffi::allocation_usage();
        store_runtime_and_complete(action, shadow, &memory_report, spi).unwrap();
        let after = crate::ffi::allocation_usage();
        assert_eq!(
            validated.events, before.events,
            "publication validation allocated"
        );
        assert_eq!(
            stored.events, validated.events,
            "publication prefix allocated"
        );
        assert_eq!(
            after.events, stored.events,
            "publication completion allocated"
        );
        assert_eq!(after.current_bytes, before.current_bytes);
        assert_eq!(spi.receipt_complete(), 1.0);
    });
}

#[test]
fn publication_interruptions_at_output_runtime_and_complete_never_commit() {
    let (args, mut success) = fixture();
    estimate_ppml(&args, &mut success).unwrap();
    for ordinal in [1, success.store_calls - RUNTIME_ROWS, success.store_calls] {
        let (args, mut spi) = fixture();
        spi.interrupt_store_at = Some(ordinal);
        assert_eq!(
            estimate_ppml(&args, &mut spi).unwrap_err().code,
            ErrorCode::Interrupted
        );
        assert_eq!(spi.receipt_complete(), 0.0);
        assert_eq!(spi.store_calls, ordinal);
    }
    let (args, mut following) = fixture();
    estimate_ppml(&args, &mut following).unwrap();
    assert_eq!(following.receipt_complete(), 1.0);
}

#[test]
fn every_ppml_store_failure_leaves_receipt_incomplete() {
    let (args, mut successful) = fixture();
    estimate_ppml(&args, &mut successful).expect("reference stores");
    let store_count = successful.store_calls;
    assert!(store_count > PPML_RESULT_RECEIPT_COLUMNS);
    for failing_ordinal in 1..=store_count {
        let (args, mut spi) = fixture();
        spi.fail_store_at = Some(failing_ordinal);
        assert!(estimate_ppml(&args, &mut spi).is_err());
        assert_eq!(spi.receipt_complete(), 0.0, "failure {failing_ordinal}");
    }
}

fn zero_matrix(rows: usize, columns: usize) -> Vec<Vec<f64>> {
    vec![vec![0.0; columns]; rows]
}

fn registry(values: &[String]) -> String {
    if values.is_empty() {
        return ".".to_owned();
    }
    values
        .iter()
        .map(|value| format!("{}:{value}", value.len()))
        .collect::<Vec<_>>()
        .join("|")
}

fn replace_argument(args: &mut [String], key: &str, replacement: String) {
    let prefix = format!("{key}=");
    args.iter_mut()
        .find(|argument| argument.starts_with(&prefix))
        .unwrap_or_else(|| panic!("missing fixture argument {key}"))
        .clone_from(&replacement);
}
