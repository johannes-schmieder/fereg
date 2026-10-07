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
import re
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
        elif hasattr(value, 'memory_usage'):
            size += int(value.memory_usage(deep=True).sum()) + int(value.columns.memory_usage(deep=True))
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


def scientific_accuracy(record, reference, cache):
    """Compare scientific tolerances for VCE, scalars and saved outputs.

    Named FE coordinates can be nonunique across algorithms. Their identified
    sum, published predictions and the exact sample/layout remain targets.
    """
    import numpy as np
    import pandas as pd
    def numbers(directory, filename):
        return {k:stata_hex_to_float(v) for k,v in (line.split() for line in (Path(directory)/filename).read_text().splitlines())}
    def close(a, b, rtol):
        a, b = np.asarray(a), np.asarray(b)
        ma, mb = np.abs(a) >= 8e307, np.abs(b) >= 8e307
        if (a.shape != b.shape or np.any(np.isinf(a)) or np.any(np.isinf(b)) or np.any(ma != mb)
                or np.any(np.isnan(a) != np.isnan(b)) or np.any(a[ma] != b[mb])):
            return False
        mask = ~(ma | np.isnan(a))
        return bool(np.all(np.abs(a[mask]-b[mask]) <= 1e-10 + rtol*np.maximum(np.abs(a[mask]),np.abs(b[mask]))))
    discrete = {'N','N_full','N_drop','rank','df_m','df_r','df_a','num_singletons'}
    for filename in ['science.hex','scalars.hex']:
        a = numbers(record['directory'], filename)
        b = cache.read(reference['directory'], filename, lambda p:numbers(p, filename))
        if a.keys() != b.keys() or not a: return False
        if filename == 'science.hex':
            def omitted(values):
                slots = set()
                for key, value in values.items():
                    if not key.startswith('b.') or value != 0: continue
                    slot = key.split('.')[1]
                    covariance = [k for k in values if k.startswith('V.') and slot in k.split('.')[1:]]
                    if covariance and all(values[k] == 0 for k in covariance): slots.add(slot)
                return slots
            if omitted(a) != omitted(b): return False
        for key in a:
            if key in discrete:
                if a[key] != b[key]: return False
            elif not close([a[key]], [b[key]], 1e-6 if key.startswith('V.') else 1e-7):
                return False
    directory, target = Path(record['directory']), Path(reference['directory'])
    if (directory/'outputs.csv').exists() != (target/'outputs.csv').exists(): return False
    if (directory/'outputs.csv').exists():
        reader = lambda p:pd.read_csv(Path(p)/'outputs.csv',na_values=['.'],float_precision='round_trip')
        a, b = reader(directory), cache.read(target, 'outputs.csv', reader)
        if list(a.columns) != list(b.columns) or a.shape != b.shape: return False
        if not np.array_equal(a.obs_id, b.obs_id): return False
        saved = [c for c in a.columns if c.startswith('saved')]
        retained = [c for c in a.columns if c not in saved]
        if not close(a[retained].to_numpy(float), b[retained].to_numpy(float), 1e-7): return False
        if saved and not close(a[saved].sum(axis=1,skipna=False), b[saved].sum(axis=1,skipna=False), 1e-7): return False
    return True


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


def stata_hex_to_float(value: str) -> float:
    if value == ".":
        return math.nan
    match = re.fullmatch(
        r"([+-])([0-9a-fA-F]+(?:\.[0-9a-fA-F]+)?)X([+-])([0-9a-fA-F]+)",
        value,
    )
    if match is None:
        raise ValueError(f"invalid canonical Stata hex value {value!r}")
    exponent = int(match.group(4), 16)
    if match.group(3) == "-":
        exponent = -exponent
    result = float.fromhex(f"{match.group(1)}0x{match.group(2)}p{exponent}")
    if not math.isfinite(result):
        raise ValueError("scientific evidence contains a missing/nonfinite value")
    return result


def summarize(run):
    records, policy = run['records'], run['suite']['protocol']
    cache, rows = ReferenceCache(), []
    expected = {(c['id'], t) for c in run['suite']['cells'] for t in
                run['suite']['tiers'][run['tier']]['threads']}
    for cell,threads in sorted(expected):
        reference = next((r for r in records if r['cell']==cell and r['reference']), None)
        measured = [r for r in records if r['cell']==cell and r['threads']==threads and not r['reference']]
        checks=[]
        for record in measured:
            verdict,ratio=accuracy(record,reference,policy,cache)
            try:
                full=verdict=='pass' and scientific_accuracy(record,reference,cache)
            except (OSError,ValueError,KeyError,TypeError): full=False
            checks.append(full)
        timed=[r for r in measured if r['repetition']>0]
        complete = sorted(r['repetition'] for r in measured)==list(range(policy['repetitions']+1)) and len(timed)==policy['repetitions']
        rows.append(dict(cell=cell,threads=threads,accuracy_pass=complete and all(checks),
                         median_seconds=median_metadata(timed,'seconds'), measured=len(timed)))
    return dict(complete=run['complete'], accuracy_pass=run['complete'] and bool(rows)
                and all(r['accuracy_pass'] for r in rows), workloads=rows,
                timing_scope='descriptive; no performance acceptance or independent scientific proof')


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run',type=Path)
    args=parser.parse_args()
    print(json.dumps(summarize(json.loads(args.run.read_text())),indent=2))
