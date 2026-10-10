use super::*;
use fereg_core::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledModelSpecification, CompiledTermGraph,
    ComponentKind, ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, ModelWireV2,
    RawTermKind, SourceEntry, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use fereg_core::protocol::{ACTION_PROTOCOL_VERSION, ADO_VERSION, MODEL_WIRE_VERSION};
#[cfg(feature = "ffi")]
use fereg_core::{
    fit_prepared_model_clustered_with_loaded_moments, fit_prepared_model_robust_with_loaded_moments,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
enum StoreEvent {
    Numeric(u32, usize, f64),
    Matrix(u8, usize, usize, f64),
}

#[derive(Clone)]
struct FakeSpi {
    local_bundle: Option<Vec<u8>>,
    macro_reads: usize,
    nobs: usize,
    nvars: usize,
    data: BTreeMap<(u32, usize), (f64, bool)>,
    string_data: BTreeMap<(u32, usize), Vec<u8>>,
    matrices: BTreeMap<String, Vec<Vec<f64>>>,
    string_variables: BTreeSet<u32>,
    strl_variables: BTreeSet<u32>,
    numeric_reads: usize,
    numeric_rows_enabled: bool,
    numeric_column_stores_enabled: bool,
    numeric_column_batches: usize,
    numeric_batches: Vec<(usize, usize, Vec<u32>)>,
    store_calls: usize,
    fail_store_at: Option<usize>,
    interrupt_at: Option<usize>,
    interrupt_checks: usize,
    waiting_checks: usize,
    stores: Vec<StoreEvent>,
    store_error: Option<FeregError>,
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

    fn variable_is_strl(&mut self, variable: u32) -> Result<bool> {
        Ok(self.strl_variables.contains(&variable))
    }

    fn read_numeric(&mut self, variable: u32, observation: usize) -> Result<(f64, bool)> {
        self.numeric_reads += 1;
        self.data
            .get(&(variable, observation))
            .copied()
            .ok_or_else(|| invalid("test numeric cell is absent"))
    }

    fn read_numeric_columns(
        &mut self,
        variables: &[u32],
        first_observation: usize,
        columns: &mut [&mut [f64]],
        missing: &mut [&mut [bool]],
    ) -> Result<()> {
        self.numeric_batches
            .push((first_observation, columns[0].len(), variables.to_vec()));
        for offset in 0..columns[0].len() {
            for ((variable, column), flags) in variables
                .iter()
                .zip(columns.iter_mut())
                .zip(missing.iter_mut())
            {
                let (value, is_missing) =
                    self.read_numeric(*variable, first_observation + offset)?;
                column[offset] = value;
                flags[offset] = is_missing;
            }
        }
        Ok(())
    }

    fn read_string(&mut self, variable: u32, observation: usize) -> Result<Vec<u8>> {
        self.string_data
            .get(&(variable, observation))
            .cloned()
            .ok_or_else(|| invalid("test string cell is absent"))
    }

    fn matrix_dimensions(&mut self, name: &str) -> Result<(usize, usize)> {
        let matrix = self
            .matrices
            .get(name)
            .ok_or_else(|| invalid(format!("test matrix {name:?} is absent")))?;
        Ok((matrix.len(), matrix.first().map_or(0, Vec::len)))
    }

    fn matrix_element(&mut self, name: &str, row: usize, column: usize) -> Result<f64> {
        self.matrices
            .get(name)
            .and_then(|matrix| matrix.get(row.checked_sub(1)?))
            .and_then(|values| values.get(column.checked_sub(1)?))
            .copied()
            .ok_or_else(|| invalid("test matrix read is out of range"))
    }

    fn store_numeric(&mut self, variable: u32, observation: usize, value: f64) -> Result<()> {
        self.before_store()?;
        self.data.insert((variable, observation), (value, false));
        self.stores
            .push(StoreEvent::Numeric(variable, observation, value));
        Ok(())
    }

    fn try_store_numeric_columns(&mut self, columns: &[(u32, &[f64])]) -> Result<bool> {
        if !self.numeric_column_stores_enabled {
            return Ok(false);
        }
        self.numeric_column_batches += 1;
        for row in 0..columns[0].1.len() {
            for (position, values) in columns {
                self.store_numeric(*position, row + 1, values[row])?;
            }
        }
        Ok(true)
    }

    fn store_matrix(
        &mut self,
        name: &PreparedMatrixStoreName,
        row: usize,
        column: usize,
        value: f64,
    ) -> Result<()> {
        let name = name.as_str();
        self.before_store()?;
        let destination = self
            .matrices
            .get_mut(name)
            .and_then(|matrix| matrix.get_mut(row.checked_sub(1)?))
            .and_then(|values| values.get_mut(column.checked_sub(1)?))
            .ok_or_else(|| invalid("test matrix store is out of range"))?;
        *destination = value;
        self.stores.push(StoreEvent::Matrix(
            matrix_name_code(name),
            row,
            column,
            value,
        ));
        Ok(())
    }

    fn display(&mut self, _message: &str) -> Result<()> {
        Ok(())
    }

    fn check_interrupted_while_waiting(&mut self) -> Result<()> {
        self.waiting_checks += 1;
        Ok(())
    }

    fn check_interrupted(&mut self) -> Result<()> {
        self.interrupt_checks += 1;
        if self.interrupt_at == Some(self.interrupt_checks) {
            Err(FeregError::new(
                ErrorCode::Interrupted,
                "injected precommit interruption",
            ))
        } else {
            Ok(())
        }
    }
}

impl FakeSpi {
    fn before_store(&mut self) -> Result<()> {
        self.store_calls += 1;
        if self.fail_store_at == Some(self.store_calls) {
            Err(self
                .store_error
                .take()
                .expect("test store error was prepared before commit"))
        } else {
            Ok(())
        }
    }

    fn receipt_complete(&self) -> f64 {
        self.matrices["RREC"][0][RECEIPT_COMPLETE_COLUMN - 1]
    }
}

fn matrix_name_code(name: &str) -> u8 {
    match name {
        "RB" => 1,
        "RV" => 2,
        "RSTATUS" => 3,
        "RMARKB" => 4,
        "RMARKV" => 5,
        "RFIT" => 6,
        "RDOF" => 7,
        "RSOLVER" => 8,
        "RFE" => 9,
        "RCLUSTER" => 10,
        "RSUBSET" => 11,
        "RCERT" => 12,
        "RRUNTIME" => 13,
        "RREC" => 14,
        "RMEMORY" => 15,
        _ => panic!("unprepared test matrix-store name {name:?}"),
    }
}

fn continuous_term(source: SourceId) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(1).expect("term id"),
        order: TermOrder::new(1).expect("term order"),
        coefficient_slot: CoefficientSlot::new(1).expect("coefficient slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: "c.x".to_owned(),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component order"),
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

fn categorical_term(index: u32, source: SourceId, level: f64, base: bool) -> ExpandedTerm {
    let raw_operator = if base { "0b" } else { "1" };
    ExpandedTerm {
        id: TermId::new(index).expect("term id"),
        order: TermOrder::new(index).expect("term order"),
        coefficient_slot: CoefficientSlot::new(index).expect("coefficient slot"),
        raw_kind: RawTermKind::Factor,
        whole_column_omitted: base,
        canonical_token: format!("{raw_operator}.x"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component order"),
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
            raw_operator: raw_operator.to_owned(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn replace_argument(args: &mut [String], key: &str, replacement: String) {
    let prefix = format!("{key}=");
    args.iter_mut()
        .find(|argument| argument.starts_with(&prefix))
        .unwrap_or_else(|| panic!("missing fixture argument {key}"))
        .clone_from(&replacement);
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

fn bundled(args: &[String]) -> Vec<String> {
    let mut payload = format!("{}:", args.len());
    for argument in args {
        payload.push_str(&argument.len().to_string());
        payload.push(':');
        payload.push_str(argument);
    }
    vec![format!("bundle={payload}")]
}

fn zero_matrix(rows: usize, columns: usize) -> Vec<Vec<f64>> {
    vec![vec![0.0; columns]; rows]
}

fn coefficient_shadow() -> OlsShadowResult<'static> {
    OlsShadowResult {
        solver_workspace_peak: None,
        solver_expected_heap_peak: None,
        b: vec![2.0, 3.0],
        covariance: vec![4.0, 1.0, 1.0, 5.0],
        coefficient_status: vec![1.0, 1.0, 1.0, 1.0, 2.0, 5.0, 0.0, 2.0],
        omission_b_marker: vec![1.0, 1.0],
        omission_v_marker: vec![1.0, 0.0, 0.0, 1.0],
        fit: Vec::new(),
        dof: Vec::new(),
        solver: Vec::new(),
        fixed_effects: Vec::new(),
        clusters: Vec::new(),
        subsets: Vec::new(),
        certificates: Vec::new(),
        receipt: Vec::new(),
        coefficient_count: 2,
        coefficient_capacity: 2,
        covariance_dimension: 2,
        fixed_effect_rows: 0,
        cluster_rows: 0,
        subset_rows: 0,
        drop_reasons: &[],
        retained_rows: &[],
        residuals: &[],
        execution: fereg_core::PreparedOlsExecutionReport {
            mode: PreparedOlsExecutionMode::Fast,
            poolsize: 10,
            scratch_bytes: 0,
            requested_threads: 1,
            effective_threads: 1,
            deterministic: true,
        },
    }
}

#[test]
fn memory_baseline_and_matrix_are_validated_before_any_store() {
    for replacement in [
        "stata_alloc_before=-1",
        "stata_used_before=9007199254740993",
        "output_memory_matrix=RB",
    ] {
        let (mut args, mut spi) = fixture(OlsVceKind::Iid);
        let key = replacement.split_once('=').unwrap().0;
        *args
            .iter_mut()
            .find(|arg| arg.starts_with(&format!("{key}=")))
            .unwrap() = replacement.to_owned();
        assert!(estimate_ols_test(&args, &mut spi).is_err(), "{replacement}");
        assert_eq!(spi.store_calls, 0);
        assert_eq!(spi.receipt_complete(), 0.0);
    }
}

#[test]
fn partial_memory_report_is_published_without_claiming_combined_coverage() {
    let (args, mut spi) = fixture(OlsVceKind::Iid);
    estimate_ols_test(&args, &mut spi).unwrap();
    let report = &spi.matrices["RMEMORY"];
    assert_eq!(report[0], vec![1.0, 2.0]);
    assert_eq!(report[1], vec![1.0, 1.0]);
    assert_eq!(report[3], vec![0.0, 0.0]);
    assert_eq!(report[7], vec![0.0, 0.0]);
    assert_eq!(report[8], vec![0.0, 0.0]);
    assert!(report[10][1] > 0.0);
    assert_eq!(spi.receipt_complete(), 1.0);
}

fn fixture(vce: OlsVceKind) -> (Vec<String>, FakeSpi) {
    let clustered = vce == OlsVceKind::Clustered;
    let sources = if clustered {
        vec![
            SourceEntry {
                id: SourceId::new(1).expect("x source"),
                canonical_name: "x".to_owned(),
            },
            SourceEntry {
                id: SourceId::new(2).expect("cluster source"),
                canonical_name: "g".to_owned(),
            },
        ]
    } else {
        vec![SourceEntry {
            id: SourceId::new(1).expect("x source"),
            canonical_name: "x".to_owned(),
        }]
    };
    let wire = ModelWireV2::from_graph(&CompiledTermGraph {
        sources,
        terms: vec![continuous_term(SourceId::new(1).expect("x source"))],
    })
    .expect("valid model wire");
    let binding_count = if clustered { 4 } else { 3 };
    let sample_position = if clustered { 5 } else { 4 };
    let residual_position = sample_position + 1;
    let reason_position = sample_position + 2;
    let mut bindings = vec![
        vec![1.0, 1.0, 0.0],
        vec![2.0, 2.0, 0.0],
        vec![3.0, 3.0, 1.0],
    ];
    if clustered {
        bindings.push(vec![4.0, 3.0, 2.0]);
    }
    let mut matrices = BTreeMap::from([
        ("S".to_owned(), wire.sources.rows),
        ("T".to_owned(), wire.terms.rows),
        ("C".to_owned(), wire.components.rows),
        ("A".to_owned(), wire.absorb_terms.rows),
        ("AC".to_owned(), wire.absorb_components.rows),
        ("B".to_owned(), bindings),
        (
            "CT".to_owned(),
            if clustered {
                vec![vec![1.0, 1.0]]
            } else {
                zero_matrix(1, CLUSTER_TERM_ROW_WIDTH)
            },
        ),
        (
            "CC".to_owned(),
            if clustered {
                vec![vec![1.0, 1.0, 2.0]]
            } else {
                zero_matrix(1, CLUSTER_COMPONENT_ROW_WIDTH)
            },
        ),
        ("RB".to_owned(), zero_matrix(1, 2)),
        ("RV".to_owned(), zero_matrix(2, 2)),
        (
            "RSTATUS".to_owned(),
            zero_matrix(2, COEFFICIENT_STATUS_COLUMNS),
        ),
        ("RMARKB".to_owned(), zero_matrix(1, 2)),
        ("RMARKV".to_owned(), zero_matrix(2, 2)),
        ("RFIT".to_owned(), zero_matrix(FIT_ROWS, 2)),
        ("RDOF".to_owned(), zero_matrix(DOF_ROWS, 2)),
        (
            "RSOLVER".to_owned(),
            zero_matrix(SOLVER_ROWS, SOLVER_COLUMNS),
        ),
        ("RFE".to_owned(), zero_matrix(1, FE_COLUMNS)),
        ("RCLUSTER".to_owned(), zero_matrix(1, CLUSTER_COLUMNS)),
        ("RSUBSET".to_owned(), zero_matrix(1, SUBSET_COLUMNS)),
        ("RCERT".to_owned(), zero_matrix(CERTIFICATE_ROWS, 2)),
        ("RMEMORY".to_owned(), zero_matrix(MEMORY_ROWS, 2)),
        ("RRUNTIME".to_owned(), zero_matrix(RUNTIME_ROWS, 2)),
        ("RREC".to_owned(), zero_matrix(1, RECEIPT_COLUMNS)),
    ]);
    let mut data = BTreeMap::new();
    for (offset, (y, x, g)) in [
        (1.0, 0.0, 1.0),
        (3.0, 1.0, 1.0),
        (2.0, 2.0, 2.0),
        (7.0, 3.0, 2.0),
        (4.0, 4.0, 3.0),
        (8.0, 5.0, 3.0),
    ]
    .into_iter()
    .enumerate()
    {
        let observation = offset + 1;
        data.insert((1, observation), (1.0, false));
        data.insert((2, observation), (y, false));
        data.insert((3, observation), (x, false));
        if clustered {
            data.insert((4, observation), (g, false));
        }
        data.insert((sample_position, observation), (1.0, false));
    }
    let args = vec![
        format!("ado={ADO_VERSION}"),
        format!("protocol_major={}", ACTION_PROTOCOL_VERSION.major),
        format!("protocol_minor={}", ACTION_PROTOCOL_VERSION.minor),
        "stata=18.0".to_owned(),
        format!("confirm={CONFIRMATION}"),
        format!("plugin={PLUGIN_VERSION}"),
        format!("result_major={}", OLS_RESULT_WIRE_VERSION.major),
        format!("result_minor={}", OLS_RESULT_WIRE_VERSION.minor),
        format!("model_major={}", MODEL_WIRE_VERSION.major),
        format!("model_minor={}", MODEL_WIRE_VERSION.minor),
        "source_matrix=S".to_owned(),
        format!("source_count={}", wire.source_names.len()),
        "term_matrix=T".to_owned(),
        "term_count=1".to_owned(),
        "component_matrix=C".to_owned(),
        "component_count=1".to_owned(),
        format!("source_names={}", registry(&wire.source_names)),
        format!("canonical_tokens={}", registry(&wire.canonical_tokens)),
        format!(
            "raw_component_operators={}",
            registry(&wire.raw_component_operators)
        ),
        format!(
            "raw_time_series_operators={}",
            registry(&wire.raw_time_series_operators)
        ),
        "absorb_term_matrix=A".to_owned(),
        "absorb_term_count=0".to_owned(),
        "absorb_component_matrix=AC".to_owned(),
        "absorb_component_count=0".to_owned(),
        "absorb_labels=.".to_owned(),
        "absorb_saved_names=.".to_owned(),
        "cluster_term_matrix=CT".to_owned(),
        format!("cluster_term_count={}", usize::from(clustered)),
        "cluster_component_matrix=CC".to_owned(),
        format!("cluster_component_count={}", usize::from(clustered)),
        if clustered {
            "cluster_labels=1:g".to_owned()
        } else {
            "cluster_labels=.".to_owned()
        },
        "binding_matrix=B".to_owned(),
        format!("binding_count={binding_count}"),
        "include_intercept=1".to_owned(),
        "candidate_all=0".to_owned(),
        "input_complete=0".to_owned(),
        "keep_singletons=0".to_owned(),
        format!(
            "vce={}",
            match vce {
                OlsVceKind::Iid => "iid",
                OlsVceKind::Robust => "robust",
                OlsVceKind::Clustered => "cluster",
            }
        ),
        "dof=pairwise".to_owned(),
        format!("account_cluster_nesting={}", usize::from(clustered)),
        "memory=fast".to_owned(),
        "solver=auto".to_owned(),
        "preconditioner=auto".to_owned(),
        "poolsize=10".to_owned(),
        "threads=1".to_owned(),
        "deterministic=1".to_owned(),
        "tolerance=1e-8".to_owned(),
        "iterate=16000".to_owned(),
        format!("output_sample={sample_position}"),
        format!("output_residual={residual_position}"),
        format!("output_drop_reason={reason_position}"),
        "output_xb=0".to_owned(),
        "output_xbd=0".to_owned(),
        "output_absorbed=0".to_owned(),
        "output_saved_count=0".to_owned(),
        "output_saved_channels=.".to_owned(),
        "output_b_matrix=RB".to_owned(),
        "output_v_matrix=RV".to_owned(),
        "output_status_matrix=RSTATUS".to_owned(),
        "output_marker_b_matrix=RMARKB".to_owned(),
        "output_marker_v_matrix=RMARKV".to_owned(),
        "output_fit_matrix=RFIT".to_owned(),
        "output_dof_matrix=RDOF".to_owned(),
        "output_solver_matrix=RSOLVER".to_owned(),
        "output_fe_matrix=RFE".to_owned(),
        "output_cluster_matrix=RCLUSTER".to_owned(),
        "output_subset_matrix=RSUBSET".to_owned(),
        "output_certificate_matrix=RCERT".to_owned(),
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
            nobs: 6,
            nvars: binding_count + 3,
            data,
            string_data: BTreeMap::new(),
            matrices: std::mem::take(&mut matrices),
            string_variables: BTreeSet::new(),
            strl_variables: BTreeSet::new(),
            numeric_reads: 0,
            numeric_rows_enabled: false,
            numeric_column_stores_enabled: false,
            numeric_column_batches: 0,
            numeric_batches: Vec::new(),
            store_calls: 0,
            fail_store_at: None,
            interrupt_at: None,
            interrupt_checks: 0,
            waiting_checks: 0,
            // The allocation-sensitive fake path must not grow this log
            // after the first output store.
            stores: Vec::with_capacity(512),
            store_error: Some(FeregError::new(
                ErrorCode::Internal,
                "injected SPI output-store failure",
            )),
        },
    )
}

fn saved_output_fixture() -> (Vec<String>, FakeSpi) {
    let (mut args, mut spi) = fixture(OlsVceKind::Iid);
    let x_source = SourceEntry {
        id: SourceId::new(1).expect("x source"),
        canonical_name: "x".to_owned(),
    };
    let group_source = SourceEntry {
        id: SourceId::new(2).expect("group source"),
        canonical_name: "g".to_owned(),
    };
    let wire = ModelWireV2::from_specification(&CompiledModelSpecification {
        model: CompiledTermGraph {
            sources: vec![x_source.clone(), group_source.clone()],
            terms: vec![continuous_term(x_source.id)],
        },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("absorbed term"),
            intercept: true,
            categorical_sources: vec![group_source.id],
            slope_sources: Vec::new(),
            label: "g".to_owned(),
            saved_name: "fe".to_owned(),
        }],
    })
    .expect("saved-output wire");
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
        "B".to_owned(),
        vec![
            vec![1.0, 1.0, 0.0],
            vec![2.0, 2.0, 0.0],
            vec![3.0, 3.0, 1.0],
            vec![4.0, 3.0, 2.0],
        ],
    );
    for (observation, group) in [1.0, 1.0, 2.0, 2.0, 3.0, 3.0].into_iter().enumerate() {
        spi.data.insert((4, observation + 1), (group, false));
        spi.data.insert((5, observation + 1), (1.0, false));
    }
    spi.nvars = 11;

    replace_argument(&mut args, "source_count", "source_count=2".to_owned());
    replace_argument(
        &mut args,
        "source_names",
        format!("source_names={}", registry(&wire.source_names)),
    );
    replace_argument(
        &mut args,
        "absorb_term_count",
        "absorb_term_count=1".to_owned(),
    );
    replace_argument(
        &mut args,
        "absorb_component_count",
        "absorb_component_count=1".to_owned(),
    );
    replace_argument(
        &mut args,
        "absorb_labels",
        format!("absorb_labels={}", registry(&wire.absorb_labels)),
    );
    replace_argument(
        &mut args,
        "absorb_saved_names",
        format!("absorb_saved_names={}", registry(&wire.absorb_saved_names)),
    );
    replace_argument(&mut args, "binding_count", "binding_count=4".to_owned());
    replace_argument(&mut args, "output_sample", "output_sample=5".to_owned());
    replace_argument(&mut args, "output_residual", "output_residual=6".to_owned());
    replace_argument(
        &mut args,
        "output_drop_reason",
        "output_drop_reason=7".to_owned(),
    );
    replace_argument(&mut args, "output_xb", "output_xb=8".to_owned());
    replace_argument(&mut args, "output_xbd", "output_xbd=9".to_owned());
    replace_argument(
        &mut args,
        "output_absorbed",
        "output_absorbed=10".to_owned(),
    );
    replace_argument(
        &mut args,
        "output_saved_count",
        "output_saved_count=1".to_owned(),
    );
    replace_argument(
        &mut args,
        "output_saved_channels",
        "output_saved_channels=2:11".to_owned(),
    );
    (args, spi)
}

fn stored_numeric(spi: &FakeSpi, variable: u32, observation: usize) -> f64 {
    spi.stores
        .iter()
        .find_map(|event| match event {
            StoreEvent::Numeric(found_variable, found_observation, value)
                if (*found_variable, *found_observation) == (variable, observation) =>
            {
                Some(*value)
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            spi.data
                .get(&(variable, observation))
                .map(|(value, _)| *value)
                .unwrap_or_else(|| {
                    panic!(
                        "missing stored value for variable {variable}, observation {observation}"
                    )
                })
        })
}

fn factor_scatter_fixture() -> (Vec<String>, FakeSpi) {
    let (mut args, mut spi) = fixture(OlsVceKind::Robust);
    let source = SourceEntry {
        id: SourceId::new(1).expect("factor source"),
        canonical_name: "x".to_owned(),
    };
    let wire = ModelWireV2::from_graph(&CompiledTermGraph {
        sources: vec![source.clone()],
        terms: vec![
            categorical_term(1, source.id, 0.0, true),
            categorical_term(2, source.id, 1.0, false),
        ],
    })
    .expect("factor model wire");
    spi.matrices.insert("T".to_owned(), wire.terms.rows);
    spi.matrices.insert("C".to_owned(), wire.components.rows);
    spi.matrices.insert("RB".to_owned(), zero_matrix(1, 3));
    spi.matrices.insert("RV".to_owned(), zero_matrix(3, 3));
    spi.matrices.insert(
        "RSTATUS".to_owned(),
        zero_matrix(3, COEFFICIENT_STATUS_COLUMNS),
    );
    spi.matrices.insert("RMARKB".to_owned(), zero_matrix(1, 3));
    spi.matrices.insert("RMARKV".to_owned(), zero_matrix(3, 3));
    for observation in 1..=6 {
        spi.data.insert(
            (3, observation),
            (if observation % 2 == 0 { 0.0 } else { 1.0 }, false),
        );
    }
    replace_argument(&mut args, "term_count", "term_count=2".to_owned());
    replace_argument(&mut args, "component_count", "component_count=2".to_owned());
    replace_argument(
        &mut args,
        "canonical_tokens",
        format!("canonical_tokens={}", registry(&wire.canonical_tokens)),
    );
    replace_argument(
        &mut args,
        "raw_component_operators",
        format!(
            "raw_component_operators={}",
            registry(&wire.raw_component_operators)
        ),
    );
    replace_argument(
        &mut args,
        "raw_time_series_operators",
        format!(
            "raw_time_series_operators={}",
            registry(&wire.raw_time_series_operators)
        ),
    );
    (args, spi)
}

#[test]
#[cfg(feature = "ffi")]
fn prepared_owner_accounting_matches_independent_allocator_deallocation() {
    use fereg_core::data::WeightKind;
    let _guard = instrumentation::lock_action_observation();
    fn check<T>(owner: T, bytes: u128) {
        let before = crate::ffi::allocation_usage();
        drop(owner);
        let after = crate::ffi::allocation_usage();
        assert_eq!(
            u128::from(before.current_bytes - after.current_bytes),
            bytes
        );
    }
    fn check_shared<T>(owner: T, bytes: u128) {
        // Observe the complete shared allocation independently of the
        // prepared owner accounting, then release the sole Arc and buffers.
        let payload = std::mem::size_of_val(&owner);
        let before = crate::ffi::allocation_usage();
        let owner = std::sync::Arc::new(owner);
        let allocated = crate::ffi::allocation_usage().current_bytes - before.current_bytes;
        assert_eq!(
            allocated as usize,
            payload + 2 * std::mem::size_of::<usize>()
        );
        eprintln!("prepared shared owner: payload={payload} allocation={allocated}");
        check(owner, bytes + u128::from(allocated));
    }
    for complex in [false, true] {
        for clustered in [false, true] {
            for weight in [
                None,
                Some(WeightKind::Analytic),
                Some(WeightKind::Frequency),
                Some(WeightKind::Probability),
            ] {
                let (args, mut spi) = saved_output_fixture();
                let mut action = EstimateActionSpec::parse(&args)
                    .unwrap()
                    .decode_with_memory_limit(&mut spi, || Ok(None))
                    .unwrap();
                let mut loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
                action.request.keep_singletons = true;
                action.request.absorbed_terms[0].saved_name.clear();
                if complex {
                    action.request.absorbed_terms[0]
                        .categorical_sources
                        .push(SourceId::new(1).unwrap());
                    action.request.absorbed_terms[0]
                        .slope_sources
                        .push(SourceId::new(1).unwrap());
                }
                let mut repeated = action.request.absorbed_terms[0].clone();
                repeated.id = AbsorbTermId::new(2).unwrap();
                repeated.label = "repeated".to_owned();
                action.request.absorbed_terms.push(repeated);
                if let Some(kind) = weight {
                    let column = LoadedNumericColumn {
                        values: vec![2.0; 6],
                        missing: vec![false; 6],
                    };
                    let role = match kind {
                        WeightKind::Analytic => {
                            loaded.analytic_weight = Some(column);
                            OlsInputRole::AnalyticWeight
                        }
                        WeightKind::Frequency => {
                            loaded.frequency_weight = Some(column);
                            OlsInputRole::FrequencyWeight
                        }
                        WeightKind::Probability => {
                            loaded.probability_weight = Some(column);
                            OlsInputRole::ProbabilityWeight
                        }
                        WeightKind::Importance => unreachable!("unsupported fixture weight"),
                    };
                    action.request.bindings.push(OlsInputBinding {
                        stata_var_position: 12,
                        role,
                    });
                }
                // Retained capacities deliberately exceed logical lengths.
                loaded.outcome.values.reserve(100);
                if clustered {
                    let request = OlsClusteredRequestV1 {
                        ols: action.request,
                        clusters: vec![
                            OlsClusterTermV1 {
                                label: "simple".to_owned(),
                                sources: vec![SourceId::new(2).unwrap()],
                            },
                            OlsClusterTermV1 {
                                label: "interaction".to_owned(),
                                sources: vec![SourceId::new(1).unwrap(), SourceId::new(2).unwrap()],
                            },
                        ],
                    };
                    if complex {
                        let owner =
                            prepare_model_owned(&request.ols, &request.clusters, loaded, true)
                                .unwrap();
                        let before = crate::ffi::allocation_usage();
                        let bytes = owner.owned_buffer_bytes();
                        assert_eq!(crate::ffi::allocation_usage().events, before.events);
                        check_shared(owner, bytes);
                    } else {
                        let owner =
                            prepare_model_owned(&request.ols, &request.clusters, loaded, false)
                                .unwrap();
                        let bytes = owner.owned_buffer_bytes();
                        check(owner, bytes);
                    }
                } else if complex {
                    let owner = prepare_model_owned(&action.request, &[], loaded, true).unwrap();
                    let bytes = owner.owned_buffer_bytes();
                    check_shared(owner, bytes);
                } else {
                    let mut loaded = loaded;
                    loaded.outcome.values.reserve(101);
                    let owner = prepare_model_owned(&action.request, &[], loaded, false).unwrap();
                    let before = crate::ffi::allocation_usage();
                    let bytes = owner.owned_buffer_bytes();
                    assert_eq!(crate::ffi::allocation_usage().events, before.events);
                    check(owner, bytes);
                }
            }
        }
    }
}

#[test]
#[cfg(feature = "ffi")]
fn inference_owner_accounting_matches_allocator_in_all_six_routes() {
    let _guard = instrumentation::lock_action_observation();
    fn check<T>(owner: T, bytes: u128) {
        let before = crate::ffi::allocation_usage();
        drop(owner);
        let after = crate::ffi::allocation_usage();
        assert_eq!(
            u128::from(before.current_bytes - after.current_bytes),
            bytes
        );
    }
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let (args, mut spi) = saved_output_fixture();
        let mut action = EstimateActionSpec::parse(&args)
            .unwrap()
            .decode_with_memory_limit(&mut spi, || Ok(None))
            .unwrap();
        action.fit.execution.mode = mode;
        let loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
        action.request.absorbed_terms[0].saved_name.clear();
        let clustered = OlsClusteredRequestV1 {
            ols: action.request.clone(),
            clusters: vec![OlsClusterTermV1 {
                label: "g".to_owned(),
                sources: vec![SourceId::new(2).unwrap()],
            }],
        };
        let iid = PreparedOlsUnadjustedConfig {
            fit: action.fit,
            ..Default::default()
        };
        let robust = PreparedOlsRobustConfig {
            fit: action.fit,
            ..Default::default()
        };
        let cluster = PreparedOlsClusteredConfig {
            fit: action.fit,
            ..Default::default()
        };
        let prepared = prepare_model_owned(&action.request, &[], loaded.clone(), false).unwrap();
        let inference =
            fit_prepared_model_unadjusted_with_loaded_moments(&prepared, iid, None).unwrap();
        let bytes = inference.owned_buffer_bytes();
        check(inference, bytes);
        let inference =
            fit_prepared_model_robust_with_loaded_moments(&prepared, robust, None).unwrap();
        let bytes = inference.owned_buffer_bytes();
        check(inference, bytes);
        let prepared =
            prepare_model_owned(&clustered.ols, &clustered.clusters, loaded.clone(), false)
                .unwrap();
        let inference =
            fit_prepared_model_clustered_with_loaded_moments(&prepared, cluster, None).unwrap();
        let bytes = inference.owned_buffer_bytes();
        check(inference, bytes);
        let prepared = prepare_model_owned(&action.request, &[], loaded.clone(), true).unwrap();
        let inference =
            fit_prepared_model_unadjusted_with_loaded_moments(&prepared, iid, None).unwrap();
        let bytes = inference.owned_buffer_bytes();
        check(inference, bytes);
        let inference =
            fit_prepared_model_robust_with_loaded_moments(&prepared, robust, None).unwrap();
        let bytes = inference.owned_buffer_bytes();
        check(inference, bytes);
        let prepared =
            prepare_model_owned(&clustered.ols, &clustered.clusters, loaded, true).unwrap();
        let inference =
            fit_prepared_model_clustered_with_loaded_moments(&prepared, cluster, None).unwrap();
        let bytes = inference.owned_buffer_bytes();
        check(inference, bytes);
    }
}

#[test]
fn saved_fe_model_certificate_rejects_an_in_span_corrupted_product() {
    let (args, mut spi) = saved_output_fixture();
    let action = EstimateActionSpec::parse(&args)
        .unwrap()
        .decode_with_memory_limit(&mut spi, || Ok(None))
        .unwrap();
    let loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
    let prepared = prepare_model_owned(&action.request, &[], loaded, true).unwrap();
    let result = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared,
        PreparedOlsUnadjustedConfig {
            fit: action.fit,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let fit = result.fit();
    let data = &prepared;
    let terms: Vec<_> = fit
        .coefficients()
        .iter()
        .filter_map(|value| value.generated_column_index.zip(value.value))
        .collect();
    let mut xb = vec![0.0; fit.residuals().len()];
    add_fitted_terms(data, &terms, &mut xb).unwrap();
    let mut product: Vec<_> = xb
        .iter()
        .enumerate()
        .map(|(row, xb)| data.sample().outcome[row] - xb - fit.residuals()[row])
        .collect();
    certify_recovered_model_outputs(data, fit, &terms, &product, &mut xb, 1e-12).unwrap();
    // A constant shift still lies exactly in the intercept FE span.
    // Self-reconstruction alone cannot detect this corrupted product.
    for value in &mut product {
        *value += 0.25;
    }
    assert_eq!(
        certify_recovered_model_outputs(data, fit, &terms, &product, &mut xb, 1e-12)
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );
}

#[test]
#[cfg(feature = "ffi")]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn model_recovery_workspace_bounds_observed_allocator_peak() {
    let _guard = instrumentation::lock_action_observation();
    let (args, mut spi) = saved_output_fixture();
    let action = EstimateActionSpec::parse(&args)
        .unwrap()
        .decode_with_memory_limit(&mut spi, || Ok(None))
        .unwrap();
    let loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
    let prepared = prepare_model_owned(&action.request, &[], loaded, true).unwrap();
    let raw = prepared.sample().outcome.clone();
    let residuals = vec![0.0; raw.len()];
    let map = MapConfig {
        tolerance: 1e-12,
        ..Default::default()
    };
    // This exact-span RHS exercises the complete-product ownership; the
    // independent scaled/disconnected test covers nonzero within residuals.
    let mut raw = raw;
    raw.fill(0.75);
    let run = || {
        prepared
            .recover_model_effects(&raw, &residuals, 1.0, &[true], false, true, map, None)
            .unwrap()
    };
    // Worker initialization is embedding state; RSS sampling is host
    // telemetry. Neither belongs to this numerical workspace envelope.
    drop(rayon::broadcast(|_| ()));
    drop(run());
    let mut modeled = 0;
    for iteration in 0..3 {
        let local_start = crate::ffi::thread_allocations::begin();
        let (start, _) = crate::ffi::begin_ols_allocation_observation();
        let boundary = crate::ffi::allocation_usage();
        let recovered = run();
        let usage = crate::ffi::allocation_usage();
        let observed = usage.peak_bytes - start.current_bytes;
        modeled = recovered.solver_workspace_peak().bytes().unwrap();
        for value in recovered.absorbed_sum() {
            assert!((value - 0.75).abs() < 1e-12);
        }
        drop(recovered);
        let after_drop = crate::ffi::allocation_usage();
        let local_end = crate::ffi::thread_allocations::usage();
        eprintln!(
            "recovery audit {iteration}: workers={} start={} boundary={} peak={} end={} dropped={} local_start={} local_peak={} local_end={} modeled={modeled}",
            rayon::current_num_threads(), start.current_bytes,
            boundary.peak_bytes, usage.peak_bytes, usage.current_bytes,
            after_drop.current_bytes, local_start.current_bytes,
            local_end.peak_bytes, local_end.current_bytes
        );
        assert!(
            observed <= modeled,
            "model recovery peak {observed} exceeds {modeled}"
        );
    }
    assert_eq!(
        prepared
            .recover_model_effects(
                &raw,
                &residuals,
                1.0,
                &[true],
                false,
                true,
                map,
                Some(FeSolverMemoryBudget {
                    execution_bytes: 0,
                    usable_bytes: u128::from(modeled) - 1
                })
            )
            .unwrap_err()
            .code,
        ErrorCode::Allocation
    );
}

#[test]
#[cfg(feature = "ffi")]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn saved_slope_recovery_workspace_bounds_observed_allocator_peak() {
    let _guard = instrumentation::lock_action_observation();
    let (args, mut spi) = saved_output_fixture();
    let mut action = EstimateActionSpec::parse(&args)
        .unwrap()
        .decode_with_memory_limit(&mut spi, || Ok(None))
        .unwrap();
    action.request.keep_singletons = true;
    action.request.absorbed_terms[0]
        .slope_sources
        .push(SourceId::new(1).unwrap());
    let loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
    let target = loaded.numeric_sources[0]
        .1
        .values
        .iter()
        .zip(&loaded.numeric_sources[1].1.values)
        .map(|(slope, group)| 2.0 * slope + 3.0 * group)
        .collect::<Vec<_>>();
    let prepared = prepare_model_owned(&action.request, &[], loaded, true).unwrap();
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        drop(pool.broadcast(|_| ()));
        pool.install(|| {
            for mask in [[true, true], [false, true]] {
                let run = || {
                    prepared
                        .recover_absorbed_effects_fast(
                            &target,
                            &mask,
                            None,
                            false,
                            true,
                            LsmrConfig::default(),
                            MapConfig {
                                tolerance: 1e-12,
                                ..Default::default()
                            },
                            None,
                        )
                        .unwrap()
                };
                // Worker construction belongs to its separately declared
                // runtime scope. The recovery envelope covers each live
                // numerical attempt, including pinned within setup.
                drop(run());
                for _ in 0..3 {
                    let (start, _) = crate::ffi::begin_ols_allocation_observation();
                    let recovered = run();
                    let usage = crate::ffi::allocation_usage();
                    let observed = usage.peak_bytes - start.current_bytes;
                    let modeled = recovered.solver_workspace_peak().bytes().unwrap();
                    assert!(
                        observed <= modeled,
                        "slope recovery peak {observed} exceeds {modeled}"
                    );
                    for (actual, expected) in recovered.absorbed_sum().iter().zip(&target) {
                        assert!((actual - expected).abs() < 1e-10);
                    }
                }
            }
        });
    }
}

