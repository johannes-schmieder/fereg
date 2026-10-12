//! Compatibility entry points; clusters attach to the same finalized owner.
use super::prepared_clusters::{finish_prepare_ols_clusters, validate_request};
use super::{LoadedOlsInput, OlsClusteredRequestV1, PreparedOlsData};
use crate::error::Result;
use crate::ols::prepared_categorical_data::prepare_ppml_categorical_ols_data_with_load_hint;

pub(crate) fn prepare_ppml_categorical_ols_clustered_data(
    request: &OlsClusteredRequestV1,
    loaded: &LoadedOlsInput,
    hint: Option<super::PreparedOlsLoadHint>,
    membership_scratch: u128,
) -> Result<PreparedOlsData> {
    let sources = validate_request(request)?;
    let prepared = prepare_ppml_categorical_ols_data_with_load_hint(
        &request.ols,
        loaded,
        &sources,
        hint,
        membership_scratch,
    )?;
    finish_prepare_ols_clusters(request, prepared)
}
