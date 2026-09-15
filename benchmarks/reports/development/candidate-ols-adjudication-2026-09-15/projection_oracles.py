"""Reproducible unweighted two-FE projection oracles used in this report.

Repeated FE cells carry their exact observation multiplicities. These are
independent audit utilities, not production solvers or a qualification policy.
The empirical wrappers' hashes remain in the immutable receipts. This portable
copy removes only their machine paths and redirects the identical bridge helper.
"""
from pathlib import Path
import json,sys
import numpy as np
from scipy import sparse
from scipy.sparse.csgraph import connected_components,depth_first_order
from scipy.sparse.linalg import lsmr
root=Path(__file__).resolve().parents[4]
sys.path[:0]=[str(root/'scripts'),str(root/'tests/python')]

def bridge_rows(groups):
    _,left=np.unique(groups[:,0],return_inverse=True)
    _,right=np.unique(groups[:,1],return_inverse=True);right+=int(left.max())+1
    vertices=int(right.max())+1
    keys=left.astype(np.int64)*vertices+right
    unique,inverse,count=np.unique(keys,return_inverse=True,return_counts=True)
    u=unique//vertices;v=unique%vertices
    graph=sparse.coo_matrix((np.ones(2*len(u)),(np.r_[u,v],np.r_[v,u])),shape=(vertices,vertices)).tocsr()
    components,labels=connected_components(graph,directed=False)
    _,roots=np.unique(labels,return_index=True)
    virtual=vertices
    augmented=sparse.coo_matrix((np.ones(2*(len(u)+len(roots))),
        (np.r_[u,v,roots,np.full(len(roots),virtual)],np.r_[v,u,np.full(len(roots),virtual),roots])),shape=(vertices+1,vertices+1)).tocsr()
    order,parent=depth_first_order(augmented,virtual,directed=False,return_predecessors=True)
    discovery=np.empty(vertices+1,dtype=np.int64);discovery[order]=np.arange(vertices+1)
    low=discovery.copy()
    # Work on distinct FE cells. All observations in a bridge cell share
    # the fitted FE value; the cut equation forces their residual mean to zero.
    non_tree=(parent[u]!=v)&(parent[v]!=u)
    np.minimum.at(low,u[non_tree],discovery[v[non_tree]])
    np.minimum.at(low,v[non_tree],discovery[u[non_tree]])
    for node in order[:0:-1]:
        p=parent[node]
        if low[node]<low[p]:low[p]=low[node]
    bridges=((parent[u]==v)&(low[u]>discovery[v]))|((parent[v]==u)&(low[v]>discovery[u]))
    return bridges[inverse],int(components),inverse,count

def projection(target,groups):
    bridges,_,cell,count=bridge_rows(groups)
    _,first=np.unique(cell,return_index=True);cell_groups=groups[first]
    means=np.bincount(cell,weights=target)/count
    bridge_cell=bridges[first];keep=~bridge_cell
    residual_cell=np.zeros(len(count));record=dict(cells=len(count),cycle_cells=int(keep.sum()),passed=True)
    if keep.any():
        g=cell_groups[keep];w=count[keep].astype(float)
        _,l=np.unique(g[:,0],return_inverse=True);_,r=np.unique(g[:,1],return_inverse=True);r+=int(l.max())+1;vertices=int(r.max())+1
        graph=sparse.coo_matrix((np.ones(2*len(l)),(np.r_[l,r],np.r_[r,l])),shape=(vertices,vertices)).tocsr()
        components,labels=connected_components(graph,directed=False)
        degree=np.bincount(np.r_[l,r],weights=np.r_[w,w],minlength=vertices)
        # Choose the highest-degree coordinate of each component as its ground.
        order=np.lexsort((-degree,labels));_,positions=np.unique(labels[order],return_index=True);ground=order[positions]
        coordinates=np.ones(vertices,dtype=bool);coordinates[ground]=False
        mapping=np.cumsum(coordinates)-1
        edge=np.arange(len(l));rows=np.r_[edge[coordinates[l]],edge[coordinates[r]]];cols=np.r_[mapping[l[coordinates[l]]],mapping[r[coordinates[r]]]]
        entries=np.r_[np.sqrt(w[coordinates[l]]/degree[l[coordinates[l]]]),np.sqrt(w[coordinates[r]]/degree[r[coordinates[r]]])]
        matrix=sparse.coo_matrix((entries,(rows,cols)),shape=(len(l),int(coordinates.sum()))).tocsr()
        rhs=np.sqrt(w)*means[keep]
        fit=lsmr(matrix,rhs,atol=1e-15,btol=1e-15,conlim=1e14,maxiter=5000)
        residual_cell[keep]=(rhs-matrix@fit[0])/np.sqrt(w)
        record.update(cycle_components=int(components),columns=matrix.shape[1],stop=int(fit[1]),iterations=int(fit[2]),condition=float(fit[6]),normal_l2=float(fit[4]),passed=int(fit[1])in(0,1,2,4,5))
    residual=target-means[cell]+residual_cell[cell]
    score=[];scale=max(1.,float(np.linalg.norm(target)))
    for j in range(2):
        _,codes=np.unique(groups[:,j],return_inverse=True);size=np.bincount(codes)
        score.append(float(np.max(abs(np.bincount(codes,weights=residual))/np.sqrt(size))/scale))
    record.update(fresh_normal_max=max(score),fresh_normal_pass=max(score)<=1e-12)
    record['passed']&=record['fresh_normal_pass']
    return residual,record

