//! Select positive observations for the numerical nullspace proposal. A
//! nonzero binary minor modulo two is nonzero over the reals. A zero minor
//! gives no upper rank bound: the caller still certifies the full raw model.
use super::{fallible_filled, reserve, BinaryDesign};
use crate::error::Result;

/// `max_cells` is the eight-byte scratch allowance left beside the design and
/// `stage_work` the coordinate operations allowed for the selection.
/// Rows enter in input order. Each pivot stores only the suffix beginning at
/// its first nonzero word, and no storage scales with the number of positives
/// once that number exceeds the width.
pub(super) fn select(
    design: &BinaryDesign,
    outcome: &[f64],
    (max_cells, stage_work): (usize, usize),
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Vec<usize>>> {
    interrupt()?;
    let width = design.ncols();
    let words = width.div_ceil(u64::BITS as usize);
    let capacity = width.min(outcome.len());
    let headers = std::mem::size_of::<Vec<u64>>().div_ceil(8);
    let fixed = width as u128 * headers as u128 + words as u128 + capacity as u128;
    if width == 0 || fixed > max_cells as u128 || fixed > stage_work as u128 {
        return Ok(None);
    }
    let mut free = max_cells - fixed as usize;
    let mut work = stage_work - fixed as usize;
    let mut pivots = fallible_filled(width, Vec::new(), "unable to reserve binary pivot rows")?;
    let mut row = fallible_filled(words, 0u64, "unable to reserve packed positive row")?;
    let mut selected = Vec::new();
    reserve(
        &mut selected,
        capacity,
        "unable to reserve positive row IDs",
    )?;

    'rows: for (i, &y) in outcome.iter().enumerate() {
        if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        if work == 0 {
            return Ok(None);
        }
        work -= 1;
        if y <= 0. || selected.len() == width {
            continue;
        }
        let ones = design.row(i);
        let Some(left) = work.checked_sub(2 * words + ones.len()) else {
            return Ok(None);
        };
        work = left;
        row.fill(0);
        for &column in ones.iter() {
            let j = column as usize;
            row[j / 64] |= 1u64 << (j % 64);
        }
        for word in 0..words {
            while row[word] != 0 {
                let column = 64 * word + row[word].trailing_zeros() as usize;
                let suffix = &mut row[word..];
                let Some(left) = work.checked_sub(1 + suffix.len()) else {
                    return Ok(None);
                };
                work = left;
                crate::interrupt::checkpoint()?;
                let pivot = &mut pivots[column];
                if pivot.is_empty() {
                    let Some(left) = free.checked_sub(suffix.len()) else {
                        return Ok(None);
                    };
                    free = left;
                    reserve(pivot, suffix.len(), "unable to reserve packed pivot suffix")?;
                    pivot.extend_from_slice(suffix);
                    selected.push(i);
                    continue 'rows;
                }
                for (value, previous) in suffix.iter_mut().zip(pivot.iter()) {
                    *value ^= previous;
                }
            }
        }
    }
    Ok(Some(selected))
}

/// Retry a reduced proposal over an odd prime when the packed minor
/// may have understated the positive rank. Every raw proof still replays.
pub(super) fn select_odd(
    design: &BinaryDesign,
    outcome: &[f64],
    (max_cells, stage_work): (usize, usize),
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Vec<usize>>> {
    let width = design.ncols();
    let capacity = width.min(outcome.len());
    let cells = width.saturating_mul(width + 2).saturating_add(capacity);
    if cells > max_cells {
        return Ok(None);
    }
    let mut work = stage_work.saturating_sub(cells);
    let mut space = crate::ppml::ExactNullspace::new(width)?;
    let mut row = fallible_filled(width, 0., "unable to reserve modular positive row")?;
    let mut selected = Vec::new();
    reserve(
        &mut selected,
        capacity,
        "unable to reserve modular positive row IDs",
    )?;
    let mut prior = None;
    for (i, &y) in outcome.iter().enumerate() {
        if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        if y <= 0. {
            continue;
        }
        let support = design.row(i);
        if prior.is_some_and(|p| *design.row(p) == *support) {
            continue;
        }
        prior = Some(i);
        let Some(left) = work.checked_sub((space.rank() + 3).saturating_mul(width)) else {
            return Ok(None);
        };
        work = left;
        row.fill(0.);
        for &j in support.iter() {
            row[j as usize] = 1.;
        }
        space.insert(&row)?;
        if space.rank() > selected.len() {
            selected.push(i);
            if selected.len() == width {
                break;
            }
        }
    }
    Ok(Some(selected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_uses_late_positive_pivots_across_word_boundaries() {
        let ones: &[&[usize]] = &[&[64], &[64], &[1, 64], &[1], &[0, 64], &[65]];
        let outcome = [0., 1., 1., 1., 1., 1.];
        let design = BinaryDesign::from_rows(6, 66, 256, |i, row| {
            row.fill(0.);
            for &j in ones[i] {
                row[j] = 1.;
            }
            Ok(())
        })
        .unwrap()
        .unwrap();
        let select_with = |cells| {
            select(
                &design,
                &outcome,
                (cells, super::super::MAX_WORK),
                &mut || Ok(()),
            )
            .unwrap()
        };
        let headers = 66 * std::mem::size_of::<Vec<u64>>().div_ceil(8);
        // Two scratch words, six row IDs and pivot suffixes of 1, 2, 2, 1.
        assert_eq!(select_with(headers + 14), Some(vec![1, 2, 4, 5]));
        assert_eq!(select_with(headers + 13), None);
    }

    #[test]
    fn even_determinant_can_leave_a_real_positive_direction_unrepresented() {
        let design = BinaryDesign::from_rows(3, 3, 20, |i, row| {
            row.fill(1.);
            row[i] = 0.;
            Ok(())
        })
        .unwrap()
        .unwrap();
        // J-I has determinant 2; this subset cannot certify real rank two.
        assert_eq!(
            select(&design, &[1.; 3], (64, super::super::MAX_WORK), &mut || Ok(
                ()
            ))
            .unwrap(),
            Some(vec![0, 1])
        );
        assert_eq!(
            select_odd(&design, &[1.; 3], (64, super::super::MAX_WORK), &mut || Ok(
                ()
            ))
            .unwrap(),
            Some(vec![0, 1, 2])
        );
        let error = select(&design, &[1.; 3], (64, super::super::MAX_WORK), &mut || {
            Err(crate::interrupt::interrupted())
        })
        .unwrap_err();
        assert_eq!(error.code, crate::error::ErrorCode::Interrupted);
    }
}
