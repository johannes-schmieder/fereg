#!/usr/bin/env python3
"""Keep the only mutable status ledger small enough to read on every task."""
from pathlib import Path


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    lines = (root / "MILESTONES.md").read_text(encoding="utf-8").splitlines()
    errors = []
    if len(lines) >= 300:
        errors.append(f"MILESTONES.md: {len(lines)} lines; must be fewer than 300")
    for number, line in enumerate(lines, 1):
        if len(line) > 200:
            errors.append(f"MILESTONES.md:{number}: {len(line)} characters; maximum 200")
    if errors:
        raise SystemExit("\n".join(errors))
    print(f"Governance: PASS ({len(lines)} ledger lines; maximum 200 characters)")


if __name__ == "__main__":
    main()
