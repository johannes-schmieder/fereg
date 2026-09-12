"""Acquire immutable comparator archives before offline benchmark execution."""
import argparse
import json
import tarfile
import urllib.request
from pathlib import Path

from common import sha256, write_json

PINS = {
    "reghdfe": ("sergiocorreia/reghdfe", "29b2b203f534413369d675d0c0f674d61291f536"),
    "ftools": ("sergiocorreia/ftools", "d6e24d66fd02ba1b37325c82c5040444c5b4c636"),
    "require": ("sergiocorreia/stata-require", "f4760b86e55cf73b82041ce59cf20e8e8224afb0"),
    "ppmlhdfe": ("sergiocorreia/ppmlhdfe", "b85665d8f674e93c1e07446a9a2b29be7f797910"),
    "gtools": ("mcaceresb/stata-gtools", "9dcbb835e9c8a7b30f037cdf1ec7de7f8c6ad30f"),
    "pyfixest": ("py-econometrics/pyfixest", "b6f2d284adf033f4b05a40db45a383784f73afd2"),
}


def acquire(root):
    root.mkdir(parents=True, exist_ok=True)
    receipt = {}
    sources = {name: (f"https://codeload.github.com/{repo}/tar.gz/{commit}", commit)
               for name, (repo, commit) in PINS.items()}
    sources["fixest"] = ("https://cran.r-project.org/src/contrib/fixest_0.14.2.tar.gz", "0.14.2")
    for name, (url, version) in sources.items():
        target = root / f"{name}.tar.gz"
        if not target.exists():
            with urllib.request.urlopen(url, timeout=120) as response, target.open("wb") as output:
                __import__("shutil").copyfileobj(response, output)
        with tarfile.open(target) as archive:
            members = archive.getmembers()
            if any(m.name.startswith("/") or ".." in Path(m.name).parts for m in members):
                raise ValueError("unsafe archive member")
        receipt[name] = {"url": url, "version_or_commit": version,
                         "sha256": sha256(target), "bytes": target.stat().st_size}
    write_json(root / "archives.json", receipt)
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    acquire(p.parse_args().root)
