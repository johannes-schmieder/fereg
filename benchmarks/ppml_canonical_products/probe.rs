use crate::data::DenseMatrix;
use crate::fe::{BorrowedInterceptFeDesign, BorrowedMapWeights, LevelIndex, MapConfig,
    ReusableCategoricalMapAbsorber, ValidatedBorrowedInterceptSource};
use super::finite_wls_kernel::{FixedBlockMomentSource, finite_wls_moment_layout, fill_finite_wls_moments};
use super::prepared_scratch::PreparedOlsScratch;
use std::io::{Read, Write};
use std::time::Instant;

fn word(r: &mut impl Read) -> u64 {
    let mut bytes = [0; 8]; r.read_exact(&mut bytes).unwrap(); u64::from_le_bytes(bytes)
}

fn save(path: std::path::PathBuf, values: &[f64]) {
    let mut file = std::fs::File::create(path).unwrap();
    for value in values { file.write_all(&value.to_le_bytes()).unwrap(); }
}

struct Baseline {
    gram: Vec<f64>, scales: Vec<f64>, response: Vec<f64>,
    scratch: PreparedOlsScratch, phases: [f64; 2],
}

fn panels(owner: &mut ReusableCategoricalMapAbsorber<'_>, input: &[f64], weights: &[f64],
    n: usize, width: usize, config: MapConfig) -> Baseline {
    let started = Instant::now();
    let divisor = weights.iter().copied().fold(0_f64, f64::max);
    let mut scales = vec![0.; width];
    for j in 0..width {
        let mut scale = 0_f64; let mut sum = 1.;
        for i in 0..n {
            let value = (input[i*width+j]*(weights[i]/divisor).sqrt()).abs();
            if value > scale { sum = 1. + sum*(scale/value).powi(2); scale = value; }
            else if value > 0. { sum += (value/scale).powi(2); }
        }
        scales[j] = (scale*sum.sqrt()).max(f64::MIN_POSITIVE);
    }
    let response = DenseMatrix::from_row_major(n,1,(0..n).map(|i|input[i*width]).collect()).unwrap();
    let response = owner.absorb(BorrowedMapWeights::Provided(weights), response, config).unwrap();
    assert!(response.report().certified());
    let response = response.into_parts().0.into_row_major();
    let controls = width-1;
    let mut scratch = PreparedOlsScratch::create_row_major(n,controls).unwrap();
    for first in (0..controls).step_by(10) {
        let last = (first+10).min(controls); let count = last-first;
        let raw = DenseMatrix::from_row_major(n,count,(0..n).flat_map(|i|
            input[i*width+1+first..i*width+1+last].iter().copied()).collect()).unwrap();
        let within = owner.absorb(BorrowedMapWeights::Provided(weights),raw,config).unwrap();
        assert!(within.report().certified());
        scratch.patch_row_major_block(first,count,within.residualized().as_slice()).unwrap();
    }
    let projection_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let source = FixedBlockMomentSource::RetainedScratch {
        values: &scratch, selected_columns: None, response: &response,
        weights: BorrowedMapWeights::Provided(weights), solver_weight_divisor: divisor,
        column_scales: Some(&scales[1..]),
    };
    let layout = finite_wls_moment_layout(source).unwrap();
    let mut partials = vec![0.; layout.partial_elements];
    let mut rhs = vec![0.; controls]; let mut finite = vec![0.; controls*controls];
    fill_finite_wls_moments(source,layout,&mut partials,&mut rhs,&mut finite).unwrap();
    let mut gram = vec![0.; width*width];
    gram[0] = (0..n).map(|i|(weights[i]/divisor)*(response[i]/scales[0]).powi(2)).sum();
    for j in 0..controls {
        gram[1+j] = rhs[j]/scales[0]; gram[(1+j)*width] = rhs[j]/scales[0];
        for k in 0..controls { gram[(j+1)*width+k+1] = finite[j*controls+k]; }
    }
    let moment_seconds = started.elapsed().as_secs_f64();
    Baseline { gram, scales, response, scratch, phases: [projection_seconds,moment_seconds] }
}

