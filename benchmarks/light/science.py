"""Independent checks of captured public predictions and paired outputs."""
import csv
import hashlib
import importlib.util
import math
from pathlib import Path
import sys

import numpy as np
import pandas as pd
from scipy import sparse
from scipy.sparse.linalg import lsmr
from scipy.special import gammaln, xlogy

from common import REPO, SCIENCE, read_json, write_json


def coefficients(path):
    frame = pd.read_csv(Path(path)/"coefficients.csv")
    return {r["name"]: (float(r["value"]),float(r["se"])) for r in frame.to_dict("records")}


def design(frame, c):
    blocks = []
    for g in c["effects"]:
        code, labels = pd.factorize(frame[g], sort=True)
        block = sparse.csr_matrix((np.ones(len(frame)),(np.arange(len(frame)),code)),shape=(len(frame),len(labels)))
        blocks.append(block)
        if c["slopes"] and g == "g2":
            blocks.append(block.multiply(frame.z.to_numpy()[:,None]))
    return sparse.hstack(blocks,format="csr")


def public_audit(data, c, output):
    output = Path(output)
    frame = pd.read_stata(data,convert_categoricals=False)
    raw = np.fromfile(output/"state.bin",dtype="<f8")
    if raw.size % 4:
        raise ValueError("truncated public-state capture")
    ids, fitted, xb, saved = raw.reshape(4,-1)
    if not np.isfinite(ids).all() or np.any(ids != ids.astype(np.int64)) or len(np.unique(ids)) != len(ids):
        raise ValueError("invalid public sample IDs")
    positions = pd.Index(frame.obs_id).get_indexer(ids)
    if np.any(positions<0) or np.any(np.diff(positions)<=0):
        raise ValueError("public sample/order differs from input")
    frame = frame.iloc[positions]
    if not np.isfinite(fitted).all() or not np.isfinite(xb).all():
        raise ValueError("nonfinite public predictor")
    y = frame.y.to_numpy()
    residual = y-fitted
    eta = fitted if c["model"]=="ols" else np.log(fitted)
    if not np.isfinite(eta).all():
        raise ValueError("invalid public mean domain")
    b = coefficients(output)
    controls = frame[[f"x{j}" for j in range(1,c["k"]+1)]].to_numpy()
    finite = controls @ np.array([b[f"x{j}"][0] for j in range(1,c["k"]+1)])
    # Stata includes its posted constant in xb; bind all nonconstant slopes.
    delta = xb-finite
    binding = float(np.max(np.abs(delta-delta.mean())) / max(1,np.max(np.abs(xb))))
    scale = np.abs(y)+np.abs(fitted)
    scores = [abs(float(residual.sum()))/max(1,float(scale.sum()))]
    scores += list(np.abs(controls.T@residual)/np.maximum(1,np.abs(controls).T@scale))
    groups = []
    for g in c["effects"]:
        code = pd.factorize(frame[g],sort=True)[0]
        groups.append(code)
        scores.append(float(np.max(np.abs(np.bincount(code,weights=residual))/np.maximum(1,np.bincount(code,weights=scale)))))
        if c["slopes"] and g=="g2":
            z = frame.z.to_numpy()
            scores.append(float(np.max(np.abs(np.bincount(code,weights=z*residual))/np.maximum(1,np.bincount(code,weights=np.abs(z)*scale)))))
    objective = dict(rss=float(residual@residual)) if c["model"]=="ols" else dict(
        loglik=float(np.sum(xlogy(y,fitted)-fitted-gammaln(y+1))),
        deviance=float(2*np.sum(xlogy(y,y/fitted)-(y-fitted))))
    result = dict(sample_rows=len(ids), sample_sha256=hashlib.sha256(ids.astype("<i8").tobytes()).hexdigest(),
                  all_input_rows=len(ids)==c["n"], coefficient_binding_error=binding,
                  max_normalized_score=float(max(scores)), objective=objective,
                  score_pass=bool(max(scores)<=1e-8), coefficient_binding_pass=binding<=1e-10)
    if c["save_fe"]:
        error = float(np.max(np.abs(eta-xb-saved))/max(1,np.max(np.abs(eta))))
        result.update(saved_reconstruction_error=error,saved_reconstruction_pass=bool(np.isfinite(error) and error<=1e-8))
    target = eta-xb
    # Preserve cheap fresh diagnostics if the independent projection times out.
    result["status"]="indeterminate"
    write_json(output/"science.json",result)
    if len(groups)==1 and not c["slopes"]:
        g = groups[0]
        alpha = np.bincount(g,weights=target)/np.bincount(g)
        error = target-alpha[g]
        relative = float(np.linalg.norm(error)/max(1,np.linalg.norm(target)))
        result["span"] = dict(method="independent_group_means",coefficient_binding_pass=relative<=1e-10,fresh_residual_relative=relative)
    elif len(groups)==2 and not c["slopes"]:
        spec = importlib.util.spec_from_file_location("independent_span", REPO/"scripts/two_fe_span_audit.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        result["span"] = module.audit_two_fe_span(target,np.column_stack(groups),memory_bytes=1280*len(ids),max_iterations=5000)
    else:
        F = design(frame,c)
        norms = np.sqrt(np.asarray(F.power(2).sum(axis=0)).ravel())
        normed = F.multiply(1/np.maximum(norms,1e-100))
        solution = lsmr(normed,target,atol=1e-13,btol=1e-13,maxiter=5000,conlim=1e14)
        error = target-normed@solution[0]
        relative = float(np.linalg.norm(error)/max(1,np.linalg.norm(target)))
        normal = float(np.max(np.abs(normed.T@error))/max(1,np.linalg.norm(target)))
        result["span"] = dict(method="independent_scaled_lsmr",iterations=int(solution[2]),
                              fresh_residual_relative=relative,fresh_normal_max=normal,
                              coefficient_binding_pass=bool(relative<=1e-10 and normal<=1e-12))
    meta = read_json(output/"result.json")["metadata"]
    field = "rss" if c["model"]=="ols" else "deviance"
    reported = meta.get(field)
    result["objective_replay_pass"] = isinstance(reported,(float,int)) and math.isclose(objective[field],reported,rel_tol=1e-8,abs_tol=1e-10)
    if c["model"]=="ppml":
        saturated=float(np.sum(xlogy(y,y)-y-gammaln(y+1)))
        ll=meta.get("ll",meta.get("loglik"))
        dev=meta.get("deviance")
        result["reported_poisson_identity_pass"]=bool(isinstance(ll,(float,int)) and isinstance(dev,(float,int)) and
            np.isfinite(ll) and np.isfinite(dev) and dev>=0 and math.isclose(ll+.5*dev,saturated,rel_tol=1e-8,abs_tol=1e-10))
    result["status"] = "pass" if all((result["score_pass"],result["coefficient_binding_pass"],
        result["span"]["coefficient_binding_pass"],result.get("saved_reconstruction_pass",True),result["objective_replay_pass"])) else "diagnostic_discrepancy"
    write_json(output/"science.json",result)
    return result


def compare(left, right, c):
    left,right = Path(left),Path(right)
    a,b = read_json(left/"result.json"),read_json(right/"result.json")
    checks = {}
    if a["status"]!="pass" or b["status"]!="pass":
        return dict(status="unavailable",checks=checks)
    checks["input"] = a["input_sha256"]==b["input_sha256"]
    checks["host_task"] = all(a.get(k)==b.get(k) and a.get(k) is not None for k in ("host","job_id","task_id"))
    checks["N"] = a["metadata"].get("N")==b["metadata"].get("N")
    aa = read_json(left/"science.json") if (left/"science.json").exists() else {}
    bb = read_json(right/"science.json") if (right/"science.json").exists() else {}
    checks["sample"] = bool(aa.get("sample_sha256")) and aa.get("sample_sha256")==bb.get("sample_sha256")
    checks["native_certificates"] = a["metadata"].get("converged")==1 and aa.get("status")=="pass"
    ab,bc = coefficients(left),coefficients(right)
    names = [f"x{i}" for i in range(1,c["k"]+1)]
    checks["coefficients"] = all(x in ab and x in bc for x in names) and bool(np.allclose([ab[x][0] for x in names],[bc[x][0] for x in names],rtol=1e-7,atol=1e-10))
    checks["SE"] = all(x in ab and x in bc for x in names) and bool(np.allclose([ab[x][1] for x in names],[bc[x][1] for x in names],rtol=1e-6,atol=1e-10))
    if (left/"covariance.csv").exists() and (right/"covariance.csv").exists():
        va = pd.read_csv(left/"covariance.csv").set_index(["row","column"]).value
        vb = pd.read_csv(right/"covariance.csv").set_index(["row","column"]).value
        keys = [(x,y) for x in names for y in names]
        checks["covariance"] = bool(np.allclose(va.reindex(keys),vb.reindex(keys),rtol=1e-6,atol=1e-10))
    else:
        checks["covariance"] = False
    if b["arm"]=="reghdfe":
        for k in ("df_a","df_r"):
            checks[k] = a["metadata"].get(k) is not None and a["metadata"].get(k)==b["metadata"].get(k)
    field = "rss" if c["model"]=="ols" else "deviance"
    av,bv = a["metadata"].get(field),b["metadata"].get(field)
    if isinstance(av,(int,float)) and isinstance(bv,(int,float)):
        checks["objective"] = math.isclose(av,bv,rel_tol=1e-8,abs_tol=1e-10)
        if c["model"]=="ppml" and b["arm"]=="ppmlhdfe":
            checks["reported_objective_identity"] = bb.get("reported_poisson_identity_pass",False)
            checks["objective"] = av <= bv+1e-10+1e-8*abs(bv)
            ll_a,ll_b = a["metadata"].get("ll"),b["metadata"].get("ll")
            checks["reported_ll"] = isinstance(ll_a,(int,float)) and isinstance(ll_b,(int,float)) and ll_a>=ll_b-1e-10-1e-8*abs(ll_b)
            checks["total_rank"] = a["metadata"].get("rank",-1)+a["metadata"].get("df_a",-1)==b["metadata"].get("rank",-2)+b["metadata"].get("df_a",-2)
    else:
        checks["objective"] = False
    if c["save_fe"]:
        checks["saved_FE"] = bool(aa.get("saved_reconstruction_pass")) and bool(bb.get("saved_reconstruction_pass"))
    return dict(status="pass" if all(checks.values()) else "accuracy_or_evidence_failure",checks=checks,
                comparator_prediction_diagnostics=bb.get("status","missing"))


if __name__=="__main__":
    cfg=read_json(sys.argv[1])
    public_audit(cfg["data"],cfg["cell"],cfg["output"])
