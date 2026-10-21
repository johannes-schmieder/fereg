#!/usr/bin/env python3
"""Setup attribution for the accepted and candidate CMG setup arms.

Distinguishes two explanations of the candidate's broad hierarchy phase:
counted serial raw-edge sorting work per hierarchy level and call site
(``counts`` builds), and the split of the hierarchy/finalization phase into
existing CMG boundaries (``timers`` builds). The two dimensions are separate
builds; counter-build wall time is never timing evidence.

Subcommands (no timing, sampling or production change):

  prepare --arm accepted|candidate --mode counts|timers --fereg-git REPO
          --cmg-git REPO --out DIR
      Export the exact pinned fereg and CMG commits with ``git archive`` into a
      private DIR, apply root's unchanged ``cmg_setup_profile.patch`` and the
      exact-anchor attribution edits, redirect the copy's CMG dependency to the
      patched copy (manifest and lockfile), and write a unified diff, a
      deterministic source archive and ``attribution-source.json``. Every
      anchor must occur exactly once; wrong revisions or files fail closed.
  bind DIR --plugin P --build-log L --baseline-sha256 H
      After the caller builds DIR/fereg (``./scripts/build_*.sh``), write
      DIR/profile-source.json in the format ``cmg_setup_profile.py`` binds.
  parse LOG --arm A --mode M
      Validate one call's attribution and six-phase profile trace lines.
  run REPOSITORY TASK_ROOT OUTPUT --arm accepted_hidden|setup_hidden
      --revision REV --source DIR/profile-source.json
      --attribution DIR/attribution-source.json --plugin PLUGIN [--plan --baseline]
      Bind every input through ``cmg_setup_profile.load_context`` and run its
      fixed 18 calls (high-degree-weak and weak-low at W1/W4/W8, one warmup and
      two recorded calls each) with exact science, signature, route and
      hierarchy checks plus the attribution checks below.

Trace phases are inclusive (nested) timers; ``exclusive_ms`` gives disjoint
pieces whose sum is the fereg hierarchy phase. The caller owns host
isolation, builds and build provenance.
"""
from pathlib import Path
import argparse
import hashlib
import io
import json
import math
import os
import platform
import re
import subprocess
import sys
import tarfile

HERE = Path(__file__).resolve().parent
MODULE = HERE / 'cmg_setup_attribution' / 'setup_attribution.rs'
SMOKE_SOURCE = HERE / 'cmg_setup_attribution' / 'smoke.rs'
SMOKE_MANIFEST = HERE / 'cmg_setup_attribution' / 'smoke.toml'
PROFILE_PATCH = HERE / 'cmg_setup_profile.patch'
PROFILE_PATCH_SHA256 = '571f8e1f724c4f24806068f7ac2e1fa0eec0d1cadf5913949c9e3c3f0618ac95'
CMG_URL = 'https://github.com/johannes-schmieder/CMG'

ARMS = {
    'accepted': {'hidden': 'accepted_hidden',
                 'fereg': '3f9d54eeacb2ced61bf8cb9a629daa0d1a6d23d9',
                 'cmg': '35a095f3e26972fcfa9e58c2222a1098efb8e607'},
    'candidate': {'hidden': 'setup_hidden',
                  'fereg': '0407060c2959ff5076619511fcc85d82ceac8de0',
                  'cmg': '355d328a33639b5fa50c3c1954c8ee62e59e6b20'},
}
MODES = ('counts', 'timers')

# Pristine SHA-256 of every edited file, per arm.
PRISTINE = {
    'accepted': {
        'cmg/src/lib.rs': '747c097772854e41177a05b5ec0890894f73843f9abe83e873b950f82f3ec172',
        'cmg/src/cancel.rs': '8d5e5150fa56ac38a0aff18571ffd33f74019d12763a18f9e04b1e31470ee76b',
        'cmg/src/graph.rs': '93647dc8eeb8a34d3fae25c5bc075b443c51359a1901352eddc1705cbdeed8cd',
        'cmg/src/graph/hosted.rs': '8969cc3dd228ae85f43fd2f4a5a306536586fa89a7f6811ef1232ffc68e26036',
        'cmg/src/hierarchy.rs': 'cae23d52f96308257ecec5d4c8dc0155643b3cb6185193d47b237675b6f82125',
        'cmg/src/preconditioner.rs': '7118fd352608a8f70f60a1643a8d5ece9c24afcafc5ecf5967a8f2824613ad66',
        'fereg/crates/fereg-core/src/fe/cmg_schur/bounded.rs':
            '6289b08fa76aafa5c66ab8e96f8e6813c5427dae1e622f3ae45f93262c26e6b9',
        'fereg/crates/fereg-core/Cargo.toml': 'ee7da3906b5d279d1f4361cecccad20ab4968ec623b543ac11e52f7a6858df3a',
        'fereg/Cargo.lock': '10f6826f082a0b2f04e3e8c8819ff1b050fbea9b5e16211b1f24403114663055',
    },
    'candidate': {
        'cmg/src/lib.rs': '747c097772854e41177a05b5ec0890894f73843f9abe83e873b950f82f3ec172',
        'cmg/src/cancel.rs': '2e427d4846e557e8f73657cb9685496841541f23d4419fd8edf71330f4d9f719',
        'cmg/src/graph.rs': 'a7c653788cc8bc995af30ef1f776acfdde55641b7f9f140965fa440be455901e',
        'cmg/src/graph/hosted.rs': 'de868aec2a6a01d2df7e3327b7a43fa821909ca728e5ebd7c35ed9843574b870',
        'cmg/src/graph/edge_sort.rs': 'a123a2a791b40a5348bb78985e5553001f01662d7fdb4e4201a1b94cf19257fc',
        'cmg/src/hierarchy.rs': 'cae23d52f96308257ecec5d4c8dc0155643b3cb6185193d47b237675b6f82125',
        'cmg/src/preconditioner.rs': '7118fd352608a8f70f60a1643a8d5ece9c24afcafc5ecf5967a8f2824613ad66',
        'fereg/crates/fereg-core/src/fe/cmg_schur/bounded.rs':
            '6289b08fa76aafa5c66ab8e96f8e6813c5427dae1e622f3ae45f93262c26e6b9',
        'fereg/crates/fereg-core/Cargo.toml': '601a307cd91bf69c8a7afde0f9b479f52eb655240c1d98b61f1b687323d9e2f4',
        'fereg/Cargo.lock': '423f4714550366483955763aa2ae9528b4bc9a74206727273ce5fbfbed6fd8cc',
    },
}

A = 'crate::setup_attribution::'
SERIAL_SCOPE = (f'            let scope = {A}SortScope::begin({A}PATH_SERIAL, raw.len());\n'
                '{sort}'
                '            let scope = scope.sorted();\n'
                '            let graph = Self::from_sorted_edges(vertex_count, raw, true, meter);\n'
                '            scope.finish(&graph);\n'
                '            graph\n')
