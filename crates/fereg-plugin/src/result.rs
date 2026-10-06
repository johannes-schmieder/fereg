use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::fe::{FeSolverKind, LsmrPreconditioner, SolveReport, SolveStopReason};
use fereg_core::ols::{
    CertifiedPreparedOlsClustered, CertifiedPreparedOlsFit, CertifiedPreparedOlsRobust,
    CertifiedPreparedOlsUnadjusted, PreparedOlsBreadCoordinate,
    PreparedOlsClusterSubsetCertificate, PreparedOlsClusteredCertificate,
    PreparedOlsClusteredCovariance, PreparedOlsClusteredDofReport, PreparedOlsClusteredPsdState,
    PreparedOlsExecutionReport, PreparedOlsFitStatistics, PreparedOlsFitStatus,
    PreparedOlsReferenceWeightReport, PreparedOlsResidualDofReport, PreparedOlsRobustCertificate,
    PreparedOlsUnadjustedCertificate,
};
use fereg_core::protocol::OLS_RESULT_WIRE_VERSION;
use fereg_core::{CategoricalTopologyDofEffectIdentity, CategoricalTopologyDofReport};

use fereg_core::ols::PreparedOlsData;

pub(crate) const FIT_ROWS: usize = 12;
pub(crate) const DOF_ROWS: usize = 20;
pub(crate) const SOLVER_ROWS: usize = 2;
pub(crate) const SOLVER_COLUMNS: usize = 16;
pub(crate) const FE_COLUMNS: usize = 8;
pub(crate) const CLUSTER_COLUMNS: usize = 4;
pub(crate) const SUBSET_COLUMNS: usize = 4;
pub(crate) const CERTIFICATE_ROWS: usize = 32;
pub(crate) const RUNTIME_ROWS: usize = 30;
const COVARIANCE_DETERMINANT_CERTIFICATE_ROW: usize = 30;
const COVARIANCE_DETERMINANT_PASSED_ROW: usize = 31;
pub(crate) const RECEIPT_COLUMNS: usize = 16;
pub(crate) const COEFFICIENT_STATUS_COLUMNS: usize = 4;
pub(crate) const RESULT_MAGIC: f64 = 5_354_601.0;
pub(crate) const RECEIPT_COMPLETE_COLUMN: usize = RECEIPT_COLUMNS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OlsVceKind {
    Iid,
    Robust,
    Clustered,
}

impl OlsVceKind {
    pub(crate) const fn code(self) -> f64 {
        match self {
            Self::Iid => 1.0,
            Self::Robust => 2.0,
            Self::Clustered => 3.0,
        }
    }
}

pub(crate) type PreparedAbsorbedDofData<'a> = &'a CategoricalTopologyDofReport;

pub(crate) enum CertifiedOlsInference<'a> {
    Iid(&'a CertifiedPreparedOlsUnadjusted),
    Robust(&'a CertifiedPreparedOlsRobust),
    Clustered(&'a CertifiedPreparedOlsClustered),
}

impl<'a> CertifiedOlsInference<'a> {
    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        match self {
            Self::Iid(value) => value.owned_buffer_bytes(),
            Self::Robust(value) => value.owned_buffer_bytes(),
            Self::Clustered(value) => value.owned_buffer_bytes(),
        }
    }

    pub(crate) const fn kind(&self) -> OlsVceKind {
        match self {
            Self::Iid(_) => OlsVceKind::Iid,
            Self::Robust(_) => OlsVceKind::Robust,
            Self::Clustered(_) => OlsVceKind::Clustered,
        }
    }

    pub(crate) const fn fit(&self) -> &'a CertifiedPreparedOlsFit {
        match self {
            Self::Iid(value) => value.fit(),
            Self::Robust(value) => value.fit(),
            Self::Clustered(value) => value.fit(),
        }
    }

    pub(crate) fn absorbed_dof(&self) -> PreparedAbsorbedDofData<'a> {
        match self {
            Self::Iid(value) => value.absorbed_dof(),
            Self::Robust(value) => value.absorbed_dof(),
            Self::Clustered(value) => value.absorbed_dof(),
        }
    }

    fn weights(&self) -> PreparedOlsReferenceWeightReport {
        match self {
            Self::Iid(value) => value.weights(),
            Self::Robust(value) => value.weights(),
            Self::Clustered(value) => value.weights(),
        }
    }

    fn statistics(&self) -> PreparedOlsFitStatistics {
        match self {
            Self::Iid(value) => value.statistics(),
            Self::Robust(value) => value.statistics(),
            Self::Clustered(value) => value.statistics(),
        }
    }

    fn covariance_coordinates(&self) -> &'a [PreparedOlsBreadCoordinate] {
        match self {
            Self::Iid(value) => value.covariance_coordinates(),
            Self::Robust(value) => value.covariance_coordinates(),
            Self::Clustered(value) => value.covariance_coordinates(),
        }
    }

    fn covariance_values(&self) -> &'a [f64] {
        match self {
            Self::Iid(value) => value.covariance().values(),
            Self::Robust(value) => value.covariance().values(),
            Self::Clustered(value) => value.covariance().values(),
        }
    }

    fn residual_dof(&self) -> Option<PreparedOlsResidualDofReport> {
        match self {
            Self::Iid(value) => Some(value.dof()),
            Self::Robust(value) => Some(value.dof()),
            Self::Clustered(_) => None,
        }
    }

    fn clustered_dof(&self) -> Option<PreparedOlsClusteredDofReport> {
        match self {
            Self::Clustered(value) => Some(value.dof()),
            _ => None,
        }
    }

    fn cluster_counts(&self) -> Option<&'a [usize]> {
        match self {
            Self::Clustered(value) => Some(value.cluster_counts()),
            _ => None,
        }
    }

    fn subsets(&self) -> Option<&'a [PreparedOlsClusterSubsetCertificate]> {
        match self {
            Self::Clustered(value) => Some(value.subsets()),
            _ => None,
        }
    }

    fn clustered_covariance(&self) -> Option<&'a PreparedOlsClusteredCovariance> {
        match self {
            Self::Clustered(value) => Some(value.covariance()),
            _ => None,
        }
    }

    fn iid_certificate(&self) -> Option<PreparedOlsUnadjustedCertificate> {
        match self {
            Self::Iid(value) => Some(value.certificate()),
            _ => None,
        }
    }

    fn robust_certificate(&self) -> Option<PreparedOlsRobustCertificate> {
        match self {
            Self::Robust(value) => Some(value.certificate()),
            _ => None,
        }
    }

    fn clustered_certificate(&self) -> Option<PreparedOlsClusteredCertificate> {
        match self {
            Self::Clustered(value) => Some(value.certificate()),
            _ => None,
        }
    }

    fn certificate_passed(&self) -> bool {
        self.iid_certificate().is_some_and(|value| value.passed)
            || self.robust_certificate().is_some_and(|value| value.passed)
            || self
                .clustered_certificate()
                .is_some_and(|value| value.passed)
    }

    fn is_clustered(&self) -> bool {
        matches!(self.kind(), OlsVceKind::Clustered)
    }
}

