"""Public commands for the pinned comparator capabilities."""
from common import supported


def stata_command(c, arm, threads):
    ok, reason = supported(c, arm)
    if not ok:
        raise ValueError(reason)
    effects = list(c["effects"])
    if c["slopes"]:
        effects[1] = "g2##c.z"
    if c["save_fe"] and arm not in ("greg", "gglm"):
        effects = [f"saved{j}={g}" for j, g in enumerate(effects, 1)]
    controls = " ".join(f"x{j}" for j in range(1, c["k"]+1))
    command = f"{arm} y {controls}, absorb({' '.join(effects)})"
    if arm in ("fereg", "feppml"):
        command += f" threads({threads})"
    if arm in ("greg", "gglm"):
        command += " mata(BenchG)"
        command += " predict(audit_link)" if arm=="gglm" else " resid(audit_resid)"
        if c["save_fe"]:
            command += " alphas(" + " ".join(f"saved{j}" for j in range(1, len(effects)+1)) + ")"
        if arm == "gglm":
            command += " family(poisson)"
    elif c["model"] == "ols":
        command += " residuals(audit_resid)"
    elif arm == "feppml":
        command += " mu(audit_mu)"
    else:
        command += " d(audit_d)"
    if c["clusters"]:
        clusters = " ".join(c["clusters"])
        command += f" cluster({clusters})" if arm in ("greg", "gglm") else f" vce(cluster {clusters})"
    elif c["model"] == "ppml":
        command += " robust" if arm == "gglm" else " vce(robust)"
    return command


def stata_request(config):
    c = config["cell"]
    values = dict(config, k=c["k"], dimensions=len(c["effects"]),
                  model=c["model"], save=int(c["save_fe"]), slopes=int(c["slopes"]))
    lines = []
    for name in ("source", "refs", "data", "warm", "output", "arm", "model", "k",
                 "threads", "dimensions", "save", "slopes", "inputvars"):
        value = str(values[name])
        if any(x in value for x in ('"', '`', '\n', '\r')):
            raise ValueError("unsafe Stata request value")
        lines.append(f'local {name} "{value}"')
    lines.append('local command "' + stata_command(c, config["arm"], config["threads"]) + '"')
    return "\n".join(lines) + "\n"
