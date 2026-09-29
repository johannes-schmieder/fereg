//! Reconstruct certificate evidence from row codes, independently of detector CSR scans.
use super::{invalid, ExactCertificate, ExactLevel, ExactProposal, ExactStatus, View};
use crate::{
    data::Prepared,
    error::{reserved, Result},
};

pub(super) fn proposal(view: View<'_>, proposal: &ExactProposal) -> Result<()> {
    if proposal.generation != view.generation
        || proposal.iterations != 1
        || proposal.rows.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(
            "invalid exact FE proposal generation, iterations or row order",
        ));
    }
    let expected_status = if proposal.certificate.checked_intercept_effects == 0 {
        ExactStatus::NoCandidates
    } else if proposal.rows.is_empty() {
        ExactStatus::NoSeparation
    } else {
        ExactStatus::Certified
    };
    if proposal.status != expected_status {
        return Err(invalid("inconsistent exact FE proposal status"));
    }
    let mut selected = reserved(view.sample.n_active())?;
    selected.resize(view.sample.n_active(), false);
    for &original in &proposal.rows {
        let row = view
            .sample
            .retained_rows()
            .binary_search(&original)
            .map_err(|_| invalid("separation row is absent from this generation"))?;
        if view.sample.outcome[row] != 0. {
            return Err(invalid("exact FE proposal includes a positive outcome"));
        }
        selected[row] = true;
    }
    let mut active = reserved(view.sample.n_active())?;
    active.resize(view.sample.n_active(), true);
    certificate(view.sample, &active, &selected, &proposal.certificate)
}

pub(super) fn certificate(
    sample: &Prepared,
    active: &[bool],
    selected: &[bool],
    certificate: &ExactCertificate,
) -> Result<()> {
    let identities = &certificate.zero_only_levels;
    if active.len() != sample.n_active()
        || selected.len() != active.len()
        || identities
            .windows(2)
            .any(|pair| pair[0].key() >= pair[1].key())
    {
        return Err(invalid(
            "invalid exact FE certificate dimensions or stable identities",
        ));
    }
    let mut replay = reserved(active.len())?;
    replay.resize(active.len(), false);
    let mut checked = 0;
    let mut zero_levels = 0;
    if let Some(topology) = &sample.topology {
        for (index, effect) in topology.effects().iter().enumerate() {
            if !effect.intercept {
                continue;
            }
            checked += 1;
            let partition = topology.effect_partition(index)?;
            let mut anchors = reserved(partition.n_levels())?;
            anchors.resize(partition.n_levels(), usize::MAX);
            let mut positive = reserved(partition.n_levels())?;
            positive.resize(partition.n_levels(), false);
            for (row, &code) in partition.codes().iter().enumerate() {
                if active[row] {
                    anchors[code as usize] =
                        anchors[code as usize].min(sample.retained_rows()[row]);
                    positive[code as usize] |= sample.outcome[row] > 0.;
                }
            }
            for (level, &anchor) in anchors.iter().enumerate() {
                if anchor != usize::MAX && !positive[level] {
                    zero_levels += 1;
                    let key = ExactLevel {
                        effect: effect.term_id,
                        representative_original_row: anchor,
                    }
                    .key();
                    if identities
                        .binary_search_by_key(&key, |identity| identity.key())
                        .is_err()
                    {
                        return Err(invalid(
                            "exact FE certificate omits an active zero-only level",
                        ));
                    }
                }
            }
            for (row, &code) in partition.codes().iter().enumerate() {
                replay[row] |= active[row] && !positive[code as usize];
            }
        }
    }
    if checked != certificate.checked_intercept_effects
        || zero_levels != identities.len()
        || replay != selected
    {
        return Err(invalid(
            "exact FE certificate counts or complete row union failed replay",
        ));
    }
    Ok(())
}
