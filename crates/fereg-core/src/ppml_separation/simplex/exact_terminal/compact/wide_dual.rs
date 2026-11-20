//! Fraction-free reconstruction for a compact dual with a large denominator.
use super::{fallible_filled, BinaryDesign, Result, Work};

/// Solve P'a + E'b = -sum(Z), where P indexes positive representatives and
/// E indexes zero rows with proposed additional mass. Only the independently
/// replayed certificate, with a positive baseline and b >= 0, can certify.
pub(super) fn certify(
    design: &BinaryDesign,
    outcome: &[f64],
    positive: &[usize],
    extras: &[usize],
    aggregate: &[f64],
    scratch: u128,
    work: &mut Work,
) -> Result<bool> {
    let k = design.ncols();
    let m = positive.len() + extras.len();
    if m == 0 || m > k {
        return Ok(false);
    }
    let stride = m + 1;
    let cells = k as u128 * stride as u128;
    // The matrix is released before original moments are replayed.
    if 2 * (cells + m as u128).max(k as u128 + m as u128) > scratch || !work.take(k, stride) {
        return Ok(false);
    }
    let mut system = fallible_filled(
        cells as usize,
        0i128,
        "unable to reserve exact compact dual matrix",
    )?;
    for (column, &i) in positive.iter().chain(extras).enumerate() {
        crate::interrupt::row_checkpoint(column)?;
        for &j in design.row(i) {
            system[j * stride + column] = 1;
        }
    }
    for (row, &count) in system.chunks_exact_mut(stride).zip(aggregate) {
        row[m] = -(count as i128);
    }
    let Some(denominator) = eliminate(&mut system, m, work)? else {
        return Ok(false);
    };
    let Some(solution) = substitute(&system, m, denominator, work)? else {
        return Ok(false);
    };
    drop(system);
    if solution[positive.len()..].iter().any(|&b| b < 0) {
        return Ok(false);
    }

    // Assign the baseline to every original boundary row and zero to unused
    // positive rows. A checked sum per raw column replays the certificate.
    let mut moment = fallible_filled(k, 0i128, "unable to reserve raw wide dual moments")?;
    let mut add = |i: usize, weight: i128| {
        let row = design.row(i);
        if !work.take(1, row.len() + 1) {
            return false;
        }
        for &j in row {
            let Some(sum) = moment[j].checked_add(weight) else {
                return false;
            };
            moment[j] = sum;
        }
        true
    };
    for (i, &y) in outcome.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if y == 0. && !add(i, denominator) {
            return Ok(false);
        }
    }
    for (i, (&row, &weight)) in positive.iter().chain(extras).zip(&solution).enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if !add(row, weight) {
            return Ok(false);
        }
    }
    Ok(moment.iter().all(|&w| w == 0))
}

/// Each trailing entry is a signed minor. The previous pivot divides the
/// numerator exactly after a row exchange as well.
fn eliminate(matrix: &mut [i128], m: usize, work: &mut Work) -> Result<Option<i128>> {
    let stride = m + 1;
    let mut previous = 1i128;
    for j in 0..m {
        crate::interrupt::checkpoint()?;
        let rows = matrix.len() / stride;
        if !work.take(1, rows - j + stride) {
            return Ok(None);
        }
        let pivot = (j..rows)
            .filter(|&i| matrix[i * stride + j] != 0)
            .min_by_key(|&i| matrix[i * stride + j].unsigned_abs());
        let Some(pivot) = pivot else { return Ok(None) };
        if pivot != j {
            for c in j..stride {
                matrix.swap(j * stride + c, pivot * stride + c);
            }
        }
        let (top, bottom) = matrix.split_at_mut((j + 1) * stride);
        let source = &top[j * stride..];
        let diagonal = source[j];
        if j + 1 == m {
            return Ok(diagonal.checked_abs());
        }
        for (i, row) in bottom.chunks_exact_mut(stride).enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            let factor = row[j];
            if factor == 0 && diagonal == previous {
                continue;
            }
            if !work.take(1, stride - j) {
                return Ok(None);
            }
            row[j] = 0;
            for (v, &p) in row[j + 1..].iter_mut().zip(&source[j + 1..]) {
                if *v == 0 && (p == 0 || factor == 0) {
                    continue;
                }
                let Some(value) = diagonal
                    .checked_mul(*v)
                    .and_then(|a| a.checked_sub(factor.checked_mul(p)?))
                    .and_then(|n| exact_div(n, previous))
                else {
                    return Ok(None);
                };
                *v = value;
            }
        }
        previous = diagonal;
    }
    Ok(previous.checked_abs())
}

