use super::*;
use fereg_core::ols::*;

pub(super) enum PreparedPipeline {
    Ordinary(PreparedOlsData),
    Categorical(PreparedCategoricalOlsData),
    Clustered(PreparedOlsClusteredData),
    CategoricalClustered(PreparedCategoricalOlsClusteredData),
}

pub(super) fn prepare_ols_pipeline(
    loaded: LoadedOlsInput,
    loaded_rhs: Option<Vec<f64>>,
    action: &mut EstimateAction,
    categorical: bool,
) -> Result<(PreparedPipeline, Option<LoadedOlsRows>)> {
    let clustered = action.vce == OlsVceKind::Clustered;
    match (categorical, clustered) {
        (false, false) => {
            prepare_loaded_ols_with_moments(loaded, loaded_rhs, action, |request, input| {
                prepare_ols_data_owned(request, input).map(PreparedPipeline::Ordinary)
            })
        }
        (true, false) => {
            prepare_loaded_ols_with_moments(loaded, loaded_rhs, action, |request, input| {
                prepare_categorical_ols_data_owned(request, input)
                    .map(PreparedPipeline::Categorical)
            })
        }
        (_, true) => {
            let request = OlsClusteredRequestV1 {
                ols: action.request.clone(),
                clusters: action.clusters.clone(),
            };
            let result = prepare_loaded_ols_with_moments(loaded, loaded_rhs, action, |_, input| {
                if categorical {
                    prepare_categorical_ols_clustered_data_owned(&request, input)
                        .map(PreparedPipeline::CategoricalClustered)
                } else {
                    prepare_ols_clustered_data_owned(&request, input)
                        .map(PreparedPipeline::Clustered)
                }
            });
            drop(request);
            result
        }
    }
}

impl PreparedPipeline {
    pub(super) fn owned_buffer_bytes(&self) -> u128 {
        match self {
            Self::Ordinary(p) => p.owned_buffer_bytes(),
            Self::Categorical(p) => p.owned_buffer_bytes(),
            Self::Clustered(p) => p.ols().owned_buffer_bytes() + p.clusters().owned_buffer_bytes(),
            Self::CategoricalClustered(p) => {
                p.ols().owned_buffer_bytes() + p.clusters().owned_buffer_bytes()
            }
        }
    }
    pub(super) fn data(&self) -> PreparedOlsResultData<'_> {
        match self {
            Self::Ordinary(p) => PreparedOlsResultData::legacy(p),
            Self::Categorical(p) => PreparedOlsResultData::categorical(p),
            Self::Clustered(p) => PreparedOlsResultData::legacy(p.ols()),
            Self::CategoricalClustered(p) => PreparedOlsResultData::categorical(p.ols()),
        }
    }
    pub(super) fn clusters(&self) -> Option<PreparedOlsClusteredResultData<'_>> {
        match self {
            Self::Clustered(p) => Some(PreparedOlsClusteredResultData::legacy(p)),
            Self::CategoricalClustered(p) => Some(PreparedOlsClusteredResultData::categorical(p)),
            _ => None,
        }
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
        match (self, action.vce) {
            (Self::Ordinary(p), OlsVceKind::Iid) => {
                fit_prepared_ols_unadjusted_map_with_loaded_moments(p, iid(), rows)
                    .map(OwnedInference::Iid)
            }
            (Self::Ordinary(p), OlsVceKind::Robust) => {
                fit_prepared_ols_robust_map_with_loaded_moments(p, robust(), rows)
                    .map(OwnedInference::Robust)
            }
            (Self::Categorical(p), OlsVceKind::Iid) => {
                fit_prepared_categorical_ols_unadjusted_map_with_loaded_moments(p, iid(), rows)
                    .map(OwnedInference::CategoricalIid)
            }
            (Self::Categorical(p), OlsVceKind::Robust) => {
                fit_prepared_categorical_ols_robust_map_with_loaded_moments(p, robust(), rows)
                    .map(OwnedInference::CategoricalRobust)
            }
            (Self::Clustered(p), OlsVceKind::Clustered) => {
                fit_prepared_ols_clustered_map_with_loaded_moments(p, clustered(), rows)
                    .map(OwnedInference::Clustered)
            }
            (Self::CategoricalClustered(p), OlsVceKind::Clustered) => {
                fit_prepared_categorical_ols_clustered_map_with_loaded_moments(p, clustered(), rows)
                    .map(OwnedInference::CategoricalClustered)
            }
            _ => unreachable!("preparation disposition binds clustered inference"),
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