#[derive(Debug)]
pub(crate) struct OlsShadowResult<'a> {
    pub(crate) b: Vec<f64>,
    pub(crate) covariance: Vec<f64>,
    pub(crate) coefficient_status: Vec<f64>,
    pub(crate) omission_b_marker: Vec<f64>,
    pub(crate) omission_v_marker: Vec<f64>,
    pub(crate) fit: Vec<f64>,
    pub(crate) dof: Vec<f64>,
    pub(crate) solver: Vec<f64>,
    pub(crate) fixed_effects: Vec<f64>,
    pub(crate) clusters: Vec<f64>,
    pub(crate) subsets: Vec<f64>,
    pub(crate) certificates: Vec<f64>,
    pub(crate) receipt: Vec<f64>,
    pub(crate) coefficient_count: usize,
    pub(crate) coefficient_capacity: usize,
    pub(crate) covariance_dimension: usize,
    pub(crate) fixed_effect_rows: usize,
    pub(crate) cluster_rows: usize,
    pub(crate) subset_rows: usize,
    pub(crate) drop_reasons: &'a [fereg_core::DropReason],
    pub(crate) retained_rows: &'a [usize],
    pub(crate) residuals: &'a [f64],
    pub(crate) execution: PreparedOlsExecutionReport,
    pub(crate) solver_workspace_peak: Option<u64>,
    pub(crate) solver_expected_heap_peak: Option<u64>,
}

impl OlsShadowResult<'_> {
    pub(crate) fn capacity_bytes(&self) -> u128 {
        [
            &self.b,
            &self.covariance,
            &self.coefficient_status,
            &self.omission_b_marker,
            &self.omission_v_marker,
            &self.fit,
            &self.dof,
            &self.solver,
            &self.fixed_effects,
            &self.clusters,
            &self.subsets,
            &self.certificates,
            &self.receipt,
        ]
        .into_iter()
        .map(|values| values.capacity() as u128)
        .sum::<u128>()
            * std::mem::size_of::<f64>() as u128
    }
}

