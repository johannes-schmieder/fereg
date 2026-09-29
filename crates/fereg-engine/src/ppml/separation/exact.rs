use super::{invalid, verify, View};
use crate::{
    data::Prepared,
    error::{reserved, Result},
    AbsorbTermId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactStatus {
    NoCandidates,
    NoSeparation,
    Certified,
}

/// A representative original row identifies a level across compact-code changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactLevel {
    pub effect: Option<AbsorbTermId>,
    pub representative_original_row: usize,
}
impl ExactLevel {
    pub(super) fn key(self) -> (Option<u32>, usize) {
        (
            self.effect.map(AbsorbTermId::get),
            self.representative_original_row,
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactCertificate {
    pub checked_intercept_effects: usize,
    pub zero_only_levels: Vec<ExactLevel>,
}
/// Internal detector evidence, never an observation-sized public result table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactProposal {
    pub generation: usize,
    pub rows: Vec<usize>,
    pub status: ExactStatus,
    pub iterations: usize,
    pub certificate: ExactCertificate,
}

impl View<'_> {
    pub fn exact_proposal(self) -> Result<ExactProposal> {
        let mut active = reserved(self.sample.n_active())?;
        active.resize(self.sample.n_active(), true);
        let (mask, certificate) = detect(self.sample, &active)?;
        let mut rows = reserved(mask.iter().filter(|&&drop| drop).count())?;
        rows.extend(
            mask.iter()
                .zip(self.sample.retained_rows())
                .filter_map(|(&drop, &row)| drop.then_some(row)),
        );
        let status = if certificate.checked_intercept_effects == 0 {
            ExactStatus::NoCandidates
        } else if rows.is_empty() {
            ExactStatus::NoSeparation
        } else {
            ExactStatus::Certified
        };
        Ok(ExactProposal {
            generation: self.generation,
            rows,
            status,
            iterations: 1,
            certificate,
        })
    }
}

pub(super) fn detect(sample: &Prepared, active: &[bool]) -> Result<(Vec<bool>, ExactCertificate)> {
    if active.len() != sample.n_active() {
        return Err(invalid("exact FE mask is not sample aligned"));
    }
    let mut drops = reserved(active.len())?;
    drops.resize(active.len(), false);
    let mut certificate = ExactCertificate {
        checked_intercept_effects: 0,
        zero_only_levels: Vec::new(),
    };
    let Some(topology) = &sample.topology else {
        return Ok((drops, certificate));
    };
    let mut count = 0usize;
    for (index, effect) in topology.effects().iter().enumerate() {
        if effect.intercept {
            count = count
                .checked_add(topology.effect_partition(index)?.n_levels())
                .ok_or_else(|| {
                    crate::FeregError::new(
                        crate::ErrorCode::Allocation,
                        "exact FE certificate capacity overflows",
                    )
                })?;
        }
    }
    certificate.zero_only_levels = reserved(count)?;
    for (index, effect) in topology.effects().iter().enumerate() {
        if !effect.intercept {
            continue;
        }
        certificate.checked_intercept_effects += 1;
        let partition = topology.effect_partition(index)?;
        for level in 0..partition.n_levels() {
            let members = partition.membership().members(level)?;
            let mut representative = None;
            let mut positive = false;
            for &row in members {
                if active[row] {
                    representative.get_or_insert(sample.retained_rows()[row]);
                    positive |= sample.outcome[row] > 0.;
                }
            }
            if let Some(representative_original_row) = representative.filter(|_| !positive) {
                certificate.zero_only_levels.push(ExactLevel {
                    effect: effect.term_id,
                    representative_original_row,
                });
                for &row in members {
                    drops[row] |= active[row];
                }
            }
        }
    }
    certificate
        .zero_only_levels
        .sort_unstable_by_key(|level| level.key());
    verify::certificate(sample, active, &drops, &certificate)?;
    Ok((drops, certificate))
}
