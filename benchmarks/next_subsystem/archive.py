"""Seal bounded development receipts without data, binaries or licensed logs."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import zipfile

REPO = Path(__file__).resolve().parents[2]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def archive(root, output, screens, extras):
    replacements = [(str(REPO), "$REPO"), (str(Path.home()), "$HOME")]
    manifest = {}

    def sanitized(data):
        text = data.decode()
        for before, after in replacements:
            text = text.replace(before, after)
        return text.encode()

    with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED) as bundle:
        def add(name, data, original=None):
            result = sanitized(data)
            manifest[name] = dict(original_sha256=digest(original or data), archived_sha256=digest(result))
            bundle.writestr(name, result)

        for screen in screens:
            directory = root / screen
            for path in sorted(directory.rglob("*")):
                if not path.is_file():
                    continue
                relative = path.relative_to(directory)
                data = path.read_bytes() if path.suffix not in (".dta", ".bin") else None
                if relative.parts[0] == "controller" or path.suffix in (".json", ".csv", ".tsv"):
                    add(f"{screen}/{relative}", data)
                elif path.name in ("stdout.txt", "stderr.txt") or path.suffix == ".log":
                    # Keep only native trace lines and worker protocol sentinels.
                    lines = [line for line in data.decode(errors="replace").splitlines()
                             if re.match(r"^(FEREG_PHASE\||FEREG_SOLVER_TRACE\||SYSTEMATIC_)", line)]
                    add(f"{screen}/{relative}.protocol", ("\n".join(lines) + "\n").encode(), data)
        for relative in extras:
            add(relative, (root / relative).read_bytes())
        bundle.writestr("archive-hashes.json", json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    output.with_suffix(".sha256").write_text(digest(output.read_bytes()) + "  " + output.name + "\n")
    print("ARCHIVE_PASS", len(manifest))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--screens", nargs="+", required=True)
    parser.add_argument("--extras", nargs="*", default=[])
    args = parser.parse_args()
    archive(args.root, args.output, args.screens, args.extras)
