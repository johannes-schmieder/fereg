"""Seal sanitized fixed-frame sources and receipts after every process is terminal."""
from pathlib import Path
import argparse
import hashlib
import json
import re
import shutil
import zipfile


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("output",type=Path)
    parser.add_argument("roots",nargs="+",type=Path)
    args=parser.parse_args()
    repo=Path(__file__).resolve().parents[2]
    index,blobs,hashes={}, {}, {}
    def add(name,data):
        original=sha(data)
        clean=data.replace(str(repo).encode(),b"$REPO").replace(str(Path.home()).encode(),b"$HOME")
        clean=re.sub(rb"(?:/private)?/var/folders/[^\s\"'<>]+",b"$TEMP_PATH",clean)
        assert not re.search(rb"(?m)^\s*(?:Serial number:|Licensed to:)",clean),name
        hashed=sha(clean);blob="blobs/"+hashed
        assert name not in index,name
        index[name]=blob;blobs[blob]=clean
        hashes[name]=dict(original_sha256=original,archived_sha256=hashed)
    for root in args.roots:
        label=root.name
        expected=json.loads((root/"source-hashes.json").read_text())
        for name,expected_hash in expected.items():
            data=(root/"source"/name).read_bytes()
            assert sha(data)==expected_hash,(label,name)
            add(f"{label}/source/{name}",data)
        for path in sorted(root.iterdir()):
            if path.is_file() and path.suffix in [".json",".log",".py"]:
                add(f"{label}/{path.name}",path.read_bytes())
        omitted={str(path.relative_to(root)):dict(sha256=sha(path.read_bytes()),bytes=path.stat().st_size)
                 for path in sorted(root.rglob("*.bin")) if "cargo" not in path.parts}
        add(f"{label}/omitted-data-hashes.json",(json.dumps(omitted,sort_keys=True,indent=2)+"\n").encode())
    for path in sorted(Path(__file__).parent.iterdir()):
        if path.is_file(): add("current-probe/"+path.name,path.read_bytes())
    args.output.mkdir(parents=True,exist_ok=False)
    bundle=args.output/"receipts-content-addressed.zip"
    with zipfile.ZipFile(bundle,"x",compression=zipfile.ZIP_DEFLATED) as archive:
        for name,data in sorted(blobs.items()): archive.writestr(name,data)
        archive.writestr("receipt-index.json",json.dumps(dict(schema="fereg-deduplicated-receipts-v1",files=index),sort_keys=True,indent=2)+"\n")
        archive.writestr("archive-hashes.json",json.dumps(hashes,sort_keys=True,indent=2)+"\n")
    bundle.with_suffix(".sha256").write_text(sha(bundle.read_bytes())+"  "+bundle.name+"\n")
    shutil.copyfile(repo/"benchmarks/reports/development/ppml-frame-operator-local-2026-09-24/restore_receipts.py",args.output/"restore_receipts.py")
    print(json.dumps(dict(logical_files=len(index),payloads=len(blobs),bytes=bundle.stat().st_size,sha256=sha(bundle.read_bytes()))))


if __name__=="__main__": main()