TEAM_SCOPE = (f'        let scope = {A}SortScope::begin({A}PATH_TEAM, raw.len());\n'
              '{sort}'
              '        let scope = scope.sorted();\n'
              '        let graph = Self::from_sorted_edges(vertex_count, raw, true, meter);\n'
              '        scope.finish(&graph);\n'
              '        graph\n')
HOSTED_SCOPE = (f'    let scope = {A}SortScope::begin({A}PATH_HOSTED, raw.len());\n'
                '{sort}'
                '    let scope = scope.sorted();\n'
                '    let graph = Laplacian::from_sorted_edges(vertex_count, raw, true, meter);\n'
                '    scope.finish(&graph);\n'
                '    graph\n')


def scope(template, sort):
    return template.replace('{sort}', sort)


# (file, anchor, replacement): applied in order; each anchor occurs exactly once.
COMMON_CMG = [
    ('cmg/src/lib.rs', 'mod cancel;\n', 'mod cancel;\n#[doc(hidden)]\npub mod setup_attribution;\n'),
    ('cmg/src/cancel.rs',
     "    fn poll(&mut self, phase: &'static str) -> Result<(), CmgError> {\n        self.pending = 0;\n",
     "    fn poll(&mut self, phase: &'static str) -> Result<(), CmgError> {\n        self.pending = 0;\n"
     f'        {A}count({A}QUERIES, 1);\n'),
    ('cmg/src/cancel.rs', '        self.pending += u32::from(units);\n',
     f'        {A}count({A}METER_UNITS, u64::from(units));\n        self.pending += u32::from(units);\n'),
    ('cmg/src/cancel.rs', '        loop {\n            progress.advance(phase, 64)?;\n',
     f'        loop {{\n            {A}count({A}HEAP_SIFT_STEPS, 1);\n            progress.advance(phase, 64)?;\n'),
    ('cmg/src/cancel.rs', '            values.swap(root, largest);\n',
     f'            {A}count({A}HEAP_SWAPS, 1);\n            values.swap(root, largest);\n'),
    ('cmg/src/cancel.rs', '        values.swap(0, end);\n',
     f'        {A}count({A}HEAP_SWAPS, 1);\n        values.swap(0, end);\n'),
    ('cmg/src/cancel.rs', '    progress.advance(phase, 16)?;\n    for root in (0..values.len() / 2).rev() {\n',
     f'    {A}count({A}HEAPSORTS, 1);\n    {A}count({A}HEAPSORT_RECORDS, values.len() as u64);\n'
     '    progress.advance(phase, 16)?;\n    for root in (0..values.len() / 2).rev() {\n'),
    ('cmg/src/graph.rs', 'fn compare_raw_edges(left: &Edge, right: &Edge) -> core::cmp::Ordering {\n',
     '/// Setup attribution: the canonical raw-edge comparator, its call counted.\n'
     '#[inline(always)]\n'
     'pub(crate) fn counted_compare_raw_edges(left: &Edge, right: &Edge) -> core::cmp::Ordering {\n'
     f'    {A}count({A}COMPARISONS, 1);\n    compare_raw_edges(left, right)\n}}\n\n'
     'fn compare_raw_edges(left: &Edge, right: &Edge) -> core::cmp::Ordering {\n'),
    ('cmg/src/hierarchy.rs',
     ') -> Output {\n    if PROFILE {\n        let start = Instant::now();\n        let output = operation();\n'
     '        debug_assert!(records.len() < records.capacity());\n        records.push(HierarchyPhaseRecord {\n'
     '            level,\n            phase,\n            nanoseconds: start.elapsed().as_nanos(),\n        });\n'
     '        output\n    } else {\n        operation()\n    }\n}\n',
     f') -> Output {{\n    let attribution = {A}Timer::start();\n    let output = if PROFILE {{\n'
     '        let start = Instant::now();\n        let output = operation();\n'
     '        debug_assert!(records.len() < records.capacity());\n        records.push(HierarchyPhaseRecord {\n'
     '            level,\n            phase,\n            nanoseconds: start.elapsed().as_nanos(),\n        });\n'
     '        output\n    } else {\n        operation()\n    };\n'
     '    attribution.stop_phase(phase, level);\n    output\n}\n'),
    ('cmg/src/hierarchy.rs',
     '                "coarse_edge_map_sort_merge_and_graph_finalization",\n'
     '                || aggregation.contract_with(&current, team, meter),\n',
     '                "coarse_edge_map_sort_merge_and_graph_finalization",\n'
     '                || {\n'
     f'                    let _site = {A}contraction_site(level_index);\n'
     '                    aggregation.contract_with(&current, team, meter)\n'
     '                },\n'),
    ('cmg/src/hierarchy.rs',
     '                match crate::component_experiment::prune(&aggregation, &coarse, meter)? {\n',
     f'                match {A}timed_phase("coarse_isolate_pruning", level_index, || {{\n'
     '                    crate::component_experiment::prune(&aggregation, &coarse, meter)\n'
     '                })? {\n'),
    ('cmg/src/preconditioner.rs',
     '        let (hierarchy, phases) = CmgHierarchy::build_metered::<M, PROFILE>(\n'
     '            graph,\n            options.hierarchy,\n'
     '            options.level_policy(context.budget_bytes.is_some()),\n'
     '            context.team(),\n            meter,\n        )?;\n',
     f'        let (hierarchy, phases) = {A}timed_phase("hierarchy_total", 0, || {{\n'
     '            CmgHierarchy::build_metered::<M, PROFILE>(\n'
     '                graph,\n                options.hierarchy,\n'
     '                options.level_policy(context.budget_bytes.is_some()),\n'
     '                context.team(),\n                meter,\n            )\n        })?;\n'),
    ('cmg/src/preconditioner.rs',
     '        let preconditioner =\n'
     '            Self::finalize(hierarchy, options.factor_terminal_components(), meter)?;\n',
     f'        let preconditioner = {A}timed_phase("finalize_total", 0, || {{\n'
     '            Self::finalize(hierarchy, options.factor_terminal_components(), meter)\n        })?;\n'),
    ('cmg/src/preconditioner.rs',
     '        match context.cancel {\n            Some(cancel) => Self::build_metered::<_, false>(\n'
     '                graph,\n                options,\n                context,\n'
     '                &mut crate::cancel::CancelMeter::new(cancel),\n            ),\n'
     '            None => Self::build_metered::<_, false>(graph, options, context, &mut Unmetered),\n'
     '        }\n        .map(|(preconditioner, _)| preconditioner)\n',
     f'        {A}timed_phase("build_with_total", 0, || {{\n'
     '            match context.cancel {\n                Some(cancel) => Self::build_metered::<_, false>(\n'
     '                    graph,\n                    options,\n                    context,\n'
     '                    &mut crate::cancel::CancelMeter::new(cancel),\n                ),\n'
     '                None => Self::build_metered::<_, false>(graph, options, context, &mut Unmetered),\n'
     '            }\n            .map(|(preconditioner, _)| preconditioner)\n        })\n'),
    ('cmg/src/preconditioner.rs', '        meter.work("preconditioner finest components", 64)?;\n',
     '        meter.work("preconditioner finest components", 64)?;\n'
     f'        let attribution = {A}Timer::start();\n'),
    ('cmg/src/preconditioner.rs',
     '        let mut coarse_centering =\n'
     '            meter.try_capacity(hierarchy.levels().len() - 1, "coarse centering plans")?;\n',
     '        attribution.stop_phase("finest_components", 0);\n'
     f'        let attribution = {A}Timer::start();\n'
     '        let mut coarse_centering =\n'
     '            meter.try_capacity(hierarchy.levels().len() - 1, "coarse centering plans")?;\n'),
    ('cmg/src/preconditioner.rs', '        meter.work("preconditioner terminal decision", 64)?;\n',
     '        attribution.stop_phase("coarse_centering", 0);\n'
     '        meter.work("preconditioner terminal decision", 64)?;\n'),
    ('cmg/src/preconditioner.rs',
     '            Some(GroundedLdl::factor_metered(\n                terminal.graph(),\n'
     '                factor_terminal_components,\n                meter,\n            )?)\n',
     f'            Some({A}timed_phase("terminal_factor", 0, || {{\n'
     '                GroundedLdl::factor_metered(terminal.graph(), factor_terminal_components, meter)\n'
     '            })?)\n'),
]

