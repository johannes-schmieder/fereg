//! Bounded, configurable Stata-free native benchmark input generation.
//! No dense oracle is fitted here; qualify numerical changes with replay/oracles.
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;
#[allow(dead_code)]
mod wire {
    include!("replay/wire.rs");

    impl Request {
        /// Keep the mandatory sample destination while omitting optional outputs.
        /// Call before appending saved-effect or bias columns.
        pub fn sample_only(&mut self, poisson: bool) {
            assert!(self.saved.is_empty());
            let sample = self
                .outputs
                .iter()
                .find(|(key, _)| key == "sample")
                .unwrap()
                .1;
            assert_eq!(self.outputs.first().unwrap().1, sample);
            let omitted: Vec<_> = self
                .outputs
                .iter()
                .filter(|(key, _)| key != "sample")
                .map(|(key, _)| format!("output_{key}"))
                .collect();
            for key in omitted {
                self.replace(&key, "0");
            }
            self.data.truncate(sample + 1);
            self.outputs.retain(|(key, _)| key == "sample");
            if poisson {
                self.replace("residual", "none");
            }
        }
    }

    #[cfg(test)]
    mod campaign_output_tests {
        use super::*;
        use fereg_oracle::designs::{generate, DesignSpec};

        #[test]
        fn sample_only_preserves_inputs_and_required_sample_destination() {
            for poisson in [false, true] {
                let design = generate(19, &DesignSpec::default()).unwrap();
                let mut request = make(&design, poisson, "robust", "auto", false);
                let sample = request.outputs[0].1;
                let inputs_and_sample = request.data[..=sample].to_vec();
                request.sample_only(poisson);
                assert_eq!(request.data, inputs_and_sample);
                assert_eq!(request.outputs, vec![("sample".into(), sample)]);
                assert!(request
                    .args
                    .contains(&format!("output_sample={}", sample + 1)));
                for key in if poisson {
                    vec!["residual", "drop_reason", "xb", "absorbed", "eta", "mean"]
                } else {
                    vec!["residual", "drop_reason", "xb", "xbd", "absorbed"]
                } {
                    assert!(request.args.contains(&format!("output_{key}=0")));
                }
                if poisson {
                    assert!(request.args.contains(&"residual=none".into()));
                }
            }
        }
    }
}

use fereg_oracle::designs::{generate, DesignSpec, Graph, WeightKind};
use std::{collections::BTreeMap, path::PathBuf};

const HELP: &str = "replay_large --output target/tmp/TASK/NAME.case --rows N --levels L1[,L2,...] \
--regressors K --weights unit|aw|fw|pw --vce iid|robust|cluster \
--graph akm-strong|akm-weak|chain|barbell|random-mobility|akm-campaign --solver auto|map|lsmr|cmg \
--seed N [--ppml] [--threads N] [--tolerance T] [--iterations N] [--max-cells N] [--max-edges N] \
[--zero-fraction F] [--separation METHODS] [--no-observation-outputs] [--candidate-all] [--non-deterministic]\n\
Defaults: threads=1 tolerance=1e-8 iterations=16000 max-cells=100000000 max-edges=10000000.\n\
PPML only: --zero-fraction draws that share of outcomes as zero (default 0) and\n\
--separation sets the space-separated separation methods (default none).\n\
Cluster VCE uses one cluster dimension from the oracle design. Graph labels retain\n\
the existing oracle topology (AkmStrong is adjacent-firm mobility, not random).\n\
akm-campaign requires two effects and ceil(rows/5) workers: five consecutive\n\
periods, persistence 0.8 and random destination firms. Oracle RNG draws do not\n\
reproduce frozen NumPy campaign inputs. --no-observation-outputs keeps the\n\
mandatory sample output; candidate and deterministic defaults are unchanged.\n\
One effect requires random-mobility; 1..16 effects are accepted.";

