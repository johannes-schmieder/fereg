//! Shape-only plans consumed by the reusable projection owners.
use super::borrowed_map::{
    CATEGORICAL_MAP_ACCELERATOR_MAX_RHS, FUSED_PARALLEL_MAX_RHS, PARALLEL_DOT_CHUNK_ROWS,
    PARALLEL_PROJECTION_MIN_ROWS,
};
use crate::error::{ErrorCode, FeregError, Result};
use crate::memory::Capacity;

fn product(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b).ok_or_else(overflow)
}
fn sum(values: &[u128]) -> Result<u128> {
    values
        .iter()
        .try_fold(0_u128, |a, &b| a.checked_add(b).ok_or_else(overflow))
}
fn overflow() -> FeregError {
    FeregError::new(ErrorCode::Allocation, "FE workspace capacity overflows")
}

pub(crate) struct ProjectionCapacity {
    pub(crate) gram: Capacity<f64>,
    pub(crate) rhs: Capacity<f64>,
}
impl ProjectionCapacity {
    pub(crate) fn new(channels: u128, rhs: u128) -> Result<Self> {
        Ok(Self {
            gram: Capacity::matrix(channels, channels, "projection Gram dimensions overflow")?,
            rhs: Capacity::matrix(channels, rhs, "projection RHS dimensions overflow")?,
        })
    }
}
pub(crate) struct MapCapacity {
    pub(crate) certificate: Capacity<f64>,
    projection: ProjectionCapacity,
    scalars: u128,
}
impl MapCapacity {
    pub(crate) fn new(channels: u128, members: u128, rhs: u128) -> Result<Self> {
        Ok(Self {
            certificate: Capacity::matrix(
                channels,
                members,
                "reusable MAP certificate dimensions overflow",
            )?,
            projection: ProjectionCapacity::new(channels, rhs)?,
            scalars: product(rhs, 7)?,
        })
    }
    pub(crate) fn cells(&self) -> Result<u128> {
        sum(&[
            self.projection.gram.elements(),
            self.projection.rhs.elements(),
            self.scalars,
            product(self.certificate.elements(), 2)?,
        ])
    }
}

pub(crate) fn categorical_parallel_counts(
    levels: u128,
    rows: u128,
    rhs: u128,
    threads: u128,
) -> Result<[u128; 3]> {
    if rows < PARALLEL_PROJECTION_MIN_ROWS as u128 || threads <= 1 {
        return Ok([0; 3]);
    }
    Ok([
        levels,
        product(levels, rhs)?,
        if rhs <= FUSED_PARALLEL_MAX_RHS as u128 {
            product(rows.div_ceil(PARALLEL_DOT_CHUNK_ROWS as u128), rhs)?
        } else {
            0
        },
    ])
}

#[derive(Clone, Copy)]
pub(crate) struct TwoFeCapacity {
    pub(crate) left: u128,
    pub(crate) right: u128,
    levels: u128,
    component: u128,
}
impl TwoFeCapacity {
    pub(crate) fn new(left_levels: u128, right_levels: u128, rhs: u128) -> Result<Self> {
        Ok(Self {
            left: product(left_levels, rhs)?,
            right: product(right_levels, rhs)?,
            levels: sum(&[left_levels, right_levels])?,
            component: product(right_levels, 2)?,
        })
    }
    fn cells(self) -> Result<u128> {
        sum(&[
            self.levels,
            self.component,
            product(self.left, 2)?,
            product(self.right, 4)?,
        ])
    }
}
#[derive(Clone, Copy)]
pub(crate) enum CategoricalAcceleratorCapacity {
    None,
    Observation(u128),
    TwoFe(TwoFeCapacity),
}
impl CategoricalAcceleratorCapacity {
    pub(crate) fn new(
        effects: usize,
        categorical: bool,
        rows: u128,
        rhs: u128,
        levels: u128,
        maximum_levels: u128,
    ) -> Result<Self> {
        Ok(
            if !categorical
                || !matches!(effects, 2 | 3)
                || rhs > CATEGORICAL_MAP_ACCELERATOR_MAX_RHS as u128
            {
                Self::None
            } else if effects == 2 && rows >= 128 {
                Self::TwoFe(TwoFeCapacity::new(
                    maximum_levels,
                    levels.checked_sub(maximum_levels).ok_or_else(overflow)?,
                    rhs,
                )?)
            } else {
                Self::Observation(product(rows, rhs)?)
            },
        )
    }
    pub(crate) fn cells(self, rhs: u128) -> Result<u128> {
        match self {
            Self::None => Ok(0),
            Self::Observation(n) => sum(&[product(n, 3)?, product(rhs, 5)?]),
            Self::TwoFe(plan) => sum(&[plan.cells()?, product(rhs, 5)?]),
        }
    }
}