#[test]
#[cfg(feature = "ffi")]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn numeric_dictionary_growth_stays_inside_construction_envelope() {
    let _guard = instrumentation::lock_action_observation();
    for n in [1, 7, 257, 4097, 8193, 65537] {
        for levels in [1, 31, n] {
            let values: Vec<f64> = (0..n)
                .map(|i| (i % levels) as f64 * 100000.5 + 0.25)
                .collect();
            let start = instrumentation::begin_observation();
            let encoded = fereg_core::factorize::factorize_f64(&values).unwrap();
            let peak = crate::ffi::allocation_usage().peak_bytes - start.allocation_current_bytes;
            assert!(
                u128::from(peak) <= n as u128 * 96 + 4096,
                "dictionary {n}/{levels}: {peak}"
            );
            for (i, code) in encoded.codes.iter().enumerate() {
                assert_eq!(encoded.levels[*code as usize], values[i]);
            }
        }
    }
}

#[test]
#[cfg(feature = "ffi")]
fn construction_and_transient_frame_peaks_bound_independent_allocations() {
    let _guard = instrumentation::lock_action_observation();
    let team = ThreadTeam::new(ThreadConfig::from_requested(4, false).unwrap());
    team.install(|| ()).unwrap();
    for rows in [257, 8193] {
        for complex in [false, true] {
            for vce in [OlsVceKind::Iid, OlsVceKind::Robust, OlsVceKind::Clustered] {
                let (args, mut spi) = saved_output_fixture();
                let mut action = EstimateActionSpec::parse(&args)
                    .unwrap()
                    .decode_with_memory_limit(&mut spi, || Ok(None))
                    .unwrap();
                action.request.keep_singletons = true;
                action.request.absorbed_terms[0].saved_name.clear();
                if complex {
                    action.request.absorbed_terms[0]
                        .slope_sources
                        .push(SourceId::new(1).unwrap());
                }
                let loaded = LoadedOlsInput {
                    candidate: vec![true; rows],
                    outcome: LoadedNumericColumn {
                        values: (0..rows)
                            .map(|i| {
                                let x = ((i * 7) % 109) as f64;
                                0.8 * x + (i % 31) as f64 + ((i * 3) % 7) as f64
                            })
                            .collect(),
                        missing: vec![false; rows],
                    },
                    numeric_sources: vec![
                        (
                            SourceId::new(1).unwrap(),
                            LoadedNumericColumn {
                                values: (0..rows).map(|i| ((i * 7) % 109) as f64).collect(),
                                missing: vec![false; rows],
                            },
                        ),
                        // Non-integer, widely spaced labels force dictionary
                        // factorization rather than the dense integer path.
                        (
                            SourceId::new(2).unwrap(),
                            LoadedNumericColumn {
                                values: (0..rows).map(|i| (i % 31) as f64 * 100000.5).collect(),
                                missing: vec![false; rows],
                            },
                        ),
                    ],
                    analytic_weight: None,
                    frequency_weight: None,
                    probability_weight: None,
                };
                let clusters = vec![OlsClusterTermV1 {
                    label: "g".to_owned(),
                    sources: vec![SourceId::new(2).unwrap()],
                }];
                let construction = crate::ols_preparation_memory::OlsPreparationMemory::estimate(
                    rows,
                    3,
                    false,
                    1,
                    clusters.len(),
                )
                .unwrap()
                .with_construction(rows, &action.request, &clusters)
                .unwrap();
                let bound = construction.preparation_peak_bytes(0).unwrap();
                construction.admit_numeric_arrays(Some(bound)).unwrap();
                assert_eq!(
                    construction
                        .admit_numeric_arrays(Some(bound - 1))
                        .unwrap_err()
                        .code,
                    ErrorCode::Allocation
                );
                let request = OlsClusteredRequestV1 {
                    ols: action.request,
                    clusters,
                };
                team.install(|| {
                    let start = instrumentation::begin_observation();
                    // Use the complex topology even for the intercept-only
                    // control, exercising construction overlap and validation.
                    let prepared =
                        prepare_model_owned(&request.ols, &request.clusters, loaded, true).unwrap();
                    let peak =
                        crate::ffi::allocation_usage().peak_bytes - start.allocation_current_bytes;
                    assert!(u128::from(peak) <= bound, "construction {peak} > {bound}");
                    for mode in [
                        PreparedOlsExecutionMode::Fast,
                        PreparedOlsExecutionMode::Low,
                    ] {
                        let mut fit = action.fit;
                        fit.execution.mode = mode;
                        fit.absorb.retain_coefficients = false;
                        fit.absorb.solver_memory_budget = None;
                        let model = crate::ols_frame_memory::OlsFrameMemory::estimate(
                            rows,
                            prepared.ols().model().columns.len(),
                            prepared.ols().model().coefficients.len(),
                            if vce == OlsVceKind::Clustered {
                                prepared.clusters().len()
                            } else {
                                0
                            },
                            fit.execution.poolsize,
                            vce != OlsVceKind::Iid,
                        )
                        .unwrap();
                        let start = instrumentation::begin_observation();
                        let (solver_peak, owned) = match vce {
                            OlsVceKind::Iid => {
                                let r = fit_prepared_model_unadjusted_with_loaded_moments(
                                    prepared.ols(),
                                    PreparedOlsUnadjustedConfig {
                                        fit,
                                        ..Default::default()
                                    },
                                    None,
                                )
                                .unwrap();
                                (
                                    r.fit().solver_workspace_peak().bytes(),
                                    r.owned_buffer_bytes(),
                                )
                            }
                            OlsVceKind::Robust => {
                                let r = fit_prepared_model_robust_with_loaded_moments(
                                    prepared.ols(),
                                    PreparedOlsRobustConfig {
                                        fit,
                                        ..Default::default()
                                    },
                                    None,
                                )
                                .unwrap();
                                (
                                    r.fit().solver_workspace_peak().bytes(),
                                    r.owned_buffer_bytes(),
                                )
                            }
                            OlsVceKind::Clustered => {
                                let r = fit_prepared_model_clustered_with_loaded_moments(
                                    &prepared,
                                    PreparedOlsClusteredConfig {
                                        fit,
                                        ..Default::default()
                                    },
                                    None,
                                )
                                .unwrap();
                                (
                                    r.fit().solver_workspace_peak().bytes(),
                                    r.owned_buffer_bytes(),
                                )
                            }
                        };
                        let peak = crate::ffi::allocation_usage().peak_bytes
                            - start.allocation_current_bytes;
                        let frame = if mode == PreparedOlsExecutionMode::Fast {
                            model.fast_bytes
                        } else {
                            model.low_bytes
                        };
                        let total =
                            frame + u128::from(solver_peak.expect("modeled fixture solver"));
                        assert!(
                            u128::from(peak) <= total,
                            "frame {peak} > {total}; owner {owned}"
                        );
                    }
                })
                .unwrap();
            }
        }
    }
}