pub(crate) fn build_shadow_result<'a>(
    prepared: &'a PreparedOlsData,
    clustered_prepared: Option<&'a PreparedOlsData>,
    inference: CertifiedOlsInference<'a>,
    residual_requested: bool,
) -> Result<OlsShadowResult<'a>> {
    validate_alignment(prepared, clustered_prepared, &inference)?;
    let fit = inference.fit();
    let coefficient_count = prepared.model().coefficients.len();
    let coefficient_capacity = prepared.report_plan().original_coefficient_count();
    if coefficient_count > coefficient_capacity {
        return Err(invalid(
            "prepared OLS logical coefficient count exceeds its caller allocation",
        ));
    }
    let covariance_dimension = inference.covariance_coordinates().len();
    let matrix_dimension = coefficient_count.max(1);
    let mut b = reserved_filled(matrix_dimension, 0.0, "OLS shadow coefficient vector")?;
    let mut covariance = reserved_filled(
        checked_product(matrix_dimension, matrix_dimension, "OLS shadow covariance")?,
        0.0,
        "OLS shadow covariance",
    )?;
    let mut coefficient_status = reserved_filled(
        checked_product(
            matrix_dimension,
            COEFFICIENT_STATUS_COLUMNS,
            "OLS coefficient-status table",
        )?,
        0.0,
        "OLS coefficient-status table",
    )?;
    let mut omission_b_marker =
        reserved_filled(matrix_dimension, 0.0, "OLS omission coefficient marker")?;
    let mut omission_v_marker = reserved_filled(
        checked_product(
            matrix_dimension,
            matrix_dimension,
            "OLS omission covariance marker",
        )?,
        0.0,
        "OLS omission covariance marker",
    )?;

    if fit.coefficients().len() != coefficient_count {
        return Err(invalid(
            "prepared OLS fit coefficient count is inconsistent",
        ));
    }
    let mut slot_seen = reserved_filled(coefficient_count, false, "OLS coefficient slots")?;
    for coefficient in fit.coefficients() {
        let slot = one_based_index(
            coefficient.coefficient_slot.get(),
            coefficient_count,
            "prepared OLS coefficient slot",
        )?;
        if slot_seen[slot] {
            return Err(invalid("prepared OLS coefficient slot is duplicated"));
        }
        slot_seen[slot] = true;
        let (status, value) = match coefficient.status {
            PreparedOlsFitStatus::Kept => {
                (1.0, required_finite(coefficient.value, "kept coefficient")?)
            }
            PreparedOlsFitStatus::SyntaxOmitted => {
                require_absent(coefficient.value, "syntax-omitted coefficient")?;
                (2.0, 0.0)
            }
            PreparedOlsFitStatus::CollinearWithFixedEffects => {
                require_absent(coefficient.value, "FE-collinear coefficient")?;
                (3.0, 0.0)
            }
            PreparedOlsFitStatus::NumericallyRankDeficient => {
                require_absent(coefficient.value, "rank-deficient coefficient")?;
                (4.0, 0.0)
            }
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept => (
                5.0,
                required_finite(coefficient.value, "normalized absorbed intercept")?,
            ),
            PreparedOlsFitStatus::AbsorbedInterceptDeferred => {
                return Err(invalid(
                    "deferred absorbed intercept survived the certified prepared OLS fit",
                ));
            }
        };
        b[slot] = value;
        let offset = slot * COEFFICIENT_STATUS_COLUMNS;
        coefficient_status[offset] = f64::from(coefficient.coefficient_slot.get());
        coefficient_status[offset + 1] = status;
        coefficient_status[offset + 2] = coefficient
            .generated_column_index
            .map_or(0.0, |index| (index + 1) as f64);
    }
    if slot_seen.iter().any(|seen| !seen) {
        return Err(invalid("prepared OLS coefficient slots are not contiguous"));
    }

    let coordinates = inference.covariance_coordinates();
    let values = inference.covariance_values();
    if values.len()
        != checked_product(
            covariance_dimension,
            covariance_dimension,
            "compact OLS covariance",
        )?
    {
        return Err(invalid(
            "prepared OLS compact covariance has the wrong size",
        ));
    }
    let mut covariance_slots =
        reserved_filled(coefficient_count, None, "OLS covariance-coordinate slots")?;
    for (coordinate_index, coordinate) in coordinates.iter().enumerate() {
        let slot = one_based_index(
            coordinate.coefficient_slot.get(),
            coefficient_count,
            "prepared OLS covariance slot",
        )?;
        if covariance_slots[slot].replace(coordinate_index).is_some() {
            return Err(invalid("prepared OLS covariance slot is duplicated"));
        }
        let status_offset = slot * COEFFICIENT_STATUS_COLUMNS;
        if !matches!(coefficient_status[status_offset + 1], 1.0 | 5.0) {
            return Err(invalid(
                "prepared OLS covariance coordinate refers to an unreported coefficient",
            ));
        }
        coefficient_status[status_offset + 3] = (coordinate_index + 1) as f64;
        omission_b_marker[slot] = 1.0;
        omission_v_marker[slot * matrix_dimension + slot] = 1.0;
    }
    for row in 0..covariance_dimension {
        let target_row = one_based_index(
            coordinates[row].coefficient_slot.get(),
            coefficient_count,
            "prepared OLS covariance row slot",
        )?;
        for column in 0..covariance_dimension {
            let target_column = one_based_index(
                coordinates[column].coefficient_slot.get(),
                coefficient_count,
                "prepared OLS covariance column slot",
            )?;
            let value = values[row * covariance_dimension + column];
            if !value.is_finite() {
                return Err(invalid(
                    "prepared OLS covariance contains a nonfinite value",
                ));
            }
            covariance[target_row * matrix_dimension + target_column] = value;
        }
    }

    let fit_values = build_fit_statistics(inference.statistics())?;
    let dof = build_dof(prepared, &inference)?;
    let solver = build_solver(fit)?;
    let (fixed_effects, fixed_effect_rows) = build_fixed_effects(inference.absorbed_dof())?;
    let (clusters, cluster_rows, subsets, subset_rows) =
        build_clusters(clustered_prepared, &inference)?;
    let determinant_covariance = if coefficient_count == 0 {
        &covariance[..0]
    } else {
        covariance.as_slice()
    };
    let certificates = build_certificates(&inference, determinant_covariance, coefficient_count)?;
    let receipt = build_receipt(
        prepared,
        inference.kind(),
        coefficient_count,
        covariance_dimension,
        fixed_effect_rows,
        cluster_rows,
        subset_rows,
        residual_requested,
    )?;
    if fit.residuals().len() != prepared.sample().n_active()
        || prepared.sample().drop_reasons.len() != prepared.sample().n_original
    {
        return Err(invalid("prepared OLS observation outputs are misaligned"));
    }

    Ok(OlsShadowResult {
        b,
        covariance,
        coefficient_status,
        omission_b_marker,
        omission_v_marker,
        fit: fit_values,
        dof,
        solver,
        fixed_effects,
        clusters,
        subsets,
        certificates,
        receipt,
        coefficient_count,
        coefficient_capacity,
        covariance_dimension,
        fixed_effect_rows,
        cluster_rows,
        subset_rows,
        drop_reasons: &prepared.sample().drop_reasons,
        retained_rows: prepared.sample().retained_rows(),
        residuals: fit.residuals(),
        execution: fit.execution(),
        solver_workspace_peak: fit.solver_workspace_peak().bytes(),
        solver_expected_heap_peak: fit.solver_workspace_peak().expected_heap_bytes(),
    })
}

