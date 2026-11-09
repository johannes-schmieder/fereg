//! The full system's preconditioner as a reusable handle
//! ([`FePreconditioner`]) for a caller's own Krylov method on a larger
//! system whose FE block is `W^1/2 D`.
//!
//! The handle is the preconditioner the full formulation's methods use
//! ([`super`]'s contract): Jacobi (the level masses; with slopes, the
//! levels' channel blocks) or additive Schwarz over factor-pair subdomains,
//! built for one weight state and fixed afterwards. It owns everything its
//! action reads, so it outlives the call that built it and the projector's
//! later weight states; the structure stays with the projector.
//!
//! **Scale and layout.** fedemean builds on weights divided by their
//! maximum; the handle multiplies its action by `1 / max w`, so it
//! approximates `(D'WD)^-1` for the caller's raw weights. It acts on the
//! canonical coefficient layout (per effect, channel-major) in the raw
//! basis. With offset slopes it converts to the operator's centered basis
//! and back: `M^-1 = T^-1 M_op^-1 T^-T` for the reparametrization
//! `a_op = a + o b`, which keeps symmetry and positive definiteness and
//! the spectrum of the preconditioned normal matrix.

use super::blocks::{Blocks, LEVEL_TASK};
use super::schwarz::{Refusal, Schwarz, Scratch};
use super::structure::Structure;
use super::weights::State;
use super::{filled, state_bytes, Engine, Method};
use crate::design::Design;
use crate::elimination::Cancel;
use crate::error::{Error, Result};
use crate::projector::{Decline, Preconditioner, PreconditionerUsed, SchwarzLocal, SchwarzSpec};
use crate::weights::{validate, Weights};
use std::sync::{Mutex, PoisonError};

/// One effect's coefficients in the canonical layout and the internal one.
#[derive(Clone, Copy)]
struct Span {
    canonical: usize,
    internal: usize,
    levels: usize,
    channels: usize,
}

/// The apply workspace, retained across applies.
#[derive(Default)]
struct Work {
    schwarz: Scratch,
    /// Measure partials of the channel blocks (designs with slopes).
    partials: Vec<f64>,
    /// The internal layout's input and output (designs with slopes).
    input: Vec<f64>,
    output: Vec<f64>,
}

/// A fixed symmetric positive definite approximation `M^-1 ~ (D'WD)^-1`
/// of one design and weight state, applied to vectors of FE coefficients
/// in the canonical layout ([`crate::CoefficientLayout`], one column): the
/// FE block of a caller's preconditioner, such as a block-diagonal
/// preconditioner of `W^1/2 [X D]` for modified LSMR. Built by
/// [`crate::Projector::fe_preconditioner`].
///
/// The action is nonsingular and symmetric positive definite on all of
/// coefficient space although `D'WD` is singular (modified LSMR's
/// requirement; see [`crate::Preconditioner`]), deterministic and
/// independent of the worker count. It is fixed: a caller rebuilds it for
/// new weights (between recurrences), or keeps an earlier one, which is
/// still a valid preconditioner of the current weights' system.
pub struct FePreconditioner {
    used: PreconditionerUsed,
    /// `1 / max w`.
    scale: f64,
    /// Inverse masses (zero on the effects with channel blocks).
    inverse_mass: Vec<f64>,
    schwarz: Option<Schwarz>,
    /// The channel blocks and the operator's basis (designs with slopes).
    blocks: Option<Blocks>,
    offsets: Vec<usize>,
    /// Per effect (designs with slopes): the layouts' conversion.
    spans: Vec<Span>,
    work: Mutex<Work>,
    retained: u128,
    workspace: u128,
}

impl std::fmt::Debug for FePreconditioner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FePreconditioner")
            .field("used", &self.used)
            .field("coefficients", &self.inverse_mass.len())
            .field("retained_bytes", &self.retained)
            .field("workspace_bytes", &self.workspace)
            .finish_non_exhaustive()
    }
}

/// Jacobi's report: the inverse masses, or the channel blocks of a design
/// with slopes.
fn jacobi(state: &State) -> PreconditionerUsed {
    match state.blocks() {
        Some(blocks) => PreconditionerUsed::ChannelBlocks {
            singular: u32::try_from(blocks.singular()).unwrap_or(u32::MAX),
        },
        None => PreconditionerUsed::Jacobi,
    }
}

