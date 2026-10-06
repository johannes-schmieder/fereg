use super::*;
use fereg_core::ols::*;

pub(super) struct PreparedPipeline {
    prepared: PreparedOlsData,
}

pub(super) fn prepare_ols_pipeline(
    loaded: LoadedOlsInput,
    loaded_rhs: Option<Vec<f64>>,
    action: &mut EstimateAction,
    categorical: bool,
) -> Result<(PreparedPipeline, Option<LoadedOlsRows>)> {
    let clusters = std::mem::take(&mut action.clusters);
    let result = prepare_loaded_ols_with_moments(loaded, loaded_rhs, action, |request, input| {
        prepare_model_owned(request, &clusters, input, categorical)
            .map(|prepared| PreparedPipeline { prepared })
    });
    action.clusters = clusters;
    result
}

impl PreparedPipeline {
    pub(super) fn owned_buffer_bytes(&self) -> u128 {
        self.prepared.owned_buffer_bytes()
    }

    pub(super) fn data(&self) -> PreparedOlsResultData<'_> {
        PreparedOlsResultData::new(&self.prepared)
    }

    pub(super) fn clusters(&self) -> Option<PreparedOlsClusteredResultData<'_>> {
        self.prepared
            .cluster_design()
            .map(|_| PreparedOlsClusteredResultData::new(&self.prepared))
    }

    pub(super) fn infer(
        &self,
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
        let p = &self.prepared;
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