ARM_CMG = {
    'accepted': [
        ('cmg/src/graph.rs',
         '            meter.sort_by(&mut raw, compare_raw_edges, "coarse edge sorting")?;\n'
         '            Self::from_sorted_edges(vertex_count, raw, true, meter)\n',
         scope(SERIAL_SCOPE,
               '            meter.sort_by(&mut raw, counted_compare_raw_edges, "coarse edge sorting")?;\n')),
        ('cmg/src/graph.rs',
         '        team_sort::sort(&mut raw, team, meter)?;\n'
         '        Self::from_sorted_edges(vertex_count, raw, true, meter)\n',
         scope(TEAM_SCOPE, '        team_sort::sort(&mut raw, team, meter)?;\n')),
        ('cmg/src/graph/hosted.rs',
         '    sort(&mut raw, meter, team)?;\n'
         '    // The spare sort bank has been dropped; reduce in the canonical order.\n'
         '    Laplacian::from_sorted_edges(vertex_count, raw, true, meter)\n',
         scope(HOSTED_SCOPE, '    sort(&mut raw, meter, team)?;\n')),
    ],
    'candidate': [
        ('cmg/src/graph.rs',
         '            edge_sort::sort_serial(&mut raw, meter, "coarse edge sorting")?;\n'
         '            Self::from_sorted_edges(vertex_count, raw, true, meter)\n',
         scope(SERIAL_SCOPE, '            edge_sort::sort_serial(&mut raw, meter, "coarse edge sorting")?;\n')),
        ('cmg/src/graph.rs',
         '        edge_sort::sort(&mut raw, team, meter)?;\n'
         '        Self::from_sorted_edges(vertex_count, raw, true, meter)\n',
         scope(TEAM_SCOPE, '        edge_sort::sort(&mut raw, team, meter)?;\n')),
        ('cmg/src/graph/hosted.rs',
         '    edge_sort::sort(&mut raw, team, meter)?;\n'
         '    Laplacian::from_sorted_edges(vertex_count, raw, true, meter)\n',
         scope(HOSTED_SCOPE, '    edge_sort::sort(&mut raw, team, meter)?;\n')),
        ('cmg/src/graph/edge_sort.rs', 'use super::{Edge, compare_raw_edges};\n',
         '#[cfg(test)]\nuse super::compare_raw_edges;\nuse super::{Edge, counted_compare_raw_edges};\n'),
        ('cmg/src/graph/edge_sort.rs',
         '            return heapsort(poll, values, compare_raw_edges, phase);\n',
         '            return heapsort(poll, values, counted_compare_raw_edges, phase);\n'),
        ('cmg/src/graph/edge_sort.rs',
         '            return heapsort(&mut node, values, compare_raw_edges, PHASE);\n',
         '            return heapsort(&mut node, values, counted_compare_raw_edges, PHASE);\n'),
        ('cmg/src/graph/edge_sort.rs',
         "    phase: &'static str,\n) -> Result<(), CmgError> {\n    for index in 1..values.len() {\n",
         "    phase: &'static str,\n) -> Result<(), CmgError> {\n"
         f'    {A}count({A}INSERTION_LEAVES, 1);\n'
         f'    {A}count({A}INSERTION_RECORDS, values.len() as u64);\n'
         '    for index in 1..values.len() {\n'),
        ('cmg/src/graph/edge_sort.rs',
         '        while current > 0 && compare_raw_edges(&values[current], &values[current - 1]).is_lt() {\n'
         '            values.swap(current, current - 1);\n',
         '        while current > 0\n'
         '            && counted_compare_raw_edges(&values[current], &values[current - 1]).is_lt()\n'
         '        {\n'
         f'            {A}count({A}INSERTION_SWAPS, 1);\n'
         '            values.swap(current, current - 1);\n'),
        ('cmg/src/graph/edge_sort.rs',
         '    let _probe = probe::enter(values.len());\n',
         f'    let _probe = probe::enter(values.len());\n    {A}count({A}PARTITIONS, 1);\n'),
        ('cmg/src/graph/edge_sort.rs', '    candidates.sort_unstable_by(compare_raw_edges);\n',
         '    candidates.sort_unstable_by(counted_compare_raw_edges);\n'),
        ('cmg/src/graph/edge_sort.rs', '        match compare_raw_edges(&values[index], &pivot) {\n',
         f'        {A}count({A}PARTITION_STEPS, 1);\n'
         '        match counted_compare_raw_edges(&values[index], &pivot) {\n'),
        ('cmg/src/graph/edge_sort.rs', '                values.swap(less, index);\n',
         f'                {A}count({A}PARTITION_SWAPS, 1);\n                values.swap(less, index);\n'),
        ('cmg/src/graph/edge_sort.rs', '                values.swap(index, greater);\n',
         f'                {A}count({A}PARTITION_SWAPS, 1);\n                values.swap(index, greater);\n'),
    ],
}

FEREG = [
    ('fereg/crates/fereg-core/src/fe/cmg_schur/bounded.rs',
     '            work.setup_profile = Some(SetupProfile::new());\n',
     '            work.setup_profile = Some(SetupProfile::new());\n'
     '            cmg::setup_attribution::reset();\n'),
    ('fereg/crates/fereg-core/src/fe/cmg_schur/bounded.rs',
     '            profile.emit(outcome, membership.nobs());\n',
     '            profile.emit(outcome, membership.nobs());\n'
     '            crate::diagnostics::emit_solver_trace(&format!(\n'
     '                "FEREG_SOLVER_TRACE|phase=auto_schur_setup_attribution|outcome={outcome}|{}",\n'
     '                cmg::setup_attribution::trace_fields()\n            ));\n'),
]


