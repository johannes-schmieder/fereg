//! Probe-count weighted group moments; no per-probe coefficient history.
use super::{filled, numerical, Channel};
use crate::error::Result;
#[derive(Debug, PartialEq)]
pub struct Coefficients {
    pub(super) raw: Vec<f64>,
    derivative: Vec<f64>,
    curvature: Vec<f64>,
    variance_derivative: Vec<f64>,
    variance_curvature: Vec<f64>,
    covariance: Vec<f64>,
    mass: f64,
    groups: usize,
}
impl Coefficients {
    pub(super) fn new(raw: &[f64]) -> Result<Self> {
        let mut original = filled(raw.len(), 0.)?;
        original.copy_from_slice(raw);
        Ok(Self {
            raw: original,
            derivative: filled(raw.len(), 0.)?,
            curvature: filled(raw.len(), 0.)?,
            variance_derivative: filled(raw.len(), 0.)?,
            variance_curvature: filled(raw.len(), 0.)?,
            covariance: filled(raw.len(), 0.)?,
            mass: 0.,
            groups: 0,
        })
    }
    pub(super) fn add(&mut self, mass: f64, d: &[f64], c: &[f64]) -> Result<()> {
        let next = self.mass + mass;
        for j in 0..self.raw.len() {
            let dd = d[j] - self.derivative[j];
            let cd = c[j] - self.curvature[j];
            self.derivative[j] += mass / next * dd;
            self.curvature[j] += mass / next * cd;
            self.variance_derivative[j] += mass * self.mass / next * dd * dd;
            self.variance_curvature[j] += mass * self.mass / next * cd * cd;
            self.covariance[j] += mass * self.mass / next * dd * cd;
        }
        self.mass = next;
        self.groups += 1;
        self.finite()
    }
    pub(super) fn finish(&mut self) -> Result<()> {
        if self.groups > 1 {
            let denominator = self.mass * (self.groups - 1) as f64;
            for x in self
                .variance_derivative
                .iter_mut()
                .chain(&mut self.variance_curvature)
                .chain(&mut self.covariance)
            {
                *x /= denominator;
            }
        }
        self.finite()?;
        for j in 0..self.raw.len() {
            for channel in Channel::ALL {
                self.value(j, channel)?;
            }
        }
        Ok(())
    }
    /// Linearity identifies the last group from the certified full mean and
    /// preceding groups. Only two final solves determine the point estimate;
    /// at most fourteen other final solves serve numerical variation.
    pub(super) fn add_point(
        &mut self,
        mass: f64,
        d: &[f64],
        c: &[f64],
        last_d: &mut [f64],
        last_c: &mut [f64],
    ) -> Result<()> {
        let total = self.mass + mass;
        let factor = self.mass * total / mass;
        for j in 0..self.raw.len() {
            let dd = d[j] - self.derivative[j];
            let cd = c[j] - self.curvature[j];
            self.variance_derivative[j] += factor * dd * dd;
            self.variance_curvature[j] += factor * cd * cd;
            self.covariance[j] += factor * dd * cd;
            last_d[j] = (total / mass).mul_add(dd, self.derivative[j]);
            last_c[j] = (total / mass).mul_add(cd, self.curvature[j]);
        }
        self.derivative.copy_from_slice(d);
        self.curvature.copy_from_slice(c);
        self.mass = total;
        self.groups += 1;
        self.finite()
    }
    fn finite(&self) -> Result<()> {
        if self
            .derivative
            .iter()
            .chain(&self.curvature)
            .chain(&self.variance_derivative)
            .chain(&self.variance_curvature)
            .chain(&self.covariance)
            .any(|v| !v.is_finite())
        {
            return Err(numerical("bias group moments are not representable"));
        }
        Ok(())
    }
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.raw.len()
    }
    pub fn value(&self, coordinate: usize, channel: Channel) -> Result<f64> {
        if coordinate >= self.raw.len() {
            return Err(super::invalid("bias coordinate is out of range"));
        }
        let d = self.derivative[coordinate];
        let c = self.curvature[coordinate];
        let value = match channel {
            Channel::Raw => self.raw[coordinate],
            Channel::Derivative => d,
            Channel::Curvature => c,
            Channel::Total => d + c,
            Channel::Corrected => self.raw[coordinate] + d + c,
            Channel::SeDerivative => self.variance_derivative[coordinate].sqrt(),
            Channel::SeCurvature => self.variance_curvature[coordinate].sqrt(),
            Channel::Covariance => self.covariance[coordinate],
            Channel::SeTotal => {
                let scale =
                    self.variance_derivative[coordinate] + self.variance_curvature[coordinate];
                let variance = scale + 2. * self.covariance[coordinate];
                if !variance.is_finite()
                    || !scale.is_finite()
                    || variance < -64. * f64::EPSILON * scale
                {
                    return Err(numerical(
                        "bias total numerical variance is inconsistent or unrepresentable",
                    ));
                }
                variance.max(0.).sqrt()
            }
        };
        if !value.is_finite() {
            return Err(numerical("bias output channel is not representable"));
        }
        Ok(value)
    }
}
