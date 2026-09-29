"""Stage registered compile-time fixtures and refuse incomplete core test trees."""
import hashlib
from pathlib import Path
import re
import shutil


def stage_test_inputs(deployment, source, inputs):
    for relative, digest in inputs.items():
        target = source / relative
        if not target.resolve().is_relative_to(source.resolve()):
            raise ValueError('external test input')
        original = deployment / Path(relative).name
        if hashlib.sha256(original.read_bytes()).hexdigest() != digest:
            raise ValueError('test fixture hash mismatch: ' + relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(original, target)
    for module in (source / 'crates/fereg-core').rglob('*.rs'):
        for relative in re.findall(r'include_(?:str|bytes)!\(\s*"([^"]+)"\s*\)', module.read_text()):
            included = (module.parent / relative).resolve()
            if not included.is_relative_to(source.resolve()) or not included.is_file():
                raise ValueError('missing or external compile-time input: ' + str(included))
