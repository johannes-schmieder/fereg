use crate::fe::FeDesign;

pub(crate) fn zero_only_fixed_effect_levels(
    outcome: &[f64],
    design: Option<&FeDesign>,
    active: &[bool],
) -> Vec<usize> {
    let Some(design) = design else {
        return Vec::new();
    };
    let intercept_effects = design
        .effects()
        .iter()
        .filter(|effect| effect.has_intercept())
        .collect::<Vec<_>>();
    if intercept_effects.is_empty() {
        return Vec::new();
    }

    let positive_by_effect = intercept_effects
        .iter()
        .map(|effect| {
            let mut positive = vec![false; effect.n_levels()];
            for row in 0..outcome.len() {
                if active[row] && outcome[row] > 0.0 {
                    positive[effect.codes()[row] as usize] = true;
                }
            }
            positive
        })
        .collect::<Vec<_>>();

    (0..outcome.len())
        .filter(|&row| {
            active[row]
                && outcome[row] == 0.0
                && intercept_effects
                    .iter()
                    .enumerate()
                    .any(|(index, effect)| !positive_by_effect[index][effect.codes()[row] as usize])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{EffectTerm, FeDesign};

    #[test]
    fn finds_zero_only_intercept_levels() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("effect");
        let design = FeDesign::new(6, vec![effect]).expect("design");
        let dropped = zero_only_fixed_effect_levels(
            &[0.0, 0.0, 0.0, 1.0, 2.0, 3.0],
            Some(&design),
            &[true; 6],
        );
        assert_eq!(dropped, vec![0, 1]);
    }

    #[test]
    fn slope_only_terms_do_not_prove_zero_level_separation() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, false, vec![vec![0.0, 1.0, 0.0, 1.0]])
            .expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        assert!(
            zero_only_fixed_effect_levels(&[0.0, 0.0, 1.0, 2.0], Some(&design), &[true; 4],)
                .is_empty()
        );
    }
}