def dependency_edits(arm):
    rev = ARMS[arm]['cmg']
    return [
        ('fereg/crates/fereg-core/Cargo.toml',
         f'cmg = {{ git = "{CMG_URL}", rev = "{rev}", features = ["parallel", "experimental-components"] }}\n',
         'cmg = { path = "../../../cmg", features = ["parallel", "experimental-components"] }\n'),
        ('fereg/Cargo.lock',
         f'name = "cmg"\nversion = "0.1.0"\nsource = "git+{CMG_URL}?rev={rev}#{rev}"\n',
         'name = "cmg"\nversion = "0.1.0"\n'),
    ]


MODE_LINES = {
    'counts': ('pub(crate) const COUNTS: bool = true;\n', 'pub(crate) const TIMERS: bool = false;\n',
               'const MODE: &str = "counts";\n'),
    'timers': ('pub(crate) const COUNTS: bool = false;\n', 'pub(crate) const TIMERS: bool = true;\n',
               'const MODE: &str = "timers";\n'),
}
MODULE_ANCHORS = ('pub(crate) const COUNTS: bool = false;\n', 'pub(crate) const TIMERS: bool = false;\n',
                  'const MODE: &str = "unset";\n')


class AttributionError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise AttributionError(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def file_sha256(path):
    return sha256(Path(path).read_bytes())


def apply_edit(text, anchor, replacement, label):
    """Replace the single occurrence of `anchor`; fail closed otherwise."""
    count = text.count(anchor)
    require(count == 1, f'{label}: anchor occurs {count} times')
    return text.replace(anchor, replacement)


def module_source(mode):
    require(mode in MODES, 'unknown attribution mode')
    text = MODULE.read_text()
    for anchor, line in zip(MODULE_ANCHORS, MODE_LINES[mode]):
        text = apply_edit(text, anchor, line, 'module mode')
    return text


def edits(arm):
    require(arm in ARMS, 'unknown attribution arm')
    return [*COMMON_CMG, *ARM_CMG[arm], *FEREG, *dependency_edits(arm)]


def git(repo, *args, binary=False):
    result = subprocess.run(['git', '-C', str(repo), *args], capture_output=True, check=False)
    require(result.returncode == 0, f'git {args[0]} failed in {repo}: {result.stderr.decode().strip()}')
    return result.stdout if binary else result.stdout.decode()


def export(repo, revision, target):
    require(git(repo, 'rev-parse', '--verify', f'{revision}^{{commit}}').strip() == revision,
            f'{repo} does not hold commit {revision}')
    target.mkdir(parents=True)
    data = git(repo, 'archive', '--format=tar', revision, binary=True)
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        archive.extractall(target, filter='data')


def deterministic_archive(root, names, path):
    """A tar of `names` under `root`: sorted, zero mtime/uid, regular files."""
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode='w', format=tarfile.PAX_FORMAT) as archive:
        for name in sorted(names):
            data = (root / name).read_bytes()
            info = tarfile.TarInfo(name)
            info.size, info.mtime = len(data), 0
            info.mode = 0o755 if (root / name).stat().st_mode & 0o111 else 0o644
            archive.addfile(info, io.BytesIO(data))
    path.write_bytes(buffer.getvalue())
    return sha256(buffer.getvalue())


def tree_files(root):
    return sorted(str(p.relative_to(root)) for p in root.rglob('*') if p.is_file())


def unified_diff(before, after):
    import difflib
    chunks = []
    for name in sorted(set(before) | set(after)):
        old, new = before.get(name), after.get(name)
        if old == new:
            continue
        chunks.extend(difflib.unified_diff(
            [] if old is None else old.splitlines(keepends=True), new.splitlines(keepends=True),
            '/dev/null' if old is None else f'a/{name}', f'b/{name}'))
    return ''.join(chunks)


def prepare(arm, mode, fereg_git, cmg_git, out):
    require(arm in ARMS and mode in MODES, 'unknown arm or mode')
    require(file_sha256(PROFILE_PATCH) == PROFILE_PATCH_SHA256, 'cmg_setup_profile.patch changed')
    out = Path(out).absolute()
    require(not out.exists(), f'{out} exists')
    export(fereg_git, ARMS[arm]['fereg'], out / 'fereg')
    export(cmg_git, ARMS[arm]['cmg'], out / 'cmg')
    patched = subprocess.run(['patch', '-p1', '--forward', '--no-backup-if-mismatch', '-s', '-i',
                              str(PROFILE_PATCH)], cwd=out / 'fereg', capture_output=True, check=False)
    require(patched.returncode == 0, 'root profile patch did not apply: ' + patched.stdout.decode())
    names = [*PRISTINE[arm]]
    profiled = {name: (out / name).read_text() for name in names}
    # The profile patch edits bounded.rs; its pristine identity is pre-patch.
    pristine = dict(profiled)
    pristine['fereg/crates/fereg-core/src/fe/cmg_schur/bounded.rs'] = git(
        fereg_git, 'show', f"{ARMS[arm]['fereg']}:crates/fereg-core/src/fe/cmg_schur/bounded.rs")
    after = patch_files(pristine, profiled, arm, mode)
    for name, text in after.items():
        (out / name).write_text(text)
    diff = unified_diff(profiled, after)
    (out / 'attribution.diff').write_text(diff)
    sources = [f'fereg/{n}' for n in tree_files(out / 'fereg')] + [f'cmg/{n}' for n in tree_files(out / 'cmg')]
    archive_sha = deterministic_archive(out, sources, out / 'source.tar')
    record = {
        'schema': 1, 'arm': arm, 'hidden_arm': ARMS[arm]['hidden'], 'mode': mode,
        'fereg_revision': ARMS[arm]['fereg'], 'cmg_revision': ARMS[arm]['cmg'],
        'tool_sha256': file_sha256(__file__), 'module_sha256': file_sha256(MODULE),
        'profile_patch_sha256': PROFILE_PATCH_SHA256, 'attribution_diff_sha256': sha256(diff.encode()),
        'source_archive': {'path': 'source.tar', 'sha256': archive_sha},
        'edited_files': {name: sha256(after[name].encode()) for name in sorted(after)},
        'build': 'caller builds DIR/fereg with ./scripts/build_linux.sh or build_macos.sh '
                 '(cargo --locked; the copy depends on DIR/cmg by path)',
    }
    (out / 'attribution-source.json').write_text(json.dumps(record, indent=1, sort_keys=True) + '\n')
    return record


