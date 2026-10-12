use super::*;
use fereg_core::ols::*;

pub(super) fn prepare_ols_pipeline(
    loaded: LoadedOlsInput,
    loaded_rhs: Option<Vec<f64>>,
    load_columns: Option<PreparedOlsLoadColumns>,
    action: &mut EstimateAction,
    categorical: bool,
) -> Result<(PreparedOlsData, Option<LoadedOlsRows>)> {
    let clusters = std::mem::take(&mut action.clusters);
    let result =
        prepare_loaded_ols_with_moments(loaded, loaded_rhs, action, |request, input, scratch| {
            prepare_model_owned_with_load_columns(
                request,
                &clusters,
                input,
                categorical,
                load_columns,
                scratch,
            )
        });
    action.clusters = clusters;
    result
}

pub(super) fn infer_ols(
    prepared: &PreparedOlsData,
    action: &EstimateAction,
    rows: Option<LoadedOlsRows>,
) -> Result<OwnedInference> {
    let iid = || PreparedOlsUnadjustedConfig {
        fit: action.fit,
        absorbed_dof: action.absorbed_dof,
        ..PreparedOlsUnadjustedConfig::default()
    };
    let robust = || PreparedOlsRobustConfig {
        fit: action.fit,
        absorbed_dof: action.absorbed_dof,
        ..PreparedOlsRobustConfig::default()
    };
    let clustered = || PreparedOlsClusteredConfig {
        fit: action.fit,
        absorbed_dof: action.absorbed_dof,
        account_cluster_nesting: action.account_cluster_nesting,
        ..PreparedOlsClusteredConfig::default()
    };
    let p = prepared;
    match action.vce {
        OlsVceKind::Iid => fit_prepared_model_unadjusted_with_loaded_moments(p, iid(), rows)
            .map(OwnedInference::Iid),
        OlsVceKind::Robust => fit_prepared_model_robust_with_loaded_moments(p, robust(), rows)
            .map(OwnedInference::Robust),
        OlsVceKind::Clustered => {
            fit_prepared_model_clustered_with_loaded_moments(p, clustered(), rows)
                .map(OwnedInference::Clustered)
        }
    }
}

// Keep the single certified result on the action stack; boxing would add a
// new allocation between certification and publication. The owner is bounded.
#[allow(clippy::large_enum_variant)]
pub(super) enum OwnedInference {
    Iid(CertifiedPreparedOlsUnadjusted),
    Robust(CertifiedPreparedOlsRobust),
    Clustered(CertifiedPreparedOlsClustered),
}

impl OwnedInference {
    pub(super) fn view(&self) -> CertifiedOlsInference<'_> {
        match self {
            Self::Iid(value) => CertifiedOlsInference::Iid(value),
            Self::Robust(value) => CertifiedOlsInference::Robust(value),
            Self::Clustered(value) => CertifiedOlsInference::Clustered(value),
        }
    }
}