fn exact_div(n: i128, d: i128) -> Option<i128> {
    if d == 1 || n == 0 {
        Some(n)
    } else if n.checked_rem(d)? == 0 {
        n.checked_div(d)
    } else {
        None
    }
}

/// The final pivot is a nonzero maximal minor. Its absolute value is a
/// common denominator for the unique solution on the selected equations.
fn substitute(
    matrix: &[i128],
    m: usize,
    denominator: i128,
    work: &mut Work,
) -> Result<Option<Vec<i128>>> {
    let stride = m + 1;
    let mut weights = fallible_filled(m, 0i128, "unable to reserve exact compact dual weights")?;
    for j in (0..m).rev() {
        crate::interrupt::checkpoint()?;
        if !work.take(1, m - j) {
            return Ok(None);
        }
        let row = &matrix[j * stride..(j + 1) * stride];
        let Some(mut numerator) = denominator.checked_mul(row[m]) else {
            return Ok(None);
        };
        for (&a, &w) in row[j + 1..m].iter().zip(&weights[j + 1..]) {
            let Some(rest) = a
                .checked_mul(w)
                .and_then(|product| numerator.checked_sub(product))
            else {
                return Ok(None);
            };
            numerator = rest;
        }
        let Some(value) = exact_div(numerator, row[j]) else {
            return Ok(None);
        };
        weights[j] = value;
    }
    Ok(Some(weights))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_binary_dual_has_a_large_path_count_denominator() {
        // T has diagonal 1 and superdiagonals 1 and 3. In T^{-1} the
        // magnitude of (0,n-1) counts the paths of length n-1 made of
        // steps 1 and 3. Bordering T by e_last and e_first' gives this minor.
        let n = 89;
        let m = n + 1;
        let mut ways = vec![1i128; n];
        for i in 3..n {
            ways[i] = ways[i - 1] + ways[i - 3];
        }
        assert_eq!(ways[n - 1], 248_313_737_774_419);
        for inconsistent in [false, true] {
            let k = m + 1;
            let design = BinaryDesign::from_rows(m + 1, k, 100_000, |i, row| {
                row.fill(0.);
                match i.cmp(&n) {
                    std::cmp::Ordering::Less => {
                        row[i] = 1.;
                        if i >= 1 {
                            row[i - 1] = 1.;
                        }
                        if i >= 3 {
                            row[i - 3] = 1.;
                        }
                        if i == 0 {
                            row[n] = 1.;
                        }
                    }
                    std::cmp::Ordering::Equal => row[n - 1] = 1.,
                    std::cmp::Ordering::Greater => {
                        row[n] = 1.;
                        row[m] = f64::from(inconsistent);
                    }
                }
                Ok(())
            })
            .unwrap()
            .unwrap();
            let mut outcome = vec![1.; m + 1];
            outcome[m] = 0.;
            let mut total = vec![0.; k];
            total[n] = 1.;
            total[m] = f64::from(inconsistent);
            let positive: Vec<_> = (0..m).collect();
            assert_eq!(
                certify(
                    &design,
                    &outcome,
                    &positive,
                    &[],
                    &total,
                    100_000,
                    &mut Work(10_000_000)
                )
                .unwrap(),
                !inconsistent
            );
        }
    }

    #[test]
    fn an_unneeded_basic_adjustment_can_be_zero() {
        let points = [[0, 2], [1, 3], [0, 3], [1, 2]];
        let design = BinaryDesign::from_rows(4, 4, 64, |i, row| {
            row.fill(0.);
            for &j in &points[i] {
                row[j] = 1.;
            }
            Ok(())
        })
        .unwrap()
        .unwrap();
        assert!(certify(
            &design,
            &[1., 1., 0., 0.],
            &[0, 1],
            &[2],
            &[1.; 4],
            128,
            &mut Work(1024)
        )
        .unwrap());
    }

    #[test]
    fn arithmetic_overflow_declines_a_proposal() {
        let mut matrix = [i128::MAX, 1, 0, 0, 2, 0];
        assert!(eliminate(&mut matrix, 2, &mut Work(100)).unwrap().is_none());
    }
}