def patch_files(pristine, profiled, arm, mode):
    """Check pristine identities, then edit the profile-patched texts.

    `pristine` holds every edited file at the pinned revision; `profiled` the
    same files after root's profile patch (only bounded.rs differs)."""
    require(set(pristine) == set(profiled) == set(PRISTINE[arm]), 'attribution file set changed')
    for name, digest in PRISTINE[arm].items():
        require(sha256(pristine[name].encode()) == digest, f'{name}: pristine identity mismatch')
    out = dict(profiled)
    for index, (name, anchor, replacement) in enumerate(edits(arm)):
        out[name] = apply_edit(out[name], anchor, replacement, f'{arm} edit {index} ({name})')
    out['cmg/src/setup_attribution.rs'] = module_source(mode)
    return out


# ---------------------------------------------------------------- parsing

SORT_KEYS = ('site', 'level', 'path', 'n', 'out', 'sort_ns', 'canonical_ns', 'complete', 'cmp',
             'partitions', 'partition_steps', 'partition_swaps', 'insertion_leaves',
             'insertion_records', 'insertion_swaps', 'heapsorts', 'heapsort_records',
             'heap_sift_steps', 'heap_swaps', 'meter_units', 'queries')
COUNT_KEYS = SORT_KEYS[8:]
LEVEL_PHASES = ('direct_terminal_check', 'forest_select_split_low_degree_and_label',
                'aggregation_construction', 'hierarchy_bookkeeping_and_fill_checks',
                'coarse_edge_map_sort_merge_and_graph_finalization', 'coarse_isolate_pruning',
                'repeat_count_initialization', 'inverse_diagonal_and_level_finalization')
# Once per build; the report is timed at the last level, the others at 0.
ONCE_PHASES = ('option_validation', 'graph_clone_reference_setup', 'hierarchy_report_bookkeeping',
               'hierarchy_total', 'finest_components', 'coarse_centering', 'finalize_total',
               'build_with_total')
LAST_LEVEL_PHASES = ('hierarchy_report_bookkeeping',)
OPTIONAL_ONCE = ('terminal_factor',)
CONTRACTION = 'coarse_edge_map_sort_merge_and_graph_finalization'
PREFIX = 'FEREG_SOLVER_TRACE|phase=auto_schur_setup_attribution|'
PROFILE_PREFIX = 'FEREG_SOLVER_TRACE|phase=auto_schur_setup_profile|'


def fields_of(line):
    parts = line.rstrip('\n').split('|')
    pairs = [part.split('=', 1) for part in parts[1:]]
    require(all(len(pair) == 2 for pair in pairs), 'malformed trace field')
    keys = [key for key, _ in pairs]
    require(len(keys) == len(set(keys)), 'duplicate trace field')
    return dict(pairs)


def integer(text, what):
    require(re.fullmatch(r'0|[1-9][0-9]*', text) is not None, f'invalid integer for {what}: {text!r}')
    return int(text)


def parse_sort(text):
    pairs = [item.split(':', 1) for item in text.split(',')]
    require(all(len(pair) == 2 for pair in pairs), 'malformed sort record')
    keys = [key for key, _ in pairs]
    require(tuple(keys) == SORT_KEYS, 'sort record keys changed')
    record = dict(pairs)
    require(record['site'] in ('contraction', 'outside'), 'invalid sort site')
    require(record['path'] in ('serial', 'team', 'hosted'), 'invalid sort path')
    return {key: value if key in ('site', 'path') else integer(value, key) for key, value in record.items()}


def parse_timers(text):
    timers = {}
    for item in filter(None, text.split(';')):
        match = re.fullmatch(r'([a-z_]+)@([0-9]+):([0-9]+):([0-9]+)', item)
        require(match is not None, f'malformed timer {item!r}')
        name, level, ns, calls = match.group(1), *map(int, match.groups()[1:])
        require((name, level) not in timers, f'duplicate timer {name}@{level}')
        require(name in LEVEL_PHASES + ONCE_PHASES + OPTIONAL_ONCE, f'unknown timer phase {name}')
        require(calls > 0, f'timer {name}@{level} without calls')
        timers[(name, level)] = {'ns': ns, 'calls': calls}
    return timers


def parse_attribution(fields):
    require(fields.get('phase') == 'auto_schur_setup_attribution', 'not an attribution line')
    expected = {'phase', 'outcome', 'schema', 'mode', 'sorts', 'sort_overflow', 'unknown_phases',
                'level_overflow', 'timers'}
    sorts = integer(fields.get('sorts', ''), 'sorts')
    require(set(fields) == expected | {f'sort{i}' for i in range(sorts)}, 'attribution fields changed')
    require(fields['schema'] == '1' and fields['mode'] in MODES, 'attribution schema/mode changed')
    for key in ('sort_overflow', 'unknown_phases', 'level_overflow'):
        require(integer(fields[key], key) == 0, f'attribution {key} is nonzero')
    return {'outcome': fields['outcome'], 'mode': fields['mode'],
            'sorts': [parse_sort(fields[f'sort{i}']) for i in range(sorts)],
            'timers': parse_timers(fields['timers'])}


def validate_counts(sort, arm):
    """Arm-specific identities of a counted serial canonical sort."""
    n, c = sort['n'], sort
    require(sort['complete'] == 1, 'incomplete sort record')
    require(sort['sort_ns'] == sort['canonical_ns'] == 0, 'counts build recorded time')
    require(c['out'] <= n, 'canonical output exceeds input')
    if sort['path'] != 'serial':
        require(all(c[key] == 0 for key in COUNT_KEYS), 'team/hosted lanes are not counted')
        return
    if n >= 2:
        require(c['cmp'] > 0, 'serial sort without comparisons')
    if arm == 'accepted':
        require(c['heapsorts'] == 1 and c['heapsort_records'] == n, 'accepted sort is one heapsort')
        require(all(c[key] == 0 for key in ('partitions', 'partition_steps', 'partition_swaps',
                                             'insertion_leaves', 'insertion_records', 'insertion_swaps')),
                'accepted sort partitioned')
        # Each sift step compares at most twice.
        require(c['cmp'] <= 2 * c['heap_sift_steps'], 'heap comparisons exceed sift steps')
    else:
        if n <= 16:
            require(c['partitions'] == 0 and c['insertion_leaves'] == 1 and c['insertion_records'] == n,
                    'small candidate sort is one insertion leaf')
        else:
            require(c['partitions'] >= 1 and c['partition_steps'] >= n, 'candidate root partition missing')
        require(c['heapsort_records'] <= n * max(1, c['heapsorts']), 'fallback larger than input')
        require(c['insertion_swaps'] <= c['cmp'] and c['partition_swaps'] <= c['partition_steps'],
                'swap counts exceed comparisons/steps')
    # Queries are the caller's meter polls inside the sort (no allocation or
    # checkpoint there): one whenever 65,536 units have accumulated, counted
    # in steps of at most 1,024, so each period spans 65,536..66,559 units
    # and the sort may start with up to 65,535 units already pending.
    require(c['meter_units'] // 66559 <= c['queries'] <= c['meter_units'] // 65536 + 1,
            'meter queries do not match the unchanged cadence')


