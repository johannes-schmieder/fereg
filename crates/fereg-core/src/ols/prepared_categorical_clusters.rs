//! Compatibility entry points; clusters attach to the same finalized owner.
use super::prepared_clusters::{finish_prepare_ols_clusters, validate_request};
use super::{LoadedOlsInput, OlsClusteredRequestV1, PreparedOlsData};
use crate::error::Result;
use crate::prepared_categorical_data::{
    prepare_categorical_ols_data_owned_with_cluster_sources,
    prepare_categorical_ols_data_with_cluster_sources, prepare_ppml_categorical_ols_data,
};

pub type PreparedCategoricalOlsClusteredData = PreparedOlsData;

pub fn prepare_categorical_ols_clustered_data(
    request: &OlsClusteredRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedCategoricalOlsClusteredData> {
    let sources = validate_request(request)?;
    let prepared =
        prepare_categorical_ols_data_with_cluster_sources(&request.ols, loaded, &sources)?;
    finish_prepare_ols_clusters(request, prepared)
}

pub fn prepare_categorical_ols_clustered_data_owned(
    request: &OlsClusteredRequestV1,
    loaded: LoadedOlsInput,
) -> Result<PreparedCategoricalOlsClusteredData> {
    let sources = validate_request(request)?;
    let prepared =
        prepare_categorical_ols_data_owned_with_cluster_sources(&request.ols, loaded, &sources)?;
    finish_prepare_ols_clusters(request, prepared)
}

pub(crate) fn prepare_ppml_categorical_ols_clustered_data(
    request: &OlsClusteredRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedCategoricalOlsClusteredData> {
    let sources = validate_request(request)?;
    let prepared = prepare_ppml_categorical_ols_data(&request.ols, loaded, &sources)?;
    finish_prepare_ols_clusters(request, prepared)
}
