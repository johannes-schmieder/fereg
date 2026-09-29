"""Add failure attribution without changing scientific or timing eligibility."""
import re


def failure_evidence(output, meta, status):
    if status == "pass":
        return None
    stage = meta.get("error_stage")
    logs = []
    for path in [output / "stdout.txt", output / "stderr.txt", *sorted(output.glob("*.log"))]:
        if not path.is_file():
            continue
        logs.append(path.name)
        # Bounded diagnostic reads cannot become another large log-retention
        # cost after a long failed solver. Raw logs remain in the attempt.
        with path.open("rb") as stream:
            stream.seek(max(0, path.stat().st_size - 65536))
            tail = stream.read().decode("utf-8", errors="replace")
        if not meta.get("error_stage"):
            markers = re.findall(r"^SYSTEMATIC_STAGE ([a-z_]+)\s*$", tail, re.MULTILINE)
            if markers:
                stage = markers[-1]
    if stage is None:
        stage = "extraction" if (output / "DONE").exists() else (
            "estimation" if (output / "START").exists() else "initialization")
    return dict(stage=stage, application_rc=meta.get("rc"), error=meta.get("error"), logs=logs)
