#!/usr/bin/env python3
"""Freeze fresh C5 inputs and witnesses before D5; never run a candidate."""
from __future__ import annotations

import argparse
from functools import partial
from itertools import permutations
from pathlib import Path
import random
import sys

import engine_confirmation as base

campaign = base.campaign
KIND = 'compact_cells_s_owned_confirmation_v1'
DISCOVERY_KIND = campaign.S_OWNED_KIND
TEST_FILE = 'test_s_owned_confirmation.py'
ARMS = ['baseline', 'cells_s', 'cells_s_owned']
FINALIST = 'cells_s_owned'
COMPARISONS = [('baseline', FINALIST), ('cells_s', FINALIST)]
EXACT_PAIRS = [('cells_s', FINALIST)]
CASES = [dict(spec) for spec in base.CASES]
LIMITS = {**base.LIMITS, 'seed': 2026101753, 'data_seed': 2026101754,
          'inference': 'all 20 settings and both comparators must pass the frozen median allowance-excess bound'}
ORACLE_CAPS = dict(base.ORACLE_CAPS)
ANALYSIS = {
    **base.ANALYSIS, 'reference': ['baseline', 'cells_s'], 'finalist': FINALIST,
    'block_reference': 'geometric mean of two complete reference solves, separately for Auto and S',
    'block_candidate': 'geometric mean of two complete S plus ownership solves',
    'independent_unit': 'one complete six-call mirrored block within one setting; both comparisons share it',
    'pass_rule': 'E_(10) <= 0 for both comparators in every one of the 20 settings; ties pass',
    'scope': 'isolated SCC Linux confirmation on a different host from D5; no Mac performance claim',
    'interpretation': 'all-pass intersection-union nonregression decision; not 40 simultaneous confidence intervals',
    'gain_inference': 'gains are descriptive; confirmation is a nonregression decision, not general proof of speedup',
}
EXECUTION = {
    **base.EXECUTION, 'warmup_calls': 60, 'scientific_calls': 120, 'scored_calls': 1440,
    'scientific_checks': base.EXECUTION['scientific_checks'] + '; exact S versus S plus ownership in all phases',
    'candidate_prerequisite': 'complete D5 S ownership gate eligible for cells_s_owned',
    'freeze_order': 'this complete C5 plan and driver must be frozen before the first D5 native call',
}


def schedule(cell_ids):
    """Each of six three-arm permutations occurs twice as a complete mirror."""
    rng = random.Random(LIMITS['seed'])
    permutations_twice = list(permutations(ARMS)) * 2
    orders = {cid: rng.sample(permutations_twice, 12) for cid in cell_ids}
    return [[{'cell': cid, 'arms': [*orders[cid][block], *reversed(orders[cid][block])]}
             for cid in rng.sample(list(cell_ids), len(cell_ids))]
            for block in range(LIMITS['blocks'])]


# The preparation workflow is shared explicitly; C4 globals and results stay intact.
tool_files = partial(base.tool_files, profile=sys.modules[__name__])
columns = partial(base.columns, profile=sys.modules[__name__])
validate_contract = partial(base.validate_contract, profile=sys.modules[__name__])
validate = partial(base.validate, profile=sys.modules[__name__])
prepare = partial(base.prepare, profile=sys.modules[__name__])
oracle_capacity = base.oracle_capacity
source_capacity = base.source_capacity


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--discovery-plan', type=Path, required=True)
    args = parser.parse_args()
    plan = prepare(args.root, args.discovery_plan)
    print('S_OWNED_CONFIRMATION_PREPARED sha256=' + campaign.routing.identity(plan), flush=True)


if __name__ == '__main__':
    main()
