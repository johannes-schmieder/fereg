#!/usr/bin/env python3
"""Versioned runtime inventory and ratchet; see docs/development_workflow.md."""
from __future__ import annotations
import argparse
from bisect import bisect_right
from collections import Counter
import json
from pathlib import Path
import re
import subprocess
import sys

from rust_source import _production_tokens, _function_spans, _matching, _item_start_with_attributes

ROOT = Path(__file__).resolve().parents[1]
BASELINE = 'tests/reference/complexity.json'
CRATES = ('fereg-core', 'fereg-plugin')
SENSITIVE = re.compile(r'(^|_)(n|p|k|nobs|nrows|ncols|rows|cols|columns|width|threads?|workers?|weights?|weighted|memory|budget|capacity|len|size|fast|low)($|_)')
CHUNK = re.compile(r'(^|_)(chunks?|tiles?|partitions?|grains?)($|_)')


def modules(root: Path, entry: Path):
    """Follow production module declarations, retaining all non-test cfg variants."""
    seen = set()
    pending = [entry]
    while pending:
        path = pending.pop().resolve()
        if path in seen:
            continue
        if not path.is_relative_to(root.resolve()):
            raise ValueError(f'outside runtime tree: {path}')
        seen.add(path)
        source = path.read_text()
        tokens = _production_tokens(source)
        yield path, source, tokens
        directory = path.parent if path.name in ('lib.rs', 'mod.rs') else path.with_suffix('')
        for i, token in enumerate(tokens[:-2]):
            if token.value != 'mod':
                continue
            name, delimiter = tokens[i+1].value, tokens[i+2].value
            if delimiter != ';':
                # Inline modules are counted in this file. None currently declare
                # external modules; reject that shape rather than omit children.
                if delimiter == '{':
                    end = _matching(tokens, i+2, '{', '}')
                    if any(t.value == 'mod' for t in tokens[i+3:end]):
                        raise ValueError(f'nested module requires resolver support: {path}:{name}')
                continue
            start = i
            if start and tokens[start-1].value == ')':
                while start and tokens[start-1].value != 'pub':
                    start -= 1
            if start and tokens[start-1].value == 'pub':
                start -= 1
            start = _item_start_with_attributes(tokens, start)
            attrs = source[tokens[start].start:token.start]
            override = re.search(r'#\s*\[\s*path\s*=\s*"([^"]+)"\s*\]', attrs)
            candidates = [path.parent / override[1]] if override else [directory/f'{name}.rs', directory/name/'mod.rs']
            found = [p for p in candidates if p.is_file()]
            if len(found) != 1:
                raise ValueError(f'ambiguous/missing production module: {path}:{name}')
            pending.extend(found)


def file_metrics(source, tokens):
    starts = [0] + [m.end() for m in re.finditer('\n', source)]
    line = lambda pos: bisect_right(starts, pos)
    lines = set()
    for t in tokens:
        lines.update(range(line(t.start), line(t.end-1)+1))
    functions, names = {}, Counter()
    for f in _function_spans(tokens):
        names[f.name] += 1
        key = f'{f.name}#{names[f.name]}'
        functions[key] = {'line':line(tokens[f.start].start), 'lines':line(tokens[f.body_end].end-1)-line(tokens[f.start].start)+1}
    routes, chunks = [], []
    for i, t in enumerate(tokens):
        if t.value not in ('if', 'match'):
            continue
        j = i + 1
        while j < len(tokens) and tokens[j].value not in ('{', ';'):
            j += 1
        values = [v.value for v in tokens[i+1:j]]
        if any(SENSITIVE.search(v) for v in values):
            record = {'line':line(t.start), 'condition':' '.join(values)}
            (chunks if any(CHUNK.search(v) for v in values) else routes).append(record)
    return {'production_lines':len(lines), 'functions':functions, 'dispatch_candidates':routes, 'chunk_candidates':chunks}


