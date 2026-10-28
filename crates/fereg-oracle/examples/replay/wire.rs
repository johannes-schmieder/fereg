use fereg_core::{
    CompiledModelSpecification, ModelWireV2, NumericWireTable, OlsInputRole, SourceEntry, SourceId,
};
use fereg_oracle::designs::Design;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub struct Request {
    args: Vec<String>,
    data: Vec<Vec<f64>>,
    matrices: Vec<(String, usize, usize, Vec<f64>)>,
    pub outputs: Vec<(String, usize)>,
    pub saved: Vec<usize>,
    /// No residual, xb, xbd or absorbed output: the derived-residual path.
    pub derived_residual: bool,
}
impl Request {
    pub(super) fn replace(&mut self, key: &str, value: &str) {
        self.args.retain(|arg| !arg.starts_with(&format!("{key}=")));
        self.arg(key, value);
    }
    pub(super) fn arg(&mut self, key: &str, value: impl std::fmt::Display) {
        self.args.push(format!("{key}={value}"));
    }
    fn matrix(&mut self, name: &str, rows: usize, cols: usize, values: Vec<f64>) {
        assert_eq!(rows * cols, values.len());
        self.matrices.push((name.into(), rows, cols, values));
    }
    fn table(&mut self, key: &str, name: &str, table: &NumericWireTable, width: usize) {
        self.arg(&format!("{key}_matrix"), name);
        self.arg(&format!("{key}_count"), table.declared_count);
        let rows = table.rows.len().max(1);
        let values = if table.rows.is_empty() {
            vec![0.; width]
        } else {
            table.rows.iter().flatten().copied().collect()
        };
        self.matrix(name, rows, width, values);
    }
    pub(super) fn registry(&mut self, key: &str, values: &[String]) {
        let text = if values.is_empty() {
            ".".into()
        } else {
            values
                .iter()
                .map(|s| format!("{}:{s}", s.len()))
                .collect::<Vec<_>>()
                .join("|")
        };
        self.arg(key, text);
    }
    fn result(&mut self, key: &str, name: &str, rows: usize, cols: usize) {
        self.arg(&format!("output_{key}_matrix"), name);
        self.matrix(name, rows, cols, vec![0.; rows * cols]);
    }
    /// Keep only the sample and drop-reason outputs, as the public call
    /// without prediction options requests; Fast OLS may then derive its
    /// within residual. The other output variables leave the varlist.
    pub fn without_observation_outputs(&mut self) {
        assert!(
            self.saved.is_empty(),
            "saved FE outputs keep the projection"
        );
        let kept = |name: &str| matches!(name, "sample" | "drop_reason");
        let mut dropped = self
            .outputs
            .iter()
            .filter(|(name, _)| !kept(name))
            .map(|&(_, index)| index)
            .collect::<Vec<_>>();
        dropped.sort_unstable();
        for &index in dropped.iter().rev() {
            self.data.remove(index);
            let name = self
                .outputs
                .iter()
                .find(|o| o.1 == index)
                .unwrap()
                .0
                .clone();
            self.replace(&format!("output_{name}"), "0");
        }
        self.outputs.retain(|(name, _)| kept(name));
        for (name, index) in &mut self.outputs {
            *index -= dropped.iter().filter(|&&d| d < *index).count();
            let position = *index + 1;
            self.args
                .retain(|arg| !arg.starts_with(&format!("output_{name}=")));
            self.args.push(format!("output_{name}={position}"));
        }
        self.derived_residual = true;
    }
    pub fn save_effects(&mut self, design: &Design) {
        let mut positions = Vec::new();
        for effect in &design.effects {
            let scales = std::iter::once(vec![1.; design.outcome.len()])
                .filter(|_| effect.has_intercept())
                .chain(effect.slopes().iter().cloned());
            for _scale in scales {
                self.data.push(vec![
                    f64::from_bits(0x7fe0000000000000);
                    design.outcome.len()
                ]);
                self.saved.push(self.data.len() - 1);
                positions.push(self.data.len().to_string());
            }
        }
        self.args
            .retain(|arg| !arg.starts_with("absorb_saved_names="));
        self.registry(
            "absorb_saved_names",
            &(0..design.effects.len())
                .map(|j| format!("saved{j}"))
                .collect::<Vec<_>>(),
        );
        self.args
            .retain(|arg| !arg.starts_with("output_saved_channels="));
        self.registry("output_saved_channels", &positions);
    }
    pub fn bias(&mut self, design: &Design, randomized: bool) {
        self.data.push(
            (0..design.outcome.len())
                .map(|i| (i / 2 + 1) as f64)
                .collect(),
        );
        self.arg("bias_unit", self.data.len());
        self.arg(
            "bias_method",
            if randomized { "randomized" } else { "exact" },
        );
        self.arg("bias_probes", if randomized { 32 } else { 0 });
        self.arg("bias_seed", if randomized { 739391 } else { 0 });
        self.arg("bias_stream", 0);
        self.arg("output_bias_count", 0);
        self.arg("output_bias_channels", ".");
        self.result("bias", "RBIAS", 9, design.regressors.len() + 1);
        self.arg("output_bias_diagnostics", "RBDIAG");
        self.matrix("RBDIAG", 21, 1, vec![0.; 21]);
        self.arg("output_bias_support", "RBSUPPORT");
        self.matrix("RBSUPPORT", 2, 11, vec![0.; 22]);
    }
    pub fn separation(&mut self, methods: &str) {
        self.args.retain(|arg| !arg.starts_with("separation="));
        self.arg("separation", methods);
        let rows = 1 + methods.split_whitespace().count();
        let entry = self.matrices.iter_mut().find(|m| m.0 == "RSEP").unwrap();
        entry.1 = rows;
        entry.3 = vec![0.; rows * 14];
    }
    pub fn write(&self, path: &Path) {
        let mut file = BufWriter::new(File::create(path).unwrap());
        writeln!(
            file,
            "FEREG_REPLAY_V1\n{} {} {} {} 0",
            self.data[0].len(),
            self.data.len(),
            self.args.len(),
            self.matrices.len()
        )
        .unwrap();
        for arg in &self.args {
            writeln!(file, "{}\n{arg}", arg.len()).unwrap();
        }
        for column in &self.data {
            writeln!(file, "0").unwrap();
            for value in column {
                writeln!(file, "{:016x}", value.to_bits()).unwrap();
            }
        }
        for (name, rows, cols, values) in &self.matrices {
            writeln!(file, "{}\n{name}\n{rows} {cols}", name.len()).unwrap();
            for value in values {
                writeln!(file, "{:016x}", value.to_bits()).unwrap();
            }
        }
        file.flush().unwrap();
    }
}

