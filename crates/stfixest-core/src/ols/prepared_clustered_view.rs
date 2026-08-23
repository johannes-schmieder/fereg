//! Common cluster-dimension borrowing boundary for prepared OLS.
//!
//! Legacy single-source preparation and categorical-interaction preparation
//! retain different owning OLS artifacts, but expose the same final-sample
//! cluster design. This private view lets the existing clustered engine borrow
//! dimension codes without changing any covariance, correction, PSD, or Wald
//! calculation.

use crate::error::Result;
use crate::prepared_categorical_data::PreparedCategoricalOlsData;

use super::prepared_inference_view::PreparedOlsInferenceView;
use super::{
    PreparedCategoricalOlsClusteredData, PreparedOlsClusterDesign, PreparedOlsClusteredData,
    PreparedOlsData,
};

pub(crate) trait PreparedOlsClusteredView {
    type Ols: PreparedOlsInferenceView + ?Sized;

    fn clustered_ols(&self) -> &Self::Ols;

    fn clustered_design(&self) -> &PreparedOlsClusterDesign;

    fn clustered_codes(&self, dimension: usize) -> Result<&[u32]>;
}

impl PreparedOlsClusteredView for PreparedOlsClusteredData {
    type Ols = PreparedOlsData;

    fn clustered_ols(&self) -> &Self::Ols {
        self.ols()
    }

    fn clustered_design(&self) -> &PreparedOlsClusterDesign {
        self.clusters()
    }

    fn clustered_codes(&self, dimension: usize) -> Result<&[u32]> {
        self.cluster_codes(dimension)
    }
}

impl PreparedOlsClusteredView for PreparedCategoricalOlsClusteredData {
    type Ols = PreparedCategoricalOlsData;

    fn clustered_ols(&self) -> &Self::Ols {
        self.ols()
    }

    fn clustered_design(&self) -> &PreparedOlsClusterDesign {
        self.clusters()
    }

    fn clustered_codes(&self, dimension: usize) -> Result<&[u32]> {
        self.cluster_codes(dimension)
    }
}
