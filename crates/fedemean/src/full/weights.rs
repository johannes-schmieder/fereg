//! Current-weight state of the full formulation: normalized observation
//! weights and every level's mass.

use super::kernels::{adjoint, Rows, PARALLEL_WORK};
use super::refill;
use super::structure::Structure;
use crate::design::Effect;
use crate::error::Result;
use crate::weights::Validated;
use rayon::prelude::*;

/// Weights divided by their maximum and the level masses they imply.
#[derive(Default)]
pub(crate) struct State {
    /// `w_i / max w` (empty for unit weights).
    pub(super) normalized: Vec<f64>,
    /// `sqrt(w_i / max w)` for modified LSMR (empty otherwise).
    pub(super) root: Vec<f64>,
    pub(super) mass: Vec<f64>,
    pub(super) inverse_mass: Vec<f64>,
    pub(super) inverse_root_mass: Vec<f64>,
    /// The maximum weight (1 for unit weights).
    pub(super) raw_scale: f64,
    /// Total normalized weight of the sample.
    pub(super) total_mass: f64,
    pub(super) bytes: u128,
}

impl State {
    /// Normalized weights, `None` for unit weights.
    pub(super) fn weights(&self) -> Option<&[f64]> {
        (!self.normalized.is_empty()).then_some(&self.normalized[..])
    }

    /// `sqrt` of the normalized weights, `None` for unit weights.
    #[cfg_attr(not(feature = "schwarz"), allow(dead_code))]
    pub(super) fn roots(&self) -> Option<&[f64]> {
        (!self.root.is_empty()).then_some(&self.root[..])
    }

    /// Form the state of `weights` in place, keeping allocations (with
    /// `roots` for modified LSMR); `false` when a level has no weight.
    pub(super) fn fill(
        &mut self,
        st: &Structure,
        effects: &[Effect<'_>],
        weights: Validated<'_>,
        roots: bool,
        scatter: &mut Vec<f64>,
    ) -> Result<bool> {
        let n = st.nobs;
        let scale = weights.raw_scale;
        match weights.values {
            Some(values) => {
                refill(&mut self.normalized, n, 0.0)?;
                let work = if n < PARALLEL_WORK {
                    usize::MAX
                } else {
                    1 << 16
                };
                self.normalized
                    .par_chunks_mut(work.min(n))
                    .zip(values.par_chunks(work.min(n)))
                    .for_each(|(out, values)| {
                        for (o, &v) in out.iter_mut().zip(values) {
                            *o = v / scale;
                        }
                    });
                if roots {
                    refill(&mut self.root, n, 0.0)?;
                    self.root
                        .par_chunks_mut(work.min(n))
                        .zip(self.normalized.par_chunks(work.min(n)))
                        .for_each(|(out, values)| {
                            for (o, &v) in out.iter_mut().zip(values) {
                                *o = v.sqrt();
                            }
                        });
                } else {
                    self.root.clear();
                }
            }
            None => {
                self.normalized.clear();
                self.root.clear();
            }
        }
        let m = st.coefficients();
        refill(&mut self.mass, m, 0.0)?;
        refill(scatter, st.scatter_blocks() * st.scatter_levels(), 0.0)?;
        let one = [1.0];
        let src = match weights.values {
            Some(_) => Rows {
                data: &self.normalized,
                stride: 1,
                first: 0,
            },
            None => Rows {
                data: &one,
                stride: 0,
                first: 0,
            },
        };
        adjoint::<1>(st, effects, src, None, &mut self.mass, scatter);
        refill(&mut self.inverse_mass, m, 0.0)?;
        refill(&mut self.inverse_root_mass, m, 0.0)?;
        for g in 0..m {
            let mass = self.mass[g];
            if !(mass.is_finite() && mass > 0.0) {
                return Ok(false);
            }
            self.inverse_mass[g] = 1.0 / mass;
            self.inverse_root_mass[g] = 1.0 / mass.sqrt();
        }
        self.raw_scale = scale;
        let first = &self.mass[st.offsets[0]..st.offsets[1]];
        self.total_mass = first.iter().sum();
        self.bytes = (self.normalized.capacity()
            + self.root.capacity()
            + self.mass.capacity()
            + self.inverse_mass.capacity()
            + self.inverse_root_mass.capacity()) as u128
            * 8;
        Ok(true)
    }
}