fn inputs(design: &Design, poisson: bool) -> Request {
    let (mut model, mut loaded) = crate::support::inputs(design, false);
    let mut clusters = Vec::new();
    for (j, values) in design.clusters.iter().enumerate() {
        let id = SourceId::new((model.model.sources.len() + 1) as u32).unwrap();
        model.model.sources.push(SourceEntry {
            id,
            canonical_name: format!("cluster{j}"),
        });
        model.bindings.push(fereg_core::OlsInputBinding {
            stata_var_position: (model.bindings.len() + 1) as u32,
            role: OlsInputRole::NumericSource(id),
        });
        loaded.numeric_sources.push((
            id,
            fereg_core::LoadedNumericColumn {
                values: values.iter().map(|&v| v as f64).collect(),
                missing: vec![false; values.len()],
            },
        ));
        clusters.push(fereg_core::OlsClusterTermV1 {
            label: format!("cluster{j}"),
            sources: vec![id],
        });
    }
    model_input(&model, &loaded, &clusters, poisson)
}

pub(super) fn model_input(
    model: &fereg_core::OlsRequestV1,
    loaded: &fereg_core::LoadedOlsInput,
    clusters: &[fereg_core::OlsClusterTermV1],
    poisson: bool,
) -> Request {
    let mut request = Request {
        args: vec![if poisson {
            "estimate-ppml"
        } else {
            "estimate-ols"
        }
        .into()],
        data: vec![vec![]; model.bindings.len()],
        matrices: vec![],
        outputs: vec![],
        saved: vec![],
        derived_residual: false,
    };
    let mut binding = Vec::new();
    for b in &model.bindings {
        let (role, source, column) = match b.role {
            OlsInputRole::Candidate => (1, 0, None),
            OlsInputRole::Outcome => (2, 0, Some(&loaded.outcome)),
            OlsInputRole::NumericSource(id) => (
                3,
                id.get(),
                Some(
                    &loaded
                        .numeric_sources
                        .iter()
                        .find(|(s, _)| *s == id)
                        .unwrap()
                        .1,
                ),
            ),
            OlsInputRole::AnalyticWeight => (4, 0, loaded.analytic_weight.as_ref()),
            OlsInputRole::FrequencyWeight => (5, 0, loaded.frequency_weight.as_ref()),
            OlsInputRole::ProbabilityWeight => (6, 0, loaded.probability_weight.as_ref()),
        };
        request.data[b.stata_var_position as usize - 1] = if let Some(column) = column {
            column
                .values
                .iter()
                .zip(&column.missing)
                .map(|(&v, &m)| {
                    if m {
                        f64::from_bits(0x7fe0000000000000)
                    } else {
                        v
                    }
                })
                .collect()
        } else {
            loaded.candidate.iter().map(|&v| f64::from(v)).collect()
        };
        binding.push(vec![
            f64::from(b.stata_var_position),
            f64::from(role),
            f64::from(source),
        ]);
    }
    let wire = ModelWireV2::from_specification(&CompiledModelSpecification {
        model: model.model.clone(),
        absorbed_terms: model.absorbed_terms.clone(),
    })
    .unwrap();
    metadata(&mut request, &wire);
    request.table(
        "binding",
        "BIND",
        &NumericWireTable {
            declared_count: binding.len() as f64,
            rows: binding,
        },
        3,
    );
    let terms = clusters
        .iter()
        .enumerate()
        .map(|(j, c)| vec![(j + 1) as f64, c.sources.len() as f64])
        .collect::<Vec<_>>();
    let components = clusters
        .iter()
        .enumerate()
        .flat_map(|(j, c)| {
            c.sources
                .iter()
                .enumerate()
                .map(move |(k, s)| vec![(j + 1) as f64, (k + 1) as f64, f64::from(s.get())])
        })
        .collect::<Vec<_>>();
    request.table(
        "cluster_term",
        "CT",
        &NumericWireTable {
            declared_count: terms.len() as f64,
            rows: terms,
        },
        2,
    );
    request.table(
        "cluster_component",
        "CC",
        &NumericWireTable {
            declared_count: components.len() as f64,
            rows: components,
        },
        3,
    );
    request.registry(
        "cluster_labels",
        &clusters.iter().map(|c| c.label.clone()).collect::<Vec<_>>(),
    );
    request
}

