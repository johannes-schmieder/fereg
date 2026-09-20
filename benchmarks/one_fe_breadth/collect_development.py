"""Collect selected completed development series, preserving each measured arm."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n")


def collect(root, output, labels):
    output.mkdir(parents=True, exist_ok=False)
    receipts, identities, science, timings = [], {}, {}, []
    for label in labels:
        complete = root / (label + "-complete.json")
        science[label] = read(complete)
        assert science[label]["status"] == "pass", complete
        grouped = {}
        for path in sorted((root / "runs").glob(label + "-*/result.json")):
            result = read(path)
            assert result["status"] == "pass" and result["process_returncode"] == 0
            assert result["sentinel"] and result["metadata"]["rc"] == 0
            identity = result.pop("measurement_fingerprint")
            key = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
            identities[key] = identity
            result["identity_sha256"] = key
            result["run"] = path.parent.name
            result["variant"] = path.parent.name.rsplit("-", 1)[1]
            result["exports"] = {p.name: sha(p) for p in path.parent.iterdir()
                                 if p.suffix in (".csv", ".bin")}
            phases = {}
            for line in path.with_name("stderr.txt").read_text().splitlines():
                if line.startswith("FEREG_PHASE|"):
                    _, phase, *fields = line.split("|")
                    phases[phase] = {k: float(v) for k, v in (f.split("=", 1) for f in fields)}
            # This tag belongs only to the small warm-up on an overlapped fit.
            phases.pop("ols_prepare", None)
            if phases:
                result["diagnostic_phases"] = phases
            ado_phases = path.with_name("ado-phases.tsv")
            if ado_phases.exists():
                observations = {}
                for line in ado_phases.read_text().splitlines():
                    timer, seconds = line.split()
                    observations.setdefault(timer, []).append(float(seconds))
                assert all(len(values) == 2 for values in observations.values())
                result["diagnostic_ado_phases"] = {
                    timer: dict(warmup_seconds=values[0], main_seconds=values[1])
                    for timer, values in observations.items()
                }
                result["exports"][ado_phases.name] = sha(ado_phases)
            receipts.append(result)
            cell = result["cell"]
            grouped.setdefault((cell["case"], cell["k"]), {}).setdefault(result["variant"], []).append(result)
        assert grouped, label
        for (case, width), arms in grouped.items():
            counts = {len(values) for values in arms.values()}
            assert len(counts) == 1
            row = dict(series=label, case=case, k=width, repetitions=counts.pop(), arms={}, paired_ratios={})
            for arm, values in arms.items():
                times = [r["seconds"] * 1000 for r in values]
                row["arms"][arm] = dict(median_ms=statistics.median(times), minimum_ms=min(times),
                                        maximum_ms=max(times), sampled_peak_rss_gib=max(r["peak_rss_gib"] for r in values))
                for reference, refs in arms.items():
                    if arm == reference:
                        continue
                    ratios = []
                    for actual, ref in zip(values, refs):
                        assert actual["run"].rsplit("-", 1)[0] == ref["run"].rsplit("-", 1)[0]
                        assert actual["input_sha256"] == ref["input_sha256"]
                        assert actual["sample_sha256"] == ref["sample_sha256"]
                        ratios.append(actual["seconds"] / ref["seconds"])
                    row["paired_ratios"][arm + "/" + reference] = dict(
                        values=ratios, median=statistics.median(ratios), wins=sum(r < 1 for r in ratios))
            timings.append(row)
    native_checks = {}
    for identity in identities.values():
        native = identity.get("native")
        if not native:
            continue
        matches = [path for path in root.glob("identity-*.json") if read(path) == native]
        if not matches:
            # The fereg alias is the immutable preceding campaign's normonly.
            matches = [root.parent / "one-fe-optimization-20260919/identity-normonly.json"]
        assert len(matches) == 1 and read(matches[0]) == native
        variant = matches[0].stem.removeprefix("identity-")
        source = matches[0].parent / variant
        assert sha(source / "dist/fereg_macos.plugin") == native["plugin"]
        for relative, expected in native["source"].items():
            assert sha(source / relative) == expected, (variant, relative)
        native_checks[variant] = dict(plugin=native["plugin"], verified_source_files=len(native["source"]))
    write(output / "receipts.json", receipts)
    write(output / "identities.json", dict(identities=identities, native_checks=native_checks))
    write(output / "science.json", science)
    write(output / "timings.json", timings)
    write(output / "collector.json", dict(sha256=sha(Path(__file__)), labels=labels, runs=len(receipts)))
    print("DEVELOPMENT_COLLECTION_PASS", len(receipts), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--labels", nargs="+", required=True)
    args = parser.parse_args()
    collect(args.root.resolve(), args.output.resolve(), args.labels)
