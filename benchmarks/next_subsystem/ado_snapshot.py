"""Capture an ado-only overlay while preserving an existing native artifact."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys

REPO = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def stage(root, baseline, name, overlays=(), profile=False):
    identity_path = root / f'identity-{baseline}.json'
    identity = json.loads(identity_path.read_text())
    source = root / name
    source.mkdir(exist_ok=False)
    for relative, digest in identity['source'].items():
        original = root / baseline / relative
        assert sha(original) == digest, relative
        destination = source / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(original, destination)
    shutil.copytree(root / baseline / 'dist', source / 'dist')
    assert sha(source / 'dist/fereg_macos.plugin') == identity['plugin']
    for relative in overlays:
        assert relative.startswith('stata/') and relative.endswith('.ado'), relative
        shutil.copyfile(REPO / relative, source / relative)
    if profile:
        sys.path.insert(0, str(REPO / 'benchmarks/one_fe_profile'))
        from instrument_fereg import instrument_ado
        instrument_ado(source)
        path = source / 'stata/_fereg_estimate_ols.ado'
        text = path.read_text()
        marker = "        fvexpand `indepvars' if `sample_shadow'"
        assert text.count(marker) == 1
        text = text.replace(marker, "    timer clear 86\n    timer on 86\n" + marker)
        marker = "        local coefficient_names `\"`r(varlist)'\"'"
        assert text.count(marker) == 1
        text = text.replace(marker, marker + '''
    timer off 86
    quietly timer list 86
    noisily display "FEREG_ADO_DETAIL|final_fvexpand|seconds=" %12.6f r(t86)
''')
        marker = '\nend\n\n\nprogram define _fereg_eols_validate'
        assert text.count(marker) == 1
        text = text.replace(marker, '''
    tempname surface_phase_file
    file open `surface_phase_file' using "ado-phases.tsv", write append text
    forvalues surface_phase = 81/86 {
        quietly timer list `surface_phase'
        file write `surface_phase_file' %12.0f (_N) _tab %4.0f (`surface_phase') _tab %24.17g (r(t`surface_phase')) _n
    }
    file close `surface_phase_file'
''' + marker)
        path.write_text(text)
        identity['diagnostic'] = 'Ado boundary and final fvexpand timers only'
    identity['native_reused_from'] = dict(variant=baseline, identity_sha256=sha(identity_path))
    identity['ado_overlays'] = list(overlays)
    identity['harness'] = sha(Path(__file__))
    identity['source'] = {relative: sha(source / relative) for relative in identity['source']}
    with (root / f'identity-{name}.json').open('x') as stream:
        json.dump(identity, stream, indent=2); stream.write('\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('baseline'); parser.add_argument('name')
    parser.add_argument('--overlay', action='append', default=[])
    parser.add_argument('--profile', action='store_true')
    args = parser.parse_args()
    stage(args.root, args.baseline, args.name, args.overlay, args.profile)
