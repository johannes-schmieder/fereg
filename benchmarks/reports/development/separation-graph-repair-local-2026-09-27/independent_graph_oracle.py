"""Independent two-FE separation proof; diagnostic only, never estimator routing."""
from collections import deque
import hashlib
import json
from pathlib import Path
import numpy as np
import pandas as pd


def certificate(frame):
    g = np.asarray(frame.g1, dtype=np.int64) - 1
    h = np.asarray(frame.g2, dtype=np.int64) - 1 + int(g.max()) + 1
    y = np.asarray(frame.y)
    size = int(h.max()) + 1
    parent = list(range(size))
    def root(v):
        while v != parent[v]:
            parent[v] = parent[parent[v]]
            v = parent[v]
        return v
    for a,b in zip(g[y>0],h[y>0]):
        a,b=root(int(a)),root(int(b))
        if a!=b:parent[b]=a
    roots = [root(v) for v in range(size)]
    names = {v:i for i,v in enumerate(sorted(set(roots)))}
    ga = [names[roots[int(v)]] for v in g]
    hb = [names[roots[int(v)]] for v in h]
    count=len(names)
    edges=[set() for _ in range(count)];reverse=[set() for _ in range(count)]
    for a,b,positive in zip(ga,hb,y>0):
        if not positive and a!=b:edges[a].add(b);reverse[b].add(a)
    # Kosaraju with explicit stacks, independent of production FE membership code.
    seen=set();order=[]
    for v in range(count):
        if v in seen:continue
        seen.add(v);stack=[(v,iter(sorted(edges[v])))]
        while stack:
            a,it=stack[-1]
            b=next(it,None)
            if b is None:order.append(a);stack.pop()
            elif b not in seen:seen.add(b);stack.append((b,iter(sorted(edges[b]))))
    components=[-1]*count;nc=0
    for v in reversed(order):
        if components[v]>=0:continue
        stack=[v];components[v]=nc
        while stack:
            a=stack.pop()
            for b in reverse[a]:
                if components[b]<0:components[b]=nc;stack.append(b)
        nc+=1
    dag=[set() for _ in range(nc)];indegree=[0]*nc
    for a in range(count):
        for b in edges[a]:
            a1,b1=components[a],components[b]
            if a1!=b1 and b1 not in dag[a1]:dag[a1].add(b1);indegree[b1]+=1
    queue=deque(v for v,d in enumerate(indegree) if d==0);rank=[0]*nc;visited=0
    while queue:
        a=queue.popleft();visited+=1
        for b in sorted(dag[a]):
            rank[b]=max(rank[b],rank[a]+1);indegree[b]-=1
            if indegree[b]==0:queue.append(b)
    assert visited==nc
    alpha=np.array([rank[components[names[roots[v]]]] for v in range(size)],dtype=np.int64)
    z=alpha[g]-alpha[h]
    # These integer potentials directly reconstruct the model-space direction.
    # All additional finite and third-FE coefficients are exactly zero.
    assert np.all(z[y>0]==0) and np.all(z[y==0]<=0)
    rows=np.flatnonzero(z<0)
    assert len(rows)>0 and np.all(y[rows]==0)
    isolated=np.zeros(len(y),dtype=bool)
    for column in ('g1','g2','g3'):
        if column in frame:
            isolated |= frame.groupby(column).y.transform('sum').to_numpy()==0
    return {'positive_components':count,'strong_components':nc,'separated_rows':len(rows),
            'zero_only_fe_rows':int(isolated.sum()),'additional_rows':int(np.sum((z<0)&~isolated)),
            'direction_max_positive_y':int(np.max(np.abs(z[y>0]))),
            'direction_max_zero_y':int(np.max(z[y==0])),
            'minimum_strict_margin':int(np.min(-z[rows])),
            'direction_sha256':hashlib.sha256(np.asarray(z,dtype='<i8').tobytes()).hexdigest(),
            'separated_obs_id':[int(v) for v in frame.obs_id.iloc[rows]],
            'additional_obs_id':[int(v) for v in frame.obs_id[(z<0)&~isolated]]}

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path, action='append', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    records = []
    for path in args.input:
        result = certificate(pd.read_stata(path, convert_categoricals=False))
        result.update(case=path.stem, input_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        records.append(result)
    args.output.write_text(json.dumps(records, indent=2) + '\n')
