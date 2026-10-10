//! Compatibility entry points; clusters attach to the same finalized owner.
use super::prepared_clusters::{finish_prepare_ols_clusters, validate_request};
use super::{LoadedOlsInput, OlsClusteredRequestV1, PreparedOlsData};
use crate::error::Result;
use crate::prepared_categorical_data::prepare_ppml_categorical_ols_data;

pub type PreparedCategoricalOlsClusteredData = PreparedOlsData;

pub(crate) fn prepare_ppml_categorical_ols_clustered_data(
    request: &OlsClusteredRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedCategoricalOlsClusteredData> {
    let sources = validate_request(request)?;
    let prepared = prepare_ppml_categorical_ols_data(&request.ols, loaded, &sources)?;
    finish_prepare_ols_clusters(request, prepared)
}
