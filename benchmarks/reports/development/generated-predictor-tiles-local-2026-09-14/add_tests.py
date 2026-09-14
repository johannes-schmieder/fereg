import os
from pathlib import Path
repo=Path(os.environ['REPOSITORY_ROOT']);r=Path(__file__).resolve().parent
old=Path(os.path.join(os.environ['GENERATED_RHS_ROOT'], 'candidate/crates/fereg-core/src/ols/prepared_rhs.rs')).read_text();a=old.index('    #[test]\n    fn generated_rhs_tiles');b=old.index('                for workers in [1, 4, 16]',a);body=old[a:b].replace('generated_rhs_tiles_match_independent_indicators_and_bounded_blocks','generated_predictor_tiles_preserve_raw_level_oracle_and_fma_order').replace('for n in [4095, 4096, 4099, 65539]','for n in [4095, 4096, 4099]');body+='''                let selection = [(width-1,-0.0),(1,1e100),(0,1e-100),(1,-1e100),(2,0.7)];
                let coefficients: Vec<_> = selection.iter().enumerate().map(|(slot,(column,value))| crate::ols::PreparedOlsFittedCoefficient {
                    coefficient_slot:CoefficientSlot::new((slot+1) as u32).unwrap(), generated_column_index:Some(*column),
                    status:crate::ols::PreparedOlsFitStatus::Kept, value:Some(*value),
                }).collect();
                let mut wanted=vec![-0.0_f64;n];
                for &(column,coefficient) in &selection {
                    for row in 0..n {wanted[row]=coefficient.mul_add(expected[row*(width+1)+column+1],wanted[row]);}
                }
                for workers in [1,4,16] {
                    let team=ThreadTeam::new(ThreadConfig::from_requested(workers,true).unwrap());
                    let mut actual=vec![-0.0_f64;n];
                    let used=team.install(|| crate::prepared_fit_view::add_generated_predictor(prepared,&coefficients,&mut actual)).unwrap().unwrap();
                    assert_eq!(used,n>=4096);
                    if used {assert!(actual.iter().zip(&wanted).all(|(a,b)|a.to_bits()==b.to_bits()));}
                    else {assert!(actual.iter().all(|v|v.to_bits()==(-0.0_f64).to_bits()));}
                }
                if n>=4096 {
                    let mut invalid=coefficients.clone();
                    for (column,value) in [(None,Some(1.0)),(Some(width),Some(1.0)),(Some(0),None),(Some(0),Some(f64::NAN)),(Some(0),Some(f64::INFINITY))] {
                        invalid[0].generated_column_index=column;invalid[0].value=value;
                        let mut untouched=vec![7.0;n];
                        assert!(crate::prepared_fit_view::add_generated_predictor(prepared,&invalid,&mut untouched).is_err());
                        assert_eq!(untouched,vec![7.0;n]);
                    }
                    let mut short=vec![7.0;n-1];
                    assert!(crate::prepared_fit_view::add_generated_predictor(prepared,&coefficients,&mut short).is_err());
                    let mut nonfinite=vec![f64::MAX;n];
                    let overflow=vec![crate::ols::PreparedOlsFittedCoefficient {coefficient_slot:CoefficientSlot::new(1).unwrap(),generated_column_index:Some(width-1),status:crate::ols::PreparedOlsFitStatus::Kept,value:Some(f64::MAX)}];
                    assert_eq!(crate::prepared_fit_view::add_generated_predictor(prepared,&overflow,&mut nonfinite).unwrap_err().code,ErrorCode::InvalidData);
                    let omitted=vec![crate::ols::PreparedOlsFittedCoefficient {coefficient_slot:CoefficientSlot::new(1).unwrap(),generated_column_index:None,status:crate::ols::PreparedOlsFitStatus::SyntaxOmitted,value:None}];
                    let mut unchanged=vec![-0.0_f64;n];
                    assert!(crate::prepared_fit_view::add_generated_predictor(prepared,&omitted,&mut unchanged).unwrap());
                    assert!(unchanged.iter().all(|v|v.to_bits()==(-0.0_f64).to_bits()));
                }
            }
        }
    }

'''
p=repo/'crates/fereg-core/src/ols/prepared_rhs.rs';s=p.read_text();marker='    #[test]\n    fn empty_rhs_is_valid_but_map_requires_nonempty_rows()';assert marker in s;p.write_text(s.replace(marker,body+marker,1))
