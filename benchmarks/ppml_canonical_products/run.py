"""Capture, compile, and run the isolated canonical-product design experiment."""
from pathlib import Path
import argparse
import hashlib
import io
import json
import os
import re
import shutil
import struct
import subprocess
import tarfile


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, value):
    with path.open("x") as out:
        json.dump(value, out, indent=2)
        out.write("\n")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["prepare", "build", "run"])
    parser.add_argument("root", type=Path)
    parser.add_argument("--frames", type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    root = args.root.resolve()
    env = dict(os.environ, CARGO_NET_OFFLINE="true", OPENBLAS_NUM_THREADS="1",
               OMP_NUM_THREADS="1", VECLIB_MAXIMUM_THREADS="1")
    if args.action == "prepare":
        import numpy as np
        assert args.frames is not None
        root.mkdir()
        (root / "frames").mkdir()
        base = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
        source = root / "source"
        source.mkdir()
        archive = subprocess.check_output(["git", "archive", base, "Cargo.toml", "Cargo.lock", "crates", "tests/fixtures"], cwd=repo)
        with tarfile.open(fileobj=io.BytesIO(archive)) as bundle:
            bundle.extractall(source, filter="data")
        destination = source / "legacy/fereg-core/src/ols/canonical_products_probe"
        destination.mkdir()
        for name in ["kernel.rs", "probe.rs"]:
            shutil.copy2(Path(__file__).parent / name, destination / name)
        with (source / "legacy/fereg-core/src/ols/mod.rs").open("a") as out:
            out.write('\n#[cfg(test)]\nmod canonical_products_probe {\n'
                      '    #[path = "kernel.rs"] mod kernel;\n'
                      '    include!("canonical_products_probe/probe.rs");\n}\n')
        records = []
        for case in range(4):
            original = args.frames / f"c{case}.bin"
            data = bytearray(original.read_bytes())
            n, width, effects = struct.unpack_from("<QQQ", data)
            assert effects == 2
            at = 32 + effects * (8 + 4*n)
            w = np.frombuffer(data, dtype="<f8", count=n, offset=at).copy()
            for frame in [0, 1]:
                changed = bytearray(data)
                if frame:
                    replacement = w * np.exp(0.7*np.sin(np.arange(n)/17))
                    changed[at:at+8*n] = replacement.astype("<f8").tobytes()
                name = f"c{case}-f{frame}"
                path = root / "frames" / f"{name}.bin"
                path.write_bytes(changed)
                records.append(dict(case=name, n=n, width=width, original_sha256=digest(original), sha256=digest(path)))
        write_json(root / "declaration.json", dict(
            base=base, purpose="Design selection for canonical current-weight products; no public runtime or production claim.",
            candidate="Shared larger-partition centering and component-grounded Schur Cholesky. Variant 1: H-U'U plus original-row certificate. Variant 2: one bounded group-disjoint reconstruction forms direct Gram and original-row scores together, without product subtraction.",
            baseline="Accepted reusable PPML categorical projection; separate response and ten-control scratch blocks; accepted retained scratch moment kernel.",
            candidate_limits="Prototype allocations are not production budget admission. Cancellation diagnostic is not a complete factorization error certificate. Rank, inverse and final inference are not replaced.",
            frames=records, threads=[1,4,8], rotations=7, variants=[0,1,2],
            timing="Alternating paired full projection/product calls. Report and charge one complete geometry setup in cold comparison; export/oracle work outside clock.",
            acceptance="Independent explicit dummy WLS projection error <=1e-10 and normalized score <=1e-12. Full Gram checked independently, including RHS/response diagonal. No changed public tolerance.",
        ))
        write_json(root / "source-hashes.json", {str(p.relative_to(source)):digest(p) for p in sorted(source.rglob("*")) if p.is_file()})
        for name in ["run.py", "audit.py"]:
            shutil.copy2(Path(__file__).parent/name, root/name)
    elif args.action == "build":
        command = ["cargo", "test", "-p", "fereg-core", "--lib", "--release", "canonical_products_probe", "--no-run", "--offline"]
        env["CARGO_TARGET_DIR"] = str(root / "cargo")
        with (root / "build.log").open("x") as out:
            result = subprocess.run(command, cwd=root / "source", env=env, stdout=out, stderr=subprocess.STDOUT)
        write_json(root / "build.json", dict(returncode=result.returncode, command=command))
        assert result.returncode == 0, (root / "build.log").read_text()[-4000:]
    else:
        log = (root / "build.log").read_text()
        match = re.search(r"Executable unittests .*? \(([^)]+)\)", log)
        assert match, log[-2000:]
        binary = Path(match[1])
        env["FEREG_PRODUCT_ROOT"] = str(root)
        boundary_command = [str(binary), "canonical_products_probe::boundary_", "--nocapture", "--test-threads=1"]
        with (root / "boundary.log").open("x") as out:
            boundary_result = subprocess.run(boundary_command, env=env, stdout=out, stderr=subprocess.STDOUT)
        write_json(root / "boundary.json", dict(returncode=boundary_result.returncode, binary_sha256=digest(binary)))
        assert boundary_result.returncode == 0, (root / "boundary.log").read_text()[-4000:]
        assert "3 passed; 0 failed" in (root / "boundary.log").read_text()
        command = [str(binary), "frozen_canonical_products", "--ignored", "--nocapture", "--test-threads=1"]
        with (root / "run.log").open("x") as out:
            result = subprocess.run(command, env=env, stdout=out, stderr=subprocess.STDOUT)
        write_json(root / "run.json", dict(returncode=result.returncode, binary_sha256=digest(binary)))
        assert result.returncode == 0, (root / "run.log").read_text()[-4000:]


if __name__ == "__main__":
    main()
