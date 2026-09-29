"""Sanitize and seal the small diagnostic report; raw licensed logs stay local."""
import argparse
import difflib
import hashlib
import json
from pathlib import Path
import re
import statistics

from collect import collect
from run import compare


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def phases(path):
    values = {}
    for line in (path / "stderr.txt").read_text().splitlines():
        m = re.match(r"FEREG_PHASE\|([^|]+)\|(.*)", line)
        if m:
            values[m[1]] = {k: float(v) for k, v in (x.split("=", 1) for x in m[2].split("|"))}
    return values


def archive(root, output):
    output.mkdir(parents=True, exist_ok=True)
    collect(root, output)
    science = []
    folders = sorted(p.parent for p in (root / "runs").glob("*/result.json"))
    assert len(folders) == 152, len(folders)
    for path in folders:
        result = json.loads((path / "result.json").read_text())
        assert result["status"] == "pass" and result["process_returncode"] == 0 and result["sentinel"]
        record = dict(run=path.name, status="pass", sample_sha256=result["sample_sha256"],
                      result_sha256=sha(path / "result.json"),
                      output_sha256={name: sha(path / name) for name in
                                     ("coefficients.csv", "covariance.csv")})
        if result["arm"] == "fereg":
            base_name = re.sub(r"-(deep|sample|outer|publication)-", "-", path.name)
            base_name = re.sub(r"-r\d+-(stock|trace)-", "-r1-stock-", base_name)
            record["full_native_output_check"] = compare(root / "runs" / base_name, path)
        else:
            paired_name = path.name.replace("-fixest", "-fereg")
            if (root / "runs" / paired_name).exists():
                record["cross_estimator_check"] = compare(root / "runs" / paired_name, path)
        science.append(record)
    write(output / "science.json", dict(accepted_calls=len(folders), oracle=json.loads((root / "smoke.json").read_text()), checks=science))

    detail = {}
    for k in (10, 50):
        deep = [phases(p) for p in folders if f"-k{k}-" in p.name and "-deep-" in p.name]
        assert len(deep) == 3
        nested = {label: {field: statistics.median(x[label][field] for x in deep)
                          for field in deep[0][label]} for label in deep[0]}
        publication = [json.loads((p / "result.json").read_text()) for p in folders
                       if f"-k{k}-" in p.name and "-publication-" in p.name]
        labels = {81: "before_plugin", 82: "plugin_call", 83: "validate_and_name",
                  84: "ereturn_post", 85: "remaining_publication"}
        ado = {label: statistics.median(x["metadata"][f"profile_{timer}"] for x in publication)
               for timer, label in labels.items()}
        ado["outside_transaction_timers"] = statistics.median(
            x["seconds"]-sum(x["metadata"][f"profile_{t}"] for t in labels) for x in publication)
        detail[str(k)] = dict(native=nested, ado=ado,
                              diagnostic_command_median=statistics.median(x["seconds"] for x in publication))
    write(output / "detailed-phases.json", detail)

    samples = {}
    for p in folders:
        if "-sample-" not in p.name:
            continue
        text = (p / "sample.txt").read_text()
        section = text.split("Sort by top of stack, same collapsed (when >= 5):", 1)[1].split("Binary Images:", 1)[0]
        top = []
        for line in section.splitlines():
            m = re.match(r"\s*(.+?)\s{2,}(\d+)\s*$", line)
            if m:
                function = re.sub(r"::h[0-9a-f]{16}", "", m[1])
                function = re.sub(r"\s+load address.*", "", function)
                top.append(dict(function=function, samples=int(m[2])))
        samples[p.name] = dict(status="available" if top else "no_stacks_captured", top=top,
                              raw_sha256=sha(p / "sample.txt"),
                              interpretation="Across-thread stack counts, including waits; not wall-time percentages. Eight repeated commands; narrow samples can include extraction.")
    write(output / "stack-summaries.json", samples)

    environment = json.loads((root / "environment.json").read_text())
    environment["cpu"] = (root / "cpu.txt").read_text().splitlines()
    write(output / "environment.json", environment)
    identities = dict(source_commit=(root / "source-commit.txt").read_text().strip(),
                      fixest_archive_sha256=sha(root / "fixest.tar.gz"),
                      artifacts={name: sha(root / path) for name, path in {
                          "fereg_stock": "baseline/dist/fereg_macos.plugin",
                          "fereg_diagnostic": "detailed/dist/fereg_macos.plugin",
                          "fixest_stock": "R-stock/fixest/libs/fixest.so",
                          "fixest_diagnostic": "R-traced/fixest/libs/fixest.so"}.items()},
                      controller_snapshots={directory: {p.name: sha(p) for p in (root / directory).iterdir() if p.is_file()}
                                            for directory in ("controller-primary", "controller-followup", "controller")},
                      failed_or_discarded_attempts=[
                          "Initial compiler C++ include-path failure; fixed locally with the Command Line Tools C++ headers.",
                          "Initial sandbox process-sampling restriction; rerun with authorized process access.",
                          "Smoke diagnostic read an intentionally disabled Rprof output; wrapper fixed and four oracle checks repeated.",
                          "First detailed build compiled the unchanged working-directory source rather than the instrumented snapshot; missing timer signatures detected, six diagnostic calls excluded, build repeated from the snapshot directory.",
                          "First publication probe expected display inside a quiet call; fit passed but attribution was rejected. Timer values now exported after the call; six checks passed."])
    write(output / "identities.json", identities)

    patches = output / "patches"
    patches.mkdir(exist_ok=True)
    for label, a, b, files in [
        ("fixest", root / "fixest", root / "stock-src/fixest", ["R/estimation.R", "src/demeaning.cpp"]),
        ("fereg", root / "baseline", root / "detailed", ["crates/fereg-plugin/src/estimate.rs", "legacy/fereg-core/src/ols/prepared.rs", "legacy/fereg-core/src/ols/prepared_fit.rs", "stata/_fereg_estimate_ols.ado"]),
    ]:
        patch = ""
        for file in files:
            patch += "".join(difflib.unified_diff((a / file).read_text().splitlines(True), (b / file).read_text().splitlines(True), fromfile="a/"+file, tofile="b/"+file))
        (patches / (label + ".patch")).write_text(patch)
    write(output / "files.sha256.json", {str(p.relative_to(output)): sha(p) for p in output.rglob("*")
                                          if p.is_file() and p.name != "files.sha256.json"})


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("output", type=Path)
    args = p.parse_args()
    archive(args.root, args.output)