#[test]
#[cfg(feature = "ffi")]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn inference_residual_overlap_stays_within_existing_frame() {
    let _guard = instrumentation::lock_action_observation();
    let threads = ThreadConfig::from_requested(16, true).unwrap();
    let team = ThreadTeam::new(threads);
    team.install(|| ()).unwrap();
    // Exercise packed and strided score retention on an eligible narrow fit.
    for rows in [16_385, 524_301] {
        let mut request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: Vec::new(),
                terms: Vec::new(),
            },
            bindings: vec![
                OlsInputBinding {
                    stata_var_position: 1,
                    role: OlsInputRole::Candidate,
                },
                OlsInputBinding {
                    stata_var_position: 2,
                    role: OlsInputRole::Outcome,
                },
            ],
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![SourceId::new(11).unwrap()],
                slope_sources: Vec::new(),
                label: "g".to_owned(),
                saved_name: String::new(),
            }],
            include_intercept: true,
            keep_singletons: true,
        };
        let mut numeric_sources = Vec::new();
        for id in 1..=11 {
            let source = SourceId::new(id).unwrap();
            request.model.sources.push(SourceEntry {
                id: source,
                canonical_name: format!("x{id}"),
            });
            request.bindings.push(OlsInputBinding {
                stata_var_position: id + 2,
                role: OlsInputRole::NumericSource(source),
            });
            if id < 11 {
                let mut term = continuous_term(source);
                term.id = TermId::new(id).unwrap();
                term.order = TermOrder::new(id).unwrap();
                term.coefficient_slot = CoefficientSlot::new(id).unwrap();
                term.canonical_token = format!("c.x{id}");
                request.model.terms.push(term);
            }
            numeric_sources.push((
                source,
                LoadedNumericColumn {
                    values: (0..rows)
                        .map(|r| {
                            if id == 11 {
                                (r % 79) as f64
                            } else {
                                ((r * (id as usize + 17) % 1009) as f64 / 11.0).sin()
                            }
                        })
                        .collect(),
                    missing: vec![false; rows],
                },
            ));
        }
        let outcome = LoadedNumericColumn {
            values: (0..rows)
                .map(|r| {
                    1.3 * numeric_sources[0].1.values[r]
                        + 0.7 * numeric_sources[1].1.values[r]
                        + (r % 79) as f64
                        + (r as f64).cos() * 0.1
                })
                .collect(),
            missing: vec![false; rows],
        };
        let loaded = LoadedOlsInput {
            candidate: vec![true; rows],
            outcome,
            numeric_sources,
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let request = OlsClusteredRequestV1 {
            ols: request,
            clusters: vec![OlsClusterTermV1 {
                label: "g".to_owned(),
                sources: vec![SourceId::new(11).unwrap()],
            }],
        };
        let prepared = prepare_model_owned(&request.ols, &request.clusters, loaded, true).unwrap();
        team.install(|| {
            for clustered in [false, true] {
                let fit = PreparedOlsFitConfig {
                    execution: fereg_core::ols::PreparedOlsExecutionConfig {
                        threads,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let frame = crate::ols_frame_memory::OlsFrameMemory::estimate(
                    rows,
                    prepared.ols().model().columns.len(),
                    prepared.ols().model().coefficients.len(),
                    usize::from(clustered),
                    fit.execution.poolsize,
                    true,
                )
                .unwrap();
                let start = instrumentation::begin_observation();
                let solver_peak = if clustered {
                    let result = fit_prepared_model_clustered_with_loaded_moments(
                        &prepared,
                        PreparedOlsClusteredConfig {
                            fit,
                            ..Default::default()
                        },
                        None,
                    )
                    .unwrap();
                    assert!(result.fit().score_certificate().passed);
                    result.fit().solver_workspace_peak().bytes().unwrap()
                } else {
                    let result = fit_prepared_model_robust_with_loaded_moments(
                        prepared.ols(),
                        PreparedOlsRobustConfig {
                            fit,
                            ..Default::default()
                        },
                        None,
                    )
                    .unwrap();
                    assert!(result.fit().score_certificate().passed);
                    result.fit().solver_workspace_peak().bytes().unwrap()
                };
                let peak =
                    crate::ffi::allocation_usage().peak_bytes - start.allocation_current_bytes;
                let bound = frame.fast_bytes + u128::from(solver_peak);
                assert!(
                    u128::from(peak) <= bound,
                    "rows={rows}, clustered={clustered}, peak={peak}, bound={bound}"
                );
            }
        })
        .unwrap();
    }
}

#[test]
#[cfg(feature = "ffi")]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn action_metadata_matches_independent_deallocation_and_counts_spare_capacity() {
    let _guard = instrumentation::lock_action_observation();
    for (args, mut spi) in [
        fixture(OlsVceKind::Clustered),
        factor_scatter_fixture(),
        saved_output_fixture(),
    ] {
        let mut action = EstimateActionSpec::parse(&args)
            .unwrap()
            .decode_with_memory_limit(&mut spi, || Ok(None))
            .unwrap();
        action.request.model.sources.reserve(13);
        action.request.model.sources[0].canonical_name.reserve(29);
        action.request.model.terms.reserve(11);
        for term in &mut action.request.model.terms {
            term.canonical_token.reserve(7);
            term.components.reserve(5);
            for component in &mut term.components {
                component.raw_operator.reserve(3);
                component.raw_time_series_operator.reserve(17);
            }
        }
        action.request.bindings.reserve(19);
        action.request.absorbed_terms.reserve(3);
        for term in &mut action.request.absorbed_terms {
            term.categorical_sources.reserve(5);
            term.slope_sources.reserve(7);
            term.label.reserve(19);
            term.saved_name.reserve(23);
        }
        action.clusters.reserve(7);
        for cluster in &mut action.clusters {
            cluster.label.reserve(31);
            cluster.sources.reserve(3);
        }
        action.outputs.saved_channels.reserve(17);
        let before = crate::ffi::allocation_usage();
        let bytes = action.metadata_capacity_bytes();
        assert_eq!(crate::ffi::allocation_usage().events, before.events);
        assert!(bytes + action.transport_parent_bytes > action.metadata_owner_bytes);
        drop(action);
        let after = crate::ffi::allocation_usage();
        assert_eq!(
            u128::from(before.current_bytes - after.current_bytes),
            bytes
        );
        assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
    }
}

