//! Fixed-fit empirical coefficient mean correction. No PPML refits or VCE use.
use super::PpmlInterruptPoint;
use crate::error::{ErrorCode, FeregError, Result};
use crate::ols::CertifiedLinearSession;
use crate::prepared_fit_view::PreparedOlsFitView;
#[path = "bias_support.rs"]
mod support;
pub use support::PpmlBiasSupport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlBiasMethod {
    Exact,
    Randomized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlBiasRequest {
    pub method: PpmlBiasMethod,
    pub probes: usize,
    pub seed: u32,
    pub stream: u32,
    pub save_effects: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlBiasResult {
    pub request: PpmlBiasRequest,
    pub observations: usize,
    pub units: usize,
    pub support: [PpmlBiasSupport; 2],
    pub minimum_unit_rows: usize,
    pub maximum_unit_rows: usize,
    pub minimum_mean: f64,
    pub maximum_mean: f64,
    pub route: u8,
    pub certificate_tolerance: f64,
    pub minimum_level_information: f64,
    pub maximum_fe_leverage_lower_bound: f64,
    pub groups: usize,
    pub solves: usize,
    pub first_levels: usize,
    pub second_levels: usize,
    pub finite_columns: Vec<usize>,
    pub raw: Vec<f64>,
    pub derivative: Vec<f64>,
    pub curvature: Vec<f64>,
    pub variance_derivative: Vec<f64>,
    pub variance_curvature: Vec<f64>,
    pub covariance: Vec<f64>,
    pub maximum_score: f64,
    pub maximum_reconstruction: f64,
    pub elapsed_seconds: f64,
    pub workspace_bytes: u128,
    /// Nine channels per FE: raw, D, C, total, corrected, MCSE_D, MCSE_C,
    /// numerical covariance D/C, and total numerical MCSE.
    pub saved_effects: Vec<Vec<f64>>,
}

pub(super) fn memory_bound(
    rows: usize,
    levels: usize,
    columns: usize,
    saves: bool,
) -> Result<u128> {
    let n = rows as u128;
    let p = (levels as u128) + columns as u128;
    n.checked_mul(if saves { 400 } else { 240 })
        .and_then(|v| v.checked_add(p.min(256) * p.min(256) * 16))
        // Recovery reuses the admitted one-RHS projection workspace. Reserve
        // additional component anchors and retained coefficient products.
        .and_then(|v| p.checked_mul(240).and_then(|q| v.checked_add(q)))
        .and_then(|v| {
            (columns as u128)
                .checked_mul(columns as u128)
                .and_then(|k| k.checked_mul(8))
                .and_then(|k| v.checked_add(k))
        })
        .ok_or_else(|| error(ErrorCode::Allocation, "bias workspace size overflow"))
}

pub(super) struct AcceptedBiasFrame<'a> {
    pub units: &'a [f64],
    pub y: &'a [f64],
    pub mu: &'a [f64],
    pub eta: &'a [f64],
    pub offset: &'a [f64],
}

pub(super) fn compute<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    request: PpmlBiasRequest,
    frame: AcceptedBiasFrame<'_>,
    certificate_tolerance: f64,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PpmlBiasResult> {
    let started = std::time::Instant::now();
    interrupt(PpmlInterruptPoint::BiasCorrection)?;
    let support = support::check(
        session.fe_design(), session.bias_weights(), frame.units, interrupt,
    )?;
    let mut fixed = session.bias_information(certificate_tolerance)?;
    let dimension = session.bias_dimension();
    let first = session.fe_design().effects()[0].n_levels();
    let second = session.fe_design().effects()[1].n_levels();
    let mut raw = filled(dimension, 0.)?;
    session.bias_raw_coefficients(&mut fixed, frame.eta, frame.offset, &mut raw)
        .map_err(|e| bias_error_context(e, "raw", None, None, Some(fixed.route)))?;
    let mut result = calculate(
        request,
        frame.units,
        frame.y,
        frame.mu,
        dimension,
        |response, prediction, coefficients| {
            interrupt(PpmlInterruptPoint::BiasCorrection)?;
            session.bias_solve(&mut fixed, response, prediction, coefficients)
                .map_err(|mut e| {
                    e.message = format!("bias_route={}: {}", fixed.route, e.message);
                    e
                })
        },
    )?;
    result.raw = raw;
    result.support = support;
    result
        .finite_columns
        .try_reserve_exact(session.bias_finite_columns().len())
        .map_err(|_| error(ErrorCode::Allocation, "bias finite column allocation"))?;
    result
        .finite_columns
        .extend_from_slice(session.bias_finite_columns());
    result.first_levels = first;
    result.second_levels = second;
    result.route = fixed.route;
    result.certificate_tolerance = certificate_tolerance;
    result.minimum_level_information = fixed.minimum_level_information;
    result.maximum_fe_leverage_lower_bound = fixed.maximum_fe_leverage_lower_bound;
    result.maximum_score = fixed.maximum_score;
    result.maximum_reconstruction = fixed.maximum_reconstruction;
    result.workspace_bytes = memory_bound(
        frame.y.len(),
        first + second,
        dimension - first - second,
        request.save_effects,
    )?;
    if request.save_effects {
        result
            .saved_effects
            .try_reserve_exact(18)
            .map_err(|_| error(ErrorCode::Allocation, "bias saved output allocation"))?;
        for effect in 0..2 {
            let offset = if effect == 0 { 0 } else { first };
            for channel in 0..9 {
                let mut values = filled(frame.y.len(), 0.)?;
                for (row, &code) in session.fe_design().effects()[effect]
                    .codes()
                    .iter()
                    .enumerate()
                {
                    let p = offset + code as usize;
                    values[row] = result.value(p, channel)?;
                }
                result.saved_effects.push(values);
            }
        }
    }
    result.elapsed_seconds = started.elapsed().as_secs_f64();
    result.validate()?;
    Ok(result)
}

impl PpmlBiasResult {
    pub fn owned_buffer_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        bytes(&self.finite_columns)
            + [
                &self.raw,
                &self.derivative,
                &self.curvature,
                &self.variance_derivative,
                &self.variance_curvature,
                &self.covariance,
            ]
            .iter()
            .map(|v| bytes(v))
            .sum::<u128>()
            + bytes(&self.saved_effects)
            + self.saved_effects.iter().map(bytes).sum::<u128>()
    }

    pub fn value(&self, coordinate: usize, channel: usize) -> Result<f64> {
        if coordinate >= self.raw.len()
            || [
                &self.derivative,
                &self.curvature,
                &self.variance_derivative,
                &self.variance_curvature,
                &self.covariance,
            ]
            .iter()
            .any(|v| coordinate >= v.len())
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "bias coordinate is outside the result",
            ));
        }
        let d = self.derivative[coordinate];
        let c = self.curvature[coordinate];
        Ok(match channel {
            0 => self.raw[coordinate],
            1 => d,
            2 => c,
            3 => d + c,
            4 => self.raw[coordinate] + d + c,
            5 => self.variance_derivative[coordinate].sqrt(),
            6 => self.variance_curvature[coordinate].sqrt(),
            7 => self.covariance[coordinate],
            8 => {
                let variance = self.variance_derivative[coordinate]
                    + self.variance_curvature[coordinate]
                    + 2. * self.covariance[coordinate];
                if !variance.is_finite() {
                    return Err(error(
                        ErrorCode::NonConvergence,
                        "bias total numerical variance overflow",
                    ));
                }
                let scale =
                    self.variance_derivative[coordinate] + self.variance_curvature[coordinate];
                if variance < -64. * f64::EPSILON * scale {
                    return Err(error(
                        ErrorCode::Internal,
                        "bias numerical variance inconsistent",
                    ));
                }
                variance.max(0.).sqrt()
            }
            _ => return Err(error(ErrorCode::Internal, "unknown bias output channel")),
        })
    }

    pub(crate) fn validate(&self) -> Result<()> {
        self.support[0].validate(self.first_levels, self.units)?;
        self.support[1].validate(self.second_levels, self.units)?;
        let p = self.raw.len();
        let expected_groups = if self.request.method == PpmlBiasMethod::Exact {
            1
        } else {
            8.min(self.request.probes)
        };
        let expected_solves = if self.request.method == PpmlBiasMethod::Exact {
            self.units
        } else {
            self.request.probes
        }
        .checked_add(2 * expected_groups)
        .ok_or_else(|| error(ErrorCode::Internal, "bias solve count overflow"))?;
        if !self.minimum_mean.is_finite() || self.minimum_mean <= 0.
            || !self.maximum_mean.is_finite() || self.maximum_mean < self.minimum_mean
            || !self.elapsed_seconds.is_finite() || self.elapsed_seconds < 0.
            || self.observations == 0
            || self.units > self.observations
            || self.minimum_unit_rows == 0
            || self.maximum_unit_rows < self.minimum_unit_rows
            || self.maximum_unit_rows > self.observations
            || self.groups != expected_groups
            || self.solves != expected_solves
            || !matches!(self.route, 1 | 2)
            || self.first_levels == 0
            || self.second_levels == 0
            || self.first_levels + self.second_levels + self.finite_columns.len() != p
            || !self.certificate_tolerance.is_finite()
            || self.certificate_tolerance <= 0.
            || self.certificate_tolerance >= 1.
            || [self.maximum_score, self.maximum_reconstruction]
                .iter()
                .any(|v| !v.is_finite() || *v < 0. || *v > self.certificate_tolerance)
            || !self.minimum_level_information.is_finite()
            || self.minimum_level_information <= 0.
            || !self.maximum_fe_leverage_lower_bound.is_finite()
            || self.maximum_fe_leverage_lower_bound <= 0.
            || self.maximum_fe_leverage_lower_bound > 1. + 64. * f64::EPSILON
            || self.saved_effects.len() != if self.request.save_effects { 18 } else { 0 }
            || self
                .saved_effects
                .iter()
                .any(|v| v.len() != self.observations || v.iter().any(|x| !x.is_finite()))
        {
            return Err(error(
                ErrorCode::Internal,
                "bias result metadata or certificate is inconsistent",
            ));
        }
        if p == 0
            || self.units < 2
            || self.first_levels + self.second_levels > p
            || [
                &self.derivative,
                &self.curvature,
                &self.variance_derivative,
                &self.variance_curvature,
                &self.covariance,
            ]
            .iter()
            .any(|v| v.len() != p)
            || self
                .raw
                .iter()
                .chain(&self.derivative)
                .chain(&self.curvature)
                .chain(&self.covariance)
                .any(|x| !x.is_finite())
            || self
                .variance_derivative
                .iter()
                .chain(&self.variance_curvature)
                .any(|x| !x.is_finite() || *x < 0.)
        {
            return Err(error(
                ErrorCode::Internal,
                "bias result is not finite and aligned",
            ));
        }
        for j in 0..p {
            for channel in 0..9 {
                if !self.value(j, channel)?.is_finite() {
                    return Err(error(
                        ErrorCode::NonConvergence,
                        "bias corrected output is nonfinite",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn calculate(
    request: PpmlBiasRequest,
    units: &[f64],
    y: &[f64],
    mu: &[f64],
    dimension: usize,
    mut solve: impl FnMut(&[f64], &mut [f64], Option<&mut [f64]>) -> Result<()>,
) -> Result<PpmlBiasResult> {
    let n = y.len();
    if n == 0
        || units.len() != n
        || mu.len() != n
        || dimension == 0
        || units.iter().any(|x| !x.is_finite())
        || mu.iter().any(|x| !x.is_finite() || *x <= 0.)
    {
        return Err(error(
            ErrorCode::InvalidData,
            "bias inputs are missing or misaligned on retained rows",
        ));
    }
    let mut identities = filled(n, 0.)?;
    for (to, &from) in identities.iter_mut().zip(units) {
        *to = if from == 0. { 0. } else { from };
    }
    identities.sort_unstable_by(f64::total_cmp);
    identities.dedup();
    let g = identities.len();
    if g < 2 {
        return Err(error(
            ErrorCode::InsufficientObservations,
            "bias correction requires at least two independent units",
        ));
    }
    if request.method == PpmlBiasMethod::Exact
        && (g > 10_000 || (g as u128) * (n as u128) > 1_000_000_000)
    {
        return Err(error(
            ErrorCode::InvalidArgument,
            "exact bias correction exceeds its reference work bound; use randomized",
        ));
    }
    if request.method == PpmlBiasMethod::Randomized && request.probes < 2 {
        return Err(error(
            ErrorCode::InvalidArgument,
            "randomized bias correction requires at least two probes",
        ));
    }
    let mut codes = filled(n, 0_usize)?;
    for (code, &value) in codes.iter_mut().zip(units) {
        let value = if value == 0. { 0. } else { value };
        *code = identities
            .binary_search_by(|x| x.total_cmp(&value))
            .map_err(|_| error(ErrorCode::Internal, "bias unit identity absent"))?;
    }
    let mut unit_rows = filled(g, 0_usize)?;
    for &code in &codes {
        unit_rows[code] += 1;
    }
    let groups = if request.method == PpmlBiasMethod::Exact {
        1
    } else {
        8.min(request.probes)
    };
    let probes = if request.method == PpmlBiasMethod::Exact {
        g
    } else {
        request.probes
    };
    let mut response = filled(n, 0.)?;
    let mut prediction = filled(n, 0.)?;
    let mut fc = filled(n, 0.)?;
    let mut fd = filled(n, 0.)?;
    let mut signs = filled(g, 0.)?;
    let mut dc = filled(dimension, 0.)?;
    let mut cc = filled(dimension, 0.)?;
    let mut result = PpmlBiasResult {
        request,
        observations: n,
        units: g,
        support: [PpmlBiasSupport::default(); 2],
        minimum_unit_rows: *unit_rows.iter().min().unwrap(),
        maximum_unit_rows: *unit_rows.iter().max().unwrap(),
        minimum_mean: mu.iter().copied().fold(f64::INFINITY, f64::min),
        maximum_mean: mu.iter().copied().fold(0., f64::max),
        route: 1,
        certificate_tolerance: 0.,
        minimum_level_information: 0.,
        maximum_fe_leverage_lower_bound: 0.,
        groups,
        solves: 0,
        first_levels: 0,
        second_levels: 0,
        finite_columns: Vec::new(),
        raw: filled(dimension, 0.)?,
        derivative: filled(dimension, 0.)?,
        curvature: filled(dimension, 0.)?,
        variance_derivative: filled(dimension, 0.)?,
        variance_curvature: filled(dimension, 0.)?,
        covariance: filled(dimension, 0.)?,
        maximum_score: 0.,
        maximum_reconstruction: 0.,
        elapsed_seconds: 0.,
        workspace_bytes: 0,
        saved_effects: Vec::new(),
    };
    let mut mass = 0.;
    let mut first_probe = 0;
    for block in 0..groups {
        let count = probes / groups + usize::from(block < probes % groups);
        fc.fill(0.);
        fd.fill(0.);
        for probe in first_probe..first_probe + count {
            for (group, sign) in signs.iter_mut().enumerate() {
                *sign = if request.method == PpmlBiasMethod::Exact {
                    f64::from(group == probe)
                } else {
                    rademacher(
                        request.seed,
                        request.stream,
                        identities[group].to_bits(),
                        probe as u64,
                    )
                };
            }
            for row in 0..n {
                response[row] = signs[codes[row]] * ((y[row] - mu[row]) / mu[row]);
                if !response[row].is_finite() {
                    return Err(error(
                        ErrorCode::NonConvergence,
                        "bias score response is not finitely representable",
                    ));
                }
            }
            solve(&response, &mut prediction, None)
                .map_err(|e| bias_error_context(e, "probe", Some(block), Some(probe), None))?;
            result.solves += 1;
            for row in 0..n {
                fc[row] = (0.5 * prediction[row]).mul_add(prediction[row], fc[row]);
                fd[row] = signs[codes[row]].mul_add(prediction[row], fd[row]);
                if !fc[row].is_finite() || !fd[row].is_finite() {
                    return Err(error(
                        ErrorCode::NonConvergence,
                        "bias contraction overflow",
                    ));
                }
            }
        }
        let block_mass = if request.method == PpmlBiasMethod::Exact {
            1.
        } else {
            count as f64
        };
        if request.method == PpmlBiasMethod::Randomized {
            for x in fc.iter_mut().chain(&mut fd) {
                *x /= block_mass;
            }
        }
        solve(&fd, &mut prediction, Some(&mut dc))
            .map_err(|e| bias_error_context(e, "derivative", Some(block), None, None))?;
        solve(&fc, &mut prediction, Some(&mut cc))
            .map_err(|e| bias_error_context(e, "curvature", Some(block), None, None))?;
        result.solves += 2;
        let next = mass + block_mass;
        for j in 0..dimension {
            let dd = dc[j] - result.derivative[j];
            let cd = cc[j] - result.curvature[j];
            result.derivative[j] += block_mass / next * dd;
            result.curvature[j] += block_mass / next * cd;
            result.variance_derivative[j] += block_mass * mass / next * dd * dd;
            result.variance_curvature[j] += block_mass * mass / next * cd * cd;
            result.covariance[j] += block_mass * mass / next * dd * cd;
        }
        mass = next;
        first_probe += count;
    }
    if groups > 1 {
        for x in result
            .variance_derivative
            .iter_mut()
            .chain(&mut result.variance_curvature)
            .chain(&mut result.covariance)
        {
            *x /= mass * (groups - 1) as f64;
        }
    }
    Ok(result)
}

fn bias_error_context(
    mut error: FeregError,
    channel: &str,
    block: Option<usize>,
    probe: Option<usize>,
    route: Option<u8>,
) -> FeregError {
    // Errors travel outside the successful-result wire, so these diagnostics
    // survive transaction rollback without publishing partial estimates.
    let mut context = format!("bias_channel={channel}");
    if let Some(block) = block { context.push_str(&format!(" bias_block={block}")); }
    if let Some(probe) = probe { context.push_str(&format!(" bias_probe={probe}")); }
    if let Some(route) = route { context.push_str(&format!(" bias_route={route}")); }
    error.message = format!("{context}: {}", error.message);
    error
}

/// Counter is unit bits followed by probe index; the key is seed then stream.
fn rademacher(seed: u32, stream: u32, unit: u64, probe: u64) -> f64 {
    let output = philox(
        [
            unit as u32,
            (unit >> 32) as u32,
            probe as u32,
            (probe >> 32) as u32,
        ],
        [seed, stream],
    );
    if output[0] & 1 == 0 {
        -1.
    } else {
        1.
    }
}
fn philox(mut c: [u32; 4], mut k: [u32; 2]) -> [u32; 4] {
    for round in 0..10 {
        let p0 = 0xD2511F53_u64 * u64::from(c[0]);
        let p1 = 0xCD9E8D57_u64 * u64::from(c[2]);
        c = [
            (p1 >> 32) as u32 ^ c[1] ^ k[0],
            p1 as u32,
            (p0 >> 32) as u32 ^ c[3] ^ k[1],
            p0 as u32,
        ];
        if round != 9 {
            k[0] = k[0].wrapping_add(0x9E3779B9);
            k[1] = k[1].wrapping_add(0xBB67AE85);
        }
    }
    c
}
fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| error(ErrorCode::Allocation, "bias workspace allocation failed"))?;
    v.resize(n, value);
    Ok(v)
}
fn error(code: ErrorCode, message: &str) -> FeregError {
    FeregError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflowing_total_numerical_variance_is_not_reported_as_zero() {
        let mut result = scalar(PpmlBiasMethod::Randomized, 19, 8);
        result.variance_derivative[0] = 1e308;
        result.variance_curvature[0] = 1e308;
        result.covariance[0] = -1e308;
        assert!(result.value(0, 8).is_err());
    }

    fn scalar(method: PpmlBiasMethod, seed: u32, probes: usize) -> PpmlBiasResult {
        let mu = [0.4, 0.6, 1., 2.];
        calculate(
            PpmlBiasRequest {
                method,
                seed,
                probes,
                stream: 19,
                save_effects: false,
            },
            &[1., 2., 3., 4.],
            &[1.; 4],
            &mu,
            1,
            |rhs, prediction, coefficient| {
                let value = rhs.iter().zip(mu).map(|(x, w)| x * w).sum::<f64>() / 4.;
                prediction.fill(value);
                if let Some(c) = coefficient {
                    c[0] = value;
                }
                Ok(())
            },
        )
        .unwrap()
    }
    #[test]
    fn completed_duration_scalar() {
        let x = scalar(PpmlBiasMethod::Exact, 0, 0);
        assert!((x.derivative[0] + 0.095).abs() < 1e-14);
        assert!((x.curvature[0] - 0.0475).abs() < 1e-14);
        assert_eq!(x.solves, 6);
    }
    #[test]
    fn allocation_and_auxiliary_failure_return_no_correction() {
        assert_eq!(filled::<f64>(usize::MAX, 0.).unwrap_err().code, ErrorCode::Allocation);
        for failure in [ErrorCode::Allocation, ErrorCode::NonConvergence, ErrorCode::Interrupted] {
            let mut calls = 0;
            let result = calculate(
                PpmlBiasRequest { method: PpmlBiasMethod::Randomized, probes: 8,
                    seed: 17, stream: 1, save_effects: false },
                &[1., 2., 3., 4.], &[1.; 4], &[0.4, 0.6, 1., 2.], 1,
                |_, prediction, coefficients| {
                    calls += 1;
                    if calls == 4 { return Err(error(failure, "injected auxiliary failure")); }
                    prediction.fill(0.);
                    if let Some(c) = coefficients { c.fill(0.); }
                    Ok(())
                },
            );
            assert_eq!(result.unwrap_err().code, failure);
            assert_eq!(calls, 4);
        }
    }

    #[test]
    fn auxiliary_failure_identifies_probe_derivative_and_curvature_block() {
        for (stop, channel) in [(1, "probe"), (2, "derivative"), (3, "curvature")] {
            let mut calls = 0;
            let failure = calculate(
                PpmlBiasRequest { method: PpmlBiasMethod::Randomized, probes: 8,
                    seed: 17, stream: 1, save_effects: false },
                &[1., 2., 3., 4.], &[1.; 4], &[0.4, 0.6, 1., 2.], 1,
                |_, prediction, coefficients| {
                    calls += 1;
                    if calls == stop {
                        return Err(error(ErrorCode::NonConvergence, "recovery diagnostic"));
                    }
                    prediction.fill(0.);
                    if let Some(c) = coefficients { c.fill(0.); }
                    Ok(())
                },
            ).unwrap_err();
            assert_eq!(failure.code, ErrorCode::NonConvergence);
            assert!(failure.message.contains(&format!("bias_channel={channel}")), "{failure}");
            assert!(failure.message.contains("bias_block=0"), "{failure}");
        }
    }

    #[test]
    fn philox_known_answer() {
        assert_eq!(
            philox([0; 4], [0; 2]),
            [0x6627e8d5, 0xe169c58d, 0xbc57ac4c, 0x9b00dbd8]
        );
        assert_eq!(
            philox([u32::MAX; 4], [u32::MAX; 2]),
            [0x408f276d, 0x41c83b0e, 0xa20bc7c6, 0x6d5451fd]
        );
        assert_eq!(
            philox(
                [0x243f6a88, 0x85a308d3, 0x13198a2e, 0x03707344],
                [0xa4093822, 0x299f31d0]
            ),
            [0xd16cfe09, 0x94fdcceb, 0x5001e420, 0x24126ea1]
        );
    }
    #[test]
    fn independent_streams() {
        assert_ne!(
            (0..64)
                .map(|p| rademacher(17, 1, 42, p))
                .collect::<Vec<_>>(),
            (0..64)
                .map(|p| rademacher(17, 2, 42, p))
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn scalar_numerical_expectation_and_covariance() {
        let mut d = 0.;
        let mut c = 0.;
        let mut variance = 0.;
        let mut total_square = 0.;
        let reps = 512;
        for seed in 0..reps {
            let x = scalar(PpmlBiasMethod::Randomized, seed, 256);
            d += x.derivative[0];
            c += x.curvature[0];
            variance += x.value(0, 8).unwrap().powi(2);
            total_square += (x.derivative[0] + x.curvature[0] + 0.0475).powi(2);
        }
        assert!((d / f64::from(reps) + 0.095).abs() < 0.003);
        assert!((c / f64::from(reps) - 0.0475).abs() < 0.003);
        assert!((variance / total_square - 1.).abs() < 0.25);
    }
}
