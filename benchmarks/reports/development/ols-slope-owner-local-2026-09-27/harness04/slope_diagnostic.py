"""Separate phase attribution on already frozen slope inputs and binaries."""
import argparse
import json
from pathlib import Path

from slope_probe import run_case
from slope_rotation import compare, read, successful, verify, write


def run(args):
    plan = read(args.plan)
    args.smoke = False
    identity = verify(args, plan)
    diagnostic = read(args.diagnostic_plan)
    args.output.mkdir(exist_ok=False)
    write(args.output / "identity.json", identity)
    write(args.output / "plan.json", diagnostic)
    records = []
    for entry in diagnostic["cases"]:
        cell = dict(next(c for c in plan["cases"] if c["id"] == entry["input"]))
        cell.update(threads=entry["threads"], stata_processors=min(entry["threads"], 4))
        case_id = f"{cell['id']}-t{cell['threads']}"
        output = args.output / case_id
        data = args.run / "inputs" / str(diagnostic["seed"]) / entry["input"]
        record = dict(case=case_id, arms={})
        for arm in ("baseline", "candidate"):
            directory = output / arm
            result = run_case(cell, data, directory, "native", getattr(args, arm),
                              args.reference / "env/R", diagnostic=True)
            # Both warm and timed phase records remain in original order.
            result["phase_lines"] = [line for path in (directory / "stderr.txt", directory / "fit.log")
                                     for line in path.read_text(errors="replace").splitlines()
                                     if line.startswith("FEREG_PHASE|")]
            record["arms"][arm] = result
            write(args.output / "in_progress.json", record)
            if not successful(result):
                raise RuntimeError("diagnostic application failed")
        record["science"] = compare(output / "candidate", output / "baseline", plan["science"])
        records.append(record)
        write(args.output / "results.json", records)
        print(json.dumps(record), flush=True)
        if not record["science"]["ranked"]:
            raise RuntimeError("diagnostic candidate differs from baseline science")
    write(args.output / "complete.json", dict(complete=True, cases=len(records)))
    print("FEREG_SLOPE_DIAGNOSTIC_COMPLETE", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("run", "plan", "diagnostic-plan", "baseline", "candidate", "reference", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    run(parser.parse_args())
