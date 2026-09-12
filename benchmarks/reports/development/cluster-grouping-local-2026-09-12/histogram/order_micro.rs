use std::cmp::Ordering;
use std::time::Instant;
use std::hint::black_box;
type Result<T> = std::result::Result<T, &'static str>;
fn allocation(s: &'static str) -> &'static str { s }
fn invalid_data(s: &'static str) -> &'static str { s }
trait SandwichReservation {}
struct Reservation;
impl SandwichReservation for Reservation {}
fn reserved_filled<T:Clone,R:SandwichReservation>(_:&mut R,n:usize,v:T,_:&'static str)->Result<Vec<T>>{Ok(vec![v;n])}
fn reserved_empty<T,R:SandwichReservation>(_:&mut R,n:usize,_:&'static str)->Result<Vec<T>>{Ok(Vec::with_capacity(n))}
fn push_reserved<T>(v:&mut Vec<T>,x:T,_:&'static str)->Result<()> {v.push(x);Ok(())}
fn compare_rows(dimensions: &[&[u32]], mask: usize, left: usize, right: usize) -> Ordering {
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1 << dimension) == 0 {
            continue;
        }
        let ordering = codes[left].cmp(&codes[right]);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

fn baseline_order<R: SandwichReservation>(
    dimensions: &[&[u32]],
    mask: usize,
    nobs: usize,
    maximum_dimensions: usize,
    reservation: &mut R,
) -> Result<Vec<usize>> {
    let mut radices = [0usize; 16];
    let mut maximum = 0usize;
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1 << dimension) == 0 {
            continue;
        }
        let radix = codes
            .iter()
            .copied()
            .max()
            .and_then(|v| usize::try_from(v).ok())
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| invalid_data("qualified clustered radix has no groups"))?;
        radices[dimension] = radix;
        maximum = maximum.max(radix);
    }
    let scratch_bytes = nobs
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("qualified clustered radix bytes overflow"))?;
    let count_bytes = maximum
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("qualified clustered radix count bytes overflow"))?;
    if dimensions.len() > maximum_dimensions
        || count_bytes > scratch_bytes.saturating_mul(2).max(1 << 20)
    {
        let mut order = reserved_empty(
            reservation,
            nobs,
            "unable to reserve qualified clustered ordering",
        )?;
        for row in 0..nobs {
            push_reserved(
                &mut order,
                row,
                "qualified clustered ordering capacity exhausted",
            )?;
        }
        order.sort_unstable_by(|left, right| {
            compare_rows(dimensions, mask, *left, *right).then_with(|| left.cmp(right))
        });
        return Ok(order);
    }
    let mut order = reserved_filled(
        reservation,
        nobs,
        0usize,
        "unable to reserve qualified clustered radix order",
    )?;
    for (row, target) in order.iter_mut().enumerate() {
        *target = row;
    }
    let mut scratch = reserved_filled(
        reservation,
        nobs,
        0usize,
        "unable to reserve qualified clustered radix scratch",
    )?;
    let mut counts = reserved_filled(
        reservation,
        maximum,
        0usize,
        "unable to reserve qualified clustered radix counts",
    )?;
    for dimension in (0..dimensions.len()).rev() {
        if mask & (1 << dimension) == 0 {
            continue;
        }
        let active = &mut counts[..radices[dimension]];
        active.fill(0);
        for row in order.iter().copied() {
            let key = dimensions[dimension][row] as usize;
            active[key] += 1;
        }
        let mut offset = 0;
        for count in active.iter_mut() {
            let frequency = *count;
            *count = offset;
            offset += frequency;
        }
        for row in order.iter().copied() {
            let key = dimensions[dimension][row] as usize;
            let position = active[key];
            scratch[position] = row;
            active[key] = position + 1;
        }
        std::mem::swap(&mut order, &mut scratch);
    }
    Ok(order)
}
fn candidate_order<R: SandwichReservation>(
    dimensions: &[&[u32]],
    mask: usize,
    nobs: usize,
    maximum_dimensions: usize,
    reservation: &mut R,
) -> Result<Vec<usize>> {
    let mut radices = [0usize; 16];
    let mut maximum = 0usize;
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1 << dimension) == 0 {
            continue;
        }
        let radix = codes
            .iter()
            .copied()
            .max()
            .and_then(|v| usize::try_from(v).ok())
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| invalid_data("qualified clustered radix has no groups"))?;
        radices[dimension] = radix;
        maximum = maximum.max(radix);
    }
    let scratch_bytes = nobs
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("qualified clustered radix bytes overflow"))?;
    let count_bytes = maximum
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("qualified clustered radix count bytes overflow"))?;
    if dimensions.len() > maximum_dimensions
        || count_bytes > scratch_bytes.saturating_mul(2).max(1 << 20)
    {
        let mut order = reserved_empty(
            reservation,
            nobs,
            "unable to reserve qualified clustered ordering",
        )?;
        for row in 0..nobs {
            push_reserved(
                &mut order,
                row,
                "qualified clustered ordering capacity exhausted",
            )?;
        }
        order.sort_unstable_by(|left, right| {
            compare_rows(dimensions, mask, *left, *right).then_with(|| left.cmp(right))
        });
        return Ok(order);
    }
    let mut order = reserved_filled(
        reservation,
        nobs,
        0usize,
        "unable to reserve qualified clustered radix order",
    )?;
    for (row, target) in order.iter_mut().enumerate() {
        *target = row;
    }
    let mut scratch = reserved_filled(
        reservation,
        nobs,
        0usize,
        "unable to reserve qualified clustered radix scratch",
    )?;
    let mut counts = reserved_filled(
        reservation,
        maximum,
        0usize,
        "unable to reserve qualified clustered radix counts",
    )?;
    for dimension in (0..dimensions.len()).rev() {
        if mask & (1 << dimension) == 0 {
            continue;
        }
        let active = &mut counts[..radices[dimension]];
        active.fill(0);
        // Order is a full permutation; histogram frequencies do not depend on it.
        for &code in dimensions[dimension] {
            active[code as usize] += 1;
        }
        let mut offset = 0;
        for count in active.iter_mut() {
            let frequency = *count;
            *count = offset;
            offset += frequency;
        }
        for row in order.iter().copied() {
            let key = dimensions[dimension][row] as usize;
            let position = active[key];
            scratch[position] = row;
            active[key] = position + 1;
        }
        std::mem::swap(&mut order, &mut scratch);
    }
    Ok(order)
}
fn main() {
 for (name,n,groups) in [("small",7680,97),("few",983040,97),("many",983040,65536),("gaps",983040,2000000)] {
  let mut state=104729u64;
  let cols:Vec<Vec<u32>>=(0..3).map(|_|(0..n).map(|_|{state=state.wrapping_mul(6364136223846793005).wrapping_add(1); ((state>>32) % groups) as u32}).collect()).collect();
  let dims:Vec<&[u32]>=cols.iter().map(Vec::as_slice).collect();
  for mask in [1,3,7] {
   let mut expected:Vec<usize>=(0..n).collect();
   expected.sort_unstable_by(|a,b|compare_rows(&dims,mask,*a,*b).then_with(||a.cmp(b)));
   for rep in 0..7 {
    for arm in if rep%2==0 {[0,1]}else{[1,0]} {
     let start=Instant::now();
     let result=if arm==0 {baseline_order(&dims,mask,n,10,&mut Reservation)}else{candidate_order(&dims,mask,n,10,&mut Reservation)}.unwrap();
     let seconds=start.elapsed().as_secs_f64();
     assert_eq!(result,expected);black_box(result);
     println!("{name},{mask},{rep},{arm},{seconds:.9}");
    }
   }
  }
 }
}