pub fn make(design: &Design, poisson: bool, vce: &str, solver: &str, low: bool) -> Request {
    configure(inputs(design, poisson), design, poisson, vce, solver, low)
}

pub(super) fn configure(
    mut request: Request,
    design: &Design,
    poisson: bool,
    vce: &str,
    solver: &str,
    low: bool,
) -> Request {
    for field in [
        "ado=0.5.0",
        "protocol_major=1",
        "protocol_minor=6",
        "stata=18.0",
        "plugin=0.5.0",
        "result_major=1",
        "result_minor=5",
        "include_intercept=1",
        "candidate_all=0",
        "input_complete=0",
        "keep_singletons=0",
        "poolsize=10",
        "threads=1",
        "deterministic=1",
        "tolerance=1e-10",
        "iterate=16000",
        "stata_used_entry=.",
        "stata_alloc_entry=.",
        "stata_used_before=.",
        "stata_alloc_before=.",
    ] {
        request.args.push(field.into());
    }
    request.arg(
        "confirm",
        if poisson {
            "feppml-estimation"
        } else {
            "fereg-estimation"
        },
    );
    let channels = design
        .effects
        .iter()
        .map(|e| usize::from(e.has_intercept()) + e.slopes().len())
        .sum::<usize>();
    request.arg("output_saved_count", channels);
    request.registry("output_saved_channels", &vec!["0".into(); channels]);
    request.arg("vce", vce);
    request.arg("solver", solver);
    request.arg("memory", if low { "low" } else { "fast" });
    let outputs = if poisson {
        vec![
            "sample",
            "residual",
            "drop_reason",
            "xb",
            "absorbed",
            "eta",
            "mean",
        ]
    } else {
        vec!["sample", "residual", "drop_reason", "xb", "xbd", "absorbed"]
    };
    for key in outputs {
        let initial = match key {
            "sample" => 1.,
            "drop_reason" => 0.,
            _ => f64::from_bits(0x7fe0000000000000),
        };
        request.data.push(if key == "sample" {
            request.data[0].clone()
        } else {
            vec![initial; design.outcome.len()]
        });
        request.arg(&format!("output_{key}"), request.data.len());
        request.outputs.push((key.into(), request.data.len() - 1));
    }
    if !poisson {
        request.arg(
            "preconditioner",
            if solver == "cmg" { "off" } else { "auto" },
        );
    }
    result_layout(&mut request, design, poisson);
    request
}

