use fereg_core::ppml::*;
use fereg_core::ols::*;
use fereg_core::metadata::*;
use fereg_core::ppml_separation::*;
    fn source(id: SourceId, label: &str) -> SourceEntry {
        SourceEntry {
            id,
            canonical_name: label.to_owned(),
        }
    }
    fn absorbed(id: u32, source: SourceId, label: &str, saved: &str) -> AbsorbedTerm {
        AbsorbedTerm {
            id: AbsorbTermId::new(id).expect("absorb id"),
            intercept: true,
            categorical_sources: vec![source],
            slope_sources: Vec::new(),
            label: label.to_owned(),
            saved_name: saved.to_owned(),
        }
    }
    fn continuous_term(source: SourceId) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(1).expect("term"),
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
    fn column(values: &[f64]) -> LoadedNumericColumn {
        LoadedNumericColumn {
            values: values.to_vec(),
            missing: vec![false; values.len()],
        }
    }
    fn fixture(
        effects: usize,
        save_first: bool,
        clustered: bool,
        probability_weighted: bool,
        offset: bool,
        separated: bool,
    ) -> (PreparedPpmlRequestV1, LoadedPpmlInput) {
        let rows = 12;
        let x = SourceId::new(1).expect("x");
        let mut sources = vec![source(x, "x")];
        let mut numeric_sources = vec![(
            x,
            column(&[
                -1.5, -0.5, -1.0, 0.5, -0.5, 1.0, 0.0, 1.5, 0.5, 2.0, 1.0, 2.5,
            ]),
        )];
        let mut absorbed_terms = Vec::new();
        if effects >= 1 {
            let g = SourceId::new(2).expect("g");
            sources.push(source(g, "g"));
            numeric_sources.push((
                g,
                column(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0, 6.0, 6.0]),
            ));
            absorbed_terms.push(absorbed(1, g, "g", if save_first { "alpha" } else { "" }));
        }
        if effects >= 2 {
            let h = SourceId::new(3).expect("h");
            sources.push(source(h, "h"));
            numeric_sources.push((
                h,
                column(&[1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0]),
            ));
            absorbed_terms.push(absorbed(2, h, "h", ""));
        }
        if effects >= 3 {
            let k = SourceId::new(4).expect("k");
            sources.push(source(k, "k"));
            numeric_sources.push((
                k,
                column(&[1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0]),
            ));
            absorbed_terms.push(absorbed(3, k, "k", ""));
        }
        if effects >= 4 {
            let m = SourceId::new(5).expect("m");
            sources.push(source(m, "m"));
            numeric_sources.push((
                m,
                column(&[1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]),
            ));
            absorbed_terms.push(absorbed(4, m, "m", ""));
        }
        let mut clusters = Vec::new();
        if clustered {
            let id = u32::try_from(sources.len() + 1).expect("cluster source id");
            let c = SourceId::new(id).expect("c");
            sources.push(source(c, "c"));
            numeric_sources.push((
                c,
                column(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 1.0, 2.0, 3.0, 4.0]),
            ));
            clusters.push(OlsClusterTermV1 {
                label: "c".to_owned(),
                sources: vec![c],
            });
        }
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ];
        for (index, (source, _)) in numeric_sources.iter().enumerate() {
            bindings.push(OlsInputBinding {
                stata_var_position: u32::try_from(index + 3).expect("source position"),
                role: OlsInputRole::NumericSource(*source),
            });
        }
        if probability_weighted {
            bindings.push(OlsInputBinding {
                stata_var_position: u32::try_from(numeric_sources.len() + 3)
                    .expect("weight position"),
                role: OlsInputRole::ProbabilityWeight,
            });
        }
        let ols = OlsRequestV1 {
            model: CompiledTermGraph {
                sources,
                terms: vec![continuous_term(x)],
            },
            absorbed_terms,
            bindings,
            include_intercept: true,
            keep_singletons: effects == 0,
        };
        let mut separation = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: separated,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
            ..SeparationConfig::default()
        };
        separation.drop_singletons = effects > 0;
        let ppml = PpmlConfig::default();
        let request = PreparedPpmlRequestV1 {
            bias: None,
            ols,
            clusters,
            vce: if clustered {
                PpmlVceRequest::Clustered
            } else {
                PpmlVceRequest::Robust
            },
            ppml,
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
                xb: true,
                absorbed: true,
                eta: true,
                mean: true,
                requested_observation_columns: 4 + usize::from(save_first && effects > 0),
                publication_matrix_cells: 512,
            },
            solver: PpmlSolverFamily::Map,
            inject_fe_recovery_allocation_failure: false,
        };
        let outcome = if separated {
            column(&[0.0, 0.0, 2.0, 4.0, 3.0, 5.0, 4.0, 7.0, 5.0, 8.0, 6.0, 9.0])
        } else {
            column(&[1.0, 2.0, 2.0, 4.0, 3.0, 5.0, 4.0, 7.0, 5.0, 8.0, 6.0, 9.0])
        };
        let probability_weight = probability_weighted
            .then(|| column(&[1.0, 2.0, 1.5, 0.5, 3.0, 2.0, 4.0, 1.0, 2.5, 3.5, 1.0, 2.0]));
        let adjustment = offset.then(|| {
            column(&[
                0.0, 0.05, 0.1, 0.0, -0.05, 0.1, 0.0, 0.05, -0.1, 0.0, 0.1, 0.05,
            ])
        });
        let loaded = LoadedPpmlInput {
            bias_units: None,
            ols: LoadedOlsInput {
                candidate: vec![true; rows],
                outcome,
                numeric_sources,
                analytic_weight: None,
                frequency_weight: None,
                probability_weight,
            },
            adjustment,
            adjustment_kind: if offset {
                PreparedPpmlAdjustmentKind::Offset
            } else {
                PreparedPpmlAdjustmentKind::None
            },
        };
        (request, loaded)
    }
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).unwrap();
    let all: Vec<Vec<f64>> = text.lines().skip(1).map(|line| line.split(',').map(|v| v.parse().unwrap()).collect()).collect();
    for ordering in 0..4 {
      let mut rows=all.clone();
      match ordering {1=>rows.reverse(),2=>rows.sort_by(|a,b| a[2].total_cmp(&b[2]).then(a[3].total_cmp(&b[3])).then(a[0].total_cmp(&b[0])).then(a[1].total_cmp(&b[1]))),3=>rows.sort_by(|a,b| a[3].total_cmp(&b[3]).then(a[2].total_cmp(&b[2])).then(a[0].total_cmp(&b[0])).then(a[1].total_cmp(&b[1]))),_=>()}
      for branch in 0..3 {
        let values=|j:usize| rows.iter().map(|r| r[j]).collect::<Vec<f64>>();
        let (mut request,mut loaded)=fixture(2,false,false,true,true,false);
        request.ppml=PpmlConfig::with_outer_controls(1e-9,300).unwrap();
        request.solver=PpmlSolverFamily::Auto;
        request.ols.keep_singletons=true;request.separation.drop_singletons=false;
        let z=SourceId::new(4).unwrap();
        request.ols.model.sources.push(source(z,"z"));
        let mut term=continuous_term(z);term.id=TermId::new(2).unwrap();term.order=TermOrder::new(2).unwrap();term.coefficient_slot=CoefficientSlot::new(2).unwrap();term.canonical_token="c.z".into();request.ols.model.terms.push(term);
        request.ols.bindings.push(OlsInputBinding{stata_var_position:7,role:OlsInputRole::NumericSource(z)});
        loaded.ols.candidate=rows.iter().map(|r| r[10]==1. && (branch==0 || r[4]==branch as f64)).collect();
        loaded.ols.outcome=column(&values(9));
        loaded.ols.numeric_sources=vec![(SourceId::new(1).unwrap(),column(&values(5))),(SourceId::new(2).unwrap(),column(&values(2))),(SourceId::new(3).unwrap(),column(&values(3))),(z,column(&values(6)))];
        loaded.ols.probability_weight=Some(column(&values(7)));
        loaded.adjustment=Some(column(&values(8)));loaded.adjustment_kind=PreparedPpmlAdjustmentKind::Exposure;
        let ordinary=fit_prepared_ppml(request.clone(),loaded.clone()).unwrap();
        for exact in [false,true] {
          if exact && ordering>0 {continue;}
          request.bias=Some(PpmlBiasRequest{method:if exact {PpmlBiasMethod::Exact}else{PpmlBiasMethod::Randomized},probes:if exact {0}else{256},seed:if exact {0}else{1411710131},stream:if exact {0}else{branch},save_effects:true});
          loaded.bias_units=Some(column(&values(0)));
          let result=fit_prepared_ppml(request.clone(),loaded.clone()).unwrap();
          assert_eq!(ordinary.coefficients(),result.coefficients());assert_eq!(ordinary.output(),result.output());assert_eq!(ordinary.inference().vce.covariance,result.inference().vce.covariance);
          let b=result.bias().unwrap();
          let mut out=String::from("raw,d,c\n");
          for i in 0..b.raw.len() {out.push_str(&format!("{:.17e},{:.17e},{:.17e}\n",b.raw[i],b.derivative[i],b.curvature[i]));}
          let method=if exact {"exact"}else{"randomized"};
          std::fs::write(format!("{}/{}-{}-{}.csv",args[2],ordering,branch,method),out).unwrap();
          println!("PASS ordering={} branch={} method={} min_mean={:.17e}",ordering,branch,method,b.minimum_mean);
        }
      }
    }
}