#[test]
#[cfg(feature = "ffi")]
fn action_ledger_covers_every_ols_stage_without_reporting_allocations() {
    let _guard = instrumentation::lock_action_observation();
    let (args, mut spi) = saved_output_fixture();
    let mut action = EstimateActionSpec::parse(&args)
        .unwrap()
        .decode_with_memory_limit(&mut spi, || Ok(None))
        .unwrap();
    let base = action.memory_plan.estimate;
    let loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
    let prepared = prepare_model_owned(&action.request, &[], loaded, true).unwrap();
    action
        .admit_prepared_parent(prepared.owned_buffer_bytes())
        .unwrap();
    let inference = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared,
        PreparedOlsUnadjustedConfig {
            fit: action.fit,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    action
        .admit_output_parent(inference.owned_buffer_bytes())
        .unwrap();
    let solver = inference.fit().solver_workspace_peak();
    let before = crate::ffi::allocation_usage();
    let ledger = action.memory_ledger(solver.bytes()).unwrap();
    assert_eq!(crate::ffi::allocation_usage().events, before.events);
    let summary = ledger.summary();
    assert_eq!(summary.missing_stages, 0);
    assert!(
        summary.accounted_peak
            >= action.execution_parent_bytes + u128::from(solver.bytes().unwrap())
    );
    assert!(
        summary.accounted_peak
            >= action.output_parent_bytes + action.publication_memory.peak_bytes()
    );
    assert_eq!(summary.complete_peak, None);
    assert_eq!(summary.ratio(u64::MAX), None);
    assert_eq!(action.memory_plan.estimate, base);
    assert_eq!(spi.store_calls, 0);
}

#[test]
fn output_parent_admission_adds_live_owners_and_rejects_before_stores() {
    let (args, mut spi) = saved_output_fixture();
    let mut action = EstimateActionSpec::parse(&args)
        .unwrap()
        .decode_with_memory_limit(&mut spi, || Ok(None))
        .unwrap();
    action.prepared_owner_bytes = 1000;
    let parent = 3000 + action.metadata_owner_bytes;
    let peak = parent + action.publication_memory.peak_bytes();
    action.fit.absorb.solver_memory_budget = Some(FeSolverMemoryBudget {
        execution_bytes: 1500,
        usable_bytes: peak - 1,
    });
    assert_eq!(
        action.admit_output_parent(2000).unwrap_err().code,
        ErrorCode::Allocation
    );
    assert_eq!(
        action
            .fit
            .absorb
            .solver_memory_budget
            .unwrap()
            .execution_bytes,
        1500
    );
    action
        .fit
        .absorb
        .solver_memory_budget
        .as_mut()
        .unwrap()
        .usable_bytes = peak;
    action.admit_output_parent(2000).unwrap();
    assert_eq!(
        action
            .fit
            .absorb
            .solver_memory_budget
            .unwrap()
            .execution_bytes,
        parent
    );
    action.admit_output_parent(2000).unwrap();
    assert_eq!(
        action
            .fit
            .absorb
            .solver_memory_budget
            .unwrap()
            .execution_bytes,
        parent
    );
    assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
    assert_eq!(spi.receipt_complete(), 0.0);
}

#[test]
fn prepared_parent_exact_budget_reaches_solver_and_preserves_public_base() {
    let (args, mut spi) = saved_output_fixture();
    let mut spec = EstimateActionSpec::parse(&args).unwrap();
    for mode in [MemoryMode::Fast, MemoryMode::Low] {
        spec.memory = mode;
        let unrestricted = spec
            .decode_with_memory_limit(&mut spi, || Ok(None))
            .unwrap();
        let public_base = unrestricted.memory_plan.estimate;
        let owned_bytes = public_base.fast_bytes.max(public_base.low_bytes) + 4096;
        let reconciled = unrestricted
            .preparation_memory
            .with_prepared_owner(public_base, owned_bytes)
            .unwrap();
        let adjusted = unrestricted
            .frame_memory
            .with_parent(
                reconciled,
                owned_bytes + unrestricted.preparation_memory.frame_metadata_bytes,
            )
            .unwrap();
        let execution = (if mode == MemoryMode::Fast {
            adjusted.fast_bytes
        } else {
            adjusted.low_bytes
        }) + unrestricted.metadata_owner_bytes;
        let usable = (execution + unrestricted.publication_memory.peak_bytes()).max(
            unrestricted
                .preparation_memory
                .preparation_peak_bytes(unrestricted.metadata_owner_bytes)
                .unwrap(),
        );
        let available = 512 * 1024 * 1024 + usable;
        let mut exact = spec
            .decode_with_memory_limit(&mut spi, || Ok(Some(available)))
            .unwrap();
        exact.admit_prepared_parent(owned_bytes).unwrap();
        assert_eq!(exact.memory_plan.estimate, public_base);
        assert_eq!(
            exact.fit.absorb.solver_memory_budget.unwrap(),
            FeSolverMemoryBudget {
                execution_bytes: execution,
                usable_bytes: usable
            }
        );
        let mut short = spec
            .decode_with_memory_limit(&mut spi, || Ok(Some(available - 1)))
            .unwrap();
        assert_eq!(
            short.admit_prepared_parent(owned_bytes).unwrap_err().code,
            ErrorCode::Allocation
        );
        assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
        assert_eq!(spi.receipt_complete(), 0.0);
    }
}

#[test]
fn publication_admission_uses_reported_axis_before_reading_rows() {
    for (args, mut spi) in [
        fixture(OlsVceKind::Iid),
        factor_scatter_fixture(),
        saved_output_fixture(),
    ] {
        let mut spec = EstimateActionSpec::parse(&args).unwrap();
        for mode in [MemoryMode::Fast, MemoryMode::Low] {
            spec.memory = mode;
            let unrestricted = spec
                .decode_with_memory_limit(&mut spi, || Ok(None))
                .unwrap();
            let frame = unrestricted
                .frame_memory
                .with_parent(
                    unrestricted.memory_plan.estimate,
                    unrestricted.preparation_memory.frame_metadata_bytes,
                )
                .unwrap();
            let base = (if mode == MemoryMode::Fast {
                frame.fast_bytes
            } else {
                frame.low_bytes
            }) + unrestricted.metadata_owner_bytes;
            let reserve = unrestricted.publication_memory.peak_bytes();
            let usable = (base + reserve)
                .max(
                    unrestricted
                        .preparation_memory
                        .preparation_peak_bytes(unrestricted.metadata_owner_bytes)
                        .unwrap(),
                )
                .max(unrestricted.metadata_peak_bytes);
            let limit = 512 * 1024 * 1024 + usable;
            let exact = spec
                .decode_with_memory_limit(&mut spi, || Ok(Some(limit)))
                .unwrap();
            assert_eq!(
                exact.memory_plan.estimate,
                unrestricted.memory_plan.estimate
            );
            let solver = exact.fit.absorb.solver_memory_budget.unwrap();
            assert_eq!(solver.execution_bytes, base);
            assert_eq!(solver.usable_bytes, usable);
            let error = spec
                .decode_with_memory_limit(&mut spi, || Ok(Some(limit - 1)))
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
            assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
            assert_eq!(spi.receipt_complete(), 0.0);
        }
    }
}

#[test]
fn saved_publication_checks_observed_capacities_before_any_store() {
    let _guard = instrumentation::lock_action_observation();
    for mode in [MemoryMode::Fast, MemoryMode::Low] {
        let (args, mut spi) = saved_output_fixture();
        let mut spec = EstimateActionSpec::parse(&args).unwrap();
        spec.memory = mode;
        let mut action = spec
            .decode_with_memory_limit(&mut spi, || Ok(None))
            .unwrap();
        let observation = ActionObservation {
            started: Instant::now(),
            resources_start: instrumentation::begin_observation(),
            preflight_seconds: 0.0,
            input_load_seconds: 0.0,
        };
        let loaded = load_ols_input(
            &action.request,
            action.candidate_all,
            action.input_complete,
            &mut spi,
        )
        .unwrap();
        let prepared = prepare_model_owned(&action.request, &[], loaded, true).unwrap();
        let inference = fit_prepared_model_unadjusted_with_loaded_moments(
            &prepared,
            PreparedOlsUnadjustedConfig {
                fit: action.fit,
                absorbed_dof: action.absorbed_dof,
                ..PreparedOlsUnadjustedConfig::default()
            },
            None,
        )
        .unwrap();
        let result_data = &prepared;
        let derived = build_derived_observation_outputs(
            result_data,
            Some(&prepared),
            inference.fit(),
            &action.outputs,
            normalization_known_connected(inference.absorbed_dof()),
            action.fit,
        )
        .unwrap();
        let mut shadow = build_shadow_result(
            result_data,
            None,
            CertifiedOlsInference::Iid(&inference),
            action.outputs.residual.is_some(),
        )
        .unwrap();
        let runtime = build_runtime_shadow(&action, &shadow, &observation).unwrap();
        let bytes = shadow.capacity_bytes()
            + derived.capacity_bytes()
            + runtime.capacity() as u128 * std::mem::size_of::<f64>() as u128;
        assert_eq!(
            shadow.capacity_bytes(),
            action.publication_memory.shadow_bytes
        );
        assert_eq!(bytes, action.publication_memory.publication_bytes);
        drop(runtime);

        action.publication_memory.publication_bytes = bytes - 1;
        let error =
            precommit_and_store(&action, &shadow, &derived, &observation, &mut spi).unwrap_err();
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!((spi.store_calls, spi.receipt_complete()), (0, 0.0));
        action.publication_memory.publication_bytes = bytes;
        // Lengths and scientific payload are unchanged; excess reserved
        // capacity alone must also fail before publication starts.
        shadow.b.reserve_exact(1);
        assert!(shadow.capacity_bytes() > action.publication_memory.shadow_bytes);
        assert_eq!(
            precommit_and_store(&action, &shadow, &derived, &observation, &mut spi)
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        assert_eq!((spi.store_calls, spi.receipt_complete()), (0, 0.0));
        shadow.b.shrink_to_fit();
        precommit_and_store(&action, &shadow, &derived, &observation, &mut spi).unwrap();
        assert_eq!(spi.receipt_complete(), 1.0);
    }
}

#[test]
fn guarded_iid_robust_and_clustered_actions_commit_complete_last() {
    for kind in [OlsVceKind::Iid, OlsVceKind::Robust, OlsVceKind::Clustered] {
        let (args, mut spi) = fixture(kind);
        estimate_ols_test(&args, &mut spi).expect("guarded estimate succeeds");
        assert_eq!(spi.receipt_complete(), 1.0);
        assert_eq!(spi.matrices["RREC"][0][0], RESULT_MAGIC);
        assert_eq!(spi.matrices["RREC"][0][3], kind.code());
        assert_eq!(spi.matrices["RREC"][0][6], 6.0);
        assert_eq!(spi.matrices["RREC"][0][8], 6.0);
        assert_eq!(
            spi.numeric_reads,
            if kind == OlsVceKind::Clustered {
                24
            } else {
                18
            },
        );
        assert!(spi.matrices["RB"][0].iter().all(|value| value.is_finite()));
        assert!(spi.matrices["RV"]
            .iter()
            .flatten()
            .all(|value| value.is_finite()));
        assert_eq!(
            spi.stores.last(),
            Some(&StoreEvent::Matrix(14, 1, RECEIPT_COMPLETE_COLUMN, 1.0,)),
        );
    }
}

#[test]
fn candidate_numeric_runs_preserve_coordinates_and_bound_bulk_reads() {
    let (_, mut spi) = fixture(OlsVceKind::Iid);
    spi.data.clear();
    let rows = INPUT_BULK_READ_CHUNK_ROWS + 300;
    let mut candidate = vec![false; rows];
    candidate[1..64].fill(true);
    candidate[65..129].fill(true);
    candidate[130..131 + INPUT_BULK_READ_CHUNK_ROWS].fill(true);
    for (row, retained) in candidate.iter().copied().enumerate() {
        if retained {
            spi.data
                .insert((2, row + 1), (row as f64 - 100.0, row % 17 == 0));
            spi.data
                .insert((3, row + 1), (-(row as f64), row % 23 == 0));
        }
    }
    let mut values = vec![12345.0; rows];
    let mut peer = vec![12345.0; rows];
    let mut missing = vec![true; rows];
    let mut peer_missing = vec![true; rows];
    read_numeric_columns_in_chunks(
        &mut spi,
        &[2, 3],
        &mut [&mut values, &mut peer],
        &mut [&mut missing, &mut peer_missing],
        Some(&candidate),
    )
    .expect("candidate run input");
    assert_eq!(
        spi.numeric_batches,
        vec![
            (66, 64, vec![2, 3]),
            (131, INPUT_BULK_READ_CHUNK_ROWS, vec![2, 3]),
        ]
    );
    assert_eq!(
        spi.numeric_reads,
        2 * candidate.iter().filter(|row| **row).count()
    );
    for row in 0..rows {
        if candidate[row] {
            assert_eq!(
                (values[row], missing[row]),
                (row as f64 - 100.0, row % 17 == 0)
            );
            assert_eq!(
                (peer[row], peer_missing[row]),
                (-(row as f64), row % 23 == 0)
            );
        } else {
            assert_eq!(
                (values[row], peer[row], missing[row], peer_missing[row]),
                (12345.0, 12345.0, true, true)
            );
        }
    }
    assert!(spi.interrupt_checks > 0);
    assert_eq!(spi.store_calls, 0);
}

#[test]
fn candidate_numeric_loading_rejects_bad_shapes_and_polls_empty_runs() {
    let (_, mut spi) = fixture(OlsVceKind::Iid);
    let mut values = [0.0; 4];
    let mut missing = [true; 3];
    assert!(read_numeric_columns_in_chunks(
        &mut spi,
        &[2],
        &mut [&mut values],
        &mut [&mut missing],
        None
    )
    .is_err());
    let mut missing = [true; 4];
    assert!(read_numeric_columns_in_chunks(
        &mut spi,
        &[2],
        &mut [&mut values],
        &mut [&mut missing],
        Some(&[true; 3])
    )
    .is_err());
    assert_eq!(spi.numeric_reads, 0);
    let mut values = vec![0.0; 2 * INPUT_INTERRUPT_POLL_STRIDE];
    let mut missing = vec![true; values.len()];
    let candidate = vec![false; values.len()];
    spi.interrupt_at = Some(2);
    let error = read_numeric_columns_in_chunks(
        &mut spi,
        &[2],
        &mut [&mut values],
        &mut [&mut missing],
        Some(&candidate),
    )
    .expect_err("excluded scan remains interruptible");
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert_eq!(spi.numeric_reads, 0);
    assert_eq!(spi.store_calls, 0);
}

#[test]
fn large_restricted_mixed_input_batches_only_numeric_candidate_runs() {
    for fragmented in [false, true] {
        let (args, mut spi) = fixture(OlsVceKind::Clustered);
        spi.nobs = INPUT_CANDIDATE_BATCH_MIN_ROWS + 32;
        spi.data.clear();
        spi.string_variables.insert(4);
        let labels = [b"z".to_vec(), b"a".to_vec(), Vec::new()];
        for row in 0..spi.nobs {
            let retained = if fragmented {
                row % 3 == 0
            } else {
                (65..spi.nobs - 1).contains(&row)
            };
            spi.data.insert((1, row + 1), (f64::from(retained), false));
            if retained {
                spi.data.insert((2, row + 1), (row as f64, row % 17 == 0));
                spi.data
                    .insert((3, row + 1), (-(row as f64), row % 23 == 0));
                spi.string_data
                    .insert((4, row + 1), labels[row % 3].clone());
            }
        }
        let action = EstimateActionSpec::parse(&args)
            .expect("specification")
            .decode_and_validate(&mut spi)
            .expect("action");
        let loaded = load_ols_input(&action.request, false, false, &mut spi).expect("mixed input");
        assert_eq!(spi.numeric_batches.is_empty(), fragmented);
        assert!(spi
            .numeric_batches
            .iter()
            .all(|(_, _, positions)| positions == &[2, 3]));
        for row in 0..spi.nobs {
            if loaded.candidate[row] {
                assert_eq!(
                    (loaded.outcome.values[row], loaded.outcome.missing[row]),
                    (row as f64, row % 17 == 0)
                );
                let x = &loaded.numeric_sources[0].1;
                assert_eq!(
                    (x.values[row], x.missing[row]),
                    (-(row as f64), row % 23 == 0)
                );
                let cluster = &loaded.numeric_sources[1].1;
                assert_eq!(cluster.missing[row], row % 3 == 2);
                if !cluster.missing[row] {
                    let code = if !fragmented && row % 3 == 0 {
                        1.0
                    } else {
                        0.0
                    };
                    assert_eq!(cluster.values[row], code);
                }
            } else {
                assert_eq!(
                    (loaded.outcome.values[row], loaded.outcome.missing[row]),
                    (0.0, false)
                );
                let x = &loaded.numeric_sources[0].1;
                assert_eq!((x.values[row], x.missing[row]), (0.0, false));
                // The existing finalizer also rebases excluded string
                // placeholders. They retain their original code-zero
                // meaning, while the candidate marker remains authority.
                let cluster = &loaded.numeric_sources[1].1;
                assert_eq!(
                    (cluster.values[row], cluster.missing[row]),
                    (if fragmented { 0.0 } else { 1.0 }, false)
                );
            }
        }
        assert_eq!(spi.store_calls, 0);
    }
}

#[test]
fn complete_candidate_universe_skips_candidate_spi_reads() {
    let (mut args, mut spi) = fixture(OlsVceKind::Iid);
    replace_argument(&mut args, "candidate_all", "candidate_all=1".to_owned());
    replace_argument(&mut args, "input_complete", "input_complete=1".to_owned());

    estimate_ols_test(&args, &mut spi).expect("complete-candidate estimate succeeds");

    // Six observations times outcome plus one source. The candidate
    // coordinate remains present on the wire but is never read.
    assert_eq!(spi.numeric_reads, 12);
    assert_eq!(spi.matrices["RREC"][0][6], 6.0);
    assert_eq!(spi.matrices["RREC"][0][7], 6.0);
    assert_eq!(spi.matrices["RREC"][0][8], 6.0);
    assert_eq!(spi.receipt_complete(), 1.0);
}

#[test]
fn parallel_shadow_preflight_matches_serial_on_full_and_partial_samples() {
    for n in [16383, 16384, 16387] {
        let mut residuals = vec![0.5; n];
        let mut retained: Vec<_> = (0..n).collect();
        let mut reasons = vec![fereg_core::DropReason::Retained; n];
        let compare =
            |residuals: &[f64], retained: &[usize], reasons: &[fereg_core::DropReason]| {
                let serial = invalid_shadow_observations(
                    residuals,
                    retained,
                    reasons,
                    ThreadConfig::from_requested(1, false).unwrap(),
                );
                for t in [8, 16] {
                    assert_eq!(
                        serial,
                        invalid_shadow_observations(
                            residuals,
                            retained,
                            reasons,
                            ThreadConfig::from_requested(t, false).unwrap()
                        )
                    );
                }
            };
        compare(&residuals, &retained, &reasons);
        for row in [0, n / 2, n - 1] {
            retained[row] = n;
            compare(&residuals, &retained, &reasons);
            retained[row] = row;
            reasons[row] = fereg_core::DropReason::MissingOutcome;
            compare(&residuals, &retained, &reasons);
            reasons[row] = fereg_core::DropReason::Retained;
            residuals[row] = f64::NAN;
            compare(&residuals, &retained, &reasons);
            residuals[row] = 0.5;
        }
        retained.swap(0, n - 1);
        compare(&residuals, &retained, &reasons);
        retained.swap(0, n - 1);
        retained.remove(n / 2);
        residuals.remove(n / 2);
        reasons[n / 2] = fereg_core::DropReason::MissingOutcome;
        compare(&residuals, &retained, &reasons);
    }
}

#[test]
fn omitted_observation_outputs_preserve_science_sample_drops_and_receipt() {
    for missing in [false, true] {
        let (mut args, mut reference) = fixture(OlsVceKind::Iid);
        replace_argument(&mut args, "candidate_all", "candidate_all=1".to_owned());
        if missing {
            reference.data.insert((2, 3), (0.0, true));
        }
        let mut no_outputs = reference.clone();
        estimate_ols_test(&args, &mut reference).unwrap();
        replace_argument(&mut args, "output_residual", "output_residual=0".to_owned());
        replace_argument(
            &mut args,
            "output_drop_reason",
            "output_drop_reason=0".to_owned(),
        );
        no_outputs.nvars -= 2;
        estimate_ols_test(&args, &mut no_outputs).unwrap();
        for name in ["RB", "RV", "RSTATUS", "RFIT", "RDOF", "RFE", "RCERT"] {
            assert_eq!(
                reference.matrices[name], no_outputs.matrices[name],
                "{name}"
            );
        }
        let rows: Vec<_> = no_outputs
            .stores
            .iter()
            .filter_map(|event| {
                if let StoreEvent::Numeric(variable, row, value) = event {
                    Some((*variable, *row, *value))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            rows,
            if missing {
                vec![(4, 3, 0.0)]
            } else {
                Vec::new()
            }
        );
        assert_eq!(no_outputs.receipt_complete(), 1.0);
    }
}

#[test]
fn complete_input_certificate_disagreement_fails_before_output() {
    let (mut args, mut spi) = fixture(OlsVceKind::Iid);
    replace_argument(&mut args, "candidate_all", "candidate_all=1".to_owned());
    replace_argument(&mut args, "input_complete", "input_complete=1".to_owned());
    spi.data.insert((2, 3), (0.0, true));

    let error =
        estimate_ols_test(&args, &mut spi).expect_err("false complete-input certificate rejected");

    assert_eq!(error.code, ErrorCode::InvalidData);
    assert_eq!(spi.store_calls, 0);
}

#[test]
fn low_memory_execution_matches_fast_across_inference_modes() {
    for kind in [OlsVceKind::Iid, OlsVceKind::Robust, OlsVceKind::Clustered] {
        let (fast_args, mut fast_spi) = fixture(kind);
        estimate_ols_test(&fast_args, &mut fast_spi).expect("fast estimate succeeds");

        let (mut low_args, mut low_spi) = fixture(kind);
        replace_argument(&mut low_args, "memory", "memory=low".to_owned());
        replace_argument(&mut low_args, "poolsize", "poolsize=1".to_owned());
        estimate_ols_test(&low_args, &mut low_spi).expect("low estimate succeeds");

        for matrix in ["RB", "RV", "RSTATUS", "RFIT", "RDOF", "RFE", "RCLUSTER"] {
            let fast = &fast_spi.matrices[matrix];
            let low = &low_spi.matrices[matrix];
            assert_eq!(fast.len(), low.len(), "{matrix} row count");
            for (fast_row, low_row) in fast.iter().zip(low) {
                assert_eq!(fast_row.len(), low_row.len(), "{matrix} column count");
                for (fast_value, low_value) in fast_row.iter().zip(low_row) {
                    let scale = 1.0_f64.max(fast_value.abs()).max(low_value.abs());
                    assert!(
                        (fast_value - low_value).abs() <= 5e-11 * scale,
                        "{matrix}: fast={fast_value}, low={low_value}"
                    );
                }
            }
        }
        assert_eq!(fast_spi.receipt_complete(), 1.0);
        assert_eq!(low_spi.receipt_complete(), 1.0);
        let fast_observations = fast_spi
            .stores
            .iter()
            .take_while(|event| matches!(event, StoreEvent::Numeric(..)));
        let low_observations = low_spi
            .stores
            .iter()
            .take_while(|event| matches!(event, StoreEvent::Numeric(..)));
        for (fast_event, low_event) in fast_observations.zip(low_observations) {
            match (fast_event, low_event) {
                (
                    StoreEvent::Numeric(fast_variable, fast_observation, fast_value),
                    StoreEvent::Numeric(low_variable, low_observation, low_value),
                ) => {
                    assert_eq!(
                        (fast_variable, fast_observation),
                        (low_variable, low_observation)
                    );
                    let scale = 1.0_f64.max(fast_value.abs()).max(low_value.abs());
                    assert!((fast_value - low_value).abs() <= 5e-11 * scale);
                }
                _ => unreachable!("observation prefix contains only numeric stores"),
            }
        }
    }
}

#[test]
fn deterministic_thread_counts_preserve_scientific_results_and_are_reported() {
    for kind in [OlsVceKind::Iid, OlsVceKind::Robust, OlsVceKind::Clustered] {
        let (single_args, mut single_spi) = fixture(kind);
        estimate_ols_test(&single_args, &mut single_spi).expect("single-thread estimate");

        for threads in [2, 4, 8, 16] {
            let (mut parallel_args, mut parallel_spi) = fixture(kind);
            replace_argument(&mut parallel_args, "threads", format!("threads={threads}"));
            estimate_ols_test(&parallel_args, &mut parallel_spi).expect("parallel estimate");

            for matrix in ["RB", "RV", "RSTATUS", "RFIT", "RDOF", "RFE", "RCLUSTER"] {
                assert_eq!(
                    single_spi.matrices[matrix], parallel_spi.matrices[matrix],
                    "{matrix} differs at {threads} threads"
                );
            }
            assert_eq!(
                parallel_spi.matrices["RRUNTIME"][27],
                vec![1.0, threads as f64]
            );
            assert_eq!(
                parallel_spi.matrices["RRUNTIME"][28],
                vec![1.0, threads as f64]
            );
            assert_eq!(parallel_spi.matrices["RRUNTIME"][29], vec![1.0, 1.0]);
            assert_eq!(parallel_spi.receipt_complete(), 1.0);
        }
    }
}

#[test]
fn saved_outputs_allow_a_generated_column_omitted_by_absorption() {
    for memory in ["fast", "low"] {
        let (mut args, mut spi) = saved_output_fixture();
        replace_argument(&mut args, "memory", format!("memory={memory}"));
        for observation in 1..=spi.nobs {
            let group = spi.data[&(4, observation)].0;
            spi.data.insert((3, observation), (group, false));
        }
        estimate_ols_test(&args, &mut spi).expect("omitted generated column has no xb term");
        assert_eq!(spi.matrices["RSTATUS"][0][1], 3.0);
        assert_eq!(spi.matrices["RB"][0][0], 0.0);
        let overall =
            (1..=spi.nobs).map(|row| spi.data[&(2, row)].0).sum::<f64>() / spi.nobs as f64;
        for observation in 1..=spi.nobs {
            let first = (observation - 1) / 2 * 2 + 1;
            let mean = (spi.data[&(2, first)].0 + spi.data[&(2, first + 1)].0) / 2.0;
            let y = spi.data[&(2, observation)].0;
            assert!((stored_numeric(&spi, 6, observation) - (y - mean)).abs() <= 1e-11);
            assert!((stored_numeric(&spi, 8, observation) - overall).abs() <= 1e-11);
            assert!((stored_numeric(&spi, 9, observation) - mean).abs() <= 1e-11);
            assert!((stored_numeric(&spi, 10, observation) - (mean - overall)).abs() <= 1e-11);
            assert!((stored_numeric(&spi, 11, observation) - (mean - overall)).abs() <= 1e-11);
        }
        assert_eq!(spi.receipt_complete(), 1.0);
    }
}

#[test]
fn loaded_rhs_admission_recipe_and_missing_fallback_preserve_estimates() {
    let (_, mut spi) = fixture(OlsVceKind::Iid);
    spi.nobs = 16_387;
    spi.nvars = 15;
    spi.numeric_rows_enabled = true;
    spi.data.clear();
    for row in 1..=spi.nobs {
        for variable in 1..=15 {
            let value = if variable == 1 {
                1.0
            } else if variable == 13 {
                ((row - 1) / 5 + 1) as f64
            } else {
                ((row * variable as usize) as f64).sin()
                    + ((row + variable as usize * 13) as f64).cos()
            };
            spi.data.insert((variable, row), (value, false));
        }
    }
    let mut request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: Vec::new(),
            terms: Vec::new(),
        },
        bindings: vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ],
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).unwrap(),
            intercept: true,
            categorical_sources: vec![SourceId::new(11).unwrap()],
            slope_sources: Vec::new(),
            label: "g".to_owned(),
            saved_name: String::new(),
        }],
        include_intercept: true,
        keep_singletons: false,
    };
    for id in 1..=11 {
        let source = SourceId::new(id).unwrap();
        request.model.sources.push(SourceEntry {
            id: source,
            canonical_name: format!("x{id}"),
        });
        request.bindings.push(OlsInputBinding {
            stata_var_position: id + 2,
            role: OlsInputRole::NumericSource(source),
        });
        if id < 11 {
            let mut term = continuous_term(source);
            term.id = TermId::new(id).unwrap();
            term.order = TermOrder::new(id).unwrap();
            term.coefficient_slot = CoefficientSlot::new(id).unwrap();
            term.canonical_token = format!("c.x{id}");
            request.model.terms.push(term);
        }
    }
    let required = spi.nobs * 12 * 8;
    for (extra_sources, effect_count) in [(0, 1), (2, 1), (2, 2), (2, 3)] {
        for (budget, threads, reorder, missing) in [
            (required, 4, false, false),
            (required - 1, 4, false, false),
            (required, 1, false, false),
            (required, 4, true, false),
            (required, 4, false, true),
            (required, 8, false, false),
            (required - 1, 8, false, false),
            (required, 16, true, false),
            (required, 16, false, true),
        ] {
            let mut request = request.clone();
            let mut spi = spi.clone();
            for id in 12..12 + extra_sources {
                let source = SourceId::new(id).unwrap();
                request.model.sources.push(SourceEntry {
                    id: source,
                    canonical_name: format!("cluster{id}"),
                });
                request.bindings.push(OlsInputBinding {
                    stata_var_position: id + 2,
                    role: OlsInputRole::NumericSource(source),
                });
            }
            for effect in 2..=effect_count {
                let source = SourceId::new(10 + effect).unwrap();
                request.absorbed_terms.push(AbsorbedTerm {
                    id: AbsorbTermId::new(effect).unwrap(),
                    intercept: true,
                    categorical_sources: vec![source],
                    slope_sources: Vec::new(),
                    label: format!("g{effect}"),
                    saved_name: String::new(),
                });
                for row in 1..=spi.nobs {
                    spi.data.insert(
                        (12 + effect, row),
                        ((row % (13 + effect as usize)) as f64, false),
                    );
                }
            }
            if reorder {
                request.model.terms[0].components[0].source = SourceId::new(2).unwrap();
                request.model.terms[1].components[0].source = SourceId::new(1).unwrap();
            }
            if missing {
                spi.data.insert((2, 3), (1e100, true));
            }
            let clustered_request = OlsClusteredRequestV1 {
                ols: request.clone(),
                clusters: vec![OlsClusterTermV1 {
                    label: "c".to_owned(),
                    sources: if extra_sources == 0 {
                        vec![SourceId::new(11).unwrap()]
                    } else {
                        vec![SourceId::new(12).unwrap(), SourceId::new(13).unwrap()]
                    },
                }],
            };
            let compiled_width =
                numeric_loaded_rhs_layout(&clustered_request.validate_and_compile().unwrap());
            let reference_loaded = load_ols_input(&request, true, false, &mut spi.clone()).unwrap();
            let mut rhs = None;
            let mut memory = loading_memory(&request, spi.nobs, 0, 0, None).unwrap();
            let loaded = load_ols_input_with_team(
                &request,
                true,
                false,
                &mut memory,
                &mut spi,
                Some(ThreadTeam::new(
                    ThreadConfig::from_requested(threads, false).unwrap(),
                )),
                &mut rhs,
                budget,
                compiled_width,
                false,
            )
            .unwrap();
            assert_eq!(loaded, reference_loaded);
            assert_eq!(rhs.is_some(), budget == required && threads > 1 && !reorder);
            if let Some(rows) = rhs.as_ref() {
                assert_eq!(rows.capacity() * 8, required);
                assert!(memory.peak() >= required as u128);
            }
            let categorical_clustered = prepare_model_owned(
                &clustered_request.ols,
                &clustered_request.clusters,
                loaded.clone(),
                true,
            )
            .unwrap();
            let categorical = categorical_clustered.ols();
            let categorical_rows = rhs.clone().map(|rows| {
                let bytes = LoadedOlsRows::moment_workspace_bytes(spi.nobs, 12).unwrap();
                ThreadTeam::new(ThreadConfig::from_requested(threads, false).unwrap())
                    .install(|| LoadedOlsRows::from(rows).with_unit_moments(spi.nobs, 12, bytes))
                    .unwrap()
            });
            let robust_rows = rhs.clone();
            let clustered_rows = rhs.clone();
            let prepared_clustered = prepare_model_owned(
                &clustered_request.ols,
                &clustered_request.clusters,
                loaded,
                false,
            )
            .unwrap();
            let prepared = prepared_clustered.ols();
            let mut config = PreparedOlsUnadjustedConfig::default();
            config.fit.execution.threads = ThreadConfig::from_requested(4, false).unwrap();
            let reference =
                fit_prepared_model_unadjusted_with_loaded_moments(prepared, config, None).unwrap();
            let actual = fereg_core::ols::fit_prepared_model_unadjusted_with_loaded_moments(
                prepared,
                config,
                rhs.map(LoadedOlsRows::from),
            )
            .unwrap();
            assert_eq!(reference.fit().coefficients(), actual.fit().coefficients());
            assert_eq!(reference.covariance(), actual.covariance());
            assert_eq!(reference.statistics(), actual.statistics());
            assert_eq!(reference.certificate(), actual.certificate());
            let reference = fereg_core::ols::fit_prepared_model_unadjusted_with_loaded_moments(
                categorical,
                config,
                None,
            )
            .unwrap();
            let actual = fereg_core::ols::fit_prepared_model_unadjusted_with_loaded_moments(
                categorical,
                config,
                categorical_rows,
            )
            .unwrap();
            assert_eq!(reference.fit().coefficients(), actual.fit().coefficients());
            assert_eq!(reference.covariance(), actual.covariance());
            let robust_config = PreparedOlsRobustConfig {
                fit: config.fit,
                ..PreparedOlsRobustConfig::default()
            };
            let reference = fereg_core::ols::fit_prepared_model_robust_with_loaded_moments(
                categorical,
                robust_config,
                None,
            )
            .unwrap();
            let actual = fereg_core::ols::fit_prepared_model_robust_with_loaded_moments(
                categorical,
                robust_config,
                robust_rows.map(LoadedOlsRows::from),
            )
            .unwrap();
            assert_eq!(reference.fit().coefficients(), actual.fit().coefficients());
            assert_eq!(reference.covariance(), actual.covariance());
            let cluster_config = PreparedOlsClusteredConfig {
                fit: config.fit,
                ..PreparedOlsClusteredConfig::default()
            };
            let reference = fereg_core::ols::fit_prepared_model_clustered_with_loaded_moments(
                &categorical_clustered,
                cluster_config,
                None,
            )
            .unwrap();
            let actual = fereg_core::ols::fit_prepared_model_clustered_with_loaded_moments(
                &categorical_clustered,
                cluster_config,
                clustered_rows.map(LoadedOlsRows::from),
            )
            .unwrap();
            assert_eq!(reference.fit().coefficients(), actual.fit().coefficients());
            assert_eq!(reference.covariance(), actual.covariance());
        }
    }
}

