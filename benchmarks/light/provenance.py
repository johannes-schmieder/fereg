"""Bind a clean candidate and reused pinned comparator closure without rewriting it."""
from pathlib import Path
import sys

from common import HERE, REPO, SOURCE, read_json, sha256, write_json


def inventory(root):
    files=[root/"candidate/dist/fereg_linux.plugin", root/"input/candidate.tar.gz"]
    files+=list((root/"candidate/stata").glob("*.ado"))
    files+=list((root/"references").glob("*"))
    for base in [root/"env/R/fixest",*root.glob("env/python/lib/python*/site-packages/pyfixest")]:
        files += [p for p in base.rglob("*") if p.is_file() and "__pycache__" not in p.parts]
    files += [p for p in HERE.iterdir() if p.is_file() and p.suffix in (".py",".R",".do",".sh",".json",".md")]
    files += [REPO/"benchmarks/systematic/generate.py",REPO/"scripts/two_fe_span_audit.py"]
    if (root/"followup-plan.json").exists():
        files.append(root/"followup-plan.json")
        files.append(root/"continuation.json")
    return {str(p.relative_to(root)):sha256(p) for p in sorted(files) if p.is_file()}


def freeze(root):
    from common import manifest
    original=read_json(root/"input/candidate.json")
    if original["commit"]!=SOURCE or original["sha256"]!=sha256(root/"input/candidate.tar.gz"):
        raise ValueError("candidate snapshot mismatch")
    prebuilt = read_json(root/"prebuilt.json") if (root/"prebuilt.json").exists() else None
    if prebuilt:
        prior = Path(prebuilt["root"])
        receipt = read_json(prior/"runtime.json")
        if (sha256(prior/"runtime.json") != prebuilt["runtime_sha256"] or
            receipt["source"] != SOURCE or receipt["candidate"] != original or
            sha256(root/"candidate/dist/fereg_linux.plugin") !=
                receipt["files"]["candidate/dist/fereg_linux.plugin"]):
            raise ValueError("prebuilt candidate identity changed")
    old=read_json(root/"deployment.json")
    previous=Path(old["comparator_root"])
    frozen=read_json(previous/"env/frozen.json")
    # The old candidate is not reused; only its recorded comparator closure is.
    count=0
    for name,digest in frozen["runtime"].items():
        if name.startswith(("references/","env/R/","env/python/")):
            if sha256(previous/name)!=digest:
                raise ValueError("pinned comparator closure changed: "+name)
            count+=1
    write_json(root/"manifest.json",manifest())
    value=dict(source=SOURCE,candidate=original,files=inventory(root),comparator_files_verified=count,
               comparator_fingerprint=frozen["fingerprint"],manifest_sha256=sha256(root/"manifest.json"),
               reused_build=prebuilt)
    write_json(root/"runtime.json",value)
    (root/"runtime.sha256").write_text(sha256(root/"runtime.json")+"\n")
    return sha256(root/"runtime.json")


def verify(root):
    digest=sha256(root/"runtime.json")
    if (root/"runtime.sha256").read_text().strip()!=digest:
        raise ValueError("runtime receipt changed")
    record=read_json(root/"runtime.json")
    for file,expected in record["files"].items():
        if sha256(root/file)!=expected:
            raise ValueError("runtime bytes changed: "+file)
    if sha256(root/"manifest.json")!=record["manifest_sha256"]:
        raise ValueError("manifest changed")
    return digest


if __name__=="__main__":
    root=Path(sys.argv[2]).resolve()
    print(freeze(root) if sys.argv[1]=="freeze" else verify(root))
