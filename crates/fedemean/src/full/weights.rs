//! Current-weight state of the full formulation: normalized observation
//! weights, every coefficient's mass (the diagonal of `D'WD`) and, for the
//! effects with slopes, their levels' channel blocks ([`Blocks`]).

use super::blocks::Blocks;
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
    /// The diagonal of `D'WD`: level masses, and a slope channel's
    /// `sum w x^2`.
    pub(super) mass: Vec<f64>,
    /// Inverse masses and their roots: zero on the effects with blocks,
    /// whose preconditioner and measures are their blocks'.
    pub(super) inverse_mass: Vec<f64>,
    pub(super) inverse_root_mass: Vec<f64>,
    /// The channel blocks (empty for an intercept-only design).
    pub(super) blocks: Blocks,
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

    /// The channel blocks of a design with slopes.
    pub(super) fn blocks(&self) -> Option<&Blocks> {
        (self.blocks.tasks() > 0).then_some(&self.blocks)
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
        if st.sloped {
            let normalized = (!self.normalized.is_empty()).then_some(&self.normalized[..]);
            self.blocks.fill(st, effects, normalized, &mut self.mass)?;
        } else {
            self.blocks = Blocks::default();
        }
        refill(&mut self.inverse_mass, m, 0.0)?;
        refill(&mut self.inverse_root_mass, m, 0.0)?;
        for (e, effect) in effects.iter().enumerate() {
            let (lo, hi) = (st.offsets[e], st.offsets[e + 1]);
            if effect.sloped() {
                // Every observed level of an intercept channel has weight.
                let channels = effect.channels();
                let observed = !effect.intercept()
                    || self.mass[lo..hi].iter().step_by(channels).all(|&m| m > 0.0);
                if !observed {
                    return Ok(false);
                }
                continue;
            }
            for g in lo..hi {
                let mass = self.mass[g];
                if !(mass.is_finite() && mass > 0.0) {
                    return Ok(false);
                }
                self.inverse_mass[g] = 1.0 / mass;
                self.inverse_root_mass[g] = 1.0 / mass.sqrt();
            }
        }
        self.raw_scale = scale;
        self.total_mass = if effects[0].sloped() {
            self.weights().map_or(n as f64, |w| {
                w.chunks(1 << 16).map(|c| c.iter().sum::<f64>()).sum()
            })
        } else {
            let first = &self.mass[st.offsets[0]..st.offsets[1]];
            first.iter().sum()
        };
        self.bytes = (self.normalized.capacity()
            + self.root.capacity()
            + self.mass.capacity()
            + self.inverse_mass.capacity()
            + self.inverse_root_mass.capacity()) as u128
            * 8
            + self.blocks.bytes();
        Ok(true)
    }
}
