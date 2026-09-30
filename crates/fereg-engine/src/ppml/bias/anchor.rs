//! One support-only linear gauge for raw, derivative and curvature coordinates.
use super::{filled, invalid, numerical};
use crate::{data::DenseMatrix, error::Result, topology::FeTopology};
pub(super) struct Anchor {
    pub first: usize,
    pub second: usize,
    roots: Vec<usize>,
    left_components: Vec<usize>,
    right_components: Vec<usize>,
}
impl Anchor {
    pub fn new(topology: &FeTopology) -> Result<Self> {
        if topology.effects().len() != 2
            || topology.effects().iter().any(|e| {
                !e.intercept
                    || !e.slopes.is_empty()
                    || e.term_id.is_none()
                    || e.categorical_sources.is_empty()
            })
        {
            return Err(invalid(
                "bias information requires two categorical intercept FE sets",
            ));
        }
        let first = topology.effect_partition(0)?.n_levels();
        let right = topology.effect_partition(1)?;
        let components = topology.components();
        let mut roots = filled(components.n_components, usize::MAX)?;
        for (level, &component) in components.component_by_effect_level[1].iter().enumerate() {
            if roots[component] == usize::MAX
                || right.original_levels()[level] < right.original_levels()[roots[component]]
            {
                roots[component] = level;
            }
        }
        if roots.contains(&usize::MAX) {
            return Err(invalid("bias component has no second-FE anchor"));
        }
        let mut left_components = filled(first, 0)?;
        left_components.copy_from_slice(&components.component_by_effect_level[0]);
        let mut right_components = filled(right.n_levels(), 0)?;
        right_components.copy_from_slice(&components.component_by_effect_level[1]);
        Ok(Self {
            first,
            second: right.n_levels(),
            roots,
            left_components,
            right_components,
        })
    }
    pub fn levels(&self) -> usize {
        self.first + self.second
    }
    pub fn apply(&self, alpha: &DenseMatrix, output: &mut [f64]) -> Result<()> {
        if alpha.nrows() != self.levels() || alpha.ncols() != 1 || output.len() < self.levels() {
            return Err(invalid("bias anchor coefficient dimensions differ"));
        }
        for (level, &component) in self.left_components.iter().enumerate() {
            output[level] = alpha[(level, 0)] + alpha[(self.first + self.roots[component], 0)];
        }
        for (level, &component) in self.right_components.iter().enumerate() {
            output[self.first + level] =
                alpha[(self.first + level, 0)] - alpha[(self.first + self.roots[component], 0)];
        }
        if output[..self.levels()].iter().any(|v| !v.is_finite()) {
            return Err(numerical("bias anchor transfer is not representable"));
        }
        Ok(())
    }
}