fn validate_alignment(
    prepared: &'_ PreparedOlsData,
    clustered: Option<&'_ PreparedOlsData>,
    inference: &CertifiedOlsInference<'_>,
) -> Result<()> {
    match (inference.is_clustered(), clustered) {
        (true, Some(value)) if std::ptr::eq(value, prepared) => {}
        (true, _) => {
            return Err(invalid(
                "clustered inference is not sample-bound to its preparation",
            ));
        }
        (false, None) => {}
        (false, Some(_)) => {
            return Err(invalid(
                "nonclustered inference unexpectedly has cluster preparation",
            ));
        }
    }
    if !inference.fit().rank_certificate().passed
        || !inference.fit().score_certificate().passed
        || !inference.fit().bread().certificate().passed
    {
        return Err(invalid("prepared OLS fit certificate is not complete"));
    }
    if !inference.certificate_passed() {
        return Err(invalid(
            "prepared OLS inference certificate is not complete",
        ));
    }
    Ok(())
}

fn build_fit_statistics(statistics: PreparedOlsFitStatistics) -> Result<Vec<f64>> {
    let mut output = reserved_filled(FIT_ROWS * 2, 0.0, "OLS fit-statistics table")?;
    set_defined(&mut output, 0, Some(statistics.rss), "RSS")?;
    set_defined(&mut output, 1, Some(statistics.tss), "TSS")?;
    set_defined(&mut output, 2, Some(statistics.within_tss), "within TSS")?;
    set_defined(
        &mut output,
        3,
        Some(statistics.tss - statistics.rss),
        "model sum of squares",
    )?;
    set_defined(&mut output, 4, statistics.r_squared, "R-squared")?;
    set_defined(
        &mut output,
        5,
        statistics.adjusted_r_squared,
        "adjusted R-squared",
    )?;
    set_defined(
        &mut output,
        6,
        statistics.within_r_squared,
        "within R-squared",
    )?;
    set_defined(
        &mut output,
        7,
        statistics.adjusted_within_r_squared,
        "adjusted within R-squared",
    )?;
    set_defined(
        &mut output,
        8,
        Some(statistics.root_mean_squared_error),
        "root mean squared error",
    )?;
    set_defined(&mut output, 9, statistics.f_statistic, "F statistic")?;
    set_defined(
        &mut output,
        10,
        statistics.gaussian_log_likelihood,
        "Gaussian log likelihood",
    )?;
    set_defined(
        &mut output,
        11,
        statistics.gaussian_null_log_likelihood,
        "Gaussian null log likelihood",
    )?;
    Ok(output)
}

fn build_dof(
    prepared: &'_ PreparedOlsData,
    inference: &CertifiedOlsInference<'_>,
) -> Result<Vec<f64>> {
    let mut output = reserved_filled(DOF_ROWS * 2, 0.0, "OLS DoF table")?;
    let weights = inference.weights();
    let absorbed = inference.absorbed_dof();
    let (
        finite_rank,
        finite_model_dof,
        absorbed_rank,
        model_rank,
        reported,
        fit,
        correction,
        nested,
        structural,
        normalized,
        approximate,
    ) = if let Some(dof) = inference.clustered_dof() {
        let model_rank = dof
            .finite_rank
            .checked_add(dof.absorbed_rank)
            .ok_or_else(|| allocation("clustered OLS model rank overflows"))?;
        (
            dof.finite_rank,
            dof.finite_model_dof,
            dof.absorbed_rank,
            model_rank,
            dof.reported_residual_dof as f64,
            dof.fit_residual_dof as f64,
            dof.correction_denominator_dof as f64,
            dof.nested_absorbed_categories,
            dof.has_structural_intercept,
            dof.has_normalized_absorbed_intercept,
            dof.absorbed_rank_is_approximate,
        )
    } else {
        let dof = inference
            .residual_dof()
            .ok_or_else(|| invalid("nonclustered OLS residual DoF is absent"))?;
        (
            dof.finite_rank,
            dof.finite_model_dof,
            dof.absorbed_rank,
            dof.model_rank,
            dof.residual_dof as f64,
            dof.residual_dof as f64,
            dof.residual_dof as f64,
            absorbed.nested_categories,
            dof.has_structural_intercept,
            dof.has_normalized_absorbed_intercept,
            dof.absorbed_rank_is_approximate,
        )
    };
    let fields = [
        Some(weights.effective_observations as f64),
        Some(weights.physical_observations as f64),
        Some(prepared.sample().n_candidate as f64),
        Some(prepared.sample().n_active() as f64),
        weights.raw_weight_sum,
        Some(finite_rank as f64),
        Some(finite_model_dof as f64),
        Some(absorbed_rank as f64),
        Some(model_rank as f64),
        Some(reported),
        Some(fit),
        Some(correction),
        Some(nested as f64),
        Some(absorbed.initial_categories as f64),
        Some(absorbed.redundant_categories as f64),
        Some(prepared.singletons().num_singletons as f64),
        Some(prepared.singletons().cascade_events as f64),
        Some(f64::from(structural)),
        Some(f64::from(normalized)),
        Some(f64::from(approximate)),
    ];
    for (row, value) in fields.into_iter().enumerate() {
        set_defined(&mut output, row, value, "OLS DoF field")?;
    }
    Ok(output)
}

