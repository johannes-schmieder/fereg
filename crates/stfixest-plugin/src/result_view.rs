//! Borrowed prepared-result views for the hidden OLS transport.
//!
//! Legacy single-source and categorical-interaction preparation own different
//! topology artifacts but expose the same final model, sample, singleton, and
//! cluster metadata. These copyable views let one serializer borrow either path
//! without cloning observation-scale state or changing result-wire dimensions.

use stfixest_core::{
    PreparedCategoricalOlsClusteredData, PreparedCategoricalOlsData, PreparedNumericSample,
    PreparedOlsClusterDesign, PreparedOlsClusteredData, PreparedOlsData, PreparedOlsModel,
    PreparedOlsSingletons,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum PreparedOlsResultData<'a> {
    Legacy(&'a PreparedOlsData),
    Categorical(&'a PreparedCategoricalOlsData),
}

impl<'a> PreparedOlsResultData<'a> {
    pub(crate) const fn legacy(value: &'a PreparedOlsData) -> Self {
        Self::Legacy(value)
    }

    pub(crate) const fn categorical(value: &'a PreparedCategoricalOlsData) -> Self {
        Self::Categorical(value)
    }

    pub(crate) const fn model(self) -> &'a PreparedOlsModel {
        match self {
            Self::Legacy(value) => &value.model,
            Self::Categorical(value) => value.model(),
        }
    }

    pub(crate) const fn sample(self) -> &'a PreparedNumericSample {
        match self {
            Self::Legacy(value) => &value.sample,
            Self::Categorical(value) => value.sample(),
        }
    }

    pub(crate) const fn singletons(self) -> PreparedOlsSingletons {
        match self {
            Self::Legacy(value) => value.singletons,
            Self::Categorical(value) => value.singletons(),
        }
    }

    /// Physical coefficient capacity allocated by Stata before the native
    /// final sample is known. Categorical preparation may report a smaller
    /// logical graph after singleton deletion removes complete factor levels.
    pub(crate) fn coefficient_capacity(self) -> usize {
        match self {
            Self::Legacy(value) => value.model.coefficients.len(),
            Self::Categorical(value) => value.report_plan().original_coefficient_count(),
        }
    }

    fn is_same_legacy(self, value: &'a PreparedOlsData) -> bool {
        match self {
            Self::Legacy(prepared) => std::ptr::eq(prepared, value),
            Self::Categorical(_) => false,
        }
    }

    fn is_same_categorical(self, value: &'a PreparedCategoricalOlsData) -> bool {
        match self {
            Self::Legacy(_) => false,
            Self::Categorical(prepared) => std::ptr::eq(prepared, value),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PreparedOlsClusteredResultData<'a> {
    Legacy(&'a PreparedOlsClusteredData),
    Categorical(&'a PreparedCategoricalOlsClusteredData),
}

impl<'a> PreparedOlsClusteredResultData<'a> {
    pub(crate) const fn legacy(value: &'a PreparedOlsClusteredData) -> Self {
        Self::Legacy(value)
    }

    pub(crate) const fn categorical(value: &'a PreparedCategoricalOlsClusteredData) -> Self {
        Self::Categorical(value)
    }

    pub(crate) const fn ols(self) -> PreparedOlsResultData<'a> {
        match self {
            Self::Legacy(value) => PreparedOlsResultData::Legacy(value.ols()),
            Self::Categorical(value) => PreparedOlsResultData::Categorical(value.ols()),
        }
    }

    pub(crate) const fn clusters(self) -> &'a PreparedOlsClusterDesign {
        match self {
            Self::Legacy(value) => value.clusters(),
            Self::Categorical(value) => value.clusters(),
        }
    }

    pub(crate) fn is_bound_to(self, prepared: PreparedOlsResultData<'a>) -> bool {
        match self.ols() {
            PreparedOlsResultData::Legacy(value) => prepared.is_same_legacy(value),
            PreparedOlsResultData::Categorical(value) => prepared.is_same_categorical(value),
        }
    }
}