def call_hierarchy(text):
    """Read terminal/level identity independently of the diagnostic timers.

    Native runs use the installed route (also matched to the frozen reference
    by check_call); the small smoke harness emits its independent digest.
    """
    routes = [fields_of(line) for line in text.splitlines()
              if line.startswith('FEREG_SOLVER_TRACE|phase=auto_schur_route|')]
    if routes:
        from routing_science import route_hierarchy
        require(len(routes) == 1, 'expected one installed route')
        hierarchy = route_hierarchy(routes[0])
        require(hierarchy is not None, 'route has no installed hierarchy')
        return {key: hierarchy[key] for key in ('terminal', 'levels', 'terminal_factor')}
    digests = [fields_of(line) for line in text.splitlines() if line.startswith('SMOKE_DIGEST|')]
    require(len(digests) == 1, 'missing route or smoke hierarchy metadata')
    digest = digests[0]
    from routing_science import TERMINAL_REASONS
    require(digest.get('terminal') in TERMINAL_REASONS, 'invalid smoke terminal')
    vertices = json.loads(digest['vertices'])
    require(isinstance(vertices, list) and vertices and all(type(n) is int and n >= 0 for n in vertices),
            'invalid smoke hierarchy levels')
    factor = digest['factor_nonzeros'] != 'None'
    require(factor == (digest['terminal'] == 'Direct'), 'smoke terminal factor mismatch')
    return {'terminal': digest['terminal'], 'levels': len(vertices), 'terminal_factor': int(factor)}


def validate_call(text, arm, mode):
    """Validate one call's log: one attribution and one built profile line."""
    require(arm in ARMS and mode in MODES, 'unknown arm or mode')
    lines = [line for line in text.splitlines() if line.startswith(PREFIX)]
    profiles = [line for line in text.splitlines() if line.startswith(PROFILE_PREFIX)]
    require(len(lines) == 1, f'expected one attribution line, found {len(lines)}')
    require(len(profiles) == 1, f'expected one profile line, found {len(profiles)}')
    profile = fields_of(profiles[0])
    require(profile.get('outcome') == 'built' and profile.get('completed_phases') == '6',
            'profile did not finish all six phases')
    record = parse_attribution(fields_of(lines[0]))
    require(record['outcome'] == 'built' and record['mode'] == mode, 'attribution outcome/mode mismatch')
    sorts, timers = record['sorts'], record['timers']
    require(sorts, 'no canonical sort recorded')
    contraction = [s for s in sorts if s['site'] == 'contraction']
    require([s['level'] for s in contraction] == list(range(len(contraction))),
            'contraction sorts are not one per level in order')
    require(all(s['site'] == 'contraction' or s['level'] == 0 for s in sorts), 'outside sort has a level')
    require(all(s['complete'] == 1 for s in sorts), 'incomplete sort record')
    hierarchy_seconds = float(profile['hierarchy_seconds'])
    require(math.isfinite(hierarchy_seconds) and hierarchy_seconds >= 0,
            'hierarchy_seconds must be finite and nonnegative')
    hierarchy = call_hierarchy(text)
    require(len(contraction) == hierarchy['levels'] - 1, 'sorts disagree with hierarchy level count')
    if mode == 'counts':
        require(not timers, 'counts build recorded timers')
        for sort in sorts:
            validate_counts(sort, arm)
        derived = None
    else:
        require(all(all(s[key] == 0 for key in COUNT_KEYS) for s in sorts), 'timers build counted operations')
        require(all(s['sort_ns'] <= s['canonical_ns'] for s in sorts), 'sort time exceeds canonical time')
        derived = validate_timers(timers, contraction, hierarchy_seconds, hierarchy)
    return {'profile': profile, 'sorts': sorts,
            'timers': {f'{name}@{level}': value for (name, level), value in sorted(timers.items())},
            'hierarchy': hierarchy, 'exclusive_ms': derived}


def validate_timers(timers, contraction, hierarchy_seconds, hierarchy):
    """Check nesting and return disjoint pieces summing to the fereg phase."""
    levels = {level for (name, level) in timers if name in LEVEL_PHASES}
    require(levels == set(range(hierarchy['levels'])), 'hierarchy level timers disagree with route')
    for name in ONCE_PHASES + OPTIONAL_ONCE:
        entries = [(level, value) for (n, level), value in timers.items() if n == name]
        level = max(levels) if name in LAST_LEVEL_PHASES else 0
        require(len(entries) in ((1,) if name in ONCE_PHASES else (0, 1)),
                f'timer {name} missing or repeated')
        require(all(at == level and value['calls'] == 1 for at, value in entries),
                f'timer {name} at the wrong level or repeated')
    for level in sorted(levels):
        required = {'direct_terminal_check', 'inverse_diagonal_and_level_finalization'}
        if level < max(levels) or hierarchy['terminal'] != 'Direct':
            required.update(('forest_select_split_low_degree_and_label', 'aggregation_construction',
                             'hierarchy_bookkeeping_and_fill_checks'))
        if level < max(levels):
            required.update((CONTRACTION, 'coarse_isolate_pruning', 'repeat_count_initialization'))
        actual = {name for (name, at) in timers if at == level and name in LEVEL_PHASES}
        require(actual == required, f'level {level} timer coverage mismatch: '
                f'missing={sorted(required - actual)}, unexpected={sorted(actual - required)}')
        require(all(value['calls'] == 1 for (name, at), value in timers.items() if at == level
                    and name in LEVEL_PHASES), f'level {level} phase timed twice')
    require(int(('terminal_factor', 0) in timers) == hierarchy['terminal_factor'],
            'terminal_factor timer disagrees with route')
    contracted = sorted(level for (name, level) in timers if name == CONTRACTION)
    require(contracted == [s['level'] for s in contraction],
            'contraction timers and contraction sort records disagree')
    def ns(name, level=None):
        return sum(v['ns'] for (n, at), v in timers.items() if n == name and level in (None, at))

    level_ns = sum(v['ns'] for (name, _), v in timers.items() if name in LEVEL_PHASES)
    once_ns = sum(ns(name) for name in ('option_validation', 'graph_clone_reference_setup',
                                         'hierarchy_report_bookkeeping'))
    finalize_parts = ns('finest_components') + ns('coarse_centering') + ns('terminal_factor')
    fereg_ns = round(hierarchy_seconds * 1e9)
    sort_ns = sum(s['sort_ns'] for s in contraction)
    canonical_ns = sum(s['canonical_ns'] for s in contraction)
    contraction_ns = sum(ns(CONTRACTION, s['level']) for s in contraction)
    checks = [
        *((s['canonical_ns'], ns(CONTRACTION, s['level']),
           f'canonical within contraction at level {s["level"]}') for s in contraction),
        (canonical_ns, contraction_ns, 'canonical within contraction'),
        (level_ns + once_ns, ns('hierarchy_total'), 'hierarchy phases within hierarchy_total'),
        (finalize_parts, ns('finalize_total'), 'finalize parts within finalize_total'),
        (ns('hierarchy_total') + ns('finalize_total'), ns('build_with_total'),
         'hierarchy and finalize within build_with_total'),
        (ns('build_with_total'), fereg_ns, 'build_with_total within the fereg hierarchy phase'),
    ]
    for inner, outer, what in checks:
        require(inner <= outer, f'nested timers inconsistent: {what}')
    pieces = {
        'contraction_sort': sort_ns,
        'contraction_post_sort_graph_finalization': canonical_ns - sort_ns,
        'contraction_mapping_other': contraction_ns - canonical_ns,
        **{name: sum(v['ns'] for (n, _), v in timers.items() if n == name)
           for name in LEVEL_PHASES if name != CONTRACTION},
        'hierarchy_once_phases': once_ns,
        'hierarchy_unattributed': ns('hierarchy_total') - level_ns - once_ns,
        'finest_components': ns('finest_components'),
        'coarse_centering': ns('coarse_centering'),
        'terminal_factor': ns('terminal_factor'),
        'finalize_other': ns('finalize_total') - finalize_parts,
        'build_with_other': ns('build_with_total') - ns('hierarchy_total') - ns('finalize_total'),
        'fereg_hierarchy_other': fereg_ns - ns('build_with_total'),
    }
    require(sum(pieces.values()) == fereg_ns, 'exclusive pieces do not partition the fereg phase')
    require(all(value >= 0 for value in pieces.values()), 'negative exclusive timer duration')
    return {name: value / 1e6 for name, value in pieces.items()}