fn build_solver(fit: &CertifiedPreparedOlsFit) -> Result<Vec<f64>> {
    let mut output = reserved_filled(SOLVER_ROWS * SOLVER_COLUMNS, 0.0, "OLS solver diagnostics")?;
    for (row, report) in [fit.absorption_report(), fit.residual_absorption_report()]
        .into_iter()
        .enumerate()
    {
        write_solver_row(&mut output, row, report)?;
    }
    Ok(output)
}

fn write_solver_row(output: &mut [f64], row: usize, report: &SolveReport) -> Result<()> {
    if !report.certified() {
        return Err(invalid("prepared OLS solver report is not certified"));
    }
    let values = [
        solver_code(report.requested_solver),
        solver_code(report.used_solver),
        report.iterations as f64,
        stop_code(report.stop_reason),
        report.certificate.absolute_infinity_norm,
        report.certificate.relative_infinity_norm,
        report.certificate.relative_two_norm,
        report.normal_equation_residual,
        report.setup_seconds,
        report.solve_seconds,
        f64::from(report.fallback_used),
        report.warnings.len() as f64,
        preconditioner_code(report.requested_preconditioner),
        preconditioner_code(report.used_preconditioner),
        report.pilot_iterations as f64,
        report.pilot_contraction,
    ];
    let start = row * SOLVER_COLUMNS;
    for (index, value) in values.into_iter().enumerate() {
        if !value.is_finite() {
            return Err(invalid("prepared OLS solver diagnostic is nonfinite"));
        }
        output[start + index] = value;
    }
    Ok(())
}

fn build_fixed_effects(report: PreparedAbsorbedDofData<'_>) -> Result<(Vec<f64>, usize)> {
    let rows = report.effects.len().max(1);
    let mut output = reserved_filled(
        checked_product(rows, FE_COLUMNS, "OLS FE metadata")?,
        0.0,
        "OLS FE metadata",
    )?;
    for (row, effect) in report.effects.iter().enumerate() {
        let (identity, term, source) = match &effect.identity {
            CategoricalTopologyDofEffectIdentity::Explicit { term_id, sources } => {
                if let Some(slope_source) = effect.slope_source {
                    (4.0, f64::from(term_id.get()), f64::from(slope_source.get()))
                } else {
                    match sources.as_slice() {
                        [] => {
                            return Err(invalid(
                                "categorical absorbed effect has no source identity",
                            ));
                        }
                        [source] => (1.0, f64::from(term_id.get()), f64::from(source.get())),
                        _ => (3.0, f64::from(term_id.get()), 0.0),
                    }
                }
            }
            CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb => {
                if effect.slope_source.is_some() {
                    return Err(invalid(
                        "implicit categorical absorbed effect has a slope source",
                    ));
                }
                (2.0, 0.0, 0.0)
            }
        };
        write_fixed_effect_row(
            &mut output,
            row,
            identity,
            term,
            source,
            effect.categories,
            effect.redundant,
            effect.coefficients,
            effect.redundancy_exact,
            effect.nested,
        );
    }
    Ok((output, rows))
}

#[allow(clippy::too_many_arguments)]
fn write_fixed_effect_row(
    output: &mut [f64],
    row: usize,
    identity: f64,
    term: f64,
    source: f64,
    categories: usize,
    redundant: usize,
    coefficients: usize,
    redundancy_exact: bool,
    nested: bool,
) {
    let values = [
        identity,
        term,
        source,
        categories as f64,
        redundant as f64,
        coefficients as f64,
        f64::from(redundancy_exact),
        f64::from(nested),
    ];
    output[row * FE_COLUMNS..(row + 1) * FE_COLUMNS].copy_from_slice(&values);
}

