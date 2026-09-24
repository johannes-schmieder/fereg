"""Independent explicit-dummy projection and full-product checks, outside timing."""
from pathlib import Path
import argparse
import json
import statistics
import struct
import numpy as np
from scipy import sparse
from scipy.sparse.linalg import lsmr


def read_frame(path):
    data = path.read_bytes()
    n, k, effects, tol = struct.unpack_from("<QQQd", data)
    at = 32
    codes, levels = [], []
    for _ in range(effects):
        levels.append(struct.unpack_from("<Q", data, at)[0]); at += 8
        codes.append(np.frombuffer(data, dtype="<u4", count=n, offset=at).copy()); at += 4*n
    w = np.frombuffer(data, dtype="<f8", count=n, offset=at).copy(); at += 8*n
    x = np.frombuffer(data, dtype="<f8", count=n*k, offset=at).reshape(n,k).copy(); at += 8*n*k
    assert at == len(data)
    return n, k, codes, levels, w, x


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("root", type=Path)
    root = parser.parse_args().root
    variants = json.loads((root/"declaration.json").read_text()).get("variants",[0,1])
    records = []
    for path in sorted((root / "frames").glob("*.bin")):
        case = path.stem
        n, k, codes, levels, w, x = read_frame(path)
        weights = w/w.max(); roots = np.sqrt(weights)
        f = sparse.hstack([sparse.csr_matrix((np.ones(n),(np.arange(n),c)),shape=(n,l))
                           for c,l in zip(codes,levels)], format="csr")
        a = f.multiply(roots[:,None]).tocsr()
        norms = np.sqrt(np.asarray(a.power(2).sum(axis=0)).ravel())
        a = a @ sparse.diags(1/norms)
        refs, stops = [], []
        for j in range(k):
            solved = lsmr(a, roots*x[:,j], atol=2e-15, btol=2e-15, maxiter=16000)
            refs.append(x[:,j]-(a@solved[0])/roots); stops.append([solved[1],solved[2]])
        reference = np.column_stack(refs)
        scales = np.linalg.norm(roots[:,None]*x,axis=0)
        normalized = (roots[:,None]*reference)/scales
        ref_gram = normalized.T@normalized
        for threads in [1,4,8]:
            for variant in variants:
                prefix = root / f"{case}-t{threads}-v{variant}"
                within = np.fromfile(str(prefix)+"-within.bin",dtype="<f8").reshape(n,k)
                gram = np.fromfile(str(prefix)+"-gram.bin",dtype="<f8").reshape(k,k)
                saved_scales = np.fromfile(str(prefix)+"-scales.bin",dtype="<f8")
                error = np.linalg.norm(roots[:,None]*(within-reference),axis=0)/scales
                score = np.max(np.abs(a.T@(roots[:,None]*within)),axis=0)/scales
                gram_error = np.max(np.abs(gram-ref_gram))
                row_gram = (roots[:,None]*within/saved_scales).T@(roots[:,None]*within/saved_scales)
                gram_self_error = np.max(np.abs(gram-row_gram))
                passed = bool(np.max(error)<=1e-10 and np.max(score)<=1e-12 and
                              gram_error<=1e-10 and gram_self_error<=1e-10)
                records.append(dict(case=case,threads=threads,variant=variant,projection_errors=error.tolist(),
                    scores=score.tolist(),gram_error=float(gram_error),gram_self_error=float(gram_self_error),
                    scale_error=float(np.max(np.abs(saved_scales/scales-1))),lsmr_stops=stops,passed=passed))
    with (root/"oracle.json").open("x") as out: json.dump(records,out,indent=2);out.write("\n")
    setup, timings = {}, {}
    for line in (root/"run.log").read_text().splitlines():
        if "SETUP|" in line: line=line[line.index("SETUP|"):]
        if line.startswith(("SETUP|","TIME|")):
            tag,*parts=line.split("|"); record=dict(p.split("=",1) for p in parts)
            key=(record["case"],int(record["threads"]))
            if tag=="SETUP": setup[key]=record
            else: timings.setdefault(key,[]).append(record)
    summary=[]
    for key,values in timings.items():
        b=sorted((v for v in values if v["variant"]=="0"),key=lambda v:int(v["rep"]))
        for variant in variants[1:]:
          c=sorted((v for v in values if v["variant"]==str(variant)),key=lambda v:int(v["rep"]))
          assert len(b)==len(c)==7
          bt=[1000*float(v["seconds"]) for v in b];ct=[1000*float(v["seconds"]) for v in c]
          bs=1000*float(setup[key]["baseline"]);cs=1000*float(setup[key]["candidate"])
          summary.append(dict(case=key[0],threads=key[1],variant=variant,baseline_ms=bt,candidate_ms=ct,
            baseline_setup_ms=bs,candidate_setup_ms=cs,
            cold_ratio=(statistics.median(ct)+cs)/(statistics.median(bt)+bs),
            warm_ratio=statistics.median(ct)/statistics.median(bt),
            paired_ms=[cc-bb for cc,bb in zip(ct,bt)],
            candidate_phases_ms={phase:1000*statistics.median(float(v[phase]) for v in c)
                for phase in ["reference","moments","solve","certificate_seconds"]},
            candidate_usable=all(v["usable"]=="true" for v in c),
            candidate_payload_bytes=int(c[0]["bytes"])))
    with (root/"summary.json").open("x") as out: json.dump(summary,out,indent=2);out.write("\n")
    print(f"{len(records)} oracle configurations; {sum(v['passed'] for v in records)} passed")
    for v in summary:
        print(v["case"],v["threads"],v["variant"],round(statistics.median(v["baseline_ms"]),3),
              round(statistics.median(v["candidate_ms"]),3),round(v["cold_ratio"],3),v["candidate_usable"])
    assert all(v["passed"] for v in records)


if __name__=="__main__": main()
