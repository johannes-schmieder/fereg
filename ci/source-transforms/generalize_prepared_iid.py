#!/usr/bin/env python3
"""Repair the categorical iid static source-contract parser boundary."""

from pathlib import Path

root = Path(__file__).resolve().parents[2]
path = root / "tests/python/test_prepared_ols_boundary.py"
text = path.read_text(encoding="utf-8")

old = '        production = inference.split("#[cfg(test)]", maxsplit=1)[0]\n'
new = (
    '        production = inference.split(\n'
    '            "\\n#[cfg(test)]\\nmod tests {", maxsplit=1\n'
    '        )[0]\n'
)
if text.count(old) != 1:
    raise SystemExit(f"expected one broad cfg(test) split, found {text.count(old)}")
text = text.replace(old, new, 1)
path.write_text(text, encoding="utf-8")
print("repaired categorical iid source-contract parser")