fn build_clusters(
    prepared: Option<&'_ PreparedOlsData>,
    inference: &CertifiedOlsInference<'_>,
) -> Result<(Vec<f64>, usize, Vec<f64>, usize)> {
    let cluster_counts = inference.cluster_counts().unwrap_or(&[]);
    let subset_reports = inference.subsets().unwrap_or(&[]);
    let cluster_rows = cluster_counts.len();
    let subset_rows = subset_reports.len();
    let physical_cluster_rows = cluster_rows.max(1);
    let physical_subset_rows = subset_rows.max(1);
    let mut clusters = reserved_filled(
        checked_product(
            physical_cluster_rows,
            CLUSTER_COLUMNS,
            "OLS cluster metadata",
        )?,
        0.0,
        "OLS cluster metadata",
    )?;
    let mut subsets = reserved_filled(
        checked_product(physical_subset_rows, SUBSET_COLUMNS, "OLS subset metadata")?,
        0.0,
        "OLS subset metadata",
    )?;
    if inference.is_clustered() {
        let prepared = prepared.ok_or_else(|| invalid("clustered OLS preparation is absent"))?;
        if prepared.clusters().dimensions().len() != cluster_counts.len() {
            return Err(invalid("prepared OLS cluster counts are inconsistent"));
        }
        for (row, (dimension, count)) in prepared
            .clusters()
            .dimensions()
            .iter()
            .zip(cluster_counts)
            .enumerate()
        {
            if dimension.groups() != *count {
                return Err(invalid(
                    "prepared OLS cluster group count changed after fit",
                ));
            }
            let values = [
                (row + 1) as f64,
                dimension.sources().len() as f64,
                *count as f64,
                f64::from(dimension.is_interaction()),
            ];
            clusters[row * CLUSTER_COLUMNS..(row + 1) * CLUSTER_COLUMNS].copy_from_slice(&values);
        }
        for (row, subset) in subset_reports.iter().enumerate() {
            let values = [
                f64::from(subset.mask),
                subset.dimensions as f64,
                f64::from(subset.sign),
                subset.groups as f64,
            ];
            subsets[row * SUBSET_COLUMNS..(row + 1) * SUBSET_COLUMNS].copy_from_slice(&values);
        }
    }
    Ok((
        clusters,
        physical_cluster_rows,
        subsets,
        physical_subset_rows,
    ))
}

fn build_certificates(
    inference: &CertifiedOlsInference<'_>,
    reported_covariance: &[f64],
    reported_dimension: usize,
) -> Result<Vec<f64>> {
    let mut output = reserved_filled(CERTIFICATE_ROWS * 2, 0.0, "OLS certificate diagnostics")?;
    let fit = inference.fit();
    let rank = fit.rank_certificate();
    let score = fit.score_certificate();
    let bread = fit.bread().certificate();
    let common = [
        Some(f64::from(rank.passed)),
        Some(f64::from(score.passed)),
        Some(f64::from(bread.passed)),
        Some(rank.relative_threshold),
        Some(rank.maximum_dropped_trailing_norm),
        Some(rank.condition_proxy),
        Some(score.relative_infinity_norm),
        Some(score.relative_two_norm),
        Some(bread.left_inverse_relative_infinity_norm),
        Some(bread.right_inverse_relative_infinity_norm),
        Some(bread.reported_left_inverse_relative_infinity_norm),
        Some(bread.reported_right_inverse_relative_infinity_norm),
        Some(bread.symmetry_relative_infinity_norm),
        Some(bread.minimum_diagonal),
        Some(bread.intercept_formula_relative_error),
        Some(bread.intercept_normalization_relative_error),
    ];
    for (row, value) in common.into_iter().enumerate() {
        set_defined(&mut output, row, value, "OLS fit certificate")?;
    }
    if let Some(certificate) = inference.iid_certificate() {
        let selected = [
            certificate.reference_weight_sum_relative_error,
            certificate.covariance_scale_relative_infinity_norm,
            certificate.covariance_symmetry_relative_infinity_norm,
            0.0,
            certificate.minimum_covariance_diagonal,
            0.0,
            certificate.statistic_formula_relative_infinity_norm,
            f64::from(certificate.passed),
        ];
        set_selected_certificates(&mut output, &selected)?;
    } else if let Some(certificate) = inference.robust_certificate() {
        let selected = [
            certificate.reference_weight_sum_relative_error,
            certificate.sandwich_formula_relative_infinity_norm,
            certificate.covariance_symmetry_relative_infinity_norm,
            certificate.standard_error_relative_infinity_norm,
            certificate.minimum_normalized_psd_pivot,
            certificate.wald_solve_relative_infinity_norm,
            certificate.statistic_formula_relative_infinity_norm,
            f64::from(certificate.passed),
        ];
        set_selected_certificates(&mut output, &selected)?;
    } else if let Some(certificate) = inference.clustered_certificate() {
        let selected = [
            certificate.reference_weight_sum_relative_error,
            certificate.corrected_covariance_relative_infinity_norm,
            certificate.covariance_symmetry_relative_infinity_norm,
            certificate.standard_error_relative_infinity_norm,
            certificate.pre_restore_full_psd_relative_error,
            certificate.wald_solve_relative_infinity_norm,
            certificate.statistic_formula_relative_infinity_norm,
            f64::from(certificate.passed),
        ];
        set_selected_certificates(&mut output, &selected)?;
        let covariance = inference
            .clustered_covariance()
            .ok_or_else(|| invalid("clustered OLS covariance is absent"))?;
        let psd = covariance.psd();
        let extras = [
            psd_state_code(psd.state),
            f64::from(psd.full_adjusted),
            f64::from(psd.slope_block_restored),
            psd.minimum_final_full_eigenvalue,
            f64::from(psd.strict_final_full_psd),
            covariance.correction(),
        ];
        for (offset, entry) in extras.into_iter().enumerate() {
            set_defined(
                &mut output,
                24 + offset,
                Some(entry),
                "clustered OLS PSD diagnostic",
            )?;
        }
    } else {
        return Err(invalid("prepared OLS inference certificate is absent"));
    }
    let determinant = covariance_determinant(reported_covariance, reported_dimension)?;
    set_defined(
        &mut output,
        COVARIANCE_DETERMINANT_CERTIFICATE_ROW,
        Some(determinant),
        "reported OLS covariance determinant",
    )?;
    set_defined(
        &mut output,
        COVARIANCE_DETERMINANT_PASSED_ROW,
        Some(1.0),
        "reported OLS covariance determinant certificate",
    )?;
    Ok(output)
}

