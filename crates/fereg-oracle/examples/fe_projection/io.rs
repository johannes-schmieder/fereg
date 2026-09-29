use super::{Input, Result};
use fereg_oracle::{designs::WeightKind, explicit_ols::DummyEffect};
use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};

fn integer(reader: &mut impl Read) -> Result<usize> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(usize::try_from(u64::from_le_bytes(bytes))?)
}
fn values(reader: &mut impl Read, length: usize) -> Result<Vec<f64>> {
    let mut result = Vec::new();
    result.try_reserve_exact(length)?;
    for _ in 0..length {
        let mut bytes = [0; 8];
        reader.read_exact(&mut bytes)?;
        let value = f64::from_le_bytes(bytes);
        if !value.is_finite() {
            return Err("nonfinite FE benchmark input".into());
        }
        result.push(value);
    }
    Ok(result)
}
pub fn read(path: &Path) -> Result<Input> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut magic = [0; 8];
    reader.read_exact(&mut magic)?;
    if &magic != b"FEREGFE1" {
        return Err("invalid FE benchmark format".into());
    }
    let n = integer(&mut reader)?;
    let k = integer(&mut reader)?;
    let q = integer(&mut reader)?;
    if n == 0 || k == 0 || !(2..=3).contains(&q) {
        return Err("invalid benchmark dimensions".into());
    }
    let mut effects = Vec::new();
    for _ in 0..q {
        let levels = integer(&mut reader)?;
        let slope_count = integer(&mut reader)?;
        if levels == 0 || levels > n || slope_count > 1 {
            return Err("invalid effect dimensions".into());
        }
        let mut codes = Vec::new();
        codes.try_reserve_exact(n)?;
        for _ in 0..n {
            let mut bytes = [0; 4];
            reader.read_exact(&mut bytes)?;
            codes.push(u32::from_le_bytes(bytes));
        }
        let mut slopes = Vec::new();
        for _ in 0..slope_count {
            slopes.push(values(&mut reader, n)?);
        }
        effects.push(DummyEffect::new(codes, levels, true, slopes)?);
    }
    let outcome = values(&mut reader, n)?;
    let mut regressors = Vec::new();
    regressors.try_reserve_exact(k)?;
    for _ in 0..k {
        regressors.push(values(&mut reader, n)?);
    }
    if reader.read(&mut [0; 1])? != 0 {
        return Err("trailing benchmark input bytes".into());
    }
    Ok(Input {
        outcome,
        regressors,
        effects,
        weight_kind: WeightKind::Unit,
        weights: vec![1.; n],
        missing: vec![false; n],
        clusters: vec![],
    })
}
pub fn write_values(path: &Path, values: &[f64]) -> Result<()> {
    let mut writer = BufWriter::new(File::create(path)?);
    for value in values {
        if !value.is_finite() {
            return Err("nonfinite benchmark result".into());
        }
        writer.write_all(&value.to_le_bytes())?;
    }
    writer.flush()?;
    Ok(())
}
