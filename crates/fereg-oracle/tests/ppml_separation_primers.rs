#![deny(clippy::too_many_lines)]
//! Bounded separation primers through the public prepared PPML entry.
//!
//! The same primers feed `tests/python/test_ppml_recession_cone_primers.py`,
//! where an exact-rational recession-cone oracle independently certifies the
//! first-round separated rows. These replace the former in-crate dense
//! separation runner, which reused production absorption and rank code.
#[allow(dead_code)]
mod support;

use fereg_core::ppml::fit_prepared_ppml;
use fereg_core::{DropReason, ErrorCode, SeparationConfig, SeparationMethod, SeparationMethods};
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_ols::DummyEffect;

const PRIMERS: &str = include_str!("../../../tests/fixtures/ppml/separation_primers.txt");

#[derive(Debug, Default)]
struct Primer {
    name: String,
    outcome: Vec<f64>,
    regressors: Vec<Vec<f64>>,
    effects: Vec<Vec<u32>>,
    methods: Vec<String>,
    simplex_cells: Option<usize>,
    drop_singletons: bool,
    dropped: Vec<usize>,
    reasons: Vec<u8>,
}

fn numbers<T: std::str::FromStr>(text: &str) -> Vec<T>
where
    T::Err: std::fmt::Debug,
{
    text.split_whitespace()
        .map(|item| item.parse().expect("number"))
        .collect()
}

fn primers() -> Vec<Primer> {
    let mut primers: Vec<Primer> = Vec::new();
    for line in PRIMERS.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            primers.push(Primer {
                name: name.to_owned(),
                ..Primer::default()
            });
            continue;
        }
        let (key, value) = line.split_once('=').expect("key = value");
        let primer = primers.last_mut().expect("primer header");
        match key.trim() {
            "outcome" => primer.outcome = numbers(value),
            "regressor" => primer.regressors.push(numbers(value)),
            "effect" => primer.effects.push(numbers(value)),
            "methods" => primer.methods = value.split_whitespace().map(str::to_owned).collect(),
            "simplex_cells" => primer.simplex_cells = Some(value.trim().parse().expect("cells")),
            "drop_singletons" => primer.drop_singletons = value.trim() == "1",
            "dropped" => primer.dropped = numbers(value),
            "reasons" => primer.reasons = numbers(value),
            "cone" => {}
            other => panic!("unknown primer key {other}"),
        }
    }
    primers
}

fn design(primer: &Primer) -> Design {
    let rows = primer.outcome.len();
    Design {
        outcome: primer.outcome.clone(),
        regressors: primer.regressors.clone(),
        effects: primer
            .effects
            .iter()
            .map(|codes| {
                let levels = codes.iter().max().map_or(1, |&max| max as usize + 1);
                DummyEffect::new(codes.clone(), levels, true, vec![]).expect("effect")
            })
            .collect(),
        weight_kind: WeightKind::Unit,
        weights: vec![1.0; rows],
        missing: vec![false; rows],
        clusters: vec![],
    }
}

fn separation(primer: &Primer, simplex_cells: Option<usize>) -> SeparationConfig {
    let has = |name: &str| primer.methods.iter().any(|method| method == name);
    let mut config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: has("fe"),
            simplex: has("simplex"),
            iterated_rectifier: has("ir"),
            mu: false,
        },
        drop_singletons: primer.drop_singletons,
        ..SeparationConfig::default()
    };
    if let Some(cells) = simplex_cells {
        config.simplex_max_tableau_cells = cells;
    }
    config
}

#[test]
fn public_fit_reproduces_every_primer_drop_set_and_reason() {
    let primers = primers();
    assert!(
        primers.len() >= 6,
        "primer file parsed {} blocks",
        primers.len()
    );
    for primer in &primers {
        let (request, loaded) = support::ppml::request(
            &design(primer),
            None,
            !primer.drop_singletons,
            separation(primer, primer.simplex_cells),
        );
        let fit = fit_prepared_ppml(request, loaded)
            .unwrap_or_else(|error| panic!("{}: {error}", primer.name));
        let reasons = fit.sample().drop_reasons();
        let dropped: Vec<usize> = (0..reasons.len())
            .filter(|&row| reasons[row] != DropReason::Retained)
            .collect();
        assert_eq!(dropped, primer.dropped, "{}", primer.name);
        let codes: Vec<u8> = dropped.iter().map(|&row| reasons[row].code()).collect();
        assert_eq!(codes, primer.reasons, "{}", primer.name);
        if primer.simplex_cells.is_some() {
            assert!(
                fit.separation()
                    .methods
                    .iter()
                    .any(|entry| entry.method == SeparationMethod::Simplex
                        && format!("{:?}", entry.status) == "SkippedMemory"),
                "{}: simplex was not skipped for memory",
                primer.name
            );
        }
    }
}

#[test]
fn no_separation_repeats_generation_identity() {
    let primer = primers()
        .into_iter()
        .find(|primer| primer.name == "no-separation")
        .expect("no-separation primer");
    let run = || {
        let (request, loaded) =
            support::ppml::request(&design(&primer), None, true, separation(&primer, None));
        fit_prepared_ppml(request, loaded).expect("no-separation fit")
    };
    let (first, second) = (run(), run());
    assert_eq!(first.separation().generations, 0);
    assert!(first.separation().dropped_observations.is_empty());
    assert_eq!(
        first.separation().generation_hashes,
        second.separation().generation_hashes
    );
}

#[test]
fn simplex_only_request_over_its_tableau_cap_fails_closed() {
    let primer = primers()
        .into_iter()
        .find(|primer| primer.name == "simplex-memory-skip-covered-by-ir")
        .expect("memory-skip primer");
    let mut config = separation(&primer, Some(1));
    config.methods.iterated_rectifier = false;
    let (request, loaded) = support::ppml::request(&design(&primer), None, true, config);
    let error = fit_prepared_ppml(request, loaded).expect_err("simplex-only over cap");
    assert_eq!(error.code, ErrorCode::Allocation, "{error}");
}
