"""Bounded accessor diagnostics; never substitutes for release qualification.

Uses unchanged plugins and temporary copies of each ado tree. Stata timers
partition frontend setup, the complete plugin call, and frontend publication.
The existing OLS runtime snapshot is kept separately because it excludes the
native publication/destruction tail. All configured observations are retained.
"""
from __future__ import annotations

import csv
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess

from parallel_input import ROOT, stata_string

FIELDS = ('arm', 'placement', 'host_mode', 'host_processors', 'block', 'cell',
          'threads', 'rep', 'n', 'wall_s', 'frontend_pre_s', 'plugin_s',
          'frontend_post_s', 'load_s', 'compute_s', 'snapshot_s')


def instrument_ado(text: str) -> str:
    """Time the sole substantive call without changing its arguments or order."""
    lines = text.splitlines()
    starts = [i for i, line in enumerate(lines) if line.startswith("    plugin call `plugin_handle'")]
    if len(starts) != 1:
        raise ValueError('expected one substantive plugin call')
    first = starts[0]
    last = first
    while lines[last].rstrip().endswith('///'):
        last += 1
    before = ['    timer off 98', '    quietly timer list 98',
              '    scalar __diag_pre = r(t98)', '    timer clear 97', '    timer on 97']
    after = ['    timer off 97', '    quietly timer list 97',
             '    scalar __diag_plugin = r(t97)', '    timer clear 99', '    timer on 99']
    lines[first:last + 1] = before + lines[first:last + 1] + after
    version = next(i for i, line in enumerate(lines) if line.strip().startswith('version '))
    lines[version + 1:version + 1] = ['    timer clear 98', '    timer on 98']
    end = lines.index('end')
    lines[end:end] = ['    timer off 99', '    quietly timer list 99',
                     '    scalar __diag_post = r(t99)']
    return '\n'.join(lines) + '\n'


def commands():
    return dict(re.findall(r'^local (c\d+) "(.*)"$',
                           (ROOT / 'bench/parallel_input.do').read_text(), re.M))


