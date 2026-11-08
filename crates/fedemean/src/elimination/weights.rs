//! Current-weight state: cell masses, segment inverses, the Schur diagonal.

use super::structure::{each_lane2, refill, refill_from, reserve, Structure};
use crate::design::Members;
use crate::error::Result;
use crate::weights::Validated as ValidatedWeights;
use std::ops::Range;

/// Current-weight cell masses, segment inverses and the Schur diagonal.
pub(crate) struct Weights {
    /// Cell weights; empty when every cell has unit weight.
    pub(super) cell: Vec<f64>,
    pub(super) seg_inverse: Vec<f64>,
    pub(super) mass: Vec<f64>,
    /// One retained block: each level's mass in the active segments.
    pub(super) active_mass: Vec<f64>,
    pub(super) inverse_diagonal: Vec<f64>,
    pub(super) inverse_root_mass: Vec<f64>,
    pub(super) raw_scale: f64,
    /// Total normalized weight of the sample.
    pub(super) total_mass: f64,
    /// Per-lane accumulators of the preparation pass.
    pub(super) lanes: Vec<WeightLane>,
    pub(super) bytes: u128,
}

#[derive(Default)]
pub(super) struct WeightLane {
    pub(super) mass: Vec<f64>,
    pub(super) diag: Vec<f64>,
    pub(super) active: Vec<f64>,
    /// Stamped per-segment level sums (two or more retained blocks).
    pub(super) sums: Vec<f64>,
    pub(super) stamp: Vec<u32>,
    pub(super) touched: Vec<u32>,
    pub(super) bad: bool,
}

impl Weights {
    pub(super) fn empty() -> Self {
        Self {
            cell: Vec::new(),
            seg_inverse: Vec::new(),
            mass: Vec::new(),
            active_mass: Vec::new(),
            inverse_diagonal: Vec::new(),
            inverse_root_mass: Vec::new(),
            raw_scale: 1.0,
            total_mass: 0.0,
            lanes: Vec::new(),
            bytes: 0,
        }
    }

    /// Prepare the state of `weights`, reusing the buffers of `recycled` (a
    /// previous state of the same structure) when given; then repeated
    /// preparations allocate nothing.
    pub(super) fn prepare(
        st: &Structure,
        index: Members<'_>,
        weights: ValidatedWeights<'_>,
        recycled: Option<Self>,
    ) -> Result<Option<Self>> {
        let mut state = recycled.unwrap_or_else(Self::empty);
        Ok(state.fill(st, index, weights)?.then_some(state))
    }

    /// Form the state in place; `false` when a mass is not positive.
    pub(super) fn fill(
        &mut self,
        st: &Structure,
        index: Members<'_>,
        weights: ValidatedWeights<'_>,
    ) -> Result<bool> {
        self.reset(st, weights)?;
        let unit = self.cell.is_empty();
        let cell_bounds = if unit {
            &st.zero_bounds
        } else {
            &st.cell_bounds
        };
        each_lane2(
            0,
            (&st.seg_bounds, &mut self.seg_inverse),
            (cell_bounds, &mut self.cell),
            &mut self.lanes,
            &|l, inverse: &mut [f64], part: &mut [f64], acc: &mut WeightLane| {
                fill_lane(st, index, weights, unit, l, inverse, part, acc);
            },
        );
        if self.lanes.iter().any(|lane| lane.bad) {
            return Ok(false);
        }
        if !self.finish(st.reduced)? {
            return Ok(false);
        }
        self.raw_scale = weights.raw_scale;
        Ok(true)
    }