#[test]
fn boundary_exact_weighted_contrasts_changing_frames_and_disconnected_scales() {
    // Full Cartesian cells give a separate analytic oracle: products of
    // independently centered margins and centered repeats have zero FE scores.
    for threads in [1,4,8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            for mass in [1.,1e-80,1e80] {
                for shift in [0.,0.7] {
                    let mut left=Vec::new();let mut right=Vec::new();
                    let mut weights=Vec::new();let mut input=Vec::new();let mut expected=Vec::new();
                    let wa: Vec<_>=(0..6).map(|a|1.+a as f64+shift).collect();
                    let wb: Vec<_>=(0..5).map(|b|0.5+b as f64+shift).collect();
                    let ma: f64=wa.iter().enumerate().map(|(a,w)|a as f64*w).sum::<f64>()/wa.iter().sum::<f64>();
                    let mb: f64=wb.iter().enumerate().map(|(b,w)|b as f64*w).sum::<f64>()/wb.iter().sum::<f64>();
                    for component in 0..2 {for a in 0..6 {for b in 0..5 {for t in 0..3 {
                        left.push((component*6+a) as u32);right.push((component*5+b) as u32);
                        weights.push(wa[a]*wb[b]*if component==0 {mass}else{1./mass});
                        let interaction=(a as f64-ma)*(b as f64-mb);
                        let noise=t as f64-1.;
                        for (j,scale) in [1.,1e70,1e-70].into_iter().enumerate() {
                            let r=match j {0=>interaction,1=>noise,_=>interaction+noise*(b as f64-mb)};
                            expected.push(r*scale);
                            input.push((r+0.3*a as f64-0.2*b as f64+component as f64)*scale);
                        }
                    }}}}
                    let index=LevelIndex::from_codes(&left,12).unwrap();
                    let geometry=kernel::Geometry::new(&left,&right,&index,[12,10],&weights).unwrap();
                    for direct in [false,true] {
                    let result=if direct {geometry.products_direct(&input,3,1e-12)} else {geometry.products(&input,3,1e-12)}.unwrap();assert!(result.usable);
                    let within=geometry.residuals(&input,3,&result);
                    for (i,(actual,truth)) in within.iter().zip(&expected).enumerate() {
                        let scale=[1.,1e70,1e-70][i%3];
                        assert!((actual-truth).abs()/scale<2e-12,"{threads} {mass} {shift} {i} {actual} {truth}");
                    }
                    for j in 0..3 {for k in 0..3 {
                        let maximum=weights.iter().copied().fold(0_f64,f64::max);
                        let exact: f64=(0..weights.len()).map(|i|(weights[i]/maximum)
                            *(expected[i*3+j]/result.scales[j])*(expected[i*3+k]/result.scales[k])).sum();
                        assert!((result.gram[j*3+k]-exact).abs()<2e-13);
                    }}
                    }
                }
            }
        });
    }
}

#[test]
fn boundary_cancellation_declines_fe_collinear_products() {
    let n=240;let width=3;
    let left: Vec<_>=(0..n).map(|i|(i%12) as u32).collect();
    let right: Vec<_>=(0..n).map(|i|((i/12)%5) as u32).collect();
    let weights: Vec<_>=(0..n).map(|i|1.+(i%7) as f64).collect();
    let index=LevelIndex::from_codes(&left,12).unwrap();
    let geometry=kernel::Geometry::new(&left,&right,&index,[12,5],&weights).unwrap();
    for noise in [0.,1e-10] {
        let input: Vec<_>=(0..n).flat_map(|i|[left[i] as f64+right[i] as f64+noise*(i as f64).sin(),
            (i as f64).sin(),(i as f64).cos()]).collect();
        let result=geometry.products(&input,width,1e-12).unwrap();
        assert!(!result.usable);assert!(result.certificate<=1e-12);
        let direct=geometry.products_direct(&input,width,1e-12).unwrap();
        assert!(direct.usable);assert!(direct.gram[0]>=0. && direct.gram[0]<1e-20);
    }
}

#[test]
fn boundary_nonfinite_and_unrepresentable_geometry_refuses() {
    let left=vec![0,0,1,1];let right=vec![0,1,0,1];
    let index=LevelIndex::from_codes(&left,2).unwrap();
    for weights in [[1.,f64::NAN,1.,1.],[1.,0.,1.,1.],[1e308,1e-308,1.,1.]] {
        assert!(kernel::Geometry::new(&left,&right,&index,[2,2],&weights).is_none());
    }
    let geometry=kernel::Geometry::new(&left,&right,&index,[2,2],&[1.;4]).unwrap();
    assert!(geometry.products(&[1.,2.,f64::INFINITY,4.],1,1e-12).is_none());
}