/// Determinant of the exact covariance matrix scattered to Stata coordinates.
///
/// The zero-coordinate result has the standard empty-product determinant one.
/// Dimensions one and two use explicit partial-pivoting elimination so the
/// hidden licensed q2/q3 anchors are bit-for-bit identical to Stata's
/// corresponding `det()`. Larger matrices use the same deterministic scheme. Any
/// nonfinite intermediate fails closed before result publication.
fn covariance_determinant(values: &[f64], dimension: usize) -> Result<f64> {
    if values.len()
        != dimension
            .checked_mul(dimension)
            .ok_or_else(|| allocation("OLS covariance determinant dimensions overflow"))?
    {
        return Err(invalid(
            "OLS covariance determinant dimensions are inconsistent",
        ));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(invalid("OLS covariance determinant input is nonfinite"));
    }
    let determinant = match dimension {
        0 => 1.0,
        1 => values[0],
        2 => {
            let (leading, trailing) = if values[0].abs() >= values[2].abs() {
                if values[0] == 0.0 {
                    return Ok(0.0);
                }
                (values[0], values[3] - (values[2] / values[0]) * values[1])
            } else {
                (-values[2], values[1] - (values[0] / values[2]) * values[3])
            };
            leading * trailing
        }
        _ => {
            let mut work = Vec::new();
            work.try_reserve_exact(values.len())
                .map_err(|_| allocation("unable to reserve OLS covariance determinant work"))?;
            if work.capacity() < values.len() {
                return Err(allocation(
                    "OLS covariance determinant work reservation was undersized",
                ));
            }
            work.extend_from_slice(values);
            let mut determinant = 1.0;
            let mut sign = 1.0;
            for column in 0..dimension {
                let mut pivot_row = column;
                let mut pivot_magnitude = work[column * dimension + column].abs();
                for row in (column + 1)..dimension {
                    let candidate = work[row * dimension + column].abs();
                    if candidate > pivot_magnitude {
                        pivot_magnitude = candidate;
                        pivot_row = row;
                    }
                }
                if pivot_magnitude == 0.0 {
                    return Ok(0.0);
                }
                if pivot_row != column {
                    for entry in column..dimension {
                        work.swap(column * dimension + entry, pivot_row * dimension + entry);
                    }
                    sign = -sign;
                }
                let pivot = work[column * dimension + column];
                determinant *= pivot;
                if !determinant.is_finite() {
                    return Err(invalid(
                        "OLS covariance determinant accumulation is nonfinite",
                    ));
                }
                for row in (column + 1)..dimension {
                    let factor = work[row * dimension + column] / pivot;
                    work[row * dimension + column] = 0.0;
                    for entry in (column + 1)..dimension {
                        work[row * dimension + entry] -= factor * work[column * dimension + entry];
                        if !work[row * dimension + entry].is_finite() {
                            return Err(invalid(
                                "OLS covariance determinant elimination is nonfinite",
                            ));
                        }
                    }
                }
            }
            sign * determinant
        }
    };
    if !determinant.is_finite() {
        return Err(invalid("OLS covariance determinant is nonfinite"));
    }
    Ok(determinant)
}

