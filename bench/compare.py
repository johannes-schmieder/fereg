#!/usr/bin/env python3
"""Compact summaries; missing/failed runs stay visible and cannot pass accuracy."""
from __future__ import annotations
import argparse
from array import array
from dataclasses import dataclass
import csv
from datetime import date
import io
import json
import math
from operator import itemgetter
from pathlib import Path
import statistics
import hashlib
from collections import OrderedDict
import sys

ROOT = Path(__file__).resolve().parents[1]


def coefficients(directory):
    with (Path(directory) / "coefficients.csv").open() as stream:
        return {row["name"]: float(row["value"]) for row in csv.DictReader(stream)}


@dataclass(frozen=True, slots=True, eq=False)
class SampleIds:
    """Immutable ordered identities; common machine integers use eight bytes.

    Unbounded Python integers retain the existing CSV contract through a cold
    tuple representation. Neither representation sorts, deduplicates or hashes
    identities in place of the required exact ordered comparison.
    """
    packed: bytes | None
    wide: tuple | None = None

    @classmethod
    def from_values(cls, values):
        values = tuple(values)
        try:
            return cls(array('q', values).tobytes())
        except OverflowError:
            return cls(None, values)

    def __len__(self):
        return len(self.packed)//8 if self.packed is not None else len(self.wide)

    def __iter__(self):
        return iter(memoryview(self.packed).cast('q') if self.packed is not None else self.wide)

    def __eq__(self, other):
        if isinstance(other, SampleIds):
            if self.packed is not None and other.packed is not None:
                return self.packed == other.packed
            return tuple(self) == tuple(other)
        if isinstance(other, tuple):
            return tuple(self) == other
        return NotImplemented

    def storage_bytes(self):
        if self.packed is not None:
            return sys.getsizeof(self.packed)
        return sys.getsizeof(self.wide) + sum(map(sys.getsizeof, self.wide))


def sample(directory):
    # The CSV reader retains field limits, quoting and blank-row semantics.
    # The common single-column case feeds C-level maps into a compact bank.
    with (Path(directory) / "sample.csv").open() as stream:
        rows = csv.reader(stream)
        header = next(rows, None)
        if header == ['obs_id']:
            try:
                values = map(int, map(itemgetter(0), filter(None, rows)))
                return SampleIds(array('q', values).tobytes())
            except OverflowError:
                stream.seek(0)
                rows = csv.reader(stream)
                header = next(rows, None)
        if header is None:
            return SampleIds.from_values(())
        column = len(header) - 1 - header[::-1].index('obs_id') if 'obs_id' in header else None
        result = []
        for row in rows:
            if not row:
                continue
            if column is None:
                raise KeyError('obs_id')
            result.append(int(row[column] if column < len(row) else None))
        return SampleIds.from_values(result)


class ReferenceCache:
    """Analysis-local immutable reference bank; actual captures are always read.

    Retained entries are bounded, including their directory and metadata. Stat checks
    surround a first read and every reuse; a changed reference is an integrity
    failure rather than a stale successful comparison. Eviction only costs I/O.
    """
    def __init__(self, maximum_bytes=64*1024*1024):
        if maximum_bytes < 0:
            raise ValueError('negative reference cache budget')
        self.maximum_bytes = maximum_bytes
        self.retained_bytes = 0
        self.entries = OrderedDict()
        self.payload_bytes = 0

    @staticmethod
    def fingerprint(path):
        value = path.stat()
        return (value.st_dev, value.st_ino, value.st_size,
                value.st_mtime_ns, value.st_ctime_ns)

    def read(self, directory, filename, reader):
        path = Path(directory) / filename
        before = self.fingerprint(path)
        key = str(path.absolute())
        if key in self.entries:
            fingerprint, value, _ = self.entries[key]
            if fingerprint != before:
                raise ValueError('immutable reference capture changed')
            self.entries.move_to_end(key)
            return value
        value = reader(directory)
        if self.fingerprint(path) != before:
            raise ValueError('reference capture changed during parsing')
        size = (sys.getsizeof(key) + sys.getsizeof(before)
                + sum(map(sys.getsizeof, before)) + sys.getsizeof(value)
                + sys.getsizeof((before, value, 0)) + sys.getsizeof(0))
        if isinstance(value, SampleIds):
            size += value.storage_bytes()
        elif isinstance(value, dict):
            size += sum(sys.getsizeof(k) + sys.getsizeof(v) for k,v in value.items())
        else:
            size += sum(sys.getsizeof(v) for v in value)
        if size + sys.getsizeof(OrderedDict()) <= self.maximum_bytes:
            self.entries[key] = (before, value, size)
            self.payload_bytes += size
            # Count the actual directory allocation: deletion alone may retain
            # its hash-table capacity, so eviction rebuilds a compact directory.
            self.retained_bytes = self.payload_bytes + sys.getsizeof(self.entries)
            while self.entries and self.retained_bytes > self.maximum_bytes:
                _, (_, _, removed) = self.entries.popitem(last=False)
                self.payload_bytes -= removed
                self.entries = OrderedDict(self.entries)
                self.retained_bytes = (self.payload_bytes + sys.getsizeof(self.entries)
                                       if self.entries else 0)

        return value


