#!/usr/bin/env python3
"""Format the new oracle and repair its within-cell rank geometry exactly once."""

from pathlib import Path

root = Path(__file__).resolve().parents[2]
path = root / "crates/stfixest-testkit/tests/categorical_fit_oracle.rs"
text = path.read_text(encoding="utf-8")
old = '''    let x2 = (0..firm.len())
        .map(|row| {
            let within = if row % 2 == 0 { 1.1 } else { -0.6 };
            within - 0.09 * row as f64 + 0.23 * f64::from(firm[row])
        })
        .collect::<Vec<_>>();
'''
new = '''    let x2 = (0..firm.len())
        .map(|row| {
            let within = if row % 2 == 0 {
                1.1 + 0.07 * f64::from(firm[row])
            } else {
                -0.6 + 0.13 * f64::from(year[row])
            };
            within - 0.09 * row as f64 + 0.23 * f64::from(firm[row])
        })
        .collect::<Vec<_>>();
'''
count = text.count(old)
if count != 1:
    raise SystemExit(f"expected one x2 geometry block, found {count}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
print("repaired categorical oracle geometry")
