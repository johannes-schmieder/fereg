import os
from pathlib import Path
p=Path(os.path.join(os.environ['REPOSITORY_ROOT'], 'crates/fereg-core/src/ols/prepared_rhs.rs'));s=p.read_text();marker='    #[test]\n    fn empty_rhs_is_valid_but_map_requires_nonempty_rows()'
new='''    #[test]
    fn generated_rhs_tiles_match_independent_indicators_and_bounded_blocks() {
        use crate::prepared_fit_view::PreparedOlsFitView;
        use crate::parallel::ThreadTeam;
        use crate::options::ThreadConfig;
        for n in [4095, 4096, 4099] {
            let x = source(1, "x");
            let g = source(2, "g");
            let h = source(3, "h");
            let mut terms = vec![continuous_term(1, x.id),
                categorical_term(2, g.id, 1.0, true),
                categorical_interaction_term(3, (g.id, 2.0), (h.id, 1.0)),
                categorical_interaction_term(4, (g.id, 99.0), (h.id, 1.0))];
            for level in 2..=67 { terms.push(categorical_term(level + 3, g.id, f64::from(level), false)); }
            let mut request = request(false, true);
            request.model = CompiledTermGraph { sources: vec![x.clone(),g.clone(),h.clone()], terms };
            request.bindings.extend([OlsInputBinding { stata_var_position:4, role:OlsInputRole::NumericSource(g.id) },
                OlsInputBinding { stata_var_position:5, role:OlsInputRole::NumericSource(h.id) }]);
            request.absorbed_terms = vec![AbsorbedTerm { id:AbsorbTermId::new(1).unwrap(), intercept:true,
                categorical_sources:vec![g.id], slope_sources:Vec::new(), label:"g".into(), saved_name:String::new() }];
            let xv: Vec<_> = (0..n).map(|r| if r%2==0 {-0.0} else {r as f64/7.0}).collect();
            let gv: Vec<_> = (0..n).map(|r| (1+r%7) as f64).collect();
            let hv: Vec<_> = (0..n).map(|r| (1+(r/7)%3) as f64).collect();
            let input = LoadedOlsInput { candidate:vec![true;n], outcome:loaded((0..n).map(|r| r as f64).collect()),
                numeric_sources:vec![(x.id,loaded(xv.clone())),(g.id,loaded(gv.clone())),(h.id,loaded(hv.clone()))],
                analytic_weight:Some(loaded((0..n).map(|r| 1.0+(r%5) as f64).collect())), frequency_weight:None, probability_weight:None };
            request.bindings.push(OlsInputBinding { stata_var_position:6, role:OlsInputRole::AnalyticWeight });
            let simple = prepare_ols_data(&request,&input).unwrap();
            let complex = crate::prepared_categorical_data::prepare_categorical_ols_data(&request,&input).unwrap();
            assert!(complex.categorical_sources().iter().any(|s| s.is_topology_backed()));
            let views: [&dyn PreparedOlsFitView;2] = [&simple,&complex];
            for prepared in views {
                let width = prepared.fit_model().columns.len();
                let expected: Vec<_> = (0..n).flat_map(|row| {
                    let mut values = vec![row as f64, xv[row], f64::from(gv[row]==2.0 && hv[row]==1.0), 0.0];
                    values.extend((2..=67).map(|level| f64::from(gv[row]==f64::from(level))));
                    values.push(1.0); values
                }).collect();
                assert_eq!(expected.len(),n*(width+1));
                for workers in [1,4,16] {
                    let team = ThreadTeam::new(ThreadConfig::from_requested(workers,true).unwrap());
                    let full = team.install(|| prepared.fit_build_rhs()).unwrap().unwrap();
                    assert!(full.as_slice().iter().zip(&expected).all(|(a,b)| a.to_bits()==b.to_bits()));
                    for (start,end,include) in [(0,width,true),(1,3,false),(3,4,false),(width-2,width,true)] {
                        let block = team.install(|| prepared.fit_build_rhs_block(start,end,include)).unwrap().unwrap();
                        for row in 0..n {
                            let mut want=Vec::new(); if include {want.push(row as f64);}
                            want.extend_from_slice(&expected[row*(width+1)+start+1..row*(width+1)+end+1]);
                            assert!(block.as_slice()[row*want.len()..(row+1)*want.len()].iter().zip(&want).all(|(a,b)|a.to_bits()==b.to_bits()));
                        }
                    }
                }
            }
            if n>=4096 {
                let mut wrong=vec![7.0;4];
                assert!(crate::prepared_fit_view::fill_generated_rhs_block(&simple,1..3,None,&mut wrong).is_err());
                assert_eq!(wrong,vec![7.0;4]);
                let mut corrupt=simple.clone();
                corrupt.categorical_sources[0].encoded.codes[0]=u32::MAX;
                assert!(build_prepared_ols_rhs(&corrupt).is_err());
                let mut reservation=InjectedReservation {fail:true,..Default::default()};
                assert_eq!(build_prepared_ols_rhs_with_reservation(&simple,&simple.sample.outcome,&mut reservation).unwrap_err().code,ErrorCode::Allocation);
                assert_eq!(reservation.calls,1);
            }
        }
    }

'''
assert marker in s;s=s.replace(marker,new+marker);p.write_text(s)
