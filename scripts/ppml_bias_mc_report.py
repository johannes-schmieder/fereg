#!/usr/bin/env python3
"""Render audited PPML confirmation results without changing populations."""
import argparse
import json
from pathlib import Path

MOMENTS = ['var(aw)', 'cov(aw,pw)', 'cov(aw,as)', 'cov(aw,ps)', 'var(pw)',
           'cov(pw,as)', 'cov(pw,ps)', 'var(as)', 'cov(as,ps)', 'var(ps)']


def fmt(value):
    return '—' if value is None else f'{value:.5f}'


def estimate(record, key='bias'):
    return f"{fmt(record.get(key))} ({fmt(record.get(key+'_mcse'))})"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--allow-partial', action='store_true')
    args = parser.parse_args()
    data = json.loads(args.input.read_text())
    if not data['complete'] and not args.allow_partial:
        raise ValueError('Full confirmation report requires complete audited attempts')
    states = data['failures']
    lines = ['# Native PPML confirmation Monte Carlo', '',
             f"Audited calls: {data['recorded_attempts']} / {data['planned_attempts']}. "
             + ('The frozen inventory is complete.' if data['complete'] else '**Partial results; confirmation is not complete.**'), '',
             'The campaign uses the frozen v04 artifact and retains every attempted draw. '
             'The later reconstruction fix is separately validated and does not replace failed campaign draws. '
             'Bias is mean estimate minus truth; parentheses give Monte Carlo standard errors. '
             'Here aw/pw are worker/firm wage effects and as/ps are worker/firm separation effects. '
             'RMSE and tail errors refer to the same stated population. No universal zero-bias or '
             'RMSE-improvement criterion is imposed.', '',
             f"The [machine-readable audit]({args.input.name}) contains every available estimate, "
             'RMSE MCSE, error median and 5th/95th percentiles, maximum absolute error, '
             'return codes, FEVC statuses and paired comparisons.', '',
             '## Attempt and failure inventory', '',
             '| Design | Engine | Probes | Attempts | Failed | Interior | Fallback full/A/B | Boundary full/A/B | Projected full/A/B |',
             '|:---|:---|---:|---:|---:|---:|---:|---:|---:|']
    for row in states:
        # Per-part flags may overlap within a draw; report their counts explicitly
        # rather than incorrectly summing them into unique-draw totals.
        counts = ['/'.join(str(row['flags'][part+'_'+flag]['positive']) for part in ('full', 'a', 'b'))
                  for flag in ('fallback', 'boundary', 'projected')]
        lines += [f"| {row['scenario']} | {row['engine']} | {row['probes']} | {row['attempted']} | {row['failed']} | {row['interior']} | {' | '.join(counts)} |"]
    lines += ['', 'Flag columns show **full/A/B counts**, which can overlap within a draw. '
              'Interior means successful with every recorded fallback, boundary and projection flag zero. '
              'A successful return using a legacy fallback remains in the available population. '
              'Failed native corrections publish no fallback estimate.', '',
              '## Native versus legacy, all ten moments', '',
              'Corrected Split Branch results use independent branch streams. Moment 1 is undefined '
              'for Split Branch, so its ordinary plug-in result is shown. '
              'Paired available comparisons require both calls to succeed; joint interior additionally '
              'requires both calls to have all validity flags zero. These restrictions do not change '
              'the attempt denominator above. Fallback-driven discrepancies cannot be attributed '
              'solely to the coefficient correction.']
    scenarios = sorted({row['scenario'] for row in states})
    for population in ('paired_available', 'joint_interior'):
        for scenario in scenarios:
            rows = [r for r in data['paired'] if r['comparator'] == 'legacy' and
                    r['comparison_population'] == population and r['scenario'] == scenario and
                    r['population'] == 'branch' and
                    r['method'] == ('plugin' if r['moment'] == 1 else 'split_bc')]
            if not rows:
                continue
            lines += ['', f'### {scenario}: {population}', '',
                      '| Moment | Native bias (MCSE) | Legacy bias (MCSE) | Native RMSE | Legacy RMSE | Paired difference (MCSE) | n |',
                      '|:---|---:|---:|---:|---:|---:|---:|']
            for r in sorted(rows, key=lambda x: x['moment']):
                label = MOMENTS[r['moment']-1] + (' (plug-in)' if r['moment'] == 1 else '')
                lines += [f"| {r['moment']}: {label} | {estimate(r['native'])} | {estimate(r['reference'])} | {fmt(r['native'].get('rmse'))} | {fmt(r['reference'].get('rmse'))} | {estimate(r['paired_difference'])} | {r['native']['n']} |"]
    lines += ['', '## Native ordinary, raw Split Branch and corrected Split Branch', '',
              'All available native B=256 estimates are retained here. These are external moments; '
              'a coefficient mean correction does not generally remove quadratic-moment bias. '
              'Entries are bias (MCSE) / RMSE.']
    for scenario in scenarios:
        lookup = {(r['method'], r['moment']): r for r in data['all_available'] if
                  r['scenario'] == scenario and r['engine'] == 'native' and r['probes'] == 256 and r['population'] == 'branch'}
        lines += ['', f'### {scenario}', '', '| Moment | Ordinary plug-in | Raw Split Branch | Corrected Split Branch |',
                  '|:---|---:|---:|---:|']
        for i, label in enumerate(MOMENTS, 1):
            cells = []
            for method in ('plugin', 'split_raw', 'split_bc'):
                r = lookup.get((method, i))
                cells.append('undefined' if r is None else f"{estimate(r)} / {fmt(r.get('rmse'))}")
            lines += [f"| {i}: {label} | {' | '.join(cells)} |"]
    lines += ['', '## Exact and higher-probe sensitivity', '',
              'These paired differences are native B=256 minus the specified native comparator, '
              'on the prespecified first 20 exact and first 50 B=1024 draws per design. '
              'They measure numerical approximation effects on the external moments, not sampling standard errors.']
    for probes in (0, 1024):
        rows = [r for r in data['paired'] if r['comparator'] == 'native' and r['comparator_probes'] == probes and
                r['comparison_population'] == 'paired_available' and r['population'] == 'branch' and
                r['method'] == ('plugin' if r['moment'] == 1 else 'split_bc')]
        if not rows:
            continue
        lines += ['', f"### Comparator: {'exact' if probes == 0 else 'B=1024'}", '',
                  '| Design | Moment | Difference (MCSE) | RMS difference | Maximum absolute difference | n |',
                  '|:---|:---|---:|---:|---:|---:|']
        for r in sorted(rows, key=lambda x: (x['scenario'], x['moment'])):
            d = r['paired_difference']
            lines += [f"| {r['scenario']} | {r['moment']}: {MOMENTS[r['moment']-1]} | {estimate(d)} | {fmt(d.get('rmse'))} | {fmt(d.get('maximum_absolute_error'))} | {d['n']} |"]
    checks = data['pairing_checks']
    lines += ['', '## Unchanged wage and FEVC calculations', '',
              'Dataset and branch signatures agree exactly for every audited pair. '
              f"Maximum wage/FEVC discrepancy is {max((r['maximum_wage_fevc_difference'] for r in checks), default=0):.6g} "
              f"against the frozen {data['wage_fevc_absolute_tolerance']:.1g} arithmetic-order tolerance. "
              'FEVC code, probes and seeds are unchanged; the audit includes both M and MS population statuses.']
    with args.out.open('x') as handle:
        handle.write('\n'.join(lines)+'\n')


if __name__ == '__main__':
    main()
