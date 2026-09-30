//! Both full-system contractions, with bounded observation/coefficient banks.
use super::{context, filled, numerical, philox, Coefficients, Information, Method, Support};
use crate::{error::Result, ppml::irls};
pub(super) struct Calculated {
    pub coefficients: Coefficients,
    pub probes: usize,
    pub groups: usize,
}
pub(super) fn calculate(
    fit: &irls::Fit<'_>,
    support: &Support,
    method: Method,
    info: &mut Information<'_, '_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Calculated> {
    let n = fit.problem().sample.n_active();
    let (probes, groups) = method.dimensions(n, support.identities().len())?;
    let mut result = Coefficients::new(info.raw())?;
    let mut response = filled(n, 0.)?;
    let mut prediction = filled(n, 0.)?;
    let mut fd = filled(n, 0.)?;
    let mut fc = filled(n, 0.)?;
    let mut point_d = filled(n, 0.)?;
    let mut point_c = filled(n, 0.)?;
    let mut signs = filled(support.identities().len(), 0.)?;
    let mut d = filled(info.dimension(), 0.)?;
    let mut c = filled(info.dimension(), 0.)?;
    let mut last_d = filled(info.dimension(), 0.)?;
    let mut last_c = filled(info.dimension(), 0.)?;
    let mut first = 0;
    for block in 0..groups {
        let count = probes / groups + usize::from(block < probes % groups);
        fd.fill(0.);
        fc.fill(0.);
        for probe in first..first + count {
            interrupt().map_err(|e| context(e, "probe", Some(block), Some(probe)))?;
            for (unit, sign) in signs.iter_mut().enumerate() {
                *sign = match method {
                    Method::Exact => f64::from(unit == probe),
                    Method::Randomized { seed, stream, .. } => {
                        philox::sign(seed, stream, support.identities()[unit], probe as u32)?
                    }
                };
            }
            for row in 0..n {
                response[row] = signs[support.codes()[row]]
                    * ((fit.problem().sample.outcome[row] - fit.evaluation.mean[row])
                        / fit.evaluation.mean[row]);
                if !response[row].is_finite() {
                    return Err(context(
                        numerical("bias score response is not representable"),
                        "probe",
                        Some(block),
                        Some(probe),
                    ));
                }
            }
            info.solve(&response, &mut prediction, None, interrupt)
                .map_err(|e| context(e, "probe", Some(block), Some(probe)))?;
            for row in 0..n {
                fd[row] = signs[support.codes()[row]].mul_add(prediction[row], fd[row]);
                fc[row] = (0.5 * prediction[row]).mul_add(prediction[row], fc[row]);
                if !fd[row].is_finite() {
                    return Err(context(
                        numerical(format!("bias contraction overflow at row {row}")),
                        "derivative",
                        Some(block),
                        Some(probe),
                    ));
                }
                if !fc[row].is_finite() {
                    return Err(context(
                        numerical(format!("bias contraction overflow at row {row}")),
                        "curvature",
                        Some(block),
                        Some(probe),
                    ));
                }
            }
        }
        let mass = if method == Method::Exact {
            1.
        } else {
            count as f64
        };
        let point = block + 1 == groups;
        if method != Method::Exact {
            for row in 0..n {
                point_d[row] += fd[row];
                point_c[row] += fc[row];
                if !point_d[row].is_finite() || !point_c[row].is_finite() {
                    return Err(context(
                        numerical("bias full-mean contraction overflow"),
                        "point",
                        Some(block),
                        None,
                    ));
                }
                let group_d = fd[row] / mass;
                let group_c = fc[row] / mass;
                fd[row] = if point {
                    point_d[row] / probes as f64
                } else {
                    group_d
                };
                fc[row] = if point {
                    point_c[row] / probes as f64
                } else {
                    group_c
                };
                if point {
                    point_d[row] = group_d;
                    point_c[row] = group_c;
                }
            }
        }
        info.solve(&fd, &mut prediction, Some(&mut d), interrupt)
            .map_err(|e| context(e, "derivative", Some(block), None))?;
        info.solve(&fc, &mut prediction, Some(&mut c), interrupt)
            .map_err(|e| context(e, "curvature", Some(block), None))?;
        if point && method != Method::Exact {
            result
                .add_point(mass, &d, &c, &mut last_d, &mut last_c)
                .map_err(|e| context(e, "uncertainty", Some(block), None))?;
            info.certify_combination(&point_d, &last_d, interrupt)
                .map_err(|e| context(e, "uncertainty_derivative", Some(block), None))?;
            info.certify_combination(&point_c, &last_c, interrupt)
                .map_err(|e| context(e, "uncertainty_curvature", Some(block), None))?;
        } else {
            result
                .add(mass, &d, &c)
                .map_err(|e| context(e, "uncertainty", Some(block), None))?;
        }
        first += count;
    }
    result
        .finish()
        .map_err(|e| context(e, "uncertainty", None, None))?;
    Ok(Calculated {
        coefficients: result,
        probes,
        groups,
    })
}