#[test]
fn slope_loaded_rhs_preserves_classification_and_inference() {
    let (_, mut base) = fixture(OlsVceKind::Iid);
    base.nobs = 16_387;
    base.nvars = 14;
    base.numeric_rows_enabled = true;
    base.data.clear();
    for row in 1..=base.nobs {
        for variable in 1..=14 {
            let value = match variable {
                1 => 1.0,
                9 => ((row - 1) / 7) as f64,
                11 => (1 + row % 7) as f64,
                _ => ((row * variable as usize) as f64).sin(),
            };
            base.data.insert((variable, row), (value, false));
        }
    }
    let mut template = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: Vec::new(),
            terms: Vec::new(),
        },
        bindings: vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ],
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).unwrap(),
            intercept: true,
            categorical_sources: vec![SourceId::new(7).unwrap()],
            slope_sources: vec![SourceId::new(8).unwrap()],
            label: "g##c.z".into(),
            saved_name: String::new(),
        }],
        include_intercept: true,
        keep_singletons: false,
    };
    for id in 1..=8 {
        let source = SourceId::new(id).unwrap();
        template.model.sources.push(SourceEntry {
            id: source,
            canonical_name: format!("x{id}"),
        });
        template.bindings.push(OlsInputBinding {
            stata_var_position: id + 2,
            role: OlsInputRole::NumericSource(source),
        });
        if id <= 6 {
            let mut term = continuous_term(source);
            term.id = TermId::new(id).unwrap();
            term.order = TermOrder::new(id).unwrap();
            term.coefficient_slot = CoefficientSlot::new(id).unwrap();
            term.canonical_token = format!("c.x{id}");
            template.model.terms.push(term);
        }
    }
    for weighted in [false, true] {
        for case in 0..11 {
            let mut request = template.clone();
            let mut spi = base.clone();
            if weighted {
                request.bindings.push(OlsInputBinding {
                    stata_var_position: 11,
                    role: OlsInputRole::AnalyticWeight,
                });
            }
            match case {
                1 => {
                    spi.data.insert((10, 3), (1e100, true));
                }
                2 => {
                    spi.data.insert((9, 3), (1e100, true));
                }
                3 => {
                    request.absorbed_terms[0].intercept = false;
                }
                _ => {}
            }
            if case >= 4 {
                request.absorbed_terms[0].intercept = false;
                request.include_intercept = false;
            }
            if case == 5 {
                spi.data.insert((10, 3), (1e100, true));
            }
            if (6..10).contains(&case) {
                for id in 9..=if case == 9 { 11 } else { 10 } {
                    let source = SourceId::new(id).unwrap();
                    request.model.sources.push(SourceEntry {
                        id: source,
                        canonical_name: format!("z{id}"),
                    });
                    request.bindings.push(OlsInputBinding {
                        stata_var_position: id + 3,
                        role: OlsInputRole::NumericSource(source),
                    });
                    request.absorbed_terms[0].slope_sources.push(source);
                }
            }
            if case == 10 {
                request.absorbed_terms[0].slope_sources = vec![
                    SourceId::new(8).unwrap(),
                    SourceId::new(5).unwrap(),
                    SourceId::new(6).unwrap(),
                ];
            }
            let compiled = numeric_loaded_rhs_layout(&request.validate_ppml_and_compile().unwrap());
            let reference = load_ols_input(&request, true, false, &mut spi.clone()).unwrap();
            let mut rhs = None;
            let mut memory = loading_memory(&request, spi.nobs, 0, 0, None).unwrap();
            let required = if case == 7 { 0 } else { spi.nobs * 8 * 8 };
            let team = (case != 8)
                .then(|| ThreadTeam::new(ThreadConfig::from_requested(8, false).unwrap()));
            let loaded = load_ols_input_with_team(
                &request,
                true,
                false,
                &mut memory,
                &mut spi,
                team,
                &mut rhs,
                required,
                compiled,
                false,
            )
            .unwrap();
            assert_eq!(loaded, reference);
            assert_eq!(rhs.is_some(), case < 7 || case == 10);
            if let Some(rows) = rhs.as_ref() {
                let layout = compiled.unwrap();
                assert_eq!(rows.len(), spi.nobs * layout.width);
                assert_eq!(layout.append_intercept, request.include_intercept);
            }
            let prepared = prepare_model_owned(&request, &[], loaded, true).unwrap();
            let mut config = PreparedOlsRobustConfig::default();
            config.fit.execution.threads = ThreadConfig::from_requested(8, false).unwrap();
            let expected = fereg_core::ols::fit_prepared_model_robust_with_loaded_moments(
                &prepared, config, None,
            );
            let actual = fereg_core::ols::fit_prepared_model_robust_with_loaded_moments(
                &prepared,
                config,
                rhs.map(LoadedOlsRows::from),
            );
            if case == 10 {
                // The existing fit rejects this exactly absorbed finite-score
                // fixture; loading reuse must preserve that failure boundary.
                assert_eq!(expected.unwrap_err(), actual.unwrap_err());
                continue;
            }
            let expected = expected
                .unwrap_or_else(|error| panic!("weighted={weighted} case={case}: {error:?}"));
            let actual = actual.unwrap();
            assert_eq!(expected.fit().coefficients(), actual.fit().coefficients());
            assert_eq!(expected.covariance(), actual.covariance());
            assert_eq!(expected.statistics(), actual.statistics());
        }
    }
}

