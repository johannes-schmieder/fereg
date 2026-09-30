//! Simultaneous owners, rather than a sum of unrelated lifetime maxima.
use super::{
    checked::{add_count, array, matrix, mul, sum},
    solver, Request,
};
use crate::{data::DropReason, error::Result, spec::Estimator, ErrorCode, FeregError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Phase {
    Preparation,
    Separation,
    Estimation,
    Inference,
    BiasCorrection,
    Publication,
}
impl Phase {
    pub const ALL: [Self; 6] = [
        Self::Preparation,
        Self::Separation,
        Self::Estimation,
        Self::Inference,
        Self::BiasCorrection,
        Self::Publication,
    ];
}
/// Modeled native storage, not process RSS or an allocator/OS guarantee.
/// The minimum excludes upper bounds for active rows, sparse fill and strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estimate {
    phases: [u128; 6],
    minimum: u128,
    bias_workspace: u128,
}
impl Estimate {
    #[must_use]
    pub fn peak_bytes(&self) -> u128 {
        self.phases.iter().copied().max().unwrap_or(0)
    }
    #[must_use]
    pub const fn minimum_bytes(&self) -> u128 {
        self.minimum
    }
    #[must_use]
    pub const fn phase_bytes(&self, phase: Phase) -> u128 {
        self.phases[phase as usize]
    }
    /// The optional diagnostic's workspace field comes from this sole plan.
    #[must_use]
    pub const fn bias_workspace_bytes(&self) -> u128 {
        self.bias_workspace
    }
}
pub(super) fn estimate(request: Request<'_>) -> Result<Estimate> {
    validate(request)?;
    let input_bytes = input(request, 9)?;
    let prepared = prepared(request)?;
    let raw = matrix(request.inputs.active_rows, request.finite_columns)?;
    let base = sum(&[input_bytes, prepared, raw])?;
    let linear = linear_workspace(request)?;
    let fit = retained_fit(request)?;
    let outputs = outputs(request)?;
    let inference = inference(request)?;
    let bias_work = bias_workspace(request)?;
    let publication = array(request.publication_matrix_cells, 8)?;
    let minimum = sum(&[
        input(request, 8)?,
        array(request.inputs.original_rows, size_of::<DropReason>())?,
        outputs,
        publication,
    ])?;
    let preparation = sum(&[
        input_bytes,
        // A staged deletion keeps both sample generations until validation.
        mul(prepared, 2)?,
        matrix(request.inputs.original_rows, request.inputs.string_sources)?,
        array(request.inputs.original_rows, 32)?,
    ])?;
    let separation = if request.estimator == Estimator::Ppml {
        sum(&[
            base,
            prepared,
            fit,
            linear,
            array(request.simplex_cells, 8)?,
            array(request.inputs.active_rows, 128)?,
        ])?
    } else {
        0
    };
    let estimation = sum(&[base, fit, linear])?;
    let inference_phase = sum(&[base, fit, inference, outputs])?;
    let bias_phase = if request.bias.is_some() {
        sum(&[base, fit, inference, outputs, bias_work])?
    } else {
        0
    };
    let publication_phase = sum(&[base, fit, inference, outputs, publication])?;
    Ok(Estimate {
        phases: [
            preparation,
            separation,
            estimation,
            inference_phase,
            bias_phase,
            publication_phase,
        ],
        minimum,
        // The public diagnostic covers the full optional correction lifetime,
        // including its requested original-axis and native shadow products.
        bias_workspace: if request.bias.is_some() {
            sum(&[bias_work, bias_outputs(request)?, publication])?
        } else {
            0
        },
    })
}
fn validate(request: Request<'_>) -> Result<()> {
    if request.inputs.active_rows > request.inputs.original_rows
        || request.inputs.string_sources > request.inputs.sources
        || request.finite_columns > request.coefficient_slots
        || request.effects.iter().any(|e| e.channels == 0)
        || request.clusters
            > if request.estimator == Estimator::Ols {
                10
            } else {
                5
            }
        || (request.estimator == Estimator::Ols
            && (request.bias.is_some() || request.simplex_cells != 0))
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "inconsistent whole-call storage dimensions",
        ));
    }
    if let Some(bias) = request.bias {
        bias.method.validate()?;
        if request.effects.len() != 2 || request.effects.iter().any(|e| e.channels != 1) {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "bias storage requires two categorical intercept effects",
            ));
        }
    }
    Ok(())
}
fn input(request: Request<'_>, numeric_bytes: usize) -> Result<u128> {
    let inputs = request.inputs;
    let numeric = add_count(
        inputs.sources - inputs.string_sources,
        1 + usize::from(inputs.weighted) + usize::from(inputs.adjustment),
    )?;
    let mut result = sum(&[
        inputs.parent_bytes,
        mul(array(inputs.original_rows, numeric_bytes)?, numeric as u128)?,
        mul(
            array(inputs.original_rows, size_of::<String>())?,
            inputs.string_sources as u128,
        )?,
        array(inputs.original_rows, size_of::<bool>())?,
    ])?;
    if numeric_bytes == 9 {
        result = sum(&[
            result,
            inputs.string_bytes,
            inputs.metadata_bytes,
            array(inputs.sources, size_of::<crate::data::LoadedSource>())?,
        ])?;
    }
    Ok(result)
}
fn prepared(request: Request<'_>) -> Result<u128> {
    let inputs = request.inputs;
    let numeric = add_count(
        inputs.sources,
        1 + usize::from(inputs.weighted) + usize::from(inputs.adjustment),
    )?;
    let mut result = sum(&[
        matrix(inputs.active_rows, numeric)?,
        array(inputs.active_rows, size_of::<usize>())?,
        array(inputs.original_rows, size_of::<DropReason>())?,
        array(inputs.sources, size_of::<Vec<f64>>())?,
    ])?;
    for effect in request.effects {
        // Codes, membership, level/component maps, sorting/union-find copies,
        // and per-effect metadata. Shared partitions may make this an upper bound.
        result = sum(&[
            result,
            array(inputs.active_rows, 40)?,
            array(add_count(effect.levels, 1)?, 64)?,
            array(effect.channels, 128)?,
        ])?;
    }
    Ok(result)
}
fn linear_workspace(request: Request<'_>) -> Result<u128> {
    let n = request.inputs.active_rows;
    let k = request.finite_columns;
    let solver = solver::estimate(request, add_count(k, 1)?)?;
    sum(&[
        solver.retained,
        solver.scratch,
        // Raw RHS, projected RHS, finite QR/scaling, recovery and checks coexist.
        mul(matrix(n, add_count(k, 1)?)?, 8)?,
        mul(matrix(k, k)?, 16)?,
        array(k, 512)?,
    ])
}
fn retained_fit(request: Request<'_>) -> Result<u128> {
    let n = request.inputs.active_rows;
    sum(&[
        matrix(n, request.finite_columns)?,
        matrix(solver::parameters(request.effects)?, 1)?,
        mul(matrix(request.finite_columns, request.finite_columns)?, 2)?,
        array(request.coefficient_slots, 256)?,
        // Finite fitted/residual, within outcome, model residual and diagnostics;
        // PPML additionally retains eta, mean, score and accepted qmu.
        array(
            n,
            if request.estimator == Estimator::Ppml {
                160
            } else {
                64
            },
        )?,
    ])
}
fn inference(request: Request<'_>) -> Result<u128> {
    let n = request.inputs.active_rows;
    let p = request.coefficient_slots;
    let square = matrix(p, p)?;
    let blocks = n.div_ceil(1024);
    sum(&[
        // Score coordinates and clustered group scores; inclusion-exclusion
        // subsets run serially. Independent replay retains fixed-block matrices.
        mul(matrix(n, p)?, 2)?,
        mul(square, 20 + blocks as u128 * 2)?,
        mul(array(p, 8)?, blocks as u128)?,
        array(n, 64 + request.clusters * 32)?,
        array(blocks, 128)?,
        array(p, 512)?,
        array((1usize << request.clusters) - 1, size_of::<usize>())?,
        array(request.clusters, 128)?,
    ])
}
fn outputs(request: Request<'_>) -> Result<u128> {
    sum(&[
        output_banks(request, request.observation_outputs)?,
        bias_outputs(request)?,
    ])
}
fn output_banks(request: Request<'_>, count: usize) -> Result<u128> {
    sum(&[
        matrix(request.inputs.original_rows, count)?,
        array(count, size_of::<crate::outputs::Column>())?,
    ])
}
fn bias_outputs(request: Request<'_>) -> Result<u128> {
    output_banks(
        request,
        if request.bias.is_some_and(|b| b.save_effects) {
            18
        } else {
            0
        },
    )
}
fn bias_workspace(request: Request<'_>) -> Result<u128> {
    if request.bias.is_none() {
        return Ok(0);
    }
    let n = request.inputs.active_rows;
    let dimension = add_count(solver::parameters(request.effects)?, request.finite_columns)?;
    sum(&[
        linear_workspace(request)?,
        // Unit identities/codes/support scratch, eight information/contraction
        // row banks and bounded group moments; no probe history or FE square.
        array(n, 192)?,
        mul(matrix(dimension, 1)?, 16)?,
        matrix(request.coefficient_slots, 9)?,
        // A separate original-axis numeric unit source can coexist with input.
        matrix(request.inputs.original_rows, 1)?,
    ])
}