def calibrate_cuts():
    # Brute-force edge deletion is independent and exact on these tiny graphs.
    from test_two_fe_span_audit import decimal_projection
    rng=np.random.default_rng(249138)
    checked=0
    for rows in [1,2,7,31,128]:
        for _ in range(10):
            groups=np.column_stack((rng.integers(0,12,rows),rng.integers(0,9,rows)))
            result,initial,cell,_=bridge_rows(groups)
            _,l=np.unique(groups[:,0],return_inverse=True);_,r=np.unique(groups[:,1],return_inverse=True);r+=int(l.max())+1;n=int(r.max())+1
            target=rng.normal(size=rows)
            for weight in [np.ones(rows,dtype=int),rng.integers(1,4,rows)]:
                oracle=target-np.bincount(cell,weights=target*weight)[cell]/np.bincount(cell,weights=weight)[cell]
                independent=decimal_projection(np.repeat(target,weight),np.repeat(groups,weight,axis=0))
                assert np.allclose(np.repeat(oracle,weight)[np.repeat(result,weight)],independent[np.repeat(result,weight)],rtol=1e-12,atol=1e-12)
                checked+=1
            for edge in range(rows):
                keep=cell!=cell[edge]
                g=sparse.coo_matrix((np.ones(2*int(keep.sum())),(np.r_[l[keep],r[keep]],np.r_[r[keep],l[keep]])),shape=(n,n)).tocsr()
                assert bool(result[edge])==(connected_components(g,directed=False,return_labels=False)>initial)
    return dict(graphs=50,high_precision_projection_checks=checked,method='exact whole-cell deletion component count, including parallel observations and disconnected components',passed=True)

def calibrate_projection():
    from test_two_fe_span_audit import decimal_projection,TwoFeSpanTests
    checks=[];rng=np.random.default_rng(11473)
    for name,groups in TwoFeSpanTests().cases().items():
        groups=np.r_[groups,[[100,100],[100,101],[101,101]]]
        for repeated in [False,True]:
            repeat=rng.integers(1,4,len(groups))if repeated else np.ones(len(groups),dtype=int)
            g=np.repeat(groups,repeat,axis=0);target=rng.normal(size=len(g))
            observed,record=projection(target,g);expected=decimal_projection(target,g)
            assert record['passed'],record
            assert np.allclose(observed,expected,rtol=1e-10,atol=1e-11),(name,float(max(abs(observed-expected))))
            checks.append(dict(case=name,repeated=repeated,max_abs=float(max(abs(observed-expected)))))
    return dict(passed=True,checks=checks,oracle='100-digit explicit-dummy projection')

if __name__=='__main__':
    print(json.dumps(dict(cuts=calibrate_cuts(),projection=calibrate_projection()),indent=2))