#[test]
fn weighted_loaded_rhs_preserves_sample_domains_and_inference() {
    let (_, mut base) = fixture(OlsVceKind::Iid);
    base.nobs = 16_387;
    base.nvars = 11;
    base.numeric_rows_enabled = true;
    base.data.clear();
    for row in 1..=base.nobs {
        for variable in 1..=11 {
            let value = match variable {
                1 => 1.0,
                9 => (row % 17) as f64,
                10 => ((row / 17) % 13) as f64,
                11 => (1 + row % 7) as f64,
                _ => ((row * variable as usize) as f64).sin(),
            };
            base.data.insert((variable, row), (value, false));
        }
    }
    let mut template = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: Vec::new(),
            terms: Vec::new(),
        },
        bindings: vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ],
        absorbed_terms: Vec::new(),
        include_intercept: true,
        keep_singletons: false,
    };
    for id in 1..=8 {
        let source = SourceId::new(id).unwrap();
        template.model.sources.push(SourceEntry {
            id: source,
            canonical_name: format!("x{id}"),
        });
        template.bindings.push(OlsInputBinding {
            stata_var_position: id + 2,
            role: OlsInputRole::NumericSource(source),
        });
        if id <= 6 {
            let mut term = continuous_term(source);
            term.id = TermId::new(id).unwrap();
            term.order = TermOrder::new(id).unwrap();
            term.coefficient_slot = CoefficientSlot::new(id).unwrap();
            term.canonical_token = format!("c.x{id}");
            template.model.terms.push(term);
        } else {
            template.absorbed_terms.push(AbsorbedTerm {
                id: AbsorbTermId::new(id - 6).unwrap(),
                intercept: true,
                categorical_sources: vec![source],
                slope_sources: Vec::new(),
                label: format!("g{id}"),
                saved_name: String::new(),
            });
        }
    }
    for k in [1, 3, 5, 6] {
        for role in [
            OlsInputRole::AnalyticWeight,
            OlsInputRole::FrequencyWeight,
            OlsInputRole::ProbabilityWeight,
        ] {
            for case in 0..9 {
                let mut request = template.clone();
                request.model.terms.truncate(k);
                request
                    .model
                    .sources
                    .retain(|source| source.id.get() <= k as u32 || source.id.get() >= 7);
                request.bindings.retain(|binding| {
                    !matches!(binding.role,
                OlsInputRole::NumericSource(id) if id.get() > k as u32 && id.get() < 7)
                });
                let compact_id = |id: SourceId| {
                    if id.get() >= 7 {
                        SourceId::new(id.get() - 6 + k as u32).unwrap()
                    } else {
                        id
                    }
                };
                for source in &mut request.model.sources {
                    source.id = compact_id(source.id);
                }
                for binding in &mut request.bindings {
                    if let OlsInputRole::NumericSource(id) = &mut binding.role {
                        *id = compact_id(*id);
                    }
                }
                for effect in &mut request.absorbed_terms {
                    effect.categorical_sources[0] = compact_id(effect.categorical_sources[0]);
                }
                request.bindings.push(OlsInputBinding {
                    stata_var_position: 11,
                    role,
                });
                let mut spi = base.clone();
                let candidate_all = case != 5;
                match case {
                    1 => {
                        spi.data.insert((11, 3), (1e100, true));
                    }
                    2 => {
                        spi.data.insert((11, 3), (0.0, false));
                    }
                    3 => {
                        spi.data.insert((11, 3), (-1.0, false));
                    }
                    4 => {
                        spi.data.insert((11, 3), (1.5, false));
                    }
                    5 => {
                        spi.data.insert((1, 3), (0.0, false));
                        spi.data.insert((11, 3), (-1.0, false));
                    }
                    8 => {
                        // Missing weight removes one row of a two-row FE level;
                        // physical singleton pruning then invalidates the speculative RHS.
                        spi.data.insert((9, 1), (100.0, false));
                        spi.data.insert((9, 2), (100.0, false));
                        spi.data.insert((11, 1), (1e100, true));
                        spi.data.insert((11, 2), (1.0, false));
                    }
                    _ => {}
                }
                let budget = if case == 6 { 0 } else { spi.nobs * (k + 2) * 8 };
                let threads =
                    ThreadConfig::from_requested(if case == 7 { 4 } else { 8 }, false).unwrap();
                let compiled_width =
                    numeric_loaded_rhs_layout(&request.validate_and_compile().unwrap());
                let reference =
                    load_ols_input(&request, candidate_all, false, &mut spi.clone()).unwrap();
                let mut rhs = None;
                let mut memory = loading_memory(&request, spi.nobs, 0, 0, None).unwrap();
                let loaded = load_ols_input_with_team(
                    &request,
                    candidate_all,
                    false,
                    &mut memory,
                    &mut spi,
                    Some(ThreadTeam::new(threads)),
                    &mut rhs,
                    budget,
                    compiled_width,
                    false,
                )
                .unwrap();
                assert_eq!(loaded, reference);
                assert_eq!(
                    rhs.is_some(),
                    candidate_all
                        && budget > 0
                        && fereg_core::parallel::StagePlan::new(
                            spi.nobs as u128 * (k + 3) as u128,
                            2,
                            threads.resolved()
                        )
                        .lanes
                            > 1
                );
                let expected = prepare_model_owned(&request, &[], reference, true);
                let actual = prepare_model_owned(&request, &[], loaded, true);
                match (expected, actual) {
                    (Err(expected), Err(actual)) => assert_eq!(actual, expected),
                    (Ok(expected), Ok(actual)) => {
                        assert_eq!(expected.sample(), actual.sample());
                        let mut config = PreparedOlsRobustConfig::default();
                        config.fit.execution.threads = threads;
                        let expected =
                            fereg_core::ols::fit_prepared_model_robust_with_loaded_moments(
                                &expected, config, None,
                            )
                            .unwrap();
                        let actual =
                            fereg_core::ols::fit_prepared_model_robust_with_loaded_moments(
                                &actual,
                                config,
                                rhs.map(LoadedOlsRows::from),
                            )
                            .unwrap();
                        assert_eq!(expected.fit().coefficients(), actual.fit().coefficients());
                        assert_eq!(expected.covariance(), actual.covariance());
                        assert_eq!(expected.statistics(), actual.statistics());
                        assert_eq!(expected.certificate(), actual.certificate());
                    }
                    _ => panic!("weighted speculative loading changed preparation success"),
                }
            }
        }
    }
}

