//! Opt-in, bounded evidence capture; never changes the solver's arithmetic.
use crate::problem::Problem;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

const MAX_UPDATE_VALUES: usize = 1_048_576;

pub struct Capture {
    path: PathBuf,
    initial: Vec<u64>,
    increments: Vec<Vec<u64>>,
    epochs: Vec<(usize, usize, Vec<u64>)>,
}

pub struct Failure<'a> {
    pub rhs: &'a [f64],
    pub x: &'a [f64],
    pub recursive: &'a [f64],
    pub truth: &'a [f64],
    pub direction: &'a [f64],
    pub product: &'a [f64],
    pub bnorm: f64,
    pub target: f64,
    pub iterations: usize,
    pub repairs: usize,
    pub trace: u64,
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

impl Capture {
    pub fn from_env(n: usize, start: Option<&[f64]>) -> Result<Option<Self>, String> {
        let Some(path) = std::env::var_os("STAGED_ACCURACY_CAPTURE") else {
            return Ok(None);
        };
        let args: Vec<_> = std::env::args().collect();
        if args.get(3).map(String::as_str) != Some("1")
            || args.get(4).map(String::as_str) != Some("auto-forecast")
        {
            return Err("accuracy_capture_requires_one_rhs_auto_forecast".into());
        }
        if n == 0 || n > MAX_UPDATE_VALUES {
            return Err("accuracy_capture_dimension_limit".into());
        }
        Ok(Some(Self {
            path: path.into(),
            initial: start.map_or_else(|| vec![0; n], bits),
            increments: Vec::new(),
            epochs: Vec::new(),
        }))
    }

    pub fn record(
        &mut self,
        alpha: f64,
        direction: &[f64],
        x: &[f64],
        repairs: usize,
    ) -> Result<(), String> {
        if direction.len() != self.initial.len()
            || self.increments.len() >= MAX_UPDATE_VALUES / direction.len()
        {
            return Err("accuracy_capture_update_limit".into());
        }
        if self
            .epochs
            .last()
            .is_none_or(|(_, previous, _)| *previous != repairs)
        {
            self.epochs.push((self.increments.len(), repairs, bits(x)));
        }
        // Observe separately rounded products. The original update loop remains
        // untouched; the independent audit requires bit-exact ordinary replay.
        self.increments
            .push(direction.iter().map(|d| (alpha * d).to_bits()).collect());
        Ok(())
    }

    pub fn write(&self, p: &Problem, f: Failure<'_>) -> Result<(), String> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
            .map_err(|e| format!("accuracy_capture_open: {e}"))?;
        let mut out = BufWriter::new(file);
        let result = (|| -> std::io::Result<()> {
            writeln!(
                out,
                "{{\"schema\":1,\"phase\":\"third_failed_certificate\","
            )?;
            writeln!(
                out,
                "\"n\":{},\"iterations\":{},\"repairs\":{},\"trace\":{},",
                p.n, f.iterations, f.repairs, f.trace
            )?;
            writeln!(
                out,
                "\"bnorm_bits\":{},\"target_bits\":{},",
                f.bnorm.to_bits(),
                f.target.to_bits()
            )?;
            writeln!(
                out,
                "\"offsets\":{:?},\"firms\":{:?},\"labels\":{:?},\"sizes\":{:?},",
                p.offsets, p.firms, p.labels, p.sizes
            )?;
            for (name, values) in [
                ("weights", p.weights.as_slice()),
                ("totals", p.totals.as_slice()),
                ("rhs", f.rhs),
                ("x", f.x),
                ("recursive", f.recursive),
                ("truth", f.truth),
                ("direction", f.direction),
                ("product", f.product),
            ] {
                writeln!(out, "\"{name}_bits\":{:?},", bits(values))?;
            }
            write!(out, "\"epochs\":[")?;
            for (i, (offset, repairs, values)) in self.epochs.iter().enumerate() {
                if i > 0 {
                    write!(out, ",")?;
                }
                write!(
                    out,
                    "{{\"update_offset\":{offset},\"repairs\":{repairs},\"x_bits\":{values:?}}}"
                )?;
            }
            writeln!(out, "],")?;
            writeln!(
                out,
                "\"initial_bits\":{:?},\"increment_bits\":{:?}}}",
                self.initial, self.increments
            )?;
            out.flush()
        })();
        result.map_err(|e| format!("accuracy_capture_write: {e}"))
    }
}