STRUCTURE_KEYS = ('site', 'level', 'path', 'n', 'out')


def require_deterministic(calls, mode):
    """Within a case, every call has the same canonical sorts; counts builds
    also the same counters (serial sorts are single-threaded and depend only
    on the input, so worker count and repeat cannot change them)."""
    keys = SORT_KEYS if mode == 'counts' else STRUCTURE_KEYS
    shapes = {json.dumps([[sort[key] for key in keys] for sort in call['sorts']]) for call in calls}
    require(len(shapes) == 1, 'canonical sorts differ between calls of one case')


# ---------------------------------------------------------------- smoke

SMOKE_CALLS = [('random', 1), ('random', 4), ('banded', 1), ('banded', 4)]


def smoke_calls(text):
    """Split smoke output into calls: (graph, workers, digest, call text)."""
    calls = []
    pattern = r'^SMOKE_CALL_BEGIN\|graph=(\w+)\|workers=(\d+)\n(.*?)^SMOKE_CALL_END$'
    for graph, workers, body in re.findall(pattern, text, re.M | re.S):
        digests = [line for line in body.splitlines() if line.startswith('SMOKE_DIGEST|')]
        require(len(digests) == 1, 'smoke call without exactly one digest')
        calls.append((graph, int(workers), digests[0].split('|', 3)[3], body))
    require([(g, w) for g, w, _, _ in calls] == SMOKE_CALLS, 'smoke calls changed')
    return calls


def smoke(directory, cmg_git):
    """Build the smoke harness on the pristine and the prepared CMG source of
    DIR's arm; require identical result digests at W1/W4 across both and
    validate the prepared build's real attribution lines."""
    directory = Path(directory).absolute()
    source = json.loads((directory / 'attribution-source.json').read_text())
    arm, mode = source['arm'], source['mode']
    require(arm in ARMS and source['cmg_revision'] == ARMS[arm]['cmg'] and mode in MODES,
            'invalid attribution source')
    work = directory / 'smoke-work'
    require(not work.exists(), f'{work} exists')
    export(cmg_git, ARMS[arm]['cmg'], work / 'pristine-cmg')
    outputs = {}
    for label, cmg, feature in (('pristine', work / 'pristine-cmg', False), ('prepared', directory / 'cmg', True)):
        crate = work / f'{label}-smoke'
        crate.mkdir()
        (crate / 'Cargo.toml').write_text(apply_edit(SMOKE_MANIFEST.read_text(), '@CMG_PATH@',
                                                     str(cmg), 'smoke manifest'))
        (crate / 'smoke.rs').write_bytes(SMOKE_SOURCE.read_bytes())
        (crate / 'Cargo.lock').write_bytes((cmg / 'Cargo.lock').read_bytes())
        command = ['cargo', 'run', '--quiet', '--offline', '--release', *(['--features', 'attribution'] if feature else [])]
        result = subprocess.run(command, cwd=crate, capture_output=True, text=True, check=False,
                                env={**os.environ, 'CARGO_TARGET_DIR': str(work / f'{label}-target')})
        (work / f'{label}.out').write_text(result.stdout + result.stderr)
        require(result.returncode == 0, f'{label} smoke failed: {result.stderr[-2000:]}')
        outputs[label] = result.stdout
    pristine, prepared = smoke_calls(outputs['pristine']), smoke_calls(outputs['prepared'])
    require([c[:3] for c in pristine] == [c[:3] for c in prepared],
            'instrumented results differ from the pristine source')
    for graph in ('random', 'banded'):
        require(len({d for g, _, d, _ in prepared if g == graph}) == 1, f'{graph}: results differ between W1 and W4')
    require(any('terminal=Direct' in d for _, _, d, _ in prepared), 'smoke never reached a direct terminal')
    for graph in ('random', 'banded'):
        require_deterministic([validate_call(body, arm, mode) for g, _, _, body in prepared if g == graph], mode)
    require(not any(line.startswith(PREFIX) for line in outputs['pristine'].splitlines()),
            'pristine smoke emitted attribution')
    calls = [{'graph': graph, 'workers': workers, 'digest': digest,
              'attribution': validate_call(body, arm, mode)} for graph, workers, digest, body in prepared]
    return {'arm': arm, 'mode': mode, 'smoke_sha256': file_sha256(SMOKE_SOURCE),
            'manifest_sha256': file_sha256(SMOKE_MANIFEST), 'calls': calls}


# ---------------------------------------------------------------- bind/run

def bind(directory, plugin, build_log, baseline_sha256):
    directory = Path(directory).absolute()
    source = json.loads((directory / 'attribution-source.json').read_text())
    require(re.fullmatch(r'[0-9a-f]{64}', baseline_sha256) is not None, 'invalid baseline hash')
    archive = directory / source['source_archive']['path']
    require(file_sha256(archive) == source['source_archive']['sha256'], 'source archive changed')
    log = Path(build_log).absolute()
    record = {
        'arm': source['hidden_arm'], 'revision': source['fereg_revision'],
        'baseline_sha256': baseline_sha256, 'plugin_sha256': file_sha256(plugin),
        'patch_sha256': PROFILE_PATCH_SHA256,
        'source_archive': {'path': os.path.relpath(archive, directory), 'sha256': file_sha256(archive)},
        'build_log': {'path': os.path.relpath(log, directory), 'sha256': file_sha256(log)},
    }
    (directory / 'profile-source.json').write_text(json.dumps(record, indent=1, sort_keys=True) + '\n')
    return record


