use std::hint::black_box;
use std::time::Instant;

#[inline]
fn update(value: f64, scale: &mut f64, sum: &mut f64) {
    let absolute = value.abs();
    if absolute == 0.0 { return; }
    if *scale < absolute {
        let ratio = *scale / absolute;
        *sum = 1.0 + *sum * ratio * ratio;
        *scale = absolute;
    } else {
        let ratio = absolute / *scale;
        *sum += ratio * ratio;
    }
}
#[inline(never)]
fn reference(values: &[f64]) -> Result<f64, u8> {
    let mut scale = 0.0; let mut sum = 1.0;
    for &value in values {
        if !value.is_finite() { return Err(1); }
        update(value, &mut scale, &mut sum);
    }
    let norm = if scale == 0.0 { 0.0 } else { scale * sum.sqrt() };
    if norm.is_finite() { Ok(norm) } else { Err(2) }
}
#[inline(never)]
fn blocked(values: &[f64]) -> Result<f64, u8> {
    let mut scale = 0.0; let mut sum = 1.0;
    let mut blocks = values.chunks_exact(8);
    for block in &mut blocks {
        let absolute: [f64; 8] = std::array::from_fn(|i| block[i].abs());
        if scale > 0.0 && absolute.iter().all(|&v| v <= scale) {
            let squares = absolute.map(|v| { let ratio = v / scale; ratio * ratio });
            for square in squares { sum += square; }
        } else {
            for &value in block {
                if !value.is_finite() { return Err(1); }
                update(value, &mut scale, &mut sum);
            }
        }
    }
    for &value in blocks.remainder() {
        if !value.is_finite() { return Err(1); }
        update(value, &mut scale, &mut sum);
    }
    let norm = if scale == 0.0 { 0.0 } else { scale * sum.sqrt() };
    if norm.is_finite() { Ok(norm) } else { Err(2) }
}
fn main() {
    for n in [0, 1, 7, 8, 9, 127, 16384, 983040] {
        for case in 0..7 {
            let mut values: Vec<_> = (0..n).map(|r| ((r * 1009 % 10007) as f64).sin()).collect();
            if n > 0 {
                match case {
                    1 => values.fill(-0.0),
                    2 => values[n/2] = f64::INFINITY,
                    3 => values[n/2] = f64::NAN,
                    4 => for v in &mut values { *v *= 1e-300; },
                    5 => for v in &mut values { *v *= 1e150; },
                    6 => { values[0] = f64::MAX; values[n/2] = f64::MIN; },
                    _ => (),
                }
            }
            assert_eq!(reference(&values).map(f64::to_bits), blocked(&values).map(f64::to_bits), "N={n}, case={case}");
        }
    }
    let values: Vec<_> = (0..983040).map(|r| ((r * 1009 % 10007) as f64).sin()).collect();
    for pair in 0..5 {
        let functions: [(&str, fn(&[f64]) -> Result<f64,u8>); 2] = if pair%2==0 { [("reference",reference),("blocked",blocked)] } else { [("blocked",blocked),("reference",reference)] };
        for (name, f) in functions {
            black_box(f(black_box(&values)).unwrap());
            let start=Instant::now();
            for _ in 0..20 { black_box(f(black_box(&values)).unwrap()); }
            println!("{pair} {name} {:.9}", start.elapsed().as_secs_f64()/20.0);
        }
    }
    println!("UNIT_NORM_KERNEL_PASS");
}