#[test]
#[ignore]
fn frozen_canonical_products() {
    let root = std::path::PathBuf::from(std::env::var("FEREG_PRODUCT_ROOT").unwrap());
    let mut paths: Vec<_> = std::fs::read_dir(root.join("frames")).unwrap().map(|p|p.unwrap().path()).collect();
    paths.sort();
    for path in paths {
        let case = path.file_stem().unwrap().to_str().unwrap();
        let mut file = std::fs::File::open(&path).unwrap();
        let n = word(&mut file) as usize; let width = word(&mut file) as usize;
        assert_eq!(word(&mut file),2); let tolerance = f64::from_bits(word(&mut file));
        let mut codes = Vec::new(); let mut levels = Vec::new();
        for _ in 0..2 {
            levels.push(word(&mut file) as usize);
            codes.push((0..n).map(|_| {let mut b=[0;4];file.read_exact(&mut b).unwrap();u32::from_le_bytes(b)}).collect::<Vec<_>>());
        }
        let weights: Vec<_> = (0..n).map(|_|f64::from_bits(word(&mut file))).collect();
        let input: Vec<_> = (0..n*width).map(|_|f64::from_bits(word(&mut file))).collect();
        let indices: Vec<_> = codes.iter().zip(&levels).map(|(c,&l)|LevelIndex::from_codes(c,l).unwrap()).collect();
        let left = usize::from(levels[1]>levels[0]); let right = 1-left;
        let design = || BorrowedInterceptFeDesign::new(n,codes.iter().zip(&levels).zip(&indices)
            .map(|((c,&l),index)|ValidatedBorrowedInterceptSource::new(c,l,index).unwrap().effect()).collect()).unwrap();
        for threads in [1,4,8] {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
            pool.install(|| {
                let started = Instant::now();
                let mut owner = ReusableCategoricalMapAbsorber::new_ppml(design(),10.min(width),threads).unwrap();
                let baseline_setup = started.elapsed().as_secs_f64();
                let started = Instant::now();
                let geometry = kernel::Geometry::new(&codes[left],&codes[right],&indices[left],
                    [levels[left],levels[right]],&weights);
                let candidate_setup = started.elapsed().as_secs_f64();
                let Some(geometry) = geometry else {
                    println!("REFUSED|case={case}|threads={threads}|reason=geometry|seconds={candidate_setup:.9}"); return;
                };
                println!("SETUP|case={case}|threads={threads}|baseline={baseline_setup:.9}|candidate={candidate_setup:.9}|candidate_bytes={}",geometry.setup_bytes);
                let config = MapConfig { tolerance, max_iterations:16000, ..MapConfig::default() };
                for rep in 0..7 {
                    for offset in 0..3 {
                        let variant = if rep%2==0 {offset} else {2-offset};
                        let started = Instant::now();
                        if variant == 0 {
                            let mut result = panels(&mut owner,&input,&weights,n,width,config);
                            let seconds = started.elapsed().as_secs_f64();
                            println!("TIME|case={case}|threads={threads}|rep={rep}|variant=0|seconds={seconds:.9}|projection={:.9}|moments={:.9}",result.phases[0],result.phases[1]);
                            if rep == 0 {
                                let mut within = vec![0.;n*width]; let mut block = vec![0.;n*(width-1)];
                                result.scratch.read_row_major_block(0,width-1,&mut block).unwrap();
                                for i in 0..n {within[i*width]=result.response[i];within[i*width+1..(i+1)*width].copy_from_slice(&block[i*(width-1)..(i+1)*(width-1)]);}
                                save(root.join(format!("{case}-t{threads}-v0-within.bin")),&within);
                                save(root.join(format!("{case}-t{threads}-v0-gram.bin")),&result.gram);
                                save(root.join(format!("{case}-t{threads}-v0-scales.bin")),&result.scales);
                            }
                        } else {
                            let result = if variant==1 {geometry.products(&input,width,tolerance)}
                                else {geometry.products_direct(&input,width,tolerance)}.unwrap();
                            let seconds = started.elapsed().as_secs_f64();
                            println!("TIME|case={case}|threads={threads}|rep={rep}|variant={variant}|seconds={seconds:.9}|reference={:.9}|moments={:.9}|solve={:.9}|certificate_seconds={:.9}|certificate={:.17e}|cancellation={:.17e}|usable={}|bytes={}",result.phases[0],result.phases[1],result.phases[2],result.phases[3],result.certificate,result.cancellation_bound,result.usable,result.bytes);
                            if rep == 0 {
                                save(root.join(format!("{case}-t{threads}-v{variant}-within.bin")),&geometry.residuals(&input,width,&result));
                                save(root.join(format!("{case}-t{threads}-v{variant}-gram.bin")),&result.gram);
                                save(root.join(format!("{case}-t{threads}-v{variant}-scales.bin")),&result.scales);
                            }
                        }
                    }
                }
            });
        }
    }
}