def run_process(config, output, arm, placement, mode, block, *, preflight):
    name = f'{arm}-{placement}-{mode}-{block}'
    work = output / name
    work.mkdir()
    ado = output / f'ado-{arm}'
    trace = work / 'trace.txt'
    science = work / 'science.txt'
    rows = output / 'results.csv'
    lines = ['version 17.0', 'clear all', 'set more off']
    for directory in ('personal', 'plus', 'oldplace', 'site'):
        (work / directory).mkdir()
        lines += [f'sysdir set {directory.upper()} {stata_string(work/directory)}']
    lines += [f'adopath ++ {stata_string(ado)}',
              f'adopath ++ {stata_string(Path(config["arms"][arm]["plugin"]).parent)}',
              f'adopath ++ {stata_string(ado)}',
              'local default_processors = c(processors)']
    if mode == 'serial':
        lines += ['set processors 1']
    lines += ['local host_processors = c(processors)',
              'display "DIAGNOSTIC_HOST|default=`default_processors\'|actual=`host_processors\'"',
              'tempname handle',
              f'file open `handle\' using {stata_string(rows)}, write append text']
    cell_commands = commands()
    for cell in config['cells']:
        data = 'inputs_10m.dta' if cell in ('c6', 'c11') else 'inputs_1m.dta'
        lines += [f'quietly use {stata_string(Path(config["data_dir"])/data)}, clear']
        command = cell_commands[cell]
        ppml = command.startswith('feppml')
        for workers in config['threads']:
            if preflight:
                lines += [f'file open diagtrace using {stata_string(trace)}, write append text',
                          f'file write diagtrace "DIAGNOSTIC_CELL|{cell}|{workers}" _n',
                          'file close diagtrace', 'capture profiler clear', 'profiler on',
                          f'quietly {command} threads({workers})', 'profiler off',
                          f'display "DIAGNOSTIC_PROFILE|{cell}|{workers}"', 'profiler report',
                          f'mata: fh=fopen({stata_string(science)}, "a"); fput(fh,"{cell}|{workers}|"+invtokens(strofreal(vec(st_matrix("e(b)"))\',"%21x"))+"|"+invtokens(strofreal(vec(st_matrix("e(V)"))\',"%21x"))+"|"+strofreal(st_numscalar("e(N)"),"%21x")); fclose(fh)']
                continue
            lines += [f'quietly {command} threads({workers})',
                      f'forvalues r = 1/{config["reps"]} {{',
                      '    timer clear 1', '    timer on 1',
                      f'    quietly {command} threads({workers})', '    timer off 1',
                      '    quietly timer list 1', '    local wall = r(t1)']
            if ppml:
                lines += ['    local load = .', '    local compute = .',
                          '    local snapshot = el(e(feppml_runtime),1,1)']
            else:
                lines += ['    local load = el(e(fereg_runtime),19,2)',
                          '    local compute = el(e(fereg_runtime),20,2)',
                          '    local snapshot = el(e(fereg_runtime),21,2)']
            lines += [f'    file write `handle\' "{arm},{placement},{mode},`host_processors\',{block},{cell},{workers},`r\',`e(N)\',`wall\',`=scalar(__diag_pre)\',`=scalar(__diag_plugin)\',`=scalar(__diag_post)\',`load\',`compute\',`snapshot\'" _n', '}']
    lines += ['file close `handle\'', 'display "END_DIAGNOSTIC|rc=0"']
    (work / 'run.do').write_text('\n'.join(lines) + '\n')
    env = dict(os.environ)
    for key in ('FEREG_SOLVER_TRACE', 'FEREG_SOLVER_TRACE_FILE', 'FEREG_PHASE_TIMING'):
        env.pop(key, None)
    if preflight:
        env.update(FEREG_SOLVER_TRACE='1', FEREG_SOLVER_TRACE_FILE=str(trace), FEREG_PHASE_TIMING='1')
    argv = [config['stata'], '-q', '-b', 'do', 'run.do']
    if placement == 'interleave':
        argv = ['numactl', '--interleave=all'] + argv
    if preflight and config.get('perf_stat'):
        argv = ['perf', 'stat', '-x,', '-o', str(work/'perf.csv'), '-e',
                'task-clock,context-switches,cpu-migrations,page-faults,cycles:u,instructions:u', '--'] + argv
    sample_hz = config.get('sample_hz') if not preflight else None
    if sample_hz:
        argv = ['perf', 'record', '-F', str(sample_hz), '-e', 'cycles:u',
                '--call-graph', 'dwarf,8192', '-o', str(work/'perf.data'), '--'] + argv
    with (work / 'process.log').open('w') as stream:
        result = subprocess.run(argv, cwd=work, env=env, stdout=stream, stderr=subprocess.STDOUT)
    log = (work / 'run.log').read_text(errors='replace') if (work/'run.log').exists() else ''
    if result.returncode or 'END_DIAGNOSTIC|rc=0' not in log:
        raise RuntimeError(f'{name} failed (rc={result.returncode}): {log[-2000:]}')
    if preflight:
        digests = re.findall(r'phase=input_digest\|[^\n]*digest=([^|\n]+)', trace.read_text())
        if len(digests) != len(config['cells']) * len(config['threads']):
            raise RuntimeError(f'{name}: incomplete input digests')
        return science.read_text(), digests
    if sample_hz:
        for mode in ('self', 'children'):
            args = ['perf', 'report', '-i', str(work/'perf.data'), '--stdio',
                    '--percent-limit', '0.05', '--sort', 'dso,symbol',
                    '--no-children' if mode == 'self' else '--children']
            if mode == 'self':
                args += ['-g', 'none']
            with (work/f'perf-{mode}.txt').open('w') as stream:
                subprocess.run(args, stdout=stream, stderr=subprocess.STDOUT, check=True)
    print(f'{name}: completed', flush=True)


