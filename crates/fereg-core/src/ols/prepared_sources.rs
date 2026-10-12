//! Finalized numeric ownership and optional release of redundant FE columns.

use super::{OlsRequestV1, PreparedOlsColumnKind, PreparedOlsData};
use crate::data::{NumericSourceBank, PreparedNumericSample, SealedNumericSources};
use crate::memory::vec_capacity_bytes;
use crate::metadata::SourceId;

/// Finalized source slots retain their original source indices. Numeric access
/// returns None for an encoded-only FE source; it never reconstructs values.
#[derive(Debug, PartialEq)]
pub struct PreparedSourceBank(Storage);

#[derive(Debug, PartialEq)]
enum Storage {
    Raw(Vec<Vec<f64>>),
    Slots(Vec<SourceSlot>),
}

#[derive(Debug, PartialEq)]
enum SourceSlot {
    Raw(Vec<f64>),
    Encoded { categorical: usize, rows: usize },
}

impl PreparedOlsData {
    /// Original retained ownership used by route and output admission.
    #[must_use]
    pub fn execution_admission_bytes(&self) -> u128 {
        self.execution_admission_bytes
            .unwrap_or_else(|| self.owned_buffer_bytes())
    }

    pub(super) fn release_fe_columns(&mut self, request: &OlsRequestV1, ceiling: u128) {
        if let Some(plan) = release_plan(self, request, ceiling) {
            let original = self.owned_buffer_bytes();
            self.sample.release(plan);
            self.execution_admission_bytes = Some(original);
        }
    }
}

pub type FinalizedNumericSample = PreparedNumericSample<PreparedSourceBank>;

impl PreparedSourceBank {
    pub(super) fn raw(columns: Vec<Vec<f64>>) -> Self {
        Self(Storage::Raw(columns))
    }

