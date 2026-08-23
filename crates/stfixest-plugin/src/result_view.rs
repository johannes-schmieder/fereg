//! Borrowed prepared-result views for the hidden OLS transport.
//!
//! The first checkpoint deliberately contains only the legacy prepared path.
//! Centralizing these accesses lets a later bounded change add categorical-
//! interaction preparation without cloning observation-scale state or
//! duplicating the result serializer.

use stfixest_core::{
    PreparedNumericSample, PreparedOlsClusterDesign, PreparedOlsClusteredData, PreparedOlsData,
    PreparedOlsModel, PreparedOlsSingletons,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum PreparedOlsResultData<'a> {
    Legacy(&'a PreparedOlsData),
}

impl<'a> PreparedOlsResultData<'a> {
    pub(crate) const fn legacy(value: &'a PreparedOlsData) -> Self {
        Self::Legacy(value)
    }

    pub(crate) const fn model(self) -> &'a PreparedOlsModel {
        match self {
            Self::Legacy(value) => &value.model,
        }
    }

    pub(crate) const fn sample(self) -> &'a PreparedNumericSample {
        match self {
            Self::Legacy(value) => &value.sample,
        }
    }

    pub(crate) const fn singletons(self) -> PreparedOlsSingletons {
        match self {
            Self::Legacy(value) => value.singletons,
        }
    }

    pub(crate) const fn is_same_legacy(self, value: &'a PreparedOlsData) -> bool {
        match self {
            Self::Legacy(prepared) => std::ptr::eq(prepared, value),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PreparedOlsClusteredResultData<'a> {
    Legacy(&'a PreparedOlsClusteredData),
}

impl<'a> PreparedOlsClusteredResultData<'a> {
    pub(crate) const fn legacy(value: &'a PreparedOlsClusteredData) -> Self {
        Self::Legacy(value)
    }

    pub(crate) const fn ols(self) -> PreparedOlsResultData<'a> {
        match self {
            Self::Legacy(value) => PreparedOlsResultData::Legacy(value.ols()),
        }
    }

    pub(crate) const fn clusters(self) -> &'a PreparedOlsClusterDesign {
        match self {
            Self::Legacy(value) => value.clusters(),
        }
    }

    pub(crate) const fn is_bound_to(self, prepared: PreparedOlsResultData<'a>) -> bool {
        match self {
            Self::Legacy(value) => prepared.is_same_legacy(value.ols()),
        }
    }
}