def accuracy(record, reference, policy, cache=None):
    if record["status"] != "ok" or reference is None or reference["status"] != "ok":
        return "unavailable", None
    try:
        actual = coefficients(record["directory"])
        expected = (coefficients(reference["directory"]) if cache is None else
                    cache.read(reference["directory"], 'coefficients.csv', coefficients))
        if set(actual) != set(expected):
            return "disagreement", None
        observed_sample = sample(record["directory"])
        expected_sample = (sample(reference["directory"]) if cache is None else
                           cache.read(reference["directory"], 'sample.csv', sample))
        if observed_sample != expected_sample:
            return "disagreement", None
        ratios = [abs(actual[key]-value)/(policy["coefficient_atol"]+policy["coefficient_rtol"]*abs(value))
                  for key, value in expected.items()]
        if any(not math.isfinite(actual[key]) or not math.isfinite(value) for key, value in expected.items()):
            return "disagreement", None
        maximum = max(ratios, default=0.0)
        return ("pass" if maximum <= 1 else "disagreement"), maximum
    except (OSError, ValueError, KeyError):
        return "unavailable", None


def median_metadata(records, name):
    values = []
    for record in records:
        try:
            value = float(record["metadata"][name])
        except (KeyError, ValueError):
            return ""
        if not math.isfinite(value) or value < 0:
            return ""
        values.append(value)
    return statistics.median(values) if values else ""


def timing_envelope(parent, candidate, policy):
    """One order block; two opposite-order blocks are required for confirmation."""
    if len(parent) != policy['repetitions'] or len(candidate) != policy['repetitions']:
        return {'verdict': 'unavailable'}
    if any(not math.isfinite(v) or v < 0 for v in [*parent, *candidate]):
        return {'verdict': 'unavailable'}
    p, c = statistics.median(parent), statistics.median(candidate)
    allowance = max(policy['short_workload_allowance_seconds'], policy['slowdown_fraction'] * p)
    if policy.get('uncertainty_method') == 'paired-bootstrap-medians-v1':
        import numpy as np
        rng = np.random.Generator(np.random.PCG64(policy['uncertainty_seed']))
        indices = rng.integers(0, len(parent), size=(policy['uncertainty_resamples'], len(parent)))
        differences = np.median(np.array(candidate)[indices], axis=1) - np.median(np.array(parent)[indices], axis=1)
        lo, hi = np.quantile(differences, [.025, .975])
        quantization = 2*policy.get('duration_quantization_seconds',.0005)
        lo, hi = float(lo)-quantization, float(hi)+quantization
        verdict = 'within' if hi <= allowance else ('outside' if lo > allowance else 'unresolved')
        return dict(verdict=verdict, delta_seconds=c-p, allowance_seconds=allowance,
                    lower_seconds=lo, upper_seconds=hi, uncertainty_seconds=max(c-p-lo, hi-(c-p)),
                    noise_resolves_allowance=hi-(c-p) <= allowance)
    uncertainty = sum(3 * statistics.median(abs(v-statistics.median(values)) for v in values)
                      + 0.0005 for values in [parent, candidate])
    delta = c-p
    verdict = 'within' if delta+uncertainty <= allowance else ('outside' if delta-uncertainty > allowance else 'unresolved')
    return dict(verdict=verdict, delta_seconds=delta, allowance_seconds=allowance,
                uncertainty_seconds=uncertainty, noise_resolves_allowance=uncertainty <= allowance)