    pub fn len(&self) -> usize {
        match &self.0 {
            Storage::Raw(columns) => columns.len(),
            Storage::Slots(slots) => slots.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<&Vec<f64>> {
        match &self.0 {
            Storage::Raw(columns) => columns.get(index),
            Storage::Slots(slots) => match slots.get(index)? {
                SourceSlot::Raw(values) => Some(values),
                SourceSlot::Encoded { .. } => None,
            },
        }
    }

    /// Iterate retained numeric columns only; positions in this iterator are not source indices.
    pub fn raw_columns(&self) -> impl Iterator<Item = &Vec<f64>> {
        (0..self.len()).filter_map(|index| self.get(index))
    }

    pub(super) fn encoded_matches(&self, index: usize, categorical: usize, rows: usize) -> bool {
        matches!(&self.0, Storage::Slots(slots) if matches!(slots.get(index),
            Some(SourceSlot::Encoded { categorical: owner, rows: length })
                if *owner == categorical && *length == rows))
    }

    pub(super) fn release(&mut self, mut plan: ReleasePlan) {
        let Storage::Raw(columns) = &mut self.0 else {
            return;
        };
        for (column, slot) in std::mem::take(columns).into_iter().zip(&mut plan.slots) {
            if let SourceSlot::Raw(destination) = slot {
                *destination = column;
            }
        }
        self.0 = Storage::Slots(plan.slots);
    }
}

impl SealedNumericSources for PreparedSourceBank {}
impl NumericSourceBank for PreparedSourceBank {
    fn len(&self) -> usize {
        self.len()
    }
    fn get(&self, index: usize) -> Option<&Vec<f64>> {
        self.get(index)
    }
    fn owned_buffer_bytes(&self) -> u128 {
        match &self.0 {
            Storage::Raw(columns) => NumericSourceBank::owned_buffer_bytes(columns),
            Storage::Slots(slots) => {
                vec_capacity_bytes(slots) + self.raw_columns().map(vec_capacity_bytes).sum::<u128>()
            }
        }
    }
    fn aligned(&self, rows: usize) -> bool {
        match &self.0 {
            Storage::Raw(columns) => columns.aligned(rows),
            Storage::Slots(slots) => slots.iter().all(|slot| match slot {
                SourceSlot::Raw(values) => values.len() == rows,
                SourceSlot::Encoded { rows: length, .. } => *length == rows,
            }),
        }
    }
}

pub(super) struct ReleasePlan {
    slots: Vec<SourceSlot>,
}

crate::fault::fault_plan! { static RESERVATION_FAULT: u8 = 0; }

pub(super) fn release_plan(
    prepared: &PreparedOlsData,
    request: &OlsRequestV1,
    ceiling: u128,
) -> Option<ReleasePlan> {
    let bank = &prepared.sample().regressors;
    let Storage::Raw(columns) = &bank.0 else {
        return None;
    };
    let original = prepared.owned_buffer_bytes();
    let requested =
        (columns.len() as u128).checked_mul(std::mem::size_of::<SourceSlot>() as u128)?;
    if original.checked_add(requested)? > ceiling {
        return None;
    }
    let fault = RESERVATION_FAULT.replace(0);
    if fault == 1 {
        return None;
    }
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(columns.len().checked_add(if fault == 2 { 1 } else { 0 })?)
        .ok()?;
    let descriptors = vec_capacity_bytes(&slots);
    if original.checked_add(descriptors)? > ceiling {
        return None;
    }
    let mut released = 0_u128;
    for (index, values) in columns.iter().enumerate() {
        let source = SourceId::new(u32::try_from(index).ok()?.checked_add(1)?).ok()?;
        let categorical = prepared
            .categorical_sources()
            .iter()
            .position(|entry| entry.source() == source);
        if eligible(prepared, request, source) {
            let categorical = categorical?;
            if !canonical_correspondence(prepared, categorical, index, values) {
                return None;
            }
            released = released.checked_add(vec_capacity_bytes(values))?;
            slots.push(SourceSlot::Encoded {
                categorical,
                rows: values.len(),
            });
        } else {
            slots.push(SourceSlot::Raw(Vec::new()));
        }
    }
    if released == 0 || descriptors > vec_capacity_bytes(columns).checked_add(released)? {
        return None;
    }
    Some(ReleasePlan { slots })
}

fn eligible(prepared: &PreparedOlsData, request: &OlsRequestV1, source: SourceId) -> bool {
    let pure = |sources: &[SourceId], intercept: bool, slopes: bool| {
        sources == [source] && intercept && !slopes
    };
    request.absorbed_terms.iter().any(|term| {
        pure(
            &term.categorical_sources,
            term.intercept,
            !term.slope_sources.is_empty(),
        )
    }) && !request.model.terms.iter().any(|term| {
        term.components
            .iter()
            .any(|component| component.source == source)
    }) && request.absorbed_terms.iter().all(|term| {
        !term.slope_sources.contains(&source)
            && (!term.categorical_sources.contains(&source)
                || pure(
                    &term.categorical_sources,
                    term.intercept,
                    !term.slope_sources.is_empty(),
                ))
    }) && prepared.effect_specs().iter().all(|effect| {
        !effect.slopes.iter().any(|slope| slope.source == source)
            && (!effect.sources.contains(&source)
                || pure(&effect.sources, effect.intercept, !effect.slopes.is_empty()))
    }) && !prepared
        .model()
        .columns
        .iter()
        .any(|column| match &column.kind {
            PreparedOlsColumnKind::Continuous { source: used, .. }
            | PreparedOlsColumnKind::Categorical { source: used, .. } => *used == source,
            PreparedOlsColumnKind::CategoricalInteraction { components } => components
                .iter()
                .any(|component| component.source == source),
            PreparedOlsColumnKind::Intercept => false,
        })
        && prepared.cluster_design().is_none_or(|design| {
            design
                .dimensions()
                .iter()
                .all(|dimension| !dimension.sources().contains(&source))
        })
}

fn canonical_correspondence(
    prepared: &PreparedOlsData,
    categorical: usize,
    index: usize,
    raw: &[f64],
) -> bool {
    let entry = &prepared.categorical_sources()[categorical];
    let Ok(codes) = entry.codes(prepared.fe_topology()) else {
        return false;
    };
    let levels = entry.level_values();
    if entry.prepared_source_index() != index
        || raw.len() != prepared.sample().n_active()
        || codes.len() != raw.len()
        || entry.n_levels(prepared.fe_topology()).ok() != Some(levels.len())
        || levels.len() > u32::MAX as usize
        || levels
            .iter()
            .any(|value| !value.is_finite() || (*value == 0.0 && value.to_bits() != 0))
        || levels
            .windows(2)
            .any(|pair| pair[0].total_cmp(&pair[1]).is_ge())
    {
        return false;
    }
    crate::parallel::all_row_chunks(raw.len(), raw.len() as u128, |range| {
        crate::interrupt::checkpoint().is_ok()
            && range.into_iter().all(|row| {
                let value = if raw[row] == 0.0 { 0.0 } else { raw[row] };
                raw[row].is_finite()
                    && levels
                        .get(codes[row] as usize)
                        .is_some_and(|level| level.to_bits() == value.to_bits())
            })
    })
}

// Mutation remains confined to malformed-owner unit fixtures, as for the
// enclosing sealed sample. Production builds expose no mutable bank access.
#[cfg(test)]
impl std::ops::Deref for PreparedSourceBank {
    type Target = Vec<Vec<f64>>;
    fn deref(&self) -> &Self::Target {
        match &self.0 {
            Storage::Raw(columns) => columns,
            Storage::Slots(_) => panic!("fixture requires raw columns"),
        }
    }
}
#[cfg(test)]
impl std::ops::DerefMut for PreparedSourceBank {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.0 {
            Storage::Raw(columns) => columns,
            Storage::Slots(_) => panic!("fixture requires raw columns"),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
