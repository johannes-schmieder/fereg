#![forbid(unsafe_code)]
#![allow(dead_code)]
mod error;
mod psd;
fn main() {
    for n in [3usize, 5, 9, 17, 33, 65] {
        for seed in 1..=20usize {
            let v: Vec<f64> = (0..n).map(|i| (((i+1)*137 + seed*53)%251) as f64 - 125.0).collect();
            let vv: f64 = v.iter().map(|x|x*x).sum();
            let q: Vec<f64> = (0..n*n).map(|i| f64::from(u8::from(i/n==i%n)) - 2.0*v[i/n]*v[i%n]/vv).collect();
            let eigen: Vec<f64> = (0..n).map(|j| if j%3==0 {1.0+j as f64} else {-(1.0+j as f64)/7.0}).collect();
            let mut matrix=vec![0.0;n*n];
            for i in 0..n {for j in i..n {let mut value=0.0;for k in 0..n {value+=q[i*n+k]*eigen[k]*q[j*n+k];}matrix[i*n+j]=value;matrix[j*n+i]=value;}}
            let (published,first)=psd::apply_ppml_psd_policy(&matrix,n,2,1e-8).unwrap();
            let (_,second)=psd::apply_ppml_psd_policy(&published,n,2,1e-8).unwrap();
            let eig_bad=first.eigensystem_reconstruction_relative_error+16.0*f64::EPSILON < second.eigensystem_reconstruction_relative_error;
            let orth_bad=first.orthogonality_relative_error+16.0*f64::EPSILON < second.orthogonality_relative_error;
            if eig_bad || orth_bad || first.final_rank!=second.final_rank {
                println!("FAIL n={n} seed={seed} eig_bad={eig_bad} orth_bad={orth_bad}\nfirst={first:?}\nsecond={second:?}");
                println!("matrix={matrix:?}");return;
            }
        }
        println!("n={n} no mismatch");
    }
}