fn main() {
    if let Err(error) = run() {
        eprintln!("replay_large: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut options = BTreeMap::new();
    let mut poisson = false;
    while let Some(key) = args.next() {
        if key == "--help" {
            println!("{HELP}");
            return Ok(());
        }
        if key == "--ppml" && !poisson {
            poisson = true;
            continue;
        }
        if [
            "--no-observation-outputs",
            "--candidate-all",
            "--non-deterministic",
        ]
        .contains(&key.as_str())
        {
            if options.insert(key.clone(), "1".into()).is_some() {
                return Err(format!("repeated option {key}"));
            }
            continue;
        }
        if ![
            "--output",
            "--rows",
            "--levels",
            "--regressors",
            "--weights",
            "--vce",
            "--graph",
            "--solver",
            "--seed",
            "--threads",
            "--tolerance",
            "--iterations",
            "--max-cells",
            "--max-edges",
            "--zero-fraction",
            "--separation",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unknown/repeated flag {key}; {HELP}"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {key}"))?;
        if options.insert(key.clone(), value).is_some() {
            return Err(format!("repeated option {key}"));
        }
    }
    let get = |key: &str| {
        options
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| format!("required option {key}; {HELP}"))
    };
    let integer = |key: &str, default: Option<&str>| -> Result<usize, String> {
        options
            .get(key)
            .map(String::as_str)
            .or(default)
            .ok_or_else(|| format!("required option {key}"))?
            .parse()
            .map_err(|_| format!("invalid integer {key}"))
    };
    let rows = integer("--rows", None)?;
    let regressors = integer("--regressors", None)?;
    let levels = get("--levels")?
        .split(',')
        .map(|v| {
            v.parse::<usize>()
                .map_err(|_| "invalid --levels".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if rows == 0
        || rows > i32::MAX as usize
        || regressors > 1000
        || levels.is_empty()
        || levels.len() > 16
        || levels
            .iter()
            .any(|&n| n < 2 || n > rows || n >= u32::MAX as usize)
    {
        return Err(
            "require 0<rows<=i32::MAX, K<=1000 and 1..16 effects with 2<=levels<=rows".into(),
        );
    }
    let graph = match get("--graph")? {
        "akm-strong" => Graph::AkmStrong,
        "akm-weak" => Graph::AkmWeak,
        "chain" => Graph::Chain,
        "barbell" => Graph::Barbell,
        "random-mobility" => Graph::RandomMobility,
        "akm-campaign" => Graph::AkmCampaign,
        _ => return Err("unsupported --graph".into()),
    };
    // A single effect has no mobility graph; only independent draws apply.
    if levels.len() == 1 && graph != Graph::RandomMobility {
        return Err("one effect requires --graph random-mobility".into());
    }
    // The oracle enumerates barbell halves densely. Refuse accidental huge
    // allocations and prefixes that omit the bridge instead of relabelling them.
    let edges = match graph {
        Graph::RandomMobility | Graph::AkmCampaign => Some(rows),
        Graph::AkmStrong => levels[0].checked_mul(4),
        Graph::AkmWeak => levels[0].checked_mul(17),
        Graph::Chain => levels[0].max(levels[1]).checked_mul(2),
        Graph::Barbell => (levels[0] / 2)
            .checked_mul(levels[1] / 2)
            .and_then(|a| {
                (levels[0] - levels[0] / 2)
                    .checked_mul(levels[1] - levels[1] / 2)
                    .and_then(|b| a.checked_add(b))
            })
            .and_then(|n| n.checked_add(1)),
        _ => unreachable!(),
    }
    .ok_or("edge count overflow")?;
    if edges > integer("--max-edges", Some("10000000"))? || rows < edges {
        return Err(format!("topology needs {edges} edges: require rows>=edges and raise --max-edges explicitly if needed"));
    }
    let weights = match get("--weights")? {
        "unit" => WeightKind::Unit,
        "aw" => WeightKind::Analytic,
        "fw" => WeightKind::Frequency,
        "pw" => WeightKind::Probability,
        _ => return Err("unsupported --weights".into()),
    };
    let vce = get("--vce")?;
    if !["iid", "robust", "cluster"].contains(&vce)
        || (poisson && (weights == WeightKind::Analytic || vce == "iid"))
        || (weights == WeightKind::Probability && vce == "iid")
    {
        return Err("unsupported weight/VCE combination (PPML requires robust or cluster)".into());
    }
    let solver = get("--solver")?;
    if !["auto", "map", "lsmr", "cmg"].contains(&solver) {
        return Err("unsupported --solver".into());
    }
    let threads = integer("--threads", Some("1"))?;
    if threads == 0 || threads > 1024 {
        return Err("threads must be 1..1024".into());
    }
    let tolerance: f64 = options
        .get("--tolerance")
        .map_or("1e-8", String::as_str)
        .parse()
        .map_err(|_| "invalid tolerance")?;
    if !tolerance.is_finite() || !(0.0..1.0).contains(&tolerance) || tolerance == 0.0 {
        return Err("require 0<tolerance<1".into());
    }
    let iterations = integer("--iterations", Some("16000"))?;
    if iterations == 0 || iterations > i32::MAX as usize {
        return Err("iterations must be 1..i32::MAX".into());
    }
    let max_cells = integer("--max-cells", Some("100000000"))?;
    let clusters = usize::from(vce == "cluster");
    let columns = 2
        + regressors
        + levels.len()
        + clusters
        + usize::from(weights != WeightKind::Unit)
        + if options.contains_key("--no-observation-outputs") {
            1
        } else if poisson {
            7
        } else {
            6
        };
    if rows.checked_mul(columns).is_none_or(|n| n > max_cells) {
        return Err("case exceeds --max-cells (includes requested output columns)".into());
    }
    let output = PathBuf::from(get("--output")?);
    let target = std::fs::canonicalize("target").map_err(|_| "create target/ before generation")?;
    let parent = output.parent().ok_or("output needs parent directory")?;
    if !std::fs::canonicalize(parent)
        .map_err(|e| e.to_string())?
        .starts_with(target)
        || output.exists()
    {
        return Err("output must be a new file under target/".into());
    }
    let seed = get("--seed")?.parse::<u64>().map_err(|_| "invalid seed")?;
    let zero_fraction = options
        .get("--zero-fraction")
        .map_or(Ok(0.0), |text| text.parse::<f64>())
        .map_err(|_| "invalid --zero-fraction")?;
    if !(0.0..1.0).contains(&zero_fraction)
        || (!poisson && (zero_fraction != 0.0 || options.contains_key("--separation")))
    {
        return Err("--zero-fraction must lie in [0, 1); it and --separation need --ppml".into());
    }
    let spec = DesignSpec {
        rows,
        levels,
        regressors,
        graph,
        weights,
        clusters,
        poisson,
        zero_fraction,
        ..DesignSpec::default()
    };
    let design = generate(seed, &spec)?;
    let mut request = wire::make(&design, poisson, vce, solver, false);
    request.replace("iterate", &iterations.to_string());
    request.replace("threads", &threads.to_string());
    request.replace("tolerance", &tolerance.to_string());
    if options.contains_key("--no-observation-outputs") {
        request.sample_only(poisson);
    }
    if options.contains_key("--candidate-all") {
        request.replace("candidate_all", "1");
    }
    if options.contains_key("--non-deterministic") {
        request.replace("deterministic", "0");
    }
    if let Some(methods) = options.get("--separation") {
        request.separation(methods);
    }
    request.write(&output);
    println!("{}: seed={seed} {spec:?}, solver={solver}, vce={vce}, threads={threads}, tolerance={tolerance}, iterations={iterations}", output.display());
    Ok(())
}