def summarize(config, output):
    with (output/'results.csv').open() as stream:
        rows = list(csv.DictReader(stream))
    for row in rows:
        for metric in FIELDS[9:]:
            if row[metric] == '.' and metric in ('load_s', 'compute_s'):
                continue
            value = float(row[metric])
            if not math.isfinite(value) or value < 0:
                raise RuntimeError(f'invalid diagnostic timing: {metric}={row[metric]}')
    expected = set(itertools.product(config['arms'], config['placements'], config['host_modes'],
                                    range(1, config['blocks']+1), config['cells'], config['threads'],
                                    range(1, config['reps']+1)))
    ids = [(x['arm'],x['placement'],x['host_mode'],int(x['block']),x['cell'],int(x['threads']),int(x['rep'])) for x in rows]
    if len(set(ids)) != len(ids) or set(ids) != expected:
        raise RuntimeError('missing or duplicate diagnostic observations')
    groups = []
    for placement,mode,cell,threads in itertools.product(config['placements'],config['host_modes'],config['cells'],config['threads']):
        group = {'placement':placement,'host_mode':mode,'cell':cell,'threads':threads}
        for arm in config['arms']:
            selected = [x for x in rows if (x['arm'],x['placement'],x['host_mode'],x['cell'],int(x['threads'])) == (arm,placement,mode,cell,threads)]
            metrics = {}
            for metric in FIELDS[9:]:
                values = [float(x[metric]) for x in selected if x[metric] != '.']
                if values:
                    metrics[metric] = statistics.median(values)
            metrics['plugin_tail_s'] = statistics.median(float(x['plugin_s'])-float(x['snapshot_s']) for x in selected)
            metrics['outside_plugin_s'] = statistics.median(float(x['wall_s'])-float(x['plugin_s']) for x in selected)
            metrics['host_processors'] = sorted({int(x['host_processors']) for x in selected})
            group[arm] = metrics
        groups.append(group)
    result = {'status':'passed','purpose':'diagnosis only; no release acceptance or replacement of failed evidence',
              'observations':len(rows),'resampling':False,'rows':groups}
    (output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    return result


def command_diagnose(config):
    output = Path(config['output']).resolve()
    output.mkdir(parents=True, exist_ok=False)
    if config['blocks'] % 2 or config['blocks'] < 2 or config['reps'] < 1:
        raise ValueError('diagnostics require mirrored blocks and positive repetitions')
    if set(config['arms']) != {'incumbent','candidate'} or not set(config['cells']) <= commands().keys():
        raise ValueError('unexpected arms or cells')
    pins = {}
    for arm,spec in config['arms'].items():
        dest = output/f'ado-{arm}'
        shutil.copytree(spec['ado'],dest)
        for path in sorted(dest.glob('*')):
            if path.is_file():
                pins[f'{arm}/ado/{path.name}'] = hashlib.sha256(path.read_bytes()).hexdigest()
        pins[f'{arm}/plugin'] = hashlib.sha256(Path(spec['plugin']).read_bytes()).hexdigest()
        for name in ('_fereg_estimate_ols.ado','_fereg_estimate_ppml.ado'):
            p=dest/name
            p.write_text(instrument_ado(p.read_text()))
    for path in sorted(Path(config['data_dir']).glob('*.dta')):
        pins[path.name]=hashlib.sha256(path.read_bytes()).hexdigest()
    (output/'plan.json').write_text(json.dumps({'config':config,'pins':pins},indent=2)+'\n')
    (output/'results.csv').write_text(','.join(FIELDS)+'\n')
    reference = None
    for placement,mode,arm in itertools.product(config['placements'],config['host_modes'],config['arms']):
        science = run_process(config,output,arm,placement,mode,0,preflight=True)
        if reference is None:
            reference = science
        elif science != reference:
            raise RuntimeError('diagnostic preflight changed coefficient/covariance/sample-count bits or input digests')
    (output/'preflight.json').write_text(json.dumps({'exact_science_and_input_bits':True,'variants':len(config['placements'])*len(config['host_modes'])*len(config['arms'])})+'\n')
    from qualify_scc import HostMonitor
    with HostMonitor(output, max(config['threads'])):
        variants = list(itertools.product(config['placements'],config['host_modes']))
        for block in range(1,config['blocks']+1):
            sequence = variants if block % 2 else list(reversed(variants))
            for placement,mode in sequence:
                for arm in (('candidate','incumbent') if block % 2 else ('incumbent','candidate')):
                    run_process(config,output,arm,placement,mode,block,preflight=False)
    return summarize(config,output)