    /// Size every buffer for `st` and these weights, keeping allocations.
    fn reset(&mut self, st: &Structure, weights: ValidatedWeights<'_>) -> Result<()> {
        let cells = st.cells();
        let m = st.reduced;
        let nb = st.blocks.len();
        let lanes = st.lanes.len();
        // Every weighted cell is re-formed by the lane pass.
        match weights.values {
            None if st.multiplicity.is_empty() => self.cell.clear(),
            None => refill_from(&mut self.cell, &st.multiplicity)?,
            Some(_) if self.cell.len() == cells => {}
            Some(_) => refill(&mut self.cell, cells, 0.0)?,
        }
        if self.seg_inverse.len() != st.segments {
            refill(&mut self.seg_inverse, st.segments, 0.0)?;
        }
        if self.lanes.len() != lanes {
            self.lanes.clear();
            reserve(&mut self.lanes, lanes)?;
            self.lanes.resize_with(lanes, WeightLane::default);
        }
        for lane in &mut self.lanes {
            refill(&mut lane.mass, m, 0.0)?;
            refill(&mut lane.diag, m, 0.0)?;
            if nb == 1 {
                refill(&mut lane.active, m, 0.0)?;
            } else {
                if lane.sums.len() != m {
                    refill(&mut lane.sums, m, 0.0)?;
                }
                refill(&mut lane.stamp, m, u32::MAX)?;
                lane.touched.clear();
                reserve(&mut lane.touched, st.max_seg_cells * nb)?;
            }
            lane.bad = false;
        }
        Ok(())
    }

    /// Lane sums in lane order, the inverse Schur diagonal and root masses,
    /// the byte count and the total mass; `false` when a mass is not
    /// positive.
    fn finish(&mut self, m: usize) -> Result<bool> {
        let (first, rest) = self.lanes.split_first().expect("one lane");
        refill_from(&mut self.mass, &first.mass)?;
        refill_from(&mut self.inverse_diagonal, &first.diag)?;
        refill_from(&mut self.active_mass, &first.active)?;
        for other in rest {
            for (a, b) in self.mass.iter_mut().zip(&other.mass) {
                *a += b;
            }
            for (a, b) in self.inverse_diagonal.iter_mut().zip(&other.diag) {
                *a += b;
            }
            for (a, b) in self.active_mass.iter_mut().zip(&other.active) {
                *a += b;
            }
        }
        refill(&mut self.inverse_root_mass, m, 0.0)?;
        for j in 0..m {
            let total = self.mass[j];
            if !(total.is_finite() && total > 0.0) {
                return Ok(false);
            }
            let d = self.inverse_diagonal[j];
            self.inverse_diagonal[j] = if d.is_finite() && d > 1e-13 * total {
                1.0 / d
            } else {
                0.0
            };
            self.inverse_root_mass[j] = 1.0 / total.sqrt();
        }
        let lane_bytes: u128 = self
            .lanes
            .iter()
            .map(|lane| {
                (lane.mass.capacity()
                    + lane.diag.capacity()
                    + lane.active.capacity()
                    + lane.sums.capacity()) as u128
                    * 8
                    + (lane.stamp.capacity() + lane.touched.capacity()) as u128 * 4
            })
            .sum();
        self.bytes = (self.cell.capacity()
            + self.seg_inverse.capacity()
            + self.mass.capacity()
            + self.active_mass.capacity()
            + self.inverse_diagonal.capacity()
            + self.inverse_root_mass.capacity()) as u128
            * 8
            + lane_bytes;
        self.total_mass = self.seg_inverse.iter().map(|inverse| 1.0 / inverse).sum();
        Ok(true)
    }
}

