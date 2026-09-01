use super::{
    DetectionProposal, ExactFeLevelIdentity, ExactFeSeparationCertificate, MethodStatus,
    SeparationMethod,
};
use crate::fe::FeDesign;
use crate::metadata::AbsorbTermId;

pub(crate) fn propose_zero_only_fixed_effect_levels(
    outcome: &[f64],
    design: Option<&FeDesign>,
    active: &[bool],
) -> DetectionProposal {
    let Some(design) = design else {
        return DetectionProposal::empty(
            SeparationMethod::FixedEffects,
            MethodStatus::NoCandidates,
            "no absorbed effects",
        );
    };
    let intercept_effects = design
        .effects()
        .iter()
        .enumerate()
        .filter(|(_, effect)| effect.has_intercept())
        .collect::<Vec<_>>();
    if intercept_effects.is_empty() {
        return DetectionProposal::empty(
            SeparationMethod::FixedEffects,
            MethodStatus::NoCandidates,
            "no absorbed-intercept effects",
        );
    }

    let positive_by_effect = intercept_effects
        .iter()
        .map(|(_, effect)| {
            let mut positive = vec![false; effect.n_levels()];
            for row in 0..outcome.len() {
                if active[row] && outcome[row] > 0.0 {
                    positive[effect.codes()[row] as usize] = true;
                }
            }
            positive
        })
        .collect::<Vec<_>>();

    let mut zero_only_levels = Vec::new();
    for (position, (effect_index, effect)) in intercept_effects.iter().enumerate() {
        let mut active_in_level = vec![false; effect.n_levels()];
        for (&is_active, &code) in active.iter().zip(effect.codes()) {
            if is_active {
                active_in_level[code as usize] = true;
            }
        }
        for (level, &level_is_active) in active_in_level.iter().enumerate() {
            if level_is_active && !positive_by_effect[position][level] {
                let representative_original_row = active
                    .iter()
                    .zip(effect.codes())
                    .enumerate()
                    .find_map(|(row, (&is_active, &code))| {
                        (is_active && code as usize == level).then_some(row)
                    })
                    .expect("active exact-FE level has a representative row");
                zero_only_levels.push(ExactFeLevelIdentity {
                    effect: AbsorbTermId::new((*effect_index + 1) as u32)
                        .expect("test-oracle effect ordinal is one-based"),
                    representative_original_row,
                });
            }
        }
    }
    let dropped = (0..outcome.len())
        .filter(|&row| {
            active[row]
                && outcome[row] == 0.0
                && intercept_effects
                    .iter()
                    .enumerate()
                    .any(|(index, (_, effect))| {
                        !positive_by_effect[index][effect.codes()[row] as usize]
                    })
        })
        .collect::<Vec<_>>();
    let status = if dropped.is_empty() {
        MethodStatus::NoSeparation
    } else {
        MethodStatus::Certified
    };
    DetectionProposal {
        method: SeparationMethod::FixedEffects,
        dropped,
        iterations: 1,
        status,
        certificate: None,
        exact_fe_certificate: Some(ExactFeSeparationCertificate {
            checked_intercept_effects: intercept_effects.len(),
            zero_only_levels,
            passed: true,
        }),
        direction: None,
        note: "exact zero-only absorbed-intercept levels".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{EffectTerm, FeDesign};

    #[test]
    fn finds_zero_only_intercept_levels() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("effect");
        let design = FeDesign::new(6, vec![effect]).expect("design");
        let proposal = propose_zero_only_fixed_effect_levels(
            &[0.0, 0.0, 0.0, 1.0, 2.0, 3.0],
            Some(&design),
            &[true; 6],
        );
        assert_eq!(proposal.dropped, vec![0, 1]);
        assert_eq!(proposal.status, MethodStatus::Certified);
        assert_eq!(
            proposal
                .exact_fe_certificate
                .expect("exact certificate")
                .zero_only_levels,
            vec![ExactFeLevelIdentity {
                effect: AbsorbTermId::new(1).expect("effect identity"),
                representative_original_row: 0,
            }]
        );
    }

    #[test]
    fn slope_only_terms_do_not_prove_zero_level_separation() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, false, vec![vec![0.0, 1.0, 0.0, 1.0]])
            .expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        assert!(propose_zero_only_fixed_effect_levels(
            &[0.0, 0.0, 1.0, 2.0],
            Some(&design),
            &[true; 4],
        )
        .dropped
        .is_empty());
    }
}