/// The Schwarz specification `choice` resolves to (`None`: Jacobi) and
/// whether a refusal of it is the call's (an explicit choice).
fn resolve(design: &Design<'_>, choice: Preconditioner) -> Result<(Option<SchwarzSpec>, bool)> {
    let effects = design.effects().len();
    let schwarz = cfg!(feature = "schwarz");
    Ok(match choice {
        Preconditioner::Jacobi => (None, true),
        Preconditioner::Auto if schwarz && effects >= 2 => (Some(SchwarzSpec::default()), false),
        Preconditioner::Auto => (None, false),
        Preconditioner::Schwarz(_) if !schwarz => {
            return Err(Error::Unsupported("additive Schwarz needs feature schwarz"))
        }
        Preconditioner::Schwarz(spec)
            if design.sloped() && effects >= 2 && spec.local == SchwarzLocal::Exact =>
        {
            return Err(Error::Unsupported(
                "exact Schwarz local solves need intercept effects",
            ))
        }
        Preconditioner::Schwarz(spec) => (Some(spec), true),
        _ => {
            return Err(Error::Unsupported(
                "the FE preconditioner is Jacobi, Schwarz or automatic",
            ))
        }
    })
}

/// The engine's structure, built once; `None` when the design is outside
/// the full formulation (remembered).
fn structure<'e>(
    engine: &'e mut Engine,
    design: &Design<'_>,
    cancel: &Cancel<'_>,
) -> Result<Option<&'e Structure>> {
    if engine.refused {
        return Ok(None);
    }
    if engine.structure.is_none() {
        match Structure::build(design, cancel)? {
            Some(st) => engine.structure = Some(st),
            None => {
                engine.refused = true;
                return Ok(None);
            }
        }
    }
    Ok(engine.structure.as_ref())
}

/// Bytes beside the Schwarz build: the structure and the weight state (the
/// inverse masses and channel blocks are kept from it), the fill's scatter
/// accumulators, the offsets and, with slopes, the conversions' columns,
/// the spans and the measure partials.
fn base(st: &Structure, design: &Design<'_>, weighted: bool) -> u128 {
    let effects = design.effects();
    let tasks: usize = effects
        .iter()
        .filter(|e| e.sloped())
        .map(|e| e.levels().div_ceil(LEVEL_TASK))
        .sum();
    let scatter = (st.scatter_blocks() * st.scatter_levels()) as u128;
    let conversion = if design.sloped() {
        8 * (2 * st.coefficients() + 3 * tasks) as u128 + 32 * effects.len() as u128
    } else {
        0
    };
    st.bytes
        + state_bytes(st, Method::Pcg, weighted)
        + 8 * (scatter + st.offsets.len() as u128)
        + conversion
}

impl FePreconditioner {
    /// An upper bound of [`FePreconditioner::workspace_bytes`] of a build of
    /// `choice` for any positive weights (see
    /// [`crate::Projector::fe_preconditioner_bytes`]).
    pub(crate) fn estimate(
        engine: &mut Engine,
        design: &Design<'_>,
        choice: Preconditioner,
        memory_limit: Option<u128>,
        cancel: &Cancel<'_>,
    ) -> Result<std::result::Result<u128, Decline>> {
        let (spec, explicit) = resolve(design, choice)?;
        let Some(st) = structure(engine, design, cancel)? else {
            return Ok(Err(Decline::OutsideRoute));
        };
        let base = base(st, design, true);
        if memory_limit.is_some_and(|limit| base > limit) {
            return Ok(Err(Decline::MemoryLimit));
        }
        let Some(spec) = spec else {
            return Ok(Ok(base));
        };
        // Unit weights: the build's charges depend on the design only.
        let mut state = State::default();
        let mut accumulators = Vec::new();
        let unit = validate(Weights::Unit, design.nobs())?;
        if !state.fill(st, design.effects(), unit, false, &mut accumulators)? {
            return Ok(Err(Decline::Weights));
        }
        drop(accumulators);
        let budget = memory_limit.map(|limit| limit - base);
        Ok(
            match Schwarz::estimate(st, design.effects(), &state, spec, (budget, 1), cancel)? {
                Ok(peak) => Ok(base + peak),
                // Automatic: Jacobi when Schwarz does not fit.
                Err(_) if !explicit => Ok(base),
                Err(Refusal::MemoryLimit) => Err(Decline::MemoryLimit),
                Err(Refusal::Numerical(reason)) => Err(Decline::Numerical(reason)),
            },
        )
    }

