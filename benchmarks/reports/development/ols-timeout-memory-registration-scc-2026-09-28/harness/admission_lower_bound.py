"""Scalar transcription of the current Schwarz apply-buffer lower bound.

No fit, topology construction or allocation is performed. This is only a subset
of the admission estimate: it can prove refusal, never complete admission.
"""
import argparse
import hashlib
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--repo', type=Path, required=True)
    args = parser.parse_args()
    n, degree, threads, rhs = 3_932_160, 2, 4, 11
    counts = [n // degree, n // degree // 40]
    levels, maximum = sum(counts), max(counts)
    reentrant = levels // 10_002
    banks = threads + reentrant
    apply = 8 * (levels + 6 * maximum + 4) * banks + 512 * banks + 256 * threads
    sources = ['crates/fereg-core/src/memory.rs',
        'crates/fereg-core/src/memory/categorical_schwarz.rs',
        'crates/fereg-core/src/fe/borrowed_map.rs',
        'crates/fereg-core/src/ols/prepared_fit.rs',
        'crates/fereg-plugin/src/estimate.rs', 'benchmarks/systematic/generate.py']
    print(json.dumps(dict(n=n, rhs=rhs, threads=threads, level_counts=counts,
        reentrant_component_bound=reentrant, buffer_banks_per_apply=banks,
        apply_bytes=apply, retained_and_active_apply_bytes=apply * (4 + rhs),
        lower_bound_only=True,
        budgets=[dict(limit_bytes=gib * 2**30, usable_bytes=gib * 2**30 - gib * 2**30 // 5,
                      rejected_by_this_subset=apply * (4 + rhs) > gib * 2**30 - gib * 2**30 // 5)
                 for gib in [192, 448]],
        sources={p: hashlib.sha256((args.repo / p).read_bytes()).hexdigest() for p in sources}), indent=2))


if __name__ == '__main__':
    main()