#[test]
fn full_sample_output_batch_preserves_store_order_and_failure_atomicity() {
    for n in [4095, 4096, 4097] {
        for missing in [false, true] {
            let (mut args, mut base) = saved_output_fixture();
            replace_argument(&mut args, "candidate_all", "candidate_all=1".to_owned());
            replace_argument(
                &mut args,
                "output_drop_reason",
                "output_drop_reason=0".to_owned(),
            );
            replace_argument(&mut args, "output_xb", "output_xb=7".to_owned());
            replace_argument(&mut args, "output_xbd", "output_xbd=8".to_owned());
            replace_argument(&mut args, "output_absorbed", "output_absorbed=9".to_owned());
            replace_argument(
                &mut args,
                "output_saved_channels",
                "output_saved_channels=2:10".to_owned(),
            );
            base.nvars = 10;
            for row in 7..=n {
                for position in 1..=5 {
                    base.data
                        .insert((position, row), base.data[&(position, (row - 1) % 6 + 1)]);
                }
            }
            base.nobs = n;
            if missing {
                base.data.insert((2, 3), (0.0, true));
            }
            base.stores.reserve(5 * n + 512);
            let mut scalar = base.clone();
            estimate_ols_test(&args, &mut scalar).unwrap();
            let mut batched = base.clone();
            batched.numeric_column_stores_enabled = true;
            estimate_ols_test(&args, &mut batched).unwrap();
            assert_eq!(
                batched.numeric_column_batches,
                usize::from(n >= 4096 && !missing)
            );
            let observation_events = |spi: &FakeSpi| {
                spi.stores
                    .iter()
                    .take_while(|event| matches!(event, StoreEvent::Numeric(..)))
                    .cloned()
                    .collect::<Vec<_>>()
            };
            assert_eq!(observation_events(&scalar), observation_events(&batched));
            for name in ["RB", "RV", "RSTATUS", "RFIT", "RDOF", "RFE", "RCERT"] {
                assert_eq!(scalar.matrices[name], batched.matrices[name], "{name}");
            }
            assert_eq!(batched.receipt_complete(), 1.0);
            if n == 4096 && !missing {
                let numeric = observation_events(&batched).len();
                for failure in [1, 2, 1024, numeric, numeric + 1, batched.store_calls] {
                    let mut spi = base.clone();
                    spi.numeric_column_stores_enabled = true;
                    spi.fail_store_at = Some(failure);
                    assert!(estimate_ols_test(&args, &mut spi).is_err());
                    assert_eq!(spi.store_calls, failure);
                    assert_eq!(spi.receipt_complete(), 0.0);
                    assert_eq!(spi.numeric_column_batches, 1);
                }
            }
        }
    }
}

#[test]
fn saved_fe_and_prediction_outputs_are_native_identity_consistent() {
    let (args, mut spi) = saved_output_fixture();
    estimate_ols_test(&args, &mut spi).expect("saved-output estimate succeeds");

    let mut saved_sum = 0.0;
    for observation in 1..=spi.nobs {
        let y = spi.data[&(2, observation)].0;
        let sample = stored_numeric(&spi, 5, observation);
        let residual = stored_numeric(&spi, 6, observation);
        let reason = stored_numeric(&spi, 7, observation);
        let xb = stored_numeric(&spi, 8, observation);
        let xbd = stored_numeric(&spi, 9, observation);
        let absorbed = stored_numeric(&spi, 10, observation);
        let saved = stored_numeric(&spi, 11, observation);
        assert_eq!(sample, 1.0);
        assert_eq!(reason, 0.0);
        assert!((saved - absorbed).abs() <= 1e-11);
        assert!((xbd - xb - absorbed).abs() <= 1e-11);
        assert!((y - xbd - residual).abs() <= 1e-11);
        saved_sum += saved;
    }
    assert!(saved_sum.abs() <= 1e-11);
    assert_eq!(stored_numeric(&spi, 11, 1), stored_numeric(&spi, 11, 2));
    assert_eq!(stored_numeric(&spi, 11, 3), stored_numeric(&spi, 11, 4));
    assert_eq!(stored_numeric(&spi, 11, 5), stored_numeric(&spi, 11, 6));
    assert_eq!(spi.receipt_complete(), 1.0);
    assert_eq!(
        spi.stores.last(),
        Some(&StoreEvent::Matrix(14, 1, RECEIPT_COMPLETE_COLUMN, 1.0)),
    );
}

#[test]
fn saved_only_fast_path_matches_fully_materialized_outputs() {
    let (full_args, mut full_spi) = saved_output_fixture();
    estimate_ols_test(&full_args, &mut full_spi).expect("full saved-output estimate succeeds");

    let (mut lean_args, mut lean_spi) = saved_output_fixture();
    replace_argument(&mut lean_args, "output_xb", "output_xb=0".to_owned());
    replace_argument(&mut lean_args, "output_xbd", "output_xbd=0".to_owned());
    replace_argument(
        &mut lean_args,
        "output_absorbed",
        "output_absorbed=0".to_owned(),
    );
    replace_argument(
        &mut lean_args,
        "output_saved_channels",
        "output_saved_channels=1:8".to_owned(),
    );
    lean_spi.nvars = 8;
    estimate_ols_test(&lean_args, &mut lean_spi).expect("saved-only estimate succeeds");

    for observation in 1..=full_spi.nobs {
        assert_eq!(
            stored_numeric(&lean_spi, 8, observation),
            stored_numeric(&full_spi, 11, observation)
        );
    }
    assert_eq!(lean_spi.receipt_complete(), 1.0);
}

#[test]
fn saved_output_store_failures_leave_receipt_incomplete() {
    let (args, mut successful) = saved_output_fixture();
    estimate_ols_test(&args, &mut successful).expect("saved-output baseline succeeds");
    let observation_stores = successful
        .stores
        .iter()
        .take_while(|event| matches!(event, StoreEvent::Numeric(..)))
        .count();
    assert_eq!(observation_stores, 36);
    for failing_ordinal in 1..=observation_stores {
        let (args, mut spi) = saved_output_fixture();
        spi.fail_store_at = Some(failing_ordinal);
        let error = estimate_ols_test(&args, &mut spi).expect_err("store failure propagates");
        assert_eq!(error.code, ErrorCode::Internal, "failure {failing_ordinal}");
        assert_eq!(spi.receipt_complete(), 0.0, "failure {failing_ordinal}");
        assert_eq!(spi.store_calls, failing_ordinal);
    }
}

#[test]
fn fixed_width_string_cluster_is_encoded_natively_and_strl_fails_pre_read() {
    let (args, mut spi) = fixture(OlsVceKind::Clustered);
    spi.string_variables.insert(4);
    for (observation, value) in ["b", "a", "b", "c", "a", "c"].into_iter().enumerate() {
        spi.data.remove(&(4, observation + 1));
        spi.string_data
            .insert((4, observation + 1), value.as_bytes().to_vec());
    }
    estimate_ols_test(&args, &mut spi).expect("fixed-width string cluster succeeds");
    assert_eq!(spi.numeric_reads, 18);
    assert_eq!(spi.matrices["RREC"][0][6], 6.0);
    assert_eq!(spi.matrices["RCLUSTER"][0][2], 3.0);
    assert_eq!(spi.receipt_complete(), 1.0);

    let (args, mut strl) = fixture(OlsVceKind::Clustered);
    strl.string_variables.insert(4);
    strl.strl_variables.insert(4);
    let error = estimate_ols_test(&args, &mut strl).expect_err("strL must fail closed");
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert_eq!(strl.numeric_reads, 0);
    assert_eq!(strl.store_calls, 0);
}

#[test]
fn fixed_width_string_codes_are_byte_exact_sorted_and_missing_aware() {
    let (_, mut spi) = fixture(OlsVceKind::Iid);
    spi.string_data = BTreeMap::from([
        ((3, 1), b"a#b".to_vec()),
        ((3, 2), b"a".to_vec()),
        ((3, 3), b"".to_vec()),
        ((3, 4), b"b".to_vec()),
        ((3, 5), b"a#b".to_vec()),
    ]);
    let mut loader = OlsSourceLoader::FixedWidthString {
        column: empty_loaded_column(5, "test string codes").expect("column"),
        provisional_codes: StringDictionary::new(),
    };
    let mut memory = InputMemory::new(None, 0).unwrap();
    for observation in 1..=5 {
        loader
            .read_into(&mut spi, 3, observation, observation - 1, &mut memory)
            .expect("string read");
    }
    let encoded = loader.finalize(&mut memory).expect("deterministic recode");
    assert_eq!(encoded.values, vec![1.0, 0.0, 0.0, 2.0, 1.0]);
    assert_eq!(encoded.missing, vec![false, false, true, false, false]);
}

#[test]
fn string_loading_budget_and_interruption_leave_no_public_stores() {
    fn run(usable: Option<u128>, interrupted: bool) -> Result<(LoadedOlsInput, u128)> {
        let (args, mut spi) = fixture(OlsVceKind::Clustered);
        spi.string_variables.insert(4);
        for row in 1..=6 {
            spi.data.remove(&(4, row));
            spi.string_data
                .insert((4, row), format!("group-{}", row % 3).into_bytes());
        }
        let spec = EstimateActionSpec::parse(&args)?;
        let action = spec.decode_and_validate(&mut spi)?;
        spi.interrupt_checks = 0;
        spi.interrupt_at = interrupted.then_some(1);
        let mut memory = loading_memory(
            &action.request,
            spi.nobs,
            0,
            action.metadata_capacity_bytes(),
            usable,
        )?;
        let result = load_ols_input_with_team(
            &action.request,
            false,
            false,
            &mut memory,
            &mut spi,
            None,
            &mut None,
            0,
            None,
            false,
        );
        assert_eq!(spi.store_calls, 0);
        Ok((result?, memory.peak()))
    }
    let (original, peak) = run(None, false).unwrap();
    let (bounded, same_peak) = run(Some(peak), false).unwrap();
    assert_eq!(original, bounded);
    assert_eq!(peak, same_peak);
    assert_eq!(
        run(Some(peak - 1), false).unwrap_err().code,
        ErrorCode::Allocation
    );
    assert_eq!(
        run(Some(peak), true).unwrap_err().code,
        ErrorCode::Interrupted
    );
}

#[test]
#[cfg(feature = "ffi")]
fn decoder_admission_precedes_allocations_and_bounds_live_compiler_frames() {
    let _guard = crate::instrumentation::lock_action_observation();
    for fixture in [
        fixture(OlsVceKind::Iid),
        fixture(OlsVceKind::Clustered),
        saved_output_fixture(),
    ] {
        let (args, mut spi) = fixture;
        let spec = EstimateActionSpec::parse(&args).unwrap();
        let bound = spec.metadata_shape().peak_bytes().unwrap();
        let (start, _) = crate::ffi::begin_ols_allocation_observation();
        let action = spec
            .decode_with_memory_limit(&mut spi, || Ok(None))
            .unwrap();
        let observed = crate::ffi::allocation_usage()
            .peak_bytes
            .saturating_sub(start.current_bytes);
        assert!(
            u128::from(observed) <= bound,
            "decoder {observed} exceeds {bound}"
        );
        drop(action);
        // These fixtures are below the 20% headroom crossover, so this
        // supplies exactly one byte less than the decoder's admission.
        assert!(bound < 512 * 1024 * 1024);
        let (start, _) = crate::ffi::begin_ols_allocation_observation();
        let error = spec
            .decode_with_memory_limit(&mut spi, || Ok(Some(512 * 1024 * 1024 + bound - 1)))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(spi.numeric_reads, 0);
        assert_eq!(spi.store_calls, 0);
        let observed = crate::ffi::allocation_usage()
            .peak_bytes
            .saturating_sub(start.current_bytes);
        assert!(observed < 1024, "budget failure allocated decoder storage");
    }
}

#[cfg(feature = "ffi")]
#[test]
fn malformed_cluster_arity_cannot_reserve_outside_wire_admission() {
    let _guard = crate::instrumentation::lock_action_observation();
    let (args, mut spi) = fixture(OlsVceKind::Clustered);
    let spec = EstimateActionSpec::parse(&args).unwrap();
    spi.matrices.get_mut(&spec.cluster_term_matrix).unwrap()[0][1] = 1e12;
    let (start, _) = crate::ffi::begin_ols_allocation_observation();
    let error = spec
        .decode_with_memory_limit(&mut spi, || Ok(None))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    let peak = crate::ffi::allocation_usage().peak_bytes - start.current_bytes;
    assert!(u128::from(peak) < spec.metadata_shape().peak_bytes().unwrap());
    assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
}

#[test]
fn native_coefficient_shadow_certification_rejects_every_scatter_family() {
    validate_shadow_coefficients(&coefficient_shadow()).expect("valid native shadow");

    let mut duplicate = coefficient_shadow();
    duplicate.coefficient_status[7] = 1.0;
    assert!(validate_shadow_coefficients(&duplicate).is_err());

    let mut asymmetric = coefficient_shadow();
    asymmetric.covariance[1] += 1.0;
    assert!(validate_shadow_coefficients(&asymmetric).is_err());

    let mut negative_diagonal = coefficient_shadow();
    negative_diagonal.covariance[0] = -1.0;
    assert!(validate_shadow_coefficients(&negative_diagonal).is_err());

    let mut marker = coefficient_shadow();
    marker.omission_v_marker[1] = 1.0;
    assert!(validate_shadow_coefficients(&marker).is_err());

    let mut omitted = coefficient_shadow();
    omitted.coefficient_status[1] = 2.0;
    omitted.coefficient_status[3] = 0.0;
    omitted.b[0] = 0.0;
    omitted.omission_b_marker[0] = 0.0;
    omitted.omission_v_marker[0] = 0.0;
    assert!(validate_shadow_coefficients(&omitted).is_err());
}

#[test]
fn native_defined_value_table_certification_rejects_malformed_rows() {
    validate_defined_value_table(&[1.0, 2.0, 0.0, 0.0], "test").expect("valid table");
    for malformed in [
        vec![2.0, 0.0],
        vec![0.0, 1.0],
        vec![1.0, f64::NAN],
        vec![1.0],
    ] {
        assert!(validate_defined_value_table(&malformed, "test").is_err());
    }
}