    /// Build the handle of `choice` for `weights` within `memory_limit`
    /// (see [`crate::Projector::fe_preconditioner`]); the structure is built
    /// once and kept by `engine`.
    pub(crate) fn build(
        engine: &mut Engine,
        design: &Design<'_>,
        (weights, choice): (Weights<'_>, Preconditioner),
        memory_limit: Option<u128>,
        cancel: &Cancel<'_>,
    ) -> Result<std::result::Result<Self, Decline>> {
        let effects = design.effects();
        let (spec, explicit) = resolve(design, choice)?;
        let Some(st) = structure(engine, design, cancel)? else {
            return Ok(Err(Decline::OutsideRoute));
        };
        let validated = validate(weights, design.nobs())?;
        let m = st.coefficients();
        let base = base(st, design, validated.values.is_some());
        if memory_limit.is_some_and(|limit| base > limit) {
            return Ok(Err(Decline::MemoryLimit));
        }
        let mut state = State::default();
        let mut accumulators = Vec::new();
        if !state.fill(st, effects, validated, false, &mut accumulators)? {
            return Ok(Err(Decline::Weights));
        }
        drop(accumulators);
        let scale = 1.0 / state.raw_scale;
        if !scale.is_finite() {
            return Err(Error::InvalidData(
                "the largest weight is too small to scale the FE preconditioner",
            ));
        }
        let mut built = None;
        if let Some(spec) = spec {
            let budget = memory_limit.map(|limit| limit - base);
            match Schwarz::build(st, effects, &state, (spec, 0), (budget, 1), cancel)? {
                Ok(schwarz) => built = Some(schwarz),
                Err(refusal) if explicit => {
                    return Ok(Err(match refusal {
                        Refusal::MemoryLimit => Decline::MemoryLimit,
                        Refusal::Numerical(reason) => Decline::Numerical(reason),
                    }))
                }
                // Automatic: Jacobi when Schwarz does not build.
                Err(_) => {}
            }
        }
        let used = built.as_ref().map_or_else(|| jacobi(&state), Schwarz::used);
        let mut work = Work::default();
        if let Some(schwarz) = &built {
            schwarz.size(&mut work.schwarz, 1)?;
        }
        let blocks = state
            .blocks()
            .is_some()
            .then(|| std::mem::take(&mut state.blocks));
        let mut spans = Vec::new();
        if let Some(blocks) = &blocks {
            work.partials = filled(3 * blocks.tasks(), 0.0)?;
            work.input = filled(m, 0.0)?;
            work.output = filled(m, 0.0)?;
            spans = super::with_capacity(effects.len())?;
            let mut canonical = 0;
            for (e, effect) in effects.iter().enumerate() {
                let (levels, channels) = (effect.levels(), effect.channels());
                spans.push(Span {
                    canonical,
                    internal: st.offsets[e],
                    levels,
                    channels,
                });
                canonical += levels * channels;
            }
        }
        let mut offsets = super::with_capacity(st.offsets.len())?;
        offsets.extend_from_slice(&st.offsets);
        let inverse_mass = std::mem::take(&mut state.inverse_mass);
        drop(state);
        let retained = 8 * (inverse_mass.capacity() + offsets.capacity()) as u128
            + blocks.as_ref().map_or(0, Blocks::bytes)
            + built.as_ref().map_or(0, |s| s.bytes() + s.scratch_bytes(1))
            + 8 * (work.partials.capacity() + work.input.capacity() + work.output.capacity())
                as u128
            + 32 * spans.capacity() as u128;
        let workspace = base + built.as_ref().map_or(0, |s| s.peak_bytes(1));
        Ok(Ok(Self {
            used,
            scale,
            inverse_mass,
            schwarz: built,
            blocks,
            offsets,
            spans,
            work: Mutex::new(work),
            retained,
            workspace,
        }))
    }

    /// The coefficient count `L` of the design (levels times channels,
    /// summed over the effects): the length of the vectors it applies to.
    #[must_use]
    pub fn coefficients(&self) -> usize {
        self.inverse_mass.len()
    }

    /// What it applies: [`PreconditionerUsed::Jacobi`],
    /// [`PreconditionerUsed::ChannelBlocks`] (Jacobi with slopes) or
    /// [`PreconditionerUsed::Schwarz`] with its subdomains and local solves.
    #[must_use]
    pub const fn used(&self) -> PreconditionerUsed {
        self.used
    }

    /// Bytes it retains, its apply workspace included.
    #[must_use]
    pub const fn retained_bytes(&self) -> u128 {
        self.retained
    }

    /// The peak bytes of its build beyond the caller's data: the
    /// projector's structure, the transient weight state and, with Schwarz,
    /// the build's peak (its transients included) or its retained storage
    /// with the apply workspace, whichever is larger. A build is admitted
    /// at exactly these bytes ([`crate::Projector::fe_preconditioner`]).
    #[must_use]
    pub const fn workspace_bytes(&self) -> u128 {
        self.workspace
    }

    /// `out = M^-1 rhs` for one vector of [`FePreconditioner::coefficients`]
    /// values in the canonical layout (an FE block's moments in, its
    /// coefficients' step out). Repeated applies allocate nothing.
    pub fn apply(&self, rhs: &[f64], out: &mut [f64]) -> Result<()> {
        let m = self.inverse_mass.len();
        if rhs.len() != m || out.len() != m {
            return Err(Error::InvalidInput(
                "FE preconditioner vectors do not match the design's coefficients",
            ));
        }
        let mut work = self.work.lock().unwrap_or_else(PoisonError::into_inner);
        let Work {
            schwarz,
            partials,
            input,
            output,
        } = &mut *work;
        if let Some(blocks) = &self.blocks {
            self.moments_in(blocks, rhs, input);
            self.internal(input, output, (schwarz, partials));
            self.coefficients_out(blocks, output, out);
        } else {
            self.internal(rhs, out, (schwarz, partials));
        }
        for value in out.iter_mut() {
            *value *= self.scale;
        }
        Ok(())
    }

    /// The action on the internal layout and the operator's basis, as the
    /// full formulation's modified LSMR applies it.
    fn internal(&self, r: &[f64], z: &mut [f64], (scratch, partials): (&mut Scratch, &mut [f64])) {
        let blocks = self.blocks.as_ref();
        if let Some(schwarz) = &self.schwarz {
            schwarz.apply::<1>(r, z, scratch, (&self.inverse_mass, blocks));
            return;
        }
        for ((z, r), d) in z.iter_mut().zip(r).zip(&self.inverse_mass) {
            *z = r * d;
        }
        if let Some(blocks) = blocks {
            blocks.measure::<1>(&self.offsets, r, Some(z), partials);
        }
    }

    /// Canonical raw-basis moments to the internal layout and the
    /// operator's basis: a slope's moment less its origin times the
    /// level's intercept moment (`T^-T`).
    fn moments_in(&self, blocks: &Blocks, rhs: &[f64], x: &mut [f64]) {
        let basis = blocks.basis();
        for (e, span) in self.spans.iter().enumerate() {
            let (levels, c) = (span.levels, span.channels);
            for l in 0..levels {
                let g = span.internal + l * c;
                for ch in 0..c {
                    x[g + ch] = rhs[span.canonical + ch * levels + l];
                }
                for (s, &origin) in basis.level(e, l).iter().enumerate() {
                    if origin != 0.0 {
                        x[g + 1 + s] -= origin * x[g];
                    }
                }
            }
        }
    }

    /// Internal coefficients in the operator's basis to the canonical
    /// layout in the raw basis: an intercept less its slopes' origins times
    /// their coefficients (`T^-1`).
    fn coefficients_out(&self, blocks: &Blocks, x: &[f64], out: &mut [f64]) {
        let basis = blocks.basis();
        for (e, span) in self.spans.iter().enumerate() {
            let (levels, c) = (span.levels, span.channels);
            for l in 0..levels {
                let g = span.internal + l * c;
                let mut intercept = x[g];
                for (s, &origin) in basis.level(e, l).iter().enumerate() {
                    if origin != 0.0 {
                        intercept -= origin * x[g + 1 + s];
                    }
                }
                out[span.canonical + l] = intercept;
                for ch in 1..c {
                    out[span.canonical + ch * levels + l] = x[g + ch];
                }
            }
        }
    }
}
