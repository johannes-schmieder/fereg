"""Check candidate WLS coefficients and inverse products against explicit FE + QR."""
from pathlib import Path
import argparse
import json
import numpy as np
from scipy import sparse
from scipy.sparse.linalg import lsmr
from audit import read_frame


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    root = parser.parse_args().root
    variants = json.loads((root/"declaration.json").read_text()).get("variants",[0,1])
    records=[]
    for path in sorted((root/"frames").glob("*.bin")):
        n,k,codes,levels,w,x=read_frame(path)
        roots=np.sqrt(w/w.max())
        fe=sparse.hstack([sparse.csr_matrix((roots,(np.arange(n),c)),shape=(n,l))
                         for c,l in zip(codes,levels)],format="csr")
        fe=fe@sparse.diags(1/np.sqrt(np.asarray(fe.power(2).sum(axis=0)).ravel()))
        scales=np.linalg.norm(roots[:,None]*x,axis=0)
        weighted=roots[:,None]*x/scales
        for j in range(k):
            solved=lsmr(fe,weighted[:,j],atol=2e-15,btol=2e-15,maxiter=16000)
            assert solved[1] in [1,2,4,5]
            weighted[:,j]-=fe@solved[0]
        q,r=np.linalg.qr(weighted[:,1:],mode="reduced")
        coefficient=np.linalg.solve(r,q.T@weighted[:,0])
        inverse_r=np.linalg.solve(r,np.eye(k-1))
        bread=inverse_r@inverse_r.T
        reference_gram=weighted[:,1:].T@weighted[:,1:]
        for threads in [1,4,8]:
            for variant in variants:
                prefix=root/f"{path.stem}-t{threads}-v{variant}"
                g=np.fromfile(str(prefix)+"-gram.bin",dtype="<f8").reshape(k,k)
                got=np.linalg.solve(g[1:,1:],g[1:,0])
                got_bread=np.linalg.solve(g[1:,1:],np.eye(k-1))
                raw_ref=coefficient*scales[0]/scales[1:]
                raw_got=got*scales[0]/scales[1:]
                coefficient_ratio=float(np.max(np.abs(raw_got-raw_ref)/(1e-10+1e-7*np.abs(raw_ref))))
                left=float(np.linalg.norm(got_bread@reference_gram-np.eye(k-1),ord=np.inf))
                right=float(np.linalg.norm(reference_gram@got_bread-np.eye(k-1),ord=np.inf))
                bread_relative=float(np.linalg.norm(got_bread-bread,ord=np.inf)/np.linalg.norm(bread,ord=np.inf))
                passed=coefficient_ratio<=1 and max(left,right)<=5e-10
                records.append(dict(case=path.stem,threads=threads,variant=variant,
                    coefficient_tolerance_ratio=coefficient_ratio,left_inverse_error=left,
                    right_inverse_error=right,bread_relative_error=bread_relative,passed=passed))
    with (root/"finite-oracle.json").open("x") as out:
        json.dump(records,out,indent=2);out.write("\n")
    print(len(records),"finite WLS/inverse configurations;",sum(v["passed"] for v in records),"passed")
    assert all(v["passed"] for v in records)


if __name__=="__main__": main()
