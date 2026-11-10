//! The standard Golub-Kahan bidiagonalization (no preconditioner) of the
//! upstream engine, kept verbatim as a test reference for the modified
//! stream; see [`super::super::reference`].

use super::super::finite;
use super::super::magnitude::Magnitude;
use super::{
    adjoint_image, axpby, axpy_with_norm, normalize, par_dot, par_norm, residual_into, BidiagStep,
    Bidiagonalization, WindowRing,
};
use crate::lsmr::{Operator, SolveError};

/// Euclidean windowed MGS over the single stored `v` lane, used by [`GolubKahan`].
impl WindowRing<1> {
    /// MGS sweep over stored slots oldest-first, subtracting each projection of `y`.
    fn reorthogonalize(&self, y: &mut [f64]) {
        for slot in self.chrono_slots() {
            let v_j = self.lane(0, slot);
            let c = par_dot(y, v_j);
            axpby(y, v_j, -c, 1.0);
        }
    }

    /// Copy `v` into the next slot, advancing the ring.
    fn push(&mut self, v: &[f64]) {
        let slot = self.advance();
        self.lane_mut(0, slot).copy_from_slice(v);
    }
}

impl<A: Operator + ?Sized> Bidiagonalization for GolubKahan<'_, A> {
    fn step(&mut self) -> Result<BidiagStep, SolveError> {
        self.operator.apply(&self.bufs.v, &mut self.bufs.av)?;
        let beta = finite(
            axpy_with_norm(&mut self.bufs.u, &self.bufs.av, -self.alpha),
            "β",
        )?;
        if beta == 0.0 {
            // Lucky breakdown: zero `v` so `solution.update` contributes nothing.
            self.bufs.v.fill(0.0);
            self.alpha = 0.0;
            return Ok(BidiagStep { alpha: 0.0, beta });
        }
        normalize(&mut self.bufs.u, beta);

        self.operator
            .apply_adjoint(&self.bufs.u, &mut self.bufs.atu)?;
        let mut alpha = axpy_with_norm(&mut self.bufs.v, &self.bufs.atu, -beta);

        // MGS runs before normalization, so α must be re-derived from the corrected `v`.
        if let Some(reorth) = &self.bufs.local_reorth {
            reorth.reorthogonalize(&mut self.bufs.v);
            alpha = par_norm(&self.bufs.v);
        }
        let alpha = finite(alpha, "α")?;
        normalize(&mut self.bufs.v, alpha);

        if let Some(reorth) = &mut self.bufs.local_reorth {
            reorth.push(&self.bufs.v);
        }

        self.alpha = alpha;
        Ok(BidiagStep { alpha, beta })
    }

    fn v(&self) -> &[f64] {
        &self.bufs.v
    }

    fn residual_norm(&mut self, x: &[f64], rhs: &[f64]) -> Result<f64, SolveError> {
        residual_into(self.operator, x, rhs, &mut self.bufs.u)
    }

    fn plain_gradient(&mut self, residual_norm: f64) -> Result<Option<Magnitude>, SolveError> {
        let bufs = &mut self.bufs;
        let image = adjoint_image(
            self.operator,
            &bufs.u,
            residual_norm,
            &mut bufs.av,
            &mut bufs.atu,
        )?;
        Ok(Some(image.norm()))
    }

    fn into_residual(self) -> Vec<f64> {
        self.bufs.u
    }

    fn restart(&mut self, beta: f64) -> Result<BidiagStep, SolveError> {
        normalize(&mut self.bufs.u, beta);
        self.operator
            .apply_adjoint(&self.bufs.u, &mut self.bufs.v)?;
        let alpha = finite(par_norm(&self.bufs.v), "α")?;
        normalize(&mut self.bufs.v, alpha);
        if let Some(reorth) = &mut self.bufs.local_reorth {
            reorth.clear();
            reorth.push(&self.bufs.v);
        }
        self.alpha = alpha;
        Ok(BidiagStep { alpha, beta })
    }
}

/// Workspaces used by [`GolubKahan`].
struct GolubKahanBuffers {
    /// `u_k` in observation space (length m), kept normalized.
    u: Vec<f64>,
    /// `v_k` in DOF space (length n), kept normalized.
    v: Vec<f64>,
    /// Scratch for `A · v` (length m).
    av: Vec<f64>,
    /// Scratch for `Aᵀ · u` (length n).
    atu: Vec<f64>,
    /// Windowed reorthogonalization buffer; `None` disables it.
    local_reorth: Option<WindowRing<1>>,
}

impl GolubKahanBuffers {
    fn new(m: usize, n: usize, local_size: usize) -> Self {
        Self {
            u: vec![0.0; m],
            v: vec![0.0; n],
            av: vec![0.0; m],
            atu: vec![0.0; n],
            local_reorth: WindowRing::<1>::new(m, n, local_size),
        }
    }
}

/// Standard Golub-Kahan bidiagonalization: no preconditioner, two normalizations per step.
pub(in crate::lsmr) struct GolubKahan<'a, A: Operator + ?Sized> {
    operator: &'a A,
    bufs: GolubKahanBuffers,
    /// Last `α` emitted; needed by the next step's u-update.
    alpha: f64,
}

impl<'a, A: Operator + ?Sized> GolubKahan<'a, A> {
    /// Initialize the bidiagonalization, returning `Self` and the first step `(α₁, β₁)`.
    pub(in crate::lsmr) fn init(
        operator: &'a A,
        b: &[f64],
        b_norm: f64,
        local_size: usize,
    ) -> Result<(Self, BidiagStep), SolveError> {
        let mut bufs = GolubKahanBuffers::new(operator.nrows(), operator.ncols(), local_size);
        bufs.u.copy_from_slice(b);
        let mut stream = Self {
            operator,
            bufs,
            alpha: 0.0,
        };
        let step1 = stream.restart(b_norm)?;
        Ok((stream, step1))
    }
}
