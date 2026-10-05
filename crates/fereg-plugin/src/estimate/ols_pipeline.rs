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
        match (p.uses_categorical_preparation(), action.vce) {
            (false, OlsVceKind::Iid) => {
                fit_prepared_ols_unadjusted_map_with_loaded_moments(p, iid(), rows)
                    .map(OwnedInference::Iid)
            }
            (false, OlsVceKind::Robust) => {
                fit_prepared_ols_robust_map_with_loaded_moments(p, robust(), rows)
                    .map(OwnedInference::Robust)
            }
            (true, OlsVceKind::Iid) => {
                fit_prepared_categorical_ols_unadjusted_map_with_loaded_moments(p, iid(), rows)
                    .map(OwnedInference::CategoricalIid)
            }
            (true, OlsVceKind::Robust) => {
                fit_prepared_categorical_ols_robust_map_with_loaded_moments(p, robust(), rows)
                    .map(OwnedInference::CategoricalRobust)
            }
            (false, OlsVceKind::Clustered) => {
                fit_prepared_ols_clustered_map_with_loaded_moments(p, clustered(), rows)
                    .map(OwnedInference::Clustered)
            }
            (true, OlsVceKind::Clustered) => {
                fit_prepared_categorical_ols_clustered_map_with_loaded_moments(p, clustered(), rows)
                    .map(OwnedInference::CategoricalClustered)
            }
        }
    }
}

pub(super) enum OwnedInference {
    Iid(CertifiedPreparedOlsUnadjusted),
    Robust(CertifiedPreparedOlsRobust),
    CategoricalIid(CertifiedPreparedCategoricalOlsUnadjusted),
    CategoricalRobust(CertifiedPreparedCategoricalOlsRobust),
    Clustered(CertifiedPreparedOlsClustered),
    CategoricalClustered(CertifiedPreparedCategoricalOlsClustered),
}

impl OwnedInference {
    pub(super) fn view(&self) -> CertifiedOlsInference<'_> {
        match self {
            Self::Iid(value) => CertifiedOlsInference::Iid(value),
            Self::Robust(value) => CertifiedOlsInference::Robust(value),
            Self::CategoricalIid(value) => CertifiedOlsInference::CategoricalIid(value),
            Self::CategoricalRobust(value) => CertifiedOlsInference::CategoricalRobust(value),
            Self::Clustered(value) => CertifiedOlsInference::Clustered(value),
            Self::CategoricalClustered(value) => CertifiedOlsInference::CategoricalClustered(value),
        }
    }
}