#[test]
fn licensed_stata_bundle_matches_ordinary_argv_and_rejects_malformed_pre_read() {
    let (args, mut ordinary) = fixture(OlsVceKind::Iid);
    estimate_ols_test(&args, &mut ordinary).expect("ordinary argv succeeds");

    let (args, mut packed) = fixture(OlsVceKind::Iid);
    estimate_ols_test(&bundled(&args), &mut packed).expect("bundle succeeds");
    let (args, mut local) = fixture(OlsVceKind::Iid);
    let bundle = bundled(&args)
        .pop()
        .unwrap()
        .strip_prefix("bundle=")
        .unwrap()
        .as_bytes()
        .to_vec();
    let local_args = [format!("bundle_local={}", bundle.len())];
    local.local_bundle = Some(bundle);
    estimate_ols_test(&local_args, &mut local).expect("local bundle succeeds");
    assert_eq!(local.macro_reads, 1);
    assert_eq!(ordinary.macro_reads, 0);
    assert_eq!(packed.macro_reads, 0);
    for name in [
        "RB", "RV", "RSTATUS", "RMARKB", "RMARKV", "RFIT", "RDOF", "RFE", "RCLUSTER", "RSUBSET",
        "RCERT", "RREC",
    ] {
        assert_eq!(packed.matrices[name], ordinary.matrices[name], "{name}");
        assert_eq!(
            local.matrices[name], ordinary.matrices[name],
            "local {name}"
        );
    }
    for runtime in [
        &ordinary.matrices["RRUNTIME"],
        &packed.matrices["RRUNTIME"],
        &local.matrices["RRUNTIME"],
    ] {
        assert_eq!(runtime[0], [1.0, 1.0]);
        assert_eq!(runtime[1], [1.0, 1.0]);
        assert_eq!(runtime[2], [1.0, 10.0]);
        assert_eq!(runtime[21], [1.0, 1e-8]);
        assert_eq!(runtime[22], [1.0, 1e-8]);
        assert_eq!(runtime[23], [1.0, 16_000.0]);
        assert!(runtime
            .iter()
            .all(|row| matches!(row[0], 0.0 | 1.0) && row[1].is_finite()));
    }
    for row in 0..SOLVER_ROWS {
        for column in 0..SOLVER_COLUMNS {
            if !matches!(column, 8 | 9) {
                assert_eq!(
                    packed.matrices["RSOLVER"][row][column],
                    ordinary.matrices["RSOLVER"][row][column]
                );
            }
        }
    }
    assert_eq!(packed.store_calls, ordinary.store_calls);
    assert_eq!(packed.stores.last(), ordinary.stores.last());
    assert_eq!(packed.numeric_reads, ordinary.numeric_reads);
    assert_eq!(local.numeric_reads, ordinary.numeric_reads);
    assert_eq!(local.stores.last(), ordinary.stores.last());
    assert_eq!(local.store_calls, ordinary.store_calls);

    let malformed = [
        "bundle=0:",
        "bundle=01:3:a=b",
        "bundle=1:03:a=b",
        "bundle=1:4:a=b",
        "bundle=1:3:a=bx",
        "bundle=1:0:",
        "bundle=1:x:a=b",
        "bundle=1:1:\u{00e9}",
        "bundle=2:3:a=b3:a=b",
        "bundle=1:3:abc",
    ];
    for argument in malformed {
        let (_, mut spi) = fixture(OlsVceKind::Iid);
        let error = estimate_ols_test(&[argument.to_owned()], &mut spi)
            .expect_err("malformed or incomplete bundle is rejected");
        assert!(matches!(
            error.code,
            ErrorCode::Syntax | ErrorCode::InvalidArgument
        ));
        assert_eq!(spi.numeric_reads, 0, "{argument}");
        assert_eq!(spi.store_calls, 0, "{argument}");
        assert_eq!(spi.receipt_complete(), 0.0, "{argument}");
    }

    let (args, mut unknown_spi) = fixture(OlsVceKind::Iid);
    let mut unknown = args;
    unknown.push("surprise=1".to_owned());
    let error = estimate_ols_test(&bundled(&unknown), &mut unknown_spi)
        .expect_err("unknown bundled field is rejected");
    assert_eq!(error.code, ErrorCode::Syntax);
    assert_eq!(unknown_spi.numeric_reads, 0);
    assert_eq!(unknown_spi.store_calls, 0);

    let (args, mut mixed_spi) = fixture(OlsVceKind::Iid);
    let mut mixed = bundled(&args);
    mixed.push("ado=0.5.0".to_owned());
    let error =
        estimate_ols_test(&mixed, &mut mixed_spi).expect_err("mixed bundle and argv are rejected");
    assert_eq!(error.code, ErrorCode::Syntax);
    assert_eq!(mixed_spi.numeric_reads, 0);
    assert_eq!(mixed_spi.store_calls, 0);
}

#[test]
fn local_estimation_bundle_validates_capacity_bytes_and_framing_before_reads() {
    for descriptor in [
        "",
        "0",
        "01",
        "+1",
        "-1",
        "1.0",
        "1 ",
        "16777217",
        "999999999999999999999999",
    ] {
        let (_, mut spi) = fixture(OlsVceKind::Iid);
        assert!(estimate_ols_test(&[format!("bundle_local={descriptor}")], &mut spi).is_err());
        assert_eq!(
            (spi.macro_reads, spi.numeric_reads, spi.store_calls),
            (0, 0, 0)
        );
    }
    let (_, mut spi) = fixture(OlsVceKind::Iid);
    assert!(estimate_ols_test(
        &["bundle_local=6".to_owned(), "ado=0.5.0".to_owned()],
        &mut spi
    )
    .is_err());
    assert_eq!(spi.macro_reads, 0);
    for (payload, declared) in [
        (None, 6),
        (Some(b"1:3:a=b".to_vec()), 6),
        (Some(b"1:3:a=b".to_vec()), 8),
        (Some(b"1:3:a\0b".to_vec()), 7),
        (Some(vec![0xff; 7]), 7),
        (Some(b"1:9:a=b".to_vec()), 7),
        (Some(b"1:3:a=b".to_vec()), 7),
    ] {
        let (_, mut spi) = fixture(OlsVceKind::Iid);
        spi.local_bundle = payload;
        assert!(estimate_ols_test(&[format!("bundle_local={declared}")], &mut spi).is_err());
        assert_eq!(spi.macro_reads, 1);
        assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
        assert_eq!(spi.receipt_complete(), 0.0);
    }
    let (args, mut overlong) = fixture(OlsVceKind::Iid);
    let mut payload = format!("{}:", args.len());
    for value in &args {
        payload.push_str(&format!("{}:{value}", value.len()));
    }
    let declared = payload.len();
    payload.push_str("trailing-data");
    overlong.local_bundle = Some(payload.into_bytes());
    assert!(estimate_ols_test(&[format!("bundle_local={declared}")], &mut overlong).is_err());
    assert_eq!(
        (
            overlong.macro_reads,
            overlong.numeric_reads,
            overlong.store_calls
        ),
        (1, 0, 0)
    );

    let (_, mut spi) = fixture(OlsVceKind::Iid);
    let payload = "1:4:a=é";
    spi.local_bundle = Some(payload.as_bytes().to_vec());
    assert_eq!(
        read_local_estimate_bundle(&[format!("bundle_local={}", payload.len())], &mut spi).unwrap(),
        Some(format!("bundle={payload}"))
    );
}

#[test]
fn every_output_store_failure_leaves_receipt_incomplete() {
    let (args, mut successful) = fixture(OlsVceKind::Iid);
    estimate_ols_test(&args, &mut successful).expect("baseline estimate succeeds");
    let total_stores = successful.store_calls;
    assert_eq!(total_stores, 284 + crate::memory_report::CELLS);
    assert_eq!(successful.stores.capacity(), 512);

    for failing_ordinal in 1..=total_stores {
        let (args, mut spi) = fixture(OlsVceKind::Iid);
        let prepared_capacity = spi.stores.capacity();
        spi.fail_store_at = Some(failing_ordinal);
        let error = estimate_ols_test(&args, &mut spi).expect_err("store failure propagates");
        assert_eq!(error.code, ErrorCode::Internal, "failure {failing_ordinal}");
        assert_eq!(spi.receipt_complete(), 0.0, "failure {failing_ordinal}");
        assert_eq!(spi.store_calls, failing_ordinal);
        assert_eq!(spi.stores.capacity(), prepared_capacity);
    }
}

#[test]
fn nonpositive_residual_dof_stores_nothing_and_leaves_receipt_incomplete() {
    let (args, mut spi) = fixture(OlsVceKind::Iid);
    spi.nobs = 2;
    let error = estimate_ols_test(&args, &mut spi)
        .expect_err("a saturated model must fail before result storage");
    assert_eq!(error.code, ErrorCode::InsufficientObservations);
    assert_eq!(spi.store_calls, 0);
    assert!(spi.stores.is_empty());
    assert_eq!(spi.receipt_complete(), 0.0);
}

#[test]
fn omitted_factor_slot_is_zero_scattered_and_noconstant_layout_is_exact() {
    let (factor_args, mut factor_spi) = factor_scatter_fixture();
    estimate_ols_test(&factor_args, &mut factor_spi).expect("factor estimate succeeds");
    assert_eq!(factor_spi.matrices["RREC"][0][4], 3.0);
    assert_eq!(factor_spi.matrices["RB"][0][0], 0.0);
    assert_eq!(factor_spi.matrices["RSTATUS"][0][0], 1.0);
    assert_eq!(factor_spi.matrices["RSTATUS"][0][1], 2.0);
    assert_eq!(factor_spi.matrices["RSTATUS"][0][3], 0.0);
    assert_eq!(factor_spi.matrices["RMARKB"][0][0], 0.0);
    assert_eq!(factor_spi.matrices["RMARKV"][0][0], 0.0);
    for index in 0..3 {
        assert_eq!(factor_spi.matrices["RV"][0][index], 0.0);
        assert_eq!(factor_spi.matrices["RV"][index][0], 0.0);
    }
    assert!(matches!(factor_spi.matrices["RSTATUS"][1][1], 1.0 | 5.0));
    assert!(matches!(factor_spi.matrices["RSTATUS"][2][1], 1.0 | 5.0));
    for index in 1..3 {
        assert_eq!(factor_spi.matrices["RMARKB"][0][index], 1.0);
        assert_eq!(factor_spi.matrices["RMARKV"][index][index], 1.0);
    }

    let (mut noconstant_args, mut noconstant_spi) = fixture(OlsVceKind::Iid);
    replace_argument(
        &mut noconstant_args,
        "include_intercept",
        "include_intercept=0".to_owned(),
    );
    noconstant_spi
        .matrices
        .insert("RB".to_owned(), zero_matrix(1, 1));
    noconstant_spi
        .matrices
        .insert("RV".to_owned(), zero_matrix(1, 1));
    noconstant_spi.matrices.insert(
        "RSTATUS".to_owned(),
        zero_matrix(1, COEFFICIENT_STATUS_COLUMNS),
    );
    noconstant_spi
        .matrices
        .insert("RMARKB".to_owned(), zero_matrix(1, 1));
    noconstant_spi
        .matrices
        .insert("RMARKV".to_owned(), zero_matrix(1, 1));
    estimate_ols_test(&noconstant_args, &mut noconstant_spi).expect("noconstant estimate succeeds");
    assert_eq!(noconstant_spi.matrices["RREC"][0][4], 1.0);
    assert_eq!(noconstant_spi.matrices["RREC"][0][5], 1.0);
    assert_eq!(noconstant_spi.matrices["RSTATUS"][0][1], 1.0);
    assert_eq!(noconstant_spi.receipt_complete(), 1.0);
}

#[test]
fn handshake_layout_and_interrupt_fail_before_any_output_write() {
    let (mut handshake_args, mut handshake_spi) = fixture(OlsVceKind::Iid);
    handshake_args
        .iter_mut()
        .find(|argument| argument.starts_with("plugin="))
        .expect("plugin argument")
        .clone_from(&"plugin=wrong".to_owned());
    let error = estimate_ols_test(&handshake_args, &mut handshake_spi)
        .expect_err("plugin mismatch rejected");
    assert_eq!(error.code, ErrorCode::ProtocolMismatch);
    assert_eq!(handshake_spi.numeric_reads, 0);
    assert_eq!(handshake_spi.store_calls, 0);

    let (mut old_action_args, mut old_action_spi) = fixture(OlsVceKind::Iid);
    replace_argument(
        &mut old_action_args,
        "protocol_minor",
        "protocol_minor=0".to_owned(),
    );
    let error = estimate_ols_test(&old_action_args, &mut old_action_spi)
        .expect_err("pre-estimation action protocol rejected");
    assert_eq!(error.code, ErrorCode::ProtocolMismatch);
    assert_eq!(old_action_spi.numeric_reads, 0);
    assert_eq!(old_action_spi.store_calls, 0);

    let (mut old_result_args, mut old_result_spi) = fixture(OlsVceKind::Iid);
    replace_argument(
        &mut old_result_args,
        "result_minor",
        "result_minor=0".to_owned(),
    );
    let error = estimate_ols_test(&old_result_args, &mut old_result_spi)
        .expect_err("pre-rebase result wire rejected");
    assert_eq!(error.code, ErrorCode::ProtocolMismatch);
    assert_eq!(old_result_spi.numeric_reads, 0);
    assert_eq!(old_result_spi.store_calls, 0);

    let (layout_args, mut layout_spi) = fixture(OlsVceKind::Iid);
    layout_spi
        .matrices
        .insert("RV".to_owned(), zero_matrix(1, 2));
    let error =
        estimate_ols_test(&layout_args, &mut layout_spi).expect_err("bad result layout rejected");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(layout_spi.numeric_reads, 0);
    assert_eq!(layout_spi.store_calls, 0);

    let (stale_args, mut stale_spi) = fixture(OlsVceKind::Iid);
    stale_spi.matrices.get_mut("RREC").expect("receipt")[0][RECEIPT_COMPLETE_COLUMN - 1] = 1.0;
    let error = estimate_ols_test(&stale_args, &mut stale_spi)
        .expect_err("stale completed receipt rejected");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(stale_spi.numeric_reads, 0);
    assert_eq!(stale_spi.store_calls, 0);

    let (sentinel_args, mut sentinel_spi) = fixture(OlsVceKind::Iid);
    sentinel_spi.matrices.get_mut("CT").expect("cluster terms")[0][0] = 1.0;
    let error = estimate_ols_test(&sentinel_args, &mut sentinel_spi)
        .expect_err("nonzero empty-cluster sentinel rejected");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(sentinel_spi.numeric_reads, 0);
    assert_eq!(sentinel_spi.store_calls, 0);

    let (interrupt_args, mut interrupt_spi) = fixture(OlsVceKind::Iid);
    let (baseline_args, mut baseline_spi) = fixture(OlsVceKind::Iid);
    estimate_ols_test(&baseline_args, &mut baseline_spi).expect("baseline estimate succeeds");
    interrupt_spi.interrupt_at = Some(baseline_spi.interrupt_checks);
    let error = estimate_ols_test(&interrupt_args, &mut interrupt_spi)
        .expect_err("last precommit interrupt propagates");
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert_eq!(interrupt_spi.store_calls, 0);
    assert_eq!(interrupt_spi.receipt_complete(), 0.0);
}

#[test]
fn shadow_fe_identity_accepts_intercept_interaction_and_slope_rows() {
    for row in [
        [1.0, 1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 0.0],
        [2.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0],
        [3.0, 1.0, 0.0, 6.0, 1.0, 5.0, 1.0, 0.0],
        [4.0, 1.0, 3.0, 8.0, 0.0, 8.0, 1.0, 0.0],
    ] {
        validate_shadow_fixed_effect_row(&row).expect("valid FE shadow row");
    }
}

#[test]
fn shadow_fe_identity_rejects_malformed_combinations() {
    for row in [
        [1.0, 1.0, 0.0, 3.0, 1.0, 2.0, 1.0, 0.0],
        [2.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0],
        [3.0, 0.0, 0.0, 6.0, 1.0, 5.0, 1.0, 0.0],
        [3.0, 1.0, 2.0, 6.0, 1.0, 5.0, 1.0, 0.0],
        [3.0, 1.0, 0.0, 2.0, 3.0, 0.0, 1.0, 0.0],
        [3.0, 1.0, 0.0, 2.0, 1.0, 2.0, 1.0, 0.0],
        [4.0, 0.0, 3.0, 8.0, 0.0, 8.0, 1.0, 0.0],
        [4.0, 1.0, 0.0, 8.0, 0.0, 8.0, 1.0, 0.0],
        [4.0, 1.0, 3.0, 8.0, 1.0, 8.0, 1.0, 0.0],
    ] {
        assert!(validate_shadow_fixed_effect_row(&row).is_err());
    }
}
