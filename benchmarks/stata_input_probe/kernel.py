"""Build and run the accessor microbenchmark, separately from other activity."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

REPO = Path(__file__).resolve().parents[2]


def main(root, previous):
    root, previous = root.resolve(), previous.resolve()
    if (root / "probe.plugin").exists():
        raise RuntimeError("refusing to overwrite an existing probe")
    shutil.copyfile(REPO / "benchmarks/stata_input_probe/probe.c", root / "probe.c")
    env = dict(os.environ, DEVELOPER_DIR="/Library/Developer/CommandLineTools")
    subprocess.run(["/usr/bin/clang", "-std=c11", "-O3", "-Wall", "-Wextra", "-Werror", "-DSYSTEM=APPLEMAC", "-I" + str(REPO / "plugin"), "-bundle", str(REPO / "plugin/stplugin.c"), str(root / "probe.c"), "-o", str(root / "probe.plugin")], env=env, check=True)
    script = '''version 19.0
clear all
set more off
set obs 270
forvalues j=1/5 {
    local type : word `j' of byte int long float double
    generate `type' x`j' = mod(_n,83)-40
    local codes ". .a .b .c .d .e .f .g .h .i .j .k .l .m .n .o .p .q .r .s .t .u .v .w .x .y .z"
    forvalues m=1/27 {
        local value : word `m' of `codes'
        replace x`j' = `value' in `m'
    }
}
generate double big = cond(mod(_n,2),8e307,-8e307)
generate str8 unused = "string"
program __input_probe, plugin using("ROOT/probe.plugin")
plugin call __input_probe x4 big x2 x5 x1 x3, semantic "ROOT/semantic.tsv"
plugin call __input_probe x3 x1 x5 big x2 x4 if mod(_n,2) in 29/200, semantic "ROOT/semantic-reordered.tsv"
foreach k in 10 50 {
    use "OLD/data/n983040-k`k'-gdefault-shuffle0.dta", clear
    plugin call __input_probe y x1-x`k' g1, timing "ROOT/kernel-k`k'.tsv"
}
display "INPUT_PROBE_COMPLETE"
'''.replace("ROOT", str(root)).replace("OLD", str(previous))
    (root / "probe.do").write_text(script)
    with (root / "probe.stdout").open("w") as out:
        result = subprocess.run([os.environ["STATA_MP_PATH"], "-b", "do", str(root / "probe.do")], cwd=root, stdout=out, stderr=subprocess.STDOUT, timeout=90)
    assert result.returncode == 0 and "\nINPUT_PROBE_COMPLETE\n" in (root / "probe.log").read_text()
    print("KERNEL_COMPLETE")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("previous", type=Path)
    a = p.parse_args()
    main(a.root, a.previous)
