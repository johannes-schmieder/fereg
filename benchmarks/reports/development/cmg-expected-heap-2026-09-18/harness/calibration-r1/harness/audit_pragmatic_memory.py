#!/usr/bin/env python3
"""Audit fixed development memory predictions against independent ABI observations.

This never rewrites predictions, promotes partial coverage, or calibrates a
multiplier from the measured run. RSS remains a distinct process observation.
"""
from __future__ import annotations
import argparse
import csv
import json
import re
from pathlib import Path


def audit_case(directory: Path, relative: float = .10, absolute: int = 64 << 20) -> dict:
    receipt = directory/'result.json'
    if receipt.exists():
        arm = json.loads(receipt.read_text()).get('arm')
        if arm not in ('fereg', 'feppml'):
            raise ValueError('native estimator identity required')
        reports = [directory/(arm+'_memory.csv')]
    else:
        reports = list(directory.glob('*_memory.csv'))
    if len(reports) != 1:
        raise ValueError('exactly one shared memory report is required')
    table = {}
    with reports[0].open() as stream:
        for item in csv.DictReader(stream):
            key = (int(item['row']), int(item['column']))
            if key in table:
                raise ValueError('duplicate report coordinate')
            table[key] = float(item['value'])
    if set(table) != {(r, c) for r in range(1, 12) for c in (1, 2)}:
        raise ValueError('incomplete shared memory report')
    def value(row):
        present, amount = table[row, 1], table[row, 2]
        if present not in (0, 1) or amount < 0 or not amount.is_integer() or amount > 2**53:
            raise ValueError('invalid memory report value')
        if not present and amount != 0:
            raise ValueError('missing memory value must be zero encoded')
        return int(amount) if present else None
    rows = [value(row) for row in range(1, 12)]
    if rows[0] != 1 or rows[1] not in (0, 1, 2):
        raise ValueError('unsupported memory schema/status')
    if rows[1] < 2 and (rows[7] is not None or rows[8] is not None):
        raise ValueError('partial coverage cannot publish a combined prediction')
    traces = re.findall(r'FEREG_WHOLE_ACTION_MEMORY_TERMINAL\|[^\n]+', (directory/'stderr.txt').read_text())
    if len(traces) != 1:
        raise ValueError('one fresh-process whole-action observation required')
    observed = dict(part.split('=', 1) for part in traces[0].split('|')[1:])
    if observed.get('observation') != 'abi_return' or observed.get('return_code') != '0':
        raise ValueError('successful ABI-return observation required')
    actual = int(observed['peak_delta_bytes'])
    estimate = rows[5]
    error = None if estimate is None else estimate - actual
    tolerance = max(absolute, relative * actual)
    result = {'native_predicted_bytes': estimate, 'native_observed_bytes': actual,
              'native_error_bytes': error, 'native_tolerance_bytes': tolerance,
              'native_accuracy_pass': error is not None and abs(error) <= tolerance,
              'estimate_status': rows[1], 'model_gap_bits': rows[10],
              'stata_allocated_before': rows[3], 'stata_growth_estimate': rows[6],
              'combined_prediction': rows[7], 'whole_action_accepted': False}
    # Complete coverage also needs separately measured Stata growth/process
    # validation and holdouts. A native-only audit cannot promote the model.
    if receipt.exists():
        r = json.loads(receipt.read_text())
        result['process_memory_observations'] = {k: v for k, v in r.items() if 'rss' in k or 'scratch' in k}
    return result


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('directories', nargs='+', type=Path)
    args = p.parse_args()
    results = {str(d): audit_case(d) for d in args.directories}
    print(json.dumps(results, indent=2, sort_keys=True))
    return 0 if all(r['native_accuracy_pass'] for r in results.values()) else 1

if __name__ == '__main__':
    raise SystemExit(main())