def inventory(root=ROOT):
    files, metrics = {}, {}
    for crate in CRATES:
        prefix = root/'crates'/crate/'src'
        total = Counter()
        for path, source, tokens in modules(prefix, prefix/'lib.rs'):
            key = path.relative_to(root).as_posix()
            item = file_metrics(source, tokens)
            files[key] = item
            total['production_lines'] += item['production_lines']
            total['files_over_1500'] += item['production_lines'] > 1500
            total['functions_over_150'] += sum(v['lines'] > 150 for v in item['functions'].values())
            total['dispatch_candidates'] += len(item['dispatch_candidates'])
            total['chunk_candidates'] += len(item['chunk_candidates'])
        metrics[crate] = dict(total)
    return {'schema':1, 'metrics':metrics, 'files':dict(sorted(files.items()))}


def measured(current):
    """Only these values ratchet; positions and condition text remain review aids."""
    result = {f'{crate}/{key}':value for crate, item in current['metrics'].items() for key,value in item.items()}
    for path, item in current['files'].items():
        if item['production_lines'] > 1500:
            result[f'file/{path}'] = item['production_lines']
        for name, f in item['functions'].items():
            if f['lines'] > 150:
                result[f'function/{path}/{name}'] = f['lines']
    return result


def growth_errors(old, new, exceptions):
    before, after = measured(old), measured(new)
    allowed = {x['metric']:x for x in exceptions}
    if len(allowed) != len(exceptions):
        return ['duplicate growth exceptions']
    errors, used = [], set()
    for key, value in after.items():
        if value <= before.get(key, 0):
            continue
        if key.startswith(('file/', 'function/')) and key not in before:
            errors.append(f'new size violation: {key} = {value}')
            continue
        item = allowed.get(key, {})
        if item.get('before') != before.get(key, 0) or item.get('after') != value or not item.get('reason','').strip():
            errors.append(f'unexplained growth: {key}: {before.get(key, 0)} -> {value}')
        else:
            used.add(key)
    errors.extend(f'unused exception: {key}' for key in allowed.keys()-used)
    return errors


def read_baseline(revision):
    result = subprocess.run(['git','show',f'{revision}:{BASELINE}'],cwd=ROOT,text=True,capture_output=True)
    if result.returncode == 0:
        return json.loads(result.stdout)
    # One audited relocation of the same inventory; fail closed for every other absence.
    previous_path = 'docs/cleanup/complexity.json'
    old = subprocess.run(['git','show',f'{revision}:{previous_path}'],cwd=ROOT,text=True,capture_output=True)
    return json.loads(old.stdout) if old.returncode == 0 else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write-inventory', type=Path, help='write measured data for review; never approves growth')
    parser.add_argument('--base', help='Git revision to audit; defaults to HEAD for an uncommitted inventory, otherwise HEAD^')
    args = parser.parse_args()
    current = inventory()
    if args.write_inventory:
        args.write_inventory.write_text(json.dumps(current, indent=2, sort_keys=True)+'\n')
        print(json.dumps(current['metrics'], indent=2))
        return
    baseline = json.loads((ROOT/BASELINE).read_text())
    if args.base is None:
        head = read_baseline('HEAD')
        # Pre-commit checks audit this working transition, not the previous
        # commit's transition. Clean CI checkouts still compare HEAD with HEAD^.
        args.base = 'HEAD' if head is not None and head != baseline else 'HEAD^'
    errors = []
    if current != baseline['inventory']:
        errors.append('inventory differs; review measurements, lower reductions, and declare scoped growth')
    parent = read_baseline(args.base)
    if subprocess.run(['git','cat-file','-e',f'{args.base}^{{commit}}'],cwd=ROOT,capture_output=True).returncode:
        errors.append('parent revision unavailable; fetch at least two commits before checking the ratchet')
    if parent is not None:
        previous = parent
        # Exceptions describe this baseline transition, not permanent allowances.
        if previous != baseline:
            errors += growth_errors(previous['inventory'], current, baseline.get('growth',[]))
    elif baseline.get('initial_source') != 'c4a98e44d0584548c6b7c19b10725767473f44d2':
        errors.append('missing parent baseline; only the declared M0 initial inventory may bootstrap')
    if errors:
        raise SystemExit('\n'.join(errors))
    print('Complexity ratchet: PASS '+json.dumps(current['metrics'],sort_keys=True))


if __name__ == '__main__': main()