fn metadata(request: &mut Request, wire: &ModelWireV2) {
    request.arg("model_major", wire.major);
    request.arg("model_minor", wire.minor);
    for (key, name, table, width) in [
        ("source", "S", &wire.sources, 2),
        ("term", "T", &wire.terms, 8),
        ("component", "C", &wire.components, 13),
        ("absorb_term", "A", &wire.absorb_terms, 4),
        ("absorb_component", "AC", &wire.absorb_components, 4),
    ] {
        request.table(key, name, table, width);
    }
    for (key, values) in [
        ("source_names", &wire.source_names),
        ("canonical_tokens", &wire.canonical_tokens),
        ("raw_component_operators", &wire.raw_component_operators),
        ("raw_time_series_operators", &wire.raw_time_series_operators),
        ("absorb_labels", &wire.absorb_labels),
        ("absorb_saved_names", &wire.absorb_saved_names),
    ] {
        request.registry(key, values);
    }
}

fn result_layout(request: &mut Request, design: &Design, poisson: bool) {
    // OLS slope-only designs do not report a constant. Match the public ado's
    // physical output capacity; an extra slot fails native publication checks.
    let report_constant = poisson
        || design.effects.is_empty()
        || design.effects.iter().any(|effect| effect.has_intercept());
    let k = (design.regressors.len() + usize::from(report_constant)).max(1);
    request.result("b", "RB", 1, k);
    request.result("v", "RV", k, k);
    request.result("status", "RSTATUS", k, 4);
    let fe = design
        .effects
        .iter()
        .map(|e| usize::from(e.has_intercept()) + e.slopes().len())
        .sum::<usize>()
        .max(1);
    request.result("fe", "RFE", fe, 8);
    request.result("cluster", "RCLUSTER", design.clusters.len().max(1), 4);
    request.result(
        "subset",
        "RSUBSET",
        ((1usize << design.clusters.len()) - 1).max(1),
        4,
    );
    request.result("memory", "RMEMORY", 11, 2);
    if poisson {
        for field in [
            "request_major=1",
            "request_minor=8",
            "adjustment=none",
            "adjustment_position=0",
            "residual=response",
            "separation=none",
            "separation_tolerance=1e-9",
            "separation_max_iterations=1000",
            "simplex_max_iterations=10000",
            "simplex_max_tableau_cells=1000000",
        ] {
            request.args.push(field.into());
        }
        for (key, name, r, c) in [
            ("fit", "RFIT", 27, 1),
            ("dof", "RDOF", 12, 1),
            ("solver", "RSOLVER", 4, 22),
            ("certificate", "RCERT", 16, 1),
            ("runtime", "RRUNTIME", 108, 1),
            ("receipt", "RREC", 1, 33),
            ("separation", "RSEP", 1, 14),
            ("inference", "RINF", 16, 1),
            ("drop_counts", "RDROP", 14, 2),
            ("separation_sequence", "RSEQ", 5, 13),
        ] {
            request.result(key, name, r, c);
        }
    } else {
        request.arg("dof", "pairwise");
        request.arg(
            "account_cluster_nesting",
            usize::from(!design.clusters.is_empty()),
        );
        for (key, name, r, c) in [
            ("marker_b", "RMARKB", 1, k),
            ("marker_v", "RMARKV", k, k),
            ("fit", "RFIT", 12, 2),
            ("dof", "RDOF", 20, 2),
            ("solver", "RSOLVER", 2, 16),
            ("certificate", "RCERT", 32, 2),
            ("runtime", "RRUNTIME", 30, 2),
            ("receipt", "RREC", 1, 16),
        ] {
            request.result(key, name, r, c);
        }
    }
}