fn set_selected_certificates(output: &mut [f64], selected: &[f64; 8]) -> Result<()> {
    for (offset, value) in selected.iter().copied().enumerate() {
        set_defined(
            output,
            16 + offset,
            Some(value),
            "selected OLS inference certificate",
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_receipt(
    prepared: &'_ PreparedOlsData,
    kind: OlsVceKind,
    coefficient_count: usize,
    covariance_dimension: usize,
    fixed_effect_rows: usize,
    cluster_rows: usize,
    subset_rows: usize,
    residual_requested: bool,
) -> Result<Vec<f64>> {
    let mut output = reserved_filled(RECEIPT_COLUMNS, 0.0, "OLS result receipt")?;
    let logical_cluster_rows = usize::from(kind == OlsVceKind::Clustered) * cluster_rows;
    let logical_subset_rows = usize::from(kind == OlsVceKind::Clustered) * subset_rows;
    let values = [
        RESULT_MAGIC,
        f64::from(OLS_RESULT_WIRE_VERSION.major),
        f64::from(OLS_RESULT_WIRE_VERSION.minor),
        kind.code(),
        coefficient_count as f64,
        covariance_dimension as f64,
        prepared.sample().n_original as f64,
        prepared.sample().n_candidate as f64,
        prepared.sample().n_active() as f64,
        logical_cluster_rows as f64,
        fixed_effect_rows as f64,
        logical_subset_rows as f64,
        1.0,
        f64::from(residual_requested),
        1.0,
        0.0,
    ];
    for (index, value) in values.into_iter().enumerate() {
        if !value.is_finite() || value.abs() > 9_007_199_254_740_992.0 {
            return Err(invalid(
                "OLS result receipt field is not exactly representable",
            ));
        }
        output[index] = value;
    }
    Ok(output)
}

fn set_defined(output: &mut [f64], row: usize, value: Option<f64>, label: &str) -> Result<()> {
    let offset = row
        .checked_mul(2)
        .ok_or_else(|| allocation("OLS result row offset overflows"))?;
    let Some(value) = value else {
        return Ok(());
    };
    if !value.is_finite() {
        return Err(invalid(format!("{label} is nonfinite")));
    }
    output[offset] = 1.0;
    output[offset + 1] = value;
    Ok(())
}

fn one_based_index(value: u32, len: usize, label: &str) -> Result<usize> {
    let index = usize::try_from(value)
        .ok()
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| invalid(format!("{label} is not positive")))?;
    if index >= len {
        return Err(invalid(format!("{label} is out of range")));
    }
    Ok(index)
}

fn required_finite(value: Option<f64>, label: &str) -> Result<f64> {
    let value = value.ok_or_else(|| invalid(format!("{label} has no value")))?;
    if !value.is_finite() {
        return Err(invalid(format!("{label} is nonfinite")));
    }
    Ok(value)
}

fn require_absent(value: Option<f64>, label: &str) -> Result<()> {
    if value.is_some() {
        Err(invalid(format!("{label} unexpectedly has a value")))
    } else {
        Ok(())
    }
}

fn solver_code(value: FeSolverKind) -> f64 {
    match value {
        FeSolverKind::Auto => 1.0,
        FeSolverKind::Map => 2.0,
        FeSolverKind::Lsmr => 3.0,
        FeSolverKind::Cg => 4.0,
        FeSolverKind::IronsTuck => 5.0,
        FeSolverKind::Squarem => 6.0,
        FeSolverKind::Cmg => 7.0,
    }
}

fn preconditioner_code(value: Option<LsmrPreconditioner>) -> f64 {
    match value {
        None => 0.0,
        Some(LsmrPreconditioner::Auto) => 1.0,
        Some(LsmrPreconditioner::Off) => 2.0,
        Some(LsmrPreconditioner::Diagonal) => 3.0,
        Some(LsmrPreconditioner::Schwarz) => 4.0,
    }
}

fn stop_code(value: SolveStopReason) -> f64 {
    match value {
        SolveStopReason::NoFixedEffects => 1.0,
        SolveStopReason::Certified => 2.0,
        SolveStopReason::MaximumIterations => 3.0,
        SolveStopReason::Uncertified => 4.0,
    }
}

fn psd_state_code(value: PreparedOlsClusteredPsdState) -> f64 {
    match value {
        PreparedOlsClusteredPsdState::OneWayStrict => 1.0,
        PreparedOlsClusteredPsdState::MultiwayRawPsd => 2.0,
        PreparedOlsClusteredPsdState::MultiwayFullClipped => 3.0,
        PreparedOlsClusteredPsdState::MultiwayFullClippedSlopeRestored => 4.0,
    }
}

fn checked_product(left: usize, right: usize, label: &'static str) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| allocation(format!("{label} size overflows")))
}

fn reserved_filled<T: Clone>(elements: usize, value: T, label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    if output.capacity() < elements {
        return Err(allocation(format!("{label} reservation was undersized")));
    }
    output.resize(elements, value);
    Ok(output)
}

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::covariance_determinant;

    #[test]
    fn covariance_determinant_has_exact_small_wire_semantics() {
        assert_eq!(covariance_determinant(&[], 0).expect("empty"), 1.0);
        assert_eq!(covariance_determinant(&[3.0], 1).expect("scalar"), 3.0);
        assert_eq!(
            covariance_determinant(&[2.0, 3.0, 1.0, 2.0], 2).expect("two by two"),
            1.0
        );
        assert_eq!(
            covariance_determinant(&[1.0, 0.0, 0.0, 0.0], 2).expect("singular"),
            0.0
        );
        for (values, expected) in [
            (
                [
                    f64::from_bits(0x3fa0_202f_479f_358f),
                    f64::from_bits(0xbf82_c793_8ea4_f98d),
                    f64::from_bits(0xbf82_c793_8ea4_f98d),
                    f64::from_bits(0x3f65_ddec_fdb9_c14b),
                ],
                f64::from_bits(0xbe4b_7ee8_115e_0f3c),
            ),
            (
                [
                    f64::from_bits(0x3f93_37c1_45ab_7e50),
                    f64::from_bits(0xbf76_a2d7_1c41_0628),
                    f64::from_bits(0xbf76_a2d7_1c41_0628),
                    f64::from_bits(0x3f5a_a6d2_b8b3_1611),
                ],
                f64::from_bits(0xbe4a_79e7_840b_02b9),
            ),
        ] {
            assert_eq!(
                covariance_determinant(&values, 2).expect("sealed two-way determinant"),
                expected
            );
        }
    }

    #[test]
    fn covariance_determinant_fails_closed_on_malformed_or_nonfinite_input() {
        assert!(covariance_determinant(&[1.0], 2).is_err());
        assert!(covariance_determinant(&[f64::NAN], 1).is_err());
        assert!(covariance_determinant(&[f64::MAX, 0.0, 0.0, f64::MAX], 2).is_err());
    }
}
