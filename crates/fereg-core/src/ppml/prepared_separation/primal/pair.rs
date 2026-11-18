//! Exact two-coordinate candidates from a positive-row contrast. This
//! recovers oblique rays whose rational slope exceeds modular reconstruction's
//! size range. Raw checks on every observation determine feasibility.
use super::*;

pub(super) fn search(search: &mut Search<'_>, work: usize) -> Result<bool> {
    let model = search.model;
    let n = model.outcome().len();
    let k = model.model().columns.len();
    let Some(anchor) = model.outcome().iter().position(|&y| y > 0.) else {
        return Ok(false);
    };
    let view = model.fit_view();
    let mut remaining = work / n.saturating_mul(3).max(1);
    for left in 0..k {
        for right in left + 1..k {
            if remaining == 0 {
                return Ok(false);
            }
            remaining -= 1;
            let origin = [
                view.column_value(left, anchor)?,
                view.column_value(right, anchor)?,
            ];
            for (r, &y) in model.outcome().iter().enumerate() {
                crate::interrupt::row_checkpoint(r)?;
                if y == 0. {
                    continue;
                }
                let a = view.column_value(left, r)? - origin[0];
                let b = view.column_value(right, r)? - origin[1];
                if a == 0. && b == 0. {
                    continue;
                }
                // Rounded differences are only proposals. The whole raw design
                // is verified again using exact dot products.
                if let Some([p, q]) = integer_pair(b, -a) {
                    for weights in [[p, q], [-p, -q]] {
                        if search.consider(Witness::Pair {
                            columns: [left, right],
                            weights,
                        })? {
                            return Ok(true);
                        }
                    }
                }
                break;
            }
        }
    }
    Ok(false)
}

fn integer_pair(left: f64, right: f64) -> Option<[i64; 2]> {
    fn dyadic(v: f64) -> Option<(u64, i32)> {
        if v == 0. || !v.is_finite() {
            return None;
        }
        let bits = v.abs().to_bits();
        let field = (bits >> 52) as i32;
        let significand = (bits & ((1u64 << 52) - 1)) | if field == 0 { 0 } else { 1u64 << 52 };
        let shift = significand.trailing_zeros();
        Some((significand >> shift, field.max(1) - 1075 + shift as i32))
    }
    let (a, e) = dyadic(left)?;
    let (b, f) = dyadic(right)?;
    let (mut u, mut v) = (a, b);
    while v != 0 {
        (u, v) = (v, u % v);
    }
    let scale = |m: u64, exp: i32, negative: bool| {
        let delta = (exp - e.min(f)) as u32;
        let reduced = m / u;
        if delta >= 63 || reduced > ((i64::MAX as u64) >> delta) {
            return None;
        }
        let x = (reduced << delta) as i64;
        Some(if negative { -x } else { x })
    };
    Some([scale(a, e, left < 0.)?, scale(b, f, right < 0.)?])
}
