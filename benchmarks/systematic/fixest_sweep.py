"""Register a current-source fixest-only successor, preserving the light grid."""
import argparse
import copy
import csv
from pathlib import Path

from common import cell_id, read_json, sha256, write_json
from light import cells, groups


def register(original, history, candidate_commit):
    manifest = copy.deepcopy(original)
    manifest['candidate_commit'] = candidate_commit
    manifest['purpose'] = 'Current-source OLS/PPML development sweep; no release qualification'
    manifest['arms'] = dict(ols=['fereg', 'fixest'], ppml=['feppml', 'fixest', 'fixest_strict'])
    manifest['historical_cost_sha256'] = sha256(history)
    manifest['skip_rules'] = []
    grid = {cell_id(c): c for c in cells(manifest)}
    expected = {(cell_id(g['cell']), g['repetition']) for g in groups(manifest)}
    observed = set()
    with Path(history).open() as stream:
        rows = list(csv.DictReader(stream))
    for row in rows:
        key = cell_id(row), int(row['repetition'])
        if row['arm'] not in manifest['arms'].get(row['model'], []):
            continue
        if key not in expected or (*key, row['arm']) in observed:
            raise ValueError('Historical grid contains an unexpected or duplicate arm')
        observed.add((*key, row['arm']))
        if row['arm'] not in ('fixest', 'fixest_strict') or row['status'] != 'command_timeout':
            continue
        cell = grid[key[0]]
        # The two original strict slope timeouts used a 120-second pilot cap.
        # Later pinned-stock adjudication completed these exact inputs. They
        # remain live comparisons; ordinary production limits are recomputed.
        if cell.get('slopes'):
            continue
        manifest['skip_rules'].append(dict(
            arm=row['arm'], match={k: cell[k] for k in ('model', 'design', 'n', 'k', 'threads', 'section')},
            repetitions=[key[1]],
            reason=f"Unchanged pinned reference exhausted {row['timeout_bound']} seconds on this exact original seed; not rerun. Historical failure is not a current result.",
            evidence='benchmarks/reports/development/systematic-light-scc-final-2026-09-27/generated/runs.csv'))
    required = {(cell_id(g['cell']), g['repetition'], a)
                for g in groups(manifest) for a in manifest['arms'][g['cell']['model']]}
    if observed != required:
        raise ValueError('Historical evidence does not cover the full registered sweep')
    # Every generator family is covered independently by the unchanged smoke.
    # This pilot checks both estimators at large N and both two-FE slope modes.
    # Known three-FE stock DoF disagreements remain visible in production;
    # they are not an appropriate comparator-eligibility launch gate.
    manifest['pilot_cell_ids'] = [cell_id(c) for c in grid.values() if
        (c.get('slopes') and c['design'] == 'two_fe_dense') or
        (c['section'] == 'main' and c['k'] == 10 and c['threads'] == 28 and
         ((c['design'] == 'strong_d6' and c['n'] == 3932160) or
          (c['design'] == 'two_fe_dense' and c['n'] == 983040)))]
    manifest['sweep_scope'] = dict(
        configurations=len(grid), groups=len(expected),
        omitted_commands=['reghdfe', 'areg', 'greg', 'ppmlhdfe', 'gglm', 'pyfixest'],
        reason='Unchanged commands retain their prior evidence and separate adjudication tasks.',
        scientific_discrepancies='Rerun and keep unranked unless all original checks pass.',
        source_policy='Reuse the verified accepted Linux artifact; no diagnostic build in headline timings.')
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--history', type=Path, required=True)
    parser.add_argument('--candidate-commit', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError('Refusing to replace a registered manifest')
    write_json(args.output, register(read_json(args.manifest), args.history, args.candidate_commit))
