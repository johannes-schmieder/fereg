"""Frozen scope for the owner-requested light development comparison."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
SOURCE = "e23f15cad16c2f0b2368326aea579921a069ca9a"
SEED = 104729
SIZES = (399960, 1599960)
ARMS = {"ols": ["fereg", "reghdfe", "fixest", "pyfixest", "greg"],
        "ppml": ["feppml", "ppmlhdfe", "fixest", "pyfixest", "gglm"]}
DESIGNS = {
    "one_fe": {"suite": "panels", "effects": ["g1"], "mobility": .20},
    "strong_d3": {"suite": "paper", "effects": ["g1", "g2"], "degree": 3},
    "weak_d3": {"suite": "paper", "effects": ["g1", "g2"], "degree": 3},
    "three_fe_sparse": {"suite": "panels", "effects": ["g1", "g2", "g3"], "mobility": .02},
}
SCIENCE = dict(coefficient_rtol=1e-7, coefficient_atol=1e-10,
               vce_rtol=1e-6, vce_atol=1e-10, objective_rtol=1e-8,
               objective_atol=1e-10)


def read_json(path):
    return json.loads(Path(path).read_text())


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n")
    temporary.replace(path)


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def cell_id(c):
    return f"{c['model']}-{c['design']}-n{c['n']}-k{c['k']}-{c['variant']}"


def cell(design, n, variant="base", k=10, save=False, cluster=(), slopes=False):
    return dict(design=design, n=n, k=k, variant=variant, save_fe=save,
                clusters=list(cluster), slopes=slopes,
                effects=DESIGNS.get(design, {"effects": ["g1", "g2"]})["effects"],
                section="main", private=design.startswith("veneto"))


def tasks():
    groups = [[cell(g, n) for n in SIZES] for g in DESIGNS]
    for k in (2, 50):
        groups.append([cell("weak_d3", SIZES[0], f"k{k}-save{int(save)}", k=k, save=save)
                       for save in (False, True)])
    groups += [
        [cell("weak_d3", SIZES[0], "save", save=True),
         cell("weak_d3", SIZES[0], "cluster-worker", cluster=("g1",))],
        [cell("weak_d3", SIZES[0], "cluster-firm", cluster=("g2",)),
         cell("weak_d3", SIZES[0], "cluster-twoway", cluster=("g1", "g2"))],
        [cell("weak_d3", SIZES[0], f"slope-save{int(save)}", slopes=True, save=save)
         for save in (False, True)],
        [cell("veneto_short", 1319994, k=3), cell("veneto_long", 5163428, k=7)],
    ]
    return [dict(id=i, cells=[dict(c, model=m) for c in cells for m in ARMS],
                 slots=28, wall_seconds=43200, max_fits=120)
            for i, cells in enumerate(groups, 1)]


def supported(c, arm):
    if arm in ("greg", "gglm"):
        if c["slopes"]:
            return False, "pinned gtools absorb accepts categorical varlists, not absorbed slopes"
        if len(c["clusters"]) > 1:
            return False, "pinned gtools supports nested clustering, not general two-way clustering"
    if arm == "pyfixest" and c["slopes"]:
        return False, "pinned PyFixest has no public absorbed varying-slope interface"
    return True, None


def arm_order(model, rep):
    arms = ARMS[model]
    offset = rep - 1
    order = arms[offset:] + arms[:offset]
    return order if rep % 2 else list(reversed(order))


def thread_environment(threads):
    return {"OMP_NUM_THREADS": str(threads), "RAYON_NUM_THREADS": str(threads),
            "NUMBA_NUM_THREADS": str(threads), "OPENBLAS_NUM_THREADS": "1",
            "MKL_NUM_THREADS": "1", "NUMEXPR_NUM_THREADS": "1",
            "OMP_MAX_ACTIVE_LEVELS": "1", "OMP_NESTED": "FALSE"}


def manifest():
    return dict(schema="fereg-light-v1", source_commit=SOURCE, seed=SEED,
                repetitions=3, threads=[4, 28], tasks=tasks(), arms=ARMS,
                science=SCIENCE, designs=DESIGNS, max_fits=1200,
                command_timeout_seconds=300, startup_timeout_seconds=180,
                extraction_timeout_seconds=180, shutdown_margin_seconds=900,
                pss_intervals_seconds={"synthetic_small": .02, "other": .05},
                timing="fresh process; small untimed initialization; command with concurrent PSS",
                prediction_capture="residual/mean audit vector included; save_fe adds individual FE channels",
                purpose="owner-requested development comparison, not release qualification")


if __name__ == "__main__":
    print(json.dumps(manifest(), indent=2))