def median_order_interval(values, tail_probability=.00625, quantization=1e-6):
    """Finite-sample median interval, exact under independent continuous draws.

    Ties make coverage conservative. Each endpoint has the specified binomial
    tail probability; quantization expands the interval, never subtracts time.
    """
    values = sorted(values)
    n = len(values)
    if not values or any(not math.isfinite(v) or v < 0 for v in values):
        raise ValueError('invalid command measurements')
    k = 0
    probability = 0.0
    for index in range(1, n//2+1):
        probability += math.comb(n, index-1) / 2**n
        if probability <= tail_probability:
            k = index
        else:
            break
    if not k:
        return (-math.inf, math.inf)
    return (values[k-1]-quantization, values[n-k]+quantization)


def median_order_envelope(parent, candidate, policy, tail_probability=.00625):
    pl, pu = median_order_interval(parent, tail_probability)
    cl, cu = median_order_interval(candidate, tail_probability)
    allowance = lambda value: max(policy['short_workload_allowance_seconds'], policy['slowdown_fraction']*value)
    upper = cu-pl-allowance(pl)
    lower = cl-pu-allowance(pu)
    p, c = statistics.median(parent), statistics.median(candidate)
    return dict(verdict='within' if upper <= 0 else 'unresolved',
                delta_seconds=c-p, allowance_seconds=allowance(p),
                parent_median_interval_seconds=[pl,pu], candidate_median_interval_seconds=[cl,cu],
                lower_excess_seconds=lower, upper_excess_seconds=upper,
                reverse_upper_excess_seconds=pu-cl-allowance(cl), pairs=len(parent))


def validate_measurement_plan(plan):
    if plan.get('mode') not in ['calibration','acceptance'] or plan.get('method') != 'bounded-median-orders-v2' or plan.get('screen_pairs') != 8:
        raise ValueError('unsupported bounded measurement protocol')
    scope = list(map(tuple, plan.get('scope', [])))
    if not scope or len(set(scope)) != len(scope) or any(len(group) != 3 for group in scope):
        raise ValueError('empty or duplicated measurement scope')
    keys = {'|'.join(map(str, group)) for group in scope}
    if set(plan.get('confirmation_pairs', {})) != keys or any(n not in [32,64] for n in plan['confirmation_pairs'].values()):
        raise ValueError('confirmation budgets must be assigned to every workload before screening')
    if plan.get('slowdown_fraction') != .05 or plan.get('short_workload_allowance_seconds') != .001:
        raise ValueError('measurement allowance changed')
    if plan.get('maximum_wall_seconds_per_block', 0) <= 0:
        raise ValueError('missing bounded wall budget')
    if set(plan.get('artifacts', {})) != {'candidate','parent','fixed_baseline'}:
        raise ValueError('missing predeclared artifacts')
    if not plan.get('calibration_sha256') and plan.get('mode') != 'calibration':
        raise ValueError('candidate measurement needs prior calibrated protocol')
    return scope


def bounded_pair_envelopes(paths):
    """Two screening orders and, if required, both targeted confirmation orders.

    Every observation remains in the final interval. The screen and final look
    each spend one-sided alpha .0125; requiring all cells/orders/comparators is
    an intersection-union gate, with family false acceptance bounded by .025.
    """
    campaigns = [json.loads(Path(path).read_text()) for path in paths]
    if len(campaigns) not in [2,4] or [c.get('reverse_order') for c in campaigns] != [False,True]*(len(campaigns)//2):
        raise ValueError('bounded campaign needs opposite screening/confirmation orders')
    first = campaigns[0]
    plan = first['measurement']['plan']
    scope = validate_measurement_plan(plan)
    if hashlib.sha256(json.dumps(plan,sort_keys=True,separators=(',',':')).encode()).hexdigest() != first['measurement']['plan_sha256']:
        raise ValueError('embedded bounded plan differs from its identity')
    comparator = first['measurement']['comparator']
    if comparator not in ['parent','fixed_baseline']:
        raise ValueError('invalid measurement comparator')
    if first['timing_identity'].get('method') != 'monotonic-command-v1':
        raise ValueError('bounded measurements require monotonic command clock')
    if first['paired_artifacts'] != {role:plan['artifacts']['candidate' if role=='candidate' else comparator] for role in ['parent','candidate']}:
        raise ValueError('artifacts differ from predeclared plan')
    screen_scope = set(scope)
    confirmations = set()
    cache = ReferenceCache()
    values = {}
    for index, campaign in enumerate(campaigns):
        stage = 'screening' if index < 2 else 'confirmation'
        if not campaign.get('complete') or not campaign.get('identities_verified'):
            raise ValueError('bounded campaign is incomplete')
        for key in ['paired_artifacts','suite_sha256','tooling_identity','protocol','platform','tier','environment','timing_identity']:
            if not campaign.get(key) or campaign[key] != first[key]:
                raise ValueError('bounded campaign identity mismatch: '+key)
        measurement = campaign.get('measurement', {})
        if measurement.get('plan') != plan or measurement.get('plan_sha256') != first['measurement']['plan_sha256'] or measurement.get('comparator') != comparator or measurement.get('stage') != stage:
            raise ValueError('bounded measurement declaration changed')
        groups = {(c,t,a) for c,t,a,role in campaign['expected_groups']}
        if stage == 'screening' and groups != screen_scope:
            raise ValueError('screening omitted a declared workload')
        if stage == 'confirmation':
            if not groups <= screen_scope or not groups:
                raise ValueError('confirmation outside original workload scope')
            if index == 2:
                confirmations = groups
            elif groups != confirmations or measurement.get('screening_blocks') != campaigns[2]['measurement'].get('screening_blocks'):
                raise ValueError('confirmation scope/evidence changed between orders')
        expected = {(c,t,a,role) for c,t,a in groups for role in ['parent','candidate']}
        actual = {(r['cell'],r['threads'],r['arm'],r['artifact_role']) for r in campaign['records'] if not r['reference']}
        if len(campaign['expected_groups']) != len(expected) or set(map(tuple,campaign['expected_groups'])) != expected or actual != expected:
            raise ValueError('bounded records differ from declared scope')
        if campaign.get('wall_seconds',math.inf) > plan['maximum_wall_seconds_per_block']:
            raise ValueError('bounded campaign exceeded wall budget')
        for group in groups:
            cell,threads,arm = group
            if campaign['datasets'].get(cell) != first['datasets'].get(cell):
                raise ValueError('bounded dataset identity changed')
            count = 8 if stage == 'screening' else plan['confirmation_pairs']['|'.join(map(str,group))]
            references = [r for r in campaign['records'] if r['reference'] and r['cell']==cell]
            refs = {r['artifact_role']:r for r in references}
            if len(references)!=2 or set(refs)!={'parent','candidate'}:
                raise ValueError('bounded command references missing or duplicated')
            for role, reference in refs.items():
                native = campaign['paired_artifacts'][role]['native_identity']
                if any(reference.get(key)!=native[key] for key in ['plugin_sha256','frontend_sha256']):
                    raise ValueError('bounded reference artifact changed')
            records = [r for r in campaign['records'] if not r['reference'] and (r['cell'],r['threads'],r['arm'])==group]
            if len(records) != 2*(count+1):
                raise ValueError('bounded pairs missing or duplicated')
            for repetition in ['warmup',*range(1,count+1)]:
                reverse = campaign['reverse_order'] ^ (isinstance(repetition,int) and repetition%2==0)
                pair = [r for r in records if r['repetition']==repetition]
                if [r['artifact_role'] for r in pair] != (['candidate','parent'] if reverse else ['parent','candidate']):
                    raise ValueError('bounded pair order changed')
            for record in records:
                native = campaign['paired_artifacts'][record['artifact_role']]['native_identity']
                if any(record[key] != native[key] for key in ['plugin_sha256','frontend_sha256']) or record.get('timing_identity') != campaign['timing_identity']:
                    raise ValueError('bounded command artifact/clock identity changed')
                metadata = record.get('metadata', {})
                if metadata.get('clock_rc') != '0' or not 0<float(metadata.get('clock_resolution_seconds',math.inf))<=.0000005:
                    raise ValueError('invalid bounded command clock')
                if accuracy(record, refs.get(record['artifact_role']), campaign['protocol'], cache)[0] != 'pass':
                    raise ValueError('bounded command failed coefficient/sample comparison')
            values[(index,group)] = {role:[float(r['metadata']['seconds']) for r in records if r['artifact_role']==role and r['repetition']!='warmup'] for role in ['parent','candidate']}
    effects = []
    for group in sorted(scope):
        screen, final = [], []
        for order in range(2):
            bank = {role:list(values[(order,group)][role]) for role in ['parent','candidate']}
            screen.append(median_order_envelope(bank['parent'],bank['candidate'],first['protocol']))
            if plan['mode']=='calibration' and screen[-1]['reverse_upper_excess_seconds'] > 0:
                screen[-1]['verdict'] = 'unresolved'
            if group in confirmations:
                for role in bank:
                    bank[role].extend(values[(order+2,group)][role])
            final.append(median_order_envelope(bank['parent'],bank['candidate'],first['protocol']))
            # Established regressions use conservative simultaneous tails over
            # all predeclared families/comparators/orders, only at the final look.
            regression = median_order_envelope(bank['parent'],bank['candidate'],first['protocol'],.025/(8*len(scope)))
            final[-1]['established_regression'] = group in confirmations and regression['lower_excess_seconds'] > 0
        verdict = 'within' if all(b['verdict']=='within' and (plan['mode']!='calibration' or b['reverse_upper_excess_seconds']<=0) for b in final) else ('outside' if all(b['established_regression'] for b in final) else 'unresolved')
        effects.append(dict(cell=group[0],threads=group[1],arm=group[2],verdict=verdict,screening_blocks=screen,blocks=final,confirmed=group in confirmations))
    return dict(paired_artifacts=first['paired_artifacts'],manifest_sha256=first['suite_sha256'],datasets=first['datasets'],
        effects=effects,context={k:first[k] for k in ['protocol','tooling_identity','platform','environment','tier','timing_identity']},
        measurement_plan=plan,measurement_plan_sha256=first['measurement']['plan_sha256'],
        passed=all(e['verdict']=='within' for e in effects),
        blocks=[dict(path=str(p),sha256=hashlib.sha256(Path(p).read_bytes()).hexdigest()) for p in paths])


def paired_envelopes(paths):
    """Validate two bounded, opposite-order blocks before evaluating effects."""
    if paths and json.loads(Path(paths[0]).read_text()).get('measurement'):
        return bounded_pair_envelopes(paths)
    if len(paths) != 2:
        raise ValueError('exactly two paired blocks are required')
    campaigns = [json.loads(Path(path).read_text()) for path in paths]
    if [c.get('reverse_order') for c in campaigns] != [False, True]:
        raise ValueError('paired blocks require forward then reverse order')
    first = campaigns[0]
    if first['protocol'].get('maximum_confirmation_blocks') != 2 or first['protocol'].get('repetitions',0) < 5:
        raise ValueError('invalid bounded paired protocol')
    for c in campaigns:
        if not c.get('complete') or not c.get('identities_verified'):
            raise ValueError('paired campaign has no verified terminal marker')
        for key in ['paired_artifacts', 'suite_sha256', 'tooling_identity', 'datasets', 'protocol', 'platform', 'tier', 'environment', 'timing_identity']:
            if not c.get(key) or c[key] != first[key]:
                raise ValueError('paired block identity mismatch: ' + key)
        if set(c['paired_artifacts']) != {'parent', 'candidate'} or set(map(tuple,c['expected_groups'])) != set(map(tuple,first['expected_groups'])):
            raise ValueError('paired block scope mismatch')
    effects = []
    cache = ReferenceCache()
    groups = {(cell, threads, arm) for cell,threads,arm,role in first['expected_groups']}
    if not groups:
        raise ValueError('paired scope is empty')
    if len(first['expected_groups']) != 2*len(groups) or any(set(role for c,t,a,role in first['expected_groups'] if (c,t,a)==group) != {'parent','candidate'} for group in groups):
        raise ValueError('every group requires both paired artifacts')
    for c in campaigns:
        actual={(r['cell'],r['threads'],r['arm'],r['artifact_role']) for r in c['records'] if not r['reference']}
        if actual != set(map(tuple,c['expected_groups'])):
            raise ValueError('paired records differ from declared scope')
    for cell, threads, arm in sorted(groups):
        blocks = []
        for c in campaigns:
            refs = {r['artifact_role']:r for r in c['records'] if r['reference'] and r['cell']==cell}
            recs = [r for r in c['records'] if not r['reference'] and (r['cell'],r['threads'],r['arm'])==(cell,threads,arm)]
            count = c['protocol']['repetitions']
            expected = ['warmup', *range(1,count+1)]
            for repetition in expected:
                pair = [r for r in recs if r['repetition']==repetition]
                reverse = c['reverse_order'] ^ (isinstance(repetition,int) and repetition%2==0)
                order = ['candidate','parent'] if reverse else ['parent','candidate']
                if [r['artifact_role'] for r in pair] != order:
                    raise ValueError('paired commands missing, duplicated or out of order')
            if len(recs) != 2*(count+1):
                raise ValueError('unexpected paired repetitions')
            for r in recs:
                native = c['paired_artifacts'][r['artifact_role']]['native_identity']
                if any(r[k] != native[k] for k in ['plugin_sha256','frontend_sha256']):
                    raise ValueError('paired record artifact differs from campaign')
                if r.get('timing_identity') != c['timing_identity']:
                    raise ValueError('paired record timing method differs from campaign')
                if c['timing_identity']['method']=='monotonic-command-v1':
                    try:
                        resolution=float(r['metadata']['clock_resolution_seconds'])
                        if r['metadata']['clock_rc']!='0' or not 0<resolution<=.0000005:
                            raise ValueError('invalid command clock capture')
                    except (KeyError,TypeError):
                        raise ValueError('missing command clock capture') from None
                if accuracy(r, refs.get(r['artifact_role']), c['protocol'], cache)[0] != 'pass':
                    raise ValueError('paired command failed coefficient/sample accuracy')
            times = {role:[float(r['metadata']['seconds']) for r in recs if r['artifact_role']==role and r['repetition']!='warmup'] for role in ['parent','candidate']}
            blocks.append(timing_envelope(times['parent'], times['candidate'], c['protocol']))
        verdicts = [b['verdict'] for b in blocks]
        verdict = 'within' if verdicts==['within','within'] else ('outside' if verdicts==['outside','outside'] else 'unresolved')
        effects.append(dict(cell=cell, threads=threads, arm=arm, verdict=verdict, blocks=blocks))
    return {'paired_artifacts':first['paired_artifacts'], 'manifest_sha256':first['suite_sha256'],
            'datasets':first['datasets'], 'effects':effects,
            'context':{k:first[k] for k in ['protocol','tooling_identity','platform','environment','tier','timing_identity']},
            'passed':all(e['verdict']=='within' for e in effects),
            'blocks':[{'path':str(p),'sha256':hashlib.sha256(Path(p).read_bytes()).hexdigest()} for p in paths]}


def acceptance_envelopes(parent_paths, baseline_paths, fixed_plugin_sha256, fixed_frontend_sha256, calibrate=False):
    parent, baseline = paired_envelopes(parent_paths), paired_envelopes(baseline_paths)
    for key in ['manifest_sha256', 'datasets', 'context']:
        if parent[key] != baseline[key]:
            raise ValueError('parent and fixed baseline comparisons have different inputs')
    if parent['paired_artifacts']['candidate'] != baseline['paired_artifacts']['candidate']:
        raise ValueError('parent and baseline candidate artifacts differ')
    if baseline['paired_artifacts']['parent']['native_identity']['plugin_sha256'] != fixed_plugin_sha256:
        raise ValueError('fixed baseline plugin differs from frozen identity')
    if baseline['paired_artifacts']['parent']['native_identity']['frontend_sha256'] != fixed_frontend_sha256:
        raise ValueError('fixed baseline frontend differs from frozen identity')
    if {(e['cell'],e['threads'],e['arm']) for e in parent['effects']} != {(e['cell'],e['threads'],e['arm']) for e in baseline['effects']}:
        raise ValueError('parent and baseline comparisons have different scope')
    if parent.get('measurement_plan'):
        if parent['measurement_plan'] != baseline.get('measurement_plan') or parent['measurement_plan_sha256'] != baseline.get('measurement_plan_sha256'):
            raise ValueError('bounded comparator declarations differ')
        unresolved = {(e['cell'],e['threads'],e['arm']) for result in [parent,baseline] for e in result['effects'] if any(b['verdict']!='within' for b in e['screening_blocks'])}
        screening_identity = parent['blocks'][:2]+baseline['blocks'][:2]
        for result in [parent,baseline]:
            for block in result['blocks'][2:]:
                campaign = json.loads(Path(block['path']).read_text())
                if campaign['measurement'].get('screening_blocks') != screening_identity:
                    raise ValueError('confirmation must retain the exact four screening artifacts')
        for result in [parent,baseline]:
            confirmed = {(e['cell'],e['threads'],e['arm']) for e in result['effects'] if e['confirmed']}
            if confirmed and confirmed != unresolved:
                raise ValueError('confirmation must cover exactly all unresolved screening workloads')
            if confirmed and len(result['blocks']) != 4:
                raise ValueError('both confirmation orders are required')
        if any(len(result['blocks'])==4 for result in [parent,baseline]) and any(len(result['blocks'])!=4 for result in [parent,baseline]):
            raise ValueError('confirmation must cover both comparators')
    passed=parent['passed'] and baseline['passed']
    if calibrate:
        for comparison in [parent,baseline]:
            if comparison['paired_artifacts']['parent'] != comparison['paired_artifacts']['candidate']:
                raise ValueError('calibration requires identical source/native/frontend artifacts')
        passed=all((b['lower_seconds'] >= -b['allowance_seconds'] and b['upper_seconds'] <= b['allowance_seconds']) if 'lower_seconds' in b else (b['upper_excess_seconds'] <= 0 and b['reverse_upper_excess_seconds'] <= 0)
                   for comparison in [parent,baseline] for e in comparison['effects'] for b in e['blocks'])
    return dict(parent=parent, fixed_baseline=baseline, calibration=calibrate, passed=passed)


def summarize(path, destination=None):
    campaign = json.loads(Path(path).read_text())
    refs = {(record["cell"],record.get('artifact_role','')): record for record in campaign["records"] if record["reference"]}
    groups = {tuple(group) if len(group)==4 else (*group,''): [] for group in campaign["expected_groups"]}
    for record in campaign["records"]:
        if not record["reference"]:
            groups.setdefault((record["cell"], record["threads"], record["arm"],record.get('artifact_role','')), []).append(record)
    rows = []
    cache = ReferenceCache()
    for (cell, threads, arm, role), records in groups.items():
        measured = [record for record in records if record["repetition"] != "warmup"]
        statuses = [record["status"] for record in records]
        count = campaign['protocol']['repetitions']
        if campaign.get('measurement',{}).get('stage')=='confirmation':
            count = campaign['measurement']['plan']['confirmation_pairs'][f"{cell}|{threads}|{arm}"]
        complete = (len(measured)==count
                    and {r["repetition"] for r in measured}==set(range(1,count+1))
                    and sum(r["repetition"]=="warmup" for r in records)==1
                    and all(s=="ok" for s in statuses))
        status = "ok" if complete else ("timeout" if "timeout" in statuses else "error")
        checks = [accuracy(record, refs.get((cell,role)), campaign["protocol"], cache) for record in measured]
        verdict = "pass" if complete and all(check=="pass" for check,_ in checks) else "unavailable"
        if any(check=="disagreement" for check,_ in checks):
            verdict = "disagreement"
        seconds = median_metadata(measured,"seconds") if complete else ""
        timings = [float(r["metadata"]["seconds"]) for r in measured] if isinstance(seconds, (int, float)) else []
        artifact = campaign.get('paired_artifacts',{}).get(role,{})
        native = artifact.get('native_identity',campaign.get('native_identity',{}))
        rows.append({"date": date.today().isoformat(), "source_sha": artifact.get('source_sha',campaign['source_sha']),
                     "dirty": campaign["source_dirty"], "platform": campaign["platform"], "tier": campaign["tier"],
                     "partial": campaign["partial"], "comparison": campaign.get("comparison", "default"), "engine": campaign["engine"], "solver": campaign["solver"],
                     "native_build_source": native.get('build_source_commit',''),
                     "plugin_sha256": native.get('plugin_sha256',''),
                     "frontend_sha256": native.get('frontend_sha256',''),
                     "cell": cell, "threads": threads, "arm": arm, "status": status, "accuracy": verdict,
                     "artifact_role":role, "manifest_sha256":campaign.get('suite_sha256',''),
                     "tooling_sha256":hashlib.sha256(json.dumps(campaign.get('tooling_identity',{}),sort_keys=True).encode()).hexdigest(),
                     "timing_method":campaign.get('timing_identity',{}).get('method','stata-timer'),
                     "clock_sha256":campaign.get('timing_identity',{}).get('plugin_sha256',''),
                     "requested_solver":records[0].get('requested_solver', campaign['solver']) if records else '',
                     "memory":records[0].get('memory','') if records else '',
                     "output_mode":records[0].get('output_mode','') if records else '',
                     "repetitions": len(measured), "seconds": seconds,
                     "seconds_min": min(timings) if timings else "",
                     "seconds_max": max(timings) if timings else "",
                     "seconds_mad": statistics.median(abs(v-seconds) for v in timings) if timings else "",
                     "input_seconds": median_metadata(measured,"input_seconds") if complete else "",
                     "native_seconds": median_metadata(measured,"native_seconds") if complete else "",
                     "total_seconds": median_metadata(measured,"total_seconds") if complete else "",
                     "max_scaled_error": max((v for _,v in checks if v is not None), default=""),
                     "ratio_fereg": ""})
    for row in rows:
        candidate = next((r for r in rows if r["cell"]==row["cell"] and r["threads"]==row["threads"] and r["arm"]=="fereg" and r['artifact_role']==row['artifact_role']), None)
        if candidate and row["status"]==candidate["status"]=="ok" and row["accuracy"]==candidate["accuracy"]=="pass" and row["seconds"]>0:
            row["ratio_fereg"] = candidate["seconds"]/row["seconds"]
    if not rows:
        raise ValueError("campaign contains no measured command groups")
    buffer = io.StringIO()
    writer = csv.DictWriter(buffer, fieldnames=list(rows[0]), lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    limit = 8*1024*1024 if campaign.get('program') else 100_000
    if len(buffer.getvalue().encode())>limit:
        raise ValueError('summary exceeds declared bound; split the campaign')
    if destination is None:
        if campaign.get('program'):
            destination = Path(path).parent / 'summary.csv'
        else:
            destination = default_summary_path(campaign)
    destination = Path(destination)
    if destination.is_relative_to(ROOT/'bench/history') and len(buffer.getvalue().encode())>100_000:
        raise ValueError('committed evidence must be compact; retain program summaries in the raw run root')
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        raise ValueError('summary already exists; do not overwrite previous evidence')
    destination.write_text(buffer.getvalue())
    for row in rows:
        print(row['cell'],row['threads'],row['arm'],row['artifact_role'],row['seconds'],row['status'],row['accuracy'])
    for arm in sorted({row['arm'] for row in rows}-{'fereg'}):
        pairs = [row for row in rows if row['arm']==arm]
        passing = [row for row in pairs if row['accuracy']=='pass' and isinstance(row['ratio_fereg'],float) and row['ratio_fereg']>0]
        values = [row['ratio_fereg'] for row in passing]
        gm = math.exp(statistics.fmean(math.log(v) for v in values)) if values else None
        worst = max(passing,key=lambda row:row['ratio_fereg'],default=None)
        print(f"{arm}: geometric mean={gm}; passing pairs={len(passing)}/{len(pairs)}; "
              f"worst={worst['cell'] if worst else 'unavailable'}; timeouts={sum(row['status']=='timeout' for row in pairs)}")
    print(f"wall_seconds={campaign.get('wall_seconds')}; summary={destination}")
    return rows


def default_summary_path(campaign):
    suffix = '-dirty' if campaign['source_dirty'] else ''
    partial = '-partial' if campaign['partial'] else ''
    if campaign.get('comparison') == 'solver':
        partial += '-solvers-' + campaign['solver']
    if campaign.get('comparison') == 'engine':
        partial += '-engine-' + campaign['engine']
    return ROOT / 'bench/history' / f"{date.today()}-{campaign['source_sha'][:8]}{suffix}-{campaign['platform']}-{campaign['tier']}{partial}.csv"


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument('results', type=Path, nargs='?')
    parser.add_argument("--output", type=Path)
    parser.add_argument('--parent-blocks', type=Path, nargs='+')
    parser.add_argument('--baseline-blocks', type=Path, nargs='+')
    parser.add_argument('--fixed-plugin-sha256')
    parser.add_argument('--fixed-frontend-sha256')
    parser.add_argument('--calibrate',action='store_true')
    args = parser.parse_args()
    if args.parent_blocks or args.baseline_blocks:
        if not args.parent_blocks or not args.baseline_blocks or not args.fixed_plugin_sha256 or not args.fixed_frontend_sha256 or not args.output:
            parser.error('acceptance needs parent/baseline block pairs, frozen plugin/frontend SHA-256 and output')
        result = acceptance_envelopes(args.parent_blocks,args.baseline_blocks,args.fixed_plugin_sha256,args.fixed_frontend_sha256,args.calibrate)
        with args.output.open('x') as stream:
            json.dump(result,stream,indent=2)
            stream.write('\n')
        print('performance envelope', 'pass' if result['passed'] else 'unresolved or outside')
        raise SystemExit(0 if result['passed'] else 1)
    else:
        if not args.results:
            parser.error('results are required')
        summarize(args.results,args.output)
