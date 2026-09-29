//! Optional correction request and publication adapter. All estimation is core-owned.
use super::*;
use fereg_core::ppml::{PpmlBiasMethod, PpmlBiasRequest};

pub(super) const CHANNELS: usize = 9;
pub(super) const DIAGNOSTICS: usize = 21;
pub(super) const SUPPORT_COLUMNS: usize = 11;

pub(super) fn parse(values: &BTreeMap<String, String>) -> Result<Option<PpmlBiasRequest>> {
    let method = match values
        .get("bias_method")
        .map(String::as_str)
        .unwrap_or("none")
    {
        "none" => {
            if values.keys().any(|k| {
                k.starts_with("bias_") && k != "bias_method" || k.starts_with("output_bias_")
            }) {
                return Err(syntax("bias options require an explicit correction method"));
            }
            return Ok(None);
        }
        "exact" => PpmlBiasMethod::Exact,
        "randomized" => PpmlBiasMethod::Randomized,
        _ => return Err(syntax("bias method must be exact or randomized")),
    };
    let probes = parse_usize(values, "bias_probes")?;
    let seed = parse_nonnegative_u32(values, "bias_seed")?;
    let stream = parse_nonnegative_u32(values, "bias_stream")?;
    if method == PpmlBiasMethod::Exact && (probes != 0 || seed != 0 || stream != 0)
        || method == PpmlBiasMethod::Randomized && probes < 2
    {
        return Err(syntax("bias method and probe controls disagree"));
    }
    let count = parse_usize(values, "output_bias_count")?;
    if !matches!(count, 0 | 18) {
        return Err(syntax("bias saved outputs require all eighteen channels"));
    }
    Ok(Some(PpmlBiasRequest {
        method,
        probes,
        seed,
        stream,
        save_effects: count == 18,
    }))
}

#[derive(Debug)]
pub(crate) struct Shadow<'a> {
    pub(crate) request: PpmlBiasRequest,
    pub(crate) coefficients: Vec<f64>,
    pub(crate) diagnostics: Vec<f64>,
    pub(crate) support: [f64; 2 * SUPPORT_COLUMNS],
    pub(crate) saved: &'a [Vec<f64>],
}

impl Shadow<'_> {
    pub(crate) fn capacity_bytes(&self) -> u128 {
        ((self.coefficients.capacity() + self.diagnostics.capacity()) * std::mem::size_of::<f64>())
            as u128
    }
}

pub(crate) fn shadow(result: &fereg_core::ppml::CertifiedPpmlResult) -> Result<Option<Shadow<'_>>> {
    let Some(bias) = result.bias() else {
        return Ok(None);
    };
    let k = result.coefficients().values().len();
    let mut coefficients = reserved_filled(
        k.checked_mul(CHANNELS)
            .ok_or_else(|| allocation("bias publication dimensions overflow"))?,
        0.,
        "bias coefficient shadow",
    )?;
    for (slot, generated) in result.coefficients().generated_columns().iter().enumerate() {
        if let Some(index) =
            generated.and_then(|column| bias.finite_columns.iter().position(|c| *c == column))
        {
            for channel in 0..CHANNELS {
                coefficients[channel * k + slot] =
                    bias.value(bias.first_levels + bias.second_levels + index, channel)?;
            }
        }
    }
    let mut diagnostics = reserved_filled(DIAGNOSTICS, 0., "bias diagnostics shadow")?;
    diagnostics.copy_from_slice(&[
        if bias.request.method == PpmlBiasMethod::Exact {
            1.
        } else {
            2.
        },
        bias.units as f64,
        bias.request.probes as f64,
        f64::from(bias.request.seed),
        f64::from(bias.request.stream),
        bias.groups as f64,
        bias.solves as f64,
        bias.maximum_score,
        bias.maximum_reconstruction,
        bias.elapsed_seconds,
        bias.workspace_bytes as f64,
        bias.derivative
            .iter()
            .zip(&bias.curvature)
            .map(|(d, c)| (d + c).abs())
            .fold(0., f64::max),
        bias.minimum_mean,
        bias.maximum_mean,
        bias.minimum_unit_rows as f64,
        bias.maximum_unit_rows as f64,
        f64::from(bias.route), // 1: reused sparse operator; 2: bounded cached information factor.
        1.,                    // Numerical checks and the necessary support screen passed.
        bias.certificate_tolerance,
        bias.minimum_level_information,
        bias.maximum_fe_leverage_lower_bound,
    ]);
    let mut support = [0.; 2 * SUPPORT_COLUMNS];
    for (row, summary) in support.chunks_exact_mut(SUPPORT_COLUMNS).zip(&bias.support) {
        row.copy_from_slice(&summary.values());
    }
    if coefficients
        .iter()
        .chain(&diagnostics)
        .chain(&support)
        .chain(bias.saved_effects.iter().flatten())
        .any(|v| !crate::ppml_result::is_stata_publishable(*v))
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "bias result is not representable in Stata",
        ));
    }
    Ok(Some(Shadow {
        request: bias.request,
        coefficients,
        diagnostics,
        support,
        saved: &bias.saved_effects,
    }))
}

pub(super) fn validate(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    match (&action.bias, &shadow.bias) {
        (None, None) => {
            if !action.outputs.bias_channels.is_empty()
                || action.matrices.bias.is_some()
                || action.matrices.bias_diagnostics.is_some()
                || action.matrices.bias_support.is_some()
            {
                return Err(internal("option-off PPML has correction outputs"));
            }
        }
        (Some(request), Some(result)) => {
            let k = shadow.coefficient_count;
            if *request != result.request
                || result.coefficients.len() != k * CHANNELS
                || result.diagnostics.len() != DIAGNOSTICS
                || result.saved.len() != action.outputs.bias_channels.len()
                || result
                    .saved
                    .iter()
                    .any(|v| v.len() != shadow.retained_original_rows.len())
            {
                return Err(internal("bias publication is not request-aligned"));
            }
            for matrix in [
                &action.matrices.bias,
                &action.matrices.bias_diagnostics,
                &action.matrices.bias_support,
            ] {
                let matrix = matrix
                    .as_ref()
                    .ok_or_else(|| internal("bias matrix is absent"))?;
                if spi.matrix_dimensions(matrix.as_str())? != (matrix.rows(), matrix.columns()) {
                    return Err(invalid("bias matrix changed before commit"));
                }
            }
            if result
                .coefficients
                .iter()
                .chain(&result.diagnostics)
                .chain(&result.support)
                .chain(result.saved.iter().flatten())
                .any(|v| !crate::ppml_result::is_stata_publishable(*v))
            {
                return Err(internal("bias publication contains invalid values"));
            }
        }
        _ => return Err(internal("bias request and certified result disagree")),
    }
    Ok(())
}
