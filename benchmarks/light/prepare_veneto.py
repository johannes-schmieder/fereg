"""Materialize model inputs from an already verified retained private panel."""
import argparse
from pathlib import Path

import numpy as np
import pandas as pd

from common import sha256, tasks, write_json
from inputs import store


def prepare(retained,output,design):
    frame=pd.read_csv(retained).sort_values("observation_key")
    selected=[c for c in tasks()[9]["cells"] if c["design"]==design]
    expected=selected[0]["n"]
    if len(frame)!=expected or frame.duplicated(["worker","year"]).any():
        raise ValueError("retained sample count/identity differs")
    years=[1999,2001] if design=="veneto_short" else list(range(1996,2002))
    if sorted(frame.year.unique().tolist())!=years:
        raise ValueError("unexpected panel years")
    provenance=dict(retained_sha256=sha256(retained),years=years,rows=expected,
                    outcome="raw log wage for OLS; exp(raw log wage) for PPML",sample="preserved fevc membership")
    base=pd.DataFrame(dict(obs_id=frame.observation_key.to_numpy(),g1=frame.worker.to_numpy(),g2=frame.firm.to_numpy(),
                           x1=((frame.age.to_numpy()-40)/40)**2,x2=((frame.age.to_numpy()-40)/40)**3))
    for j,year in enumerate(years[:-1],3):
        base[f"x{j}"]=(frame.year.to_numpy()==year).astype(float)
    result={}
    for c in selected:
        f=base.copy()
        f["y"]=frame.y.to_numpy() if c["model"]=="ols" else np.exp(frame.y.to_numpy())
        path=output/(design+"-"+c["model"]+".dta")
        info=store(f,path,c,provenance)
        result[c["model"]]=dict(path=str(path.resolve()),sha256=info["sha256"])
    return result


if __name__=="__main__":
    p=argparse.ArgumentParser()
    p.add_argument("retained",type=Path)
    p.add_argument("output",type=Path)
    p.add_argument("design",choices=["veneto_short","veneto_long"])
    a=p.parse_args()
    a.output.mkdir(mode=0o700,parents=True,exist_ok=True)
    write_json(a.output/(a.design+".json"),prepare(a.retained,a.output,a.design))
