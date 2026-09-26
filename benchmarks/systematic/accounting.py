"""Strict SGE accounting capture, independently of application exit markers."""
import argparse
import re
import subprocess
from pathlib import Path

from common import write_json


def parse(text, job_id, expected_tasks, expected_slots=28):
    records = []
    for block in re.split(r"^=+\s*$", text, flags=re.MULTILINE):
        fields = {}
        for line in block.splitlines():
            tokens = line.split(None, 1)
            if len(tokens) == 2:
                fields[tokens[0]] = tokens[1].strip()
        if "jobnumber" not in fields:
            continue
        required = ("hostname", "jobnumber", "taskid", "failed", "exit_status", "slots", "ru_wallclock", "maxvmem")
        if not all(k in fields for k in required):
            raise ValueError("incomplete scheduler record")
        if fields["jobnumber"] != str(job_id):
            raise ValueError("accounting contains an unexpected job")
        task = 1 if fields["taskid"] == "undefined" else int(fields["taskid"])
        record = {k: fields[k] for k in required}
        record.update(task_id=task, failed=int(fields["failed"].split()[0]),
                      exit_status=int(fields["exit_status"]), slots=int(fields["slots"]),
                      wall_seconds=float(fields["ru_wallclock"]))
        records.append(record)
    found = [r["task_id"] for r in records]
    if len(found) != len(set(found)):
        raise ValueError("duplicate scheduler task records")
    if set(found) - set(expected_tasks):
        raise ValueError("unexpected scheduler tasks")
    missing = sorted(set(expected_tasks) - set(found))
    status = "pending" if missing else "pass" if all(r["failed"] == 0 and r["exit_status"] == 0 and r["slots"] == expected_slots for r in records) else "failed"
    return {"schema": "systematic-accounting-v1", "job_id": str(job_id),
            "expected_tasks": list(expected_tasks), "missing_tasks": missing, "status": status, "records": records}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("run_root", type=Path)
    parser.add_argument("job_id", type=int)
    parser.add_argument("task_count", type=int)
    args = parser.parse_args()
    result = subprocess.run(["qacct", "-j", str(args.job_id)], capture_output=True, text=True, check=False)
    receipt = parse(result.stdout, args.job_id, range(1, args.task_count + 1))
    write_json(args.run_root / "accounting" / f"{args.job_id}.json", receipt)
    print(receipt["status"])
