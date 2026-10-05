#!/usr/bin/env python3
"""Compact summaries; missing/failed runs stay visible and cannot pass accuracy."""
from __future__ import annotations
import argparse
import csv
from datetime import date
import io
import json
import math
from pathlib import Path
import statistics
import hashlib
from collections import OrderedDict
import sys

ROOT = Path(__file__).resolve().parents[1]


def coefficients(directory):
    with (Path(directory) / "coefficients.csv").open() as stream:
        return {row["name"]: float(row["value"]) for row in csv.DictReader(stream)}


def sample(directory):
    # One fixed field avoids a fresh dictionary for every observation. Keep
    # DictReader's last-duplicate-header and blank-record semantics.
    with (Path(directory) / "sample.csv").open() as stream:
        rows = csv.reader(stream)
        header = next(rows, None)
        if header is None:
            return ()
        if 'obs_id' not in header:
            raise KeyError('obs_id')
        column = len(header) - 1 - header[::-1].index('obs_id')
        return tuple(int(row[column] if column < len(row) else None)
                     for row in rows if row)


class ReferenceCache:
    """Analysis-local immutable reference bank; actual captures are always read.

    Retention is bounded, including Python containers and metadata. Stat checks
    surround a first read and every reuse; a changed reference is an integrity
    failure rather than a stale successful comparison. Eviction only costs I/O.
    """
    def __init__(self, maximum_bytes=64*1024*1024):
        if maximum_bytes < 0:
            raise ValueError('negative reference cache budget')
        self.maximum_bytes = maximum_bytes
        self.retained_bytes = 0
        self.entries = OrderedDict()

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
        size = sys.getsizeof(key) + sys.getsizeof(before) + sys.getsizeof(value) + 512
        if isinstance(value, dict):
            size += sum(sys.getsizeof(k) + sys.getsizeof(v) for k,v in value.items())
        else:
            size += sum(sys.getsizeof(v) for v in value)
        if size <= self.maximum_bytes:
            while self.entries and self.retained_bytes + size > self.maximum_bytes:
                _, (_, _, removed) = self.entries.popitem(last=False)
                self.retained_bytes -= removed
            self.entries[key] = (before, value, size)
            self.retained_bytes += size
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


def paired_envelopes(paths):
    """Validate two bounded, opposite-order blocks before evaluating effects."""
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
    passed=parent['passed'] and baseline['passed']
    if calibrate:
        for comparison in [parent,baseline]:
            if comparison['paired_artifacts']['parent'] != comparison['paired_artifacts']['candidate']:
                raise ValueError('calibration requires identical source/native/frontend artifacts')
        passed=all(b['lower_seconds'] >= -b['allowance_seconds'] and b['upper_seconds'] <= b['allowance_seconds']
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
        complete = (len(measured)==campaign["protocol"]["repetitions"]
                    and {r["repetition"] for r in measured}==set(range(1,campaign["protocol"]["repetitions"]+1))
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
    parser.add_argument('--parent-blocks', type=Path, nargs=2)
    parser.add_argument('--baseline-blocks', type=Path, nargs=2)
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