/// One lane of the preparation pass, one segment at a time: its cell weights
/// (summed from the member rows), then the per-segment level sums `a_j` in a
/// stamped lane accumulator; the Schur diagonal is
/// `mass_j - sum_s a_sj^2 / W_s`.
#[allow(clippy::too_many_arguments)]
fn fill_lane(
    st: &Structure,
    index: Members<'_>,
    weights: ValidatedWeights<'_>,
    unit: bool,
    l: usize,
    inverse: &mut [f64],
    part: &mut [f64],
    acc: &mut WeightLane,
) {
    let member_offsets = index.offsets();
    let nb = st.blocks.len();
    let values = weights.values;
    let scale = weights.raw_scale;
    let lane = &st.lanes[l];
    let WeightLane {
        mass,
        diag,
        active,
        sums,
        stamp,
        touched,
        bad,
    } = acc;
    let (mass, diag, active) = (&mut mass[..], &mut diag[..], &mut active[..]);
    let (sums, stamp) = (&mut sums[..], &mut stamp[..]);
    let mut cursor = member_offsets[lane.segments.start];
    for (s, inv) in lane.segments.clone().zip(inverse.iter_mut()) {
        let (c0, c1) = (st.seg_cells[s], st.seg_cells[s + 1]);
        let cw: &[f64] = if unit {
            &[]
        } else {
            let local = &mut part[c0 - lane.cells.start..c1 - lane.cells.start];
            if let Some(values) = values {
                local.fill(0.0);
                for &row in index.level(s) {
                    local[st.member_cell[cursor] as usize] += values[row] / scale;
                    cursor += 1;
                }
            }
            local
        };
        if nb == 1 {
            // One retained block: every cell of a segment has a distinct
            // code, so `a_sj` is that cell's weight.
            let codes = &st.codes[c0..c1];
            let total: f64 = if unit {
                (c1 - c0) as f64
            } else {
                cw.iter().sum()
            };
            if !(total.is_finite() && total > 0.0) {
                *bad = true;
                continue;
            }
            let inverse_total = 1.0 / total;
            *inv = inverse_total;
            if c1 - c0 == 1 {
                // A one-cell segment adds mass but no curvature.
                mass[codes[0] as usize] += total;
                continue;
            }
            for (k, &code) in codes.iter().enumerate() {
                let w = if unit { 1.0 } else { cw[k] };
                let j = code as usize;
                mass[j] += w;
                active[j] += w;
                diag[j] += w - w * w * inverse_total;
            }
            continue;
        }
        let mut total = 0.0;
        touched.clear();
        let tag = s as u32;
        for (k, cc) in st.codes[c0 * nb..c1 * nb].chunks_exact(nb).enumerate() {
            let w = if unit { 1.0 } else { cw[k] };
            total += w;
            for &code in cc {
                let j = code as usize;
                mass[j] += w;
                if stamp[j] == tag {
                    sums[j] += w;
                } else {
                    stamp[j] = tag;
                    sums[j] = w;
                    touched.push(code);
                }
            }
        }
        if !(total.is_finite() && total > 0.0) {
            *bad = true;
            continue;
        }
        let inverse_total = 1.0 / total;
        *inv = inverse_total;
        if c1 - c0 == 1 {
            continue;
        }
        for &code in touched.iter() {
            let j = code as usize;
            let a = sums[j];
            diag[j] += a - a * a * inverse_total;
        }
    }
}

/// Identity of a weight vector: unit, or its slice address, length and a
/// digest of its bits (callers may refill one buffer between frames).
pub(super) type WeightKey = Option<(usize, usize, u64)>;

pub(super) fn weight_key(values: Option<&[f64]>) -> Result<WeightKey> {
    fn mix(h: u64, bits: u64) -> u64 {
        let h = (h ^ bits).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        h ^ (h >> 31)
    }
    let Some(values) = values else {
        return Ok(None);
    };
    // Fixed leaves and tree: the digest depends only on the bits.
    let digest = crate::reduce::fixed_tree_reduce(
        values.len(),
        1 << 16,
        &|range: Range<usize>| {
            let chunk = &values[range];
            // Four interleaved chains keep the multiplications independent.
            let mut h = [
                0x243f_6a88_85a3_08d3_u64,
                0x1319_8a2e_0370_7344,
                0xa409_3822_299f_31d0,
                0x082e_fa98_ec4e_6c89,
            ];
            let mut quads = chunk.chunks_exact(4);
            for quad in &mut quads {
                for lane in 0..4 {
                    h[lane] = mix(h[lane], quad[lane].to_bits());
                }
            }
            for (lane, v) in quads.remainder().iter().enumerate() {
                h[lane] = mix(h[lane], v.to_bits());
            }
            Ok(h.iter()
                .fold(chunk.len() as u64, |acc, &lane| mix(acc, lane)))
        },
        &|left, right| mix(mix(0xcbf2_9ce4_8422_2325, left), right),
    )?;
    Ok(Some((values.as_ptr() as usize, values.len(), digest)))
}