def check_attribution_source(path, identity):
    import qualify as q
    source = q.read(path)
    arm = next((name for name, value in ARMS.items() if value['hidden'] == identity['arm']), None)
    require(arm is not None and source['arm'] == arm and source['hidden_arm'] == identity['arm'],
            'attribution source belongs to another arm')
    require(source['fereg_revision'] == identity['base_revision'] == ARMS[arm]['fereg']
            and source['cmg_revision'] == ARMS[arm]['cmg'], 'attribution source revision mismatch')
    require(source['mode'] in MODES, 'attribution source mode')
    require(source['tool_sha256'] == file_sha256(__file__) and source['module_sha256'] == file_sha256(MODULE),
            'attribution source prepared by another tool version')
    require(source['profile_patch_sha256'] == identity['patch_sha256'] == PROFILE_PATCH_SHA256,
            'attribution source used another profile patch')
    require(source['source_archive']['sha256'] == identity['source_archive_sha256'],
            'attribution archive differs from the bound profile source archive')
    return arm, source


def run(args):
    repo, root, out = (Path(getattr(args, name)).resolve() for name in ('repository', 'task_root', 'output'))
    sys.path.insert(0, str(repo / 'bench'))
    import qualify as q
    import routing
    import cmg_setup_profile as profile
    require(Path(profile.__file__).resolve() == (repo / 'bench/cmg_setup_profile.py').resolve(strict=True),
            'profile helper imported from another repository')
    system = platform.system()
    plan, manifest, base, plugin, identity, guard = profile.load_context(
        repo, root, system, arm=args.arm, revision=args.revision, source=args.source,
        plan_path=args.plan, baseline_path=args.baseline, plugin_path=args.plugin)
    identity['attribution_source_sha256'] = guard.bind(args.attribution)
    arm, source = check_attribution_source(args.attribution, identity)
    identity.update({'attribution_mode': source['mode'], 'attribution_tool_sha256': guard.bind(Path(__file__)),
                     'attribution_module_sha256': guard.bind(MODULE),
                     'purpose': 'attribution only; counter-build time is not timing evidence'})
    out.mkdir(parents=True, exist_ok=False)
    q.write(out / 'protocol.json', identity)
    records = []
    for name in ('high-degree-weak', 'weak-low'):
        item = next(c for c in manifest['cases'] if c['id'] == name)
        guard.check()
        meta = q.metadata(manifest, item, plan)
        for workers in (1, 4, 8):
            cell = next(c for c in base['cases'] if c['id'] == f'{name}-t{workers}')
            expected = next(c for c in cell['warmups'] if c['execution'] == identity['arm'])
            request = out / f'{name}-t{workers}.case'
            routing.prepare(q.resolve(plan, item['input']), request, 'incremental-two', workers)
            guard.watch_request(request)
            for repeat in range(3):
                label = f'{name}-t{workers}-{repeat}'
                result, log = out / (label + '.json'), out / (label + '.log')
                env = {k: v for k, v in os.environ.items() if k not in q.ENVIRONMENT}
                env.update({k: '1' for k in q.THREAD_ENV})
                env.update(FEREG_SOLVER_TRACE='1', FEREG_PHASE_TIMING='1')
                guard.check()
                with log.open('x') as stream:
                    subprocess.run([str(q.resolve(plan, manifest['harness'])), str(plugin),
                                    '--replay', str(request), str(result)],
                                   stdout=stream, stderr=subprocess.STDOUT, env=env, check=True, timeout=120)
                guard.check()
                _, columns, detail = q.read_native(result, log, meta)
                trace = log.read_text()
                resources = profile.check_call(manifest, identity['arm'], expected, detail, trace)
                call = validate_call(trace, arm, source['mode'])
                guard.check()
                records.append({'case': name, 'workers': workers, 'repeat': repeat, 'warmup': repeat == 0,
                                'fingerprint': detail['fingerprint'], 'route_trace': resources['route_trace'],
                                'route_hierarchy': resources['route_hierarchy'], 'attribution': call,
                                'log_sha256': q.digest(log)})
                q.write(out / 'results.json', {'status': 'running', 'identity': identity, 'calls': records})
                require_deterministic([c['attribution'] for c in records if c['case'] == name], source['mode'])
                columns.unlink()
                result.unlink()
            guard.forget_request(request)
            request.unlink()
            print(name, workers, 'ATTRIBUTION_SCIENCE_PASS', flush=True)
    guard.check()
    require(len(records) == 18, 'attribution call count changed')
    for name in ('high-degree-weak', 'weak-low'):
        require_deterministic([r['attribution'] for r in records if r['case'] == name], source['mode'])
    q.write(out / 'results.json', {'status': 'complete', 'identity': identity, 'calls': records})
    print('SETUP_ATTRIBUTION_COMPLETE')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    p = commands.add_parser('prepare')
    p.add_argument('--arm', choices=tuple(ARMS), required=True)
    p.add_argument('--mode', choices=MODES, required=True)
    for name in ('fereg-git', 'cmg-git', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    p = commands.add_parser('bind')
    p.add_argument('directory', type=Path)
    for name in ('plugin', 'build-log'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--baseline-sha256', required=True)
    p = commands.add_parser('smoke')
    p.add_argument('directory', type=Path)
    p.add_argument('--cmg-git', type=Path, required=True)
    p = commands.add_parser('parse')
    p.add_argument('log', type=Path)
    p.add_argument('--arm', choices=tuple(ARMS), required=True)
    p.add_argument('--mode', choices=MODES, required=True)
    p = commands.add_parser('run')
    for name in ('repository', 'task_root', 'output'):
        p.add_argument(name, type=Path)
    p.add_argument('--arm', choices=tuple(value['hidden'] for value in ARMS.values()), required=True)
    p.add_argument('--revision', required=True)
    for name in ('source', 'attribution', 'plugin'):
        p.add_argument('--' + name, type=Path, required=True)
    for name in ('plan', 'baseline'):
        p.add_argument('--' + name, type=Path)
    args = parser.parse_args()
    if args.command == 'prepare':
        record = prepare(args.arm, args.mode, args.fereg_git, args.cmg_git, args.out)
        print(json.dumps(record, indent=1, sort_keys=True))
    elif args.command == 'bind':
        print(json.dumps(bind(args.directory, args.plugin, args.build_log, args.baseline_sha256), indent=1))
    elif args.command == 'smoke':
        print(json.dumps(smoke(args.directory, args.cmg_git), indent=1, sort_keys=True))
    elif args.command == 'parse':
        print(json.dumps(validate_call(args.log.read_text(), args.arm, args.mode), indent=1, sort_keys=True))
    else:
        run(args)


if __name__ == '__main__':
    main()
