"""Exact rational check of the new fixture's mathematical optimum; no fit."""
from fractions import Fraction as F
import json


def check():
    records=[]
    for kind in ['unit','aw','fw','pw']:
        rows=[]
        for r in range(48):
            g,h,rep=1+r%4,1+(r//4)%3,r//12
            u,v=(1 if rep<2 else -1),(1 if rep%2==0 else -1)
            noise=4*u*v
            x1,x2=F(u)+F(g,500_000_000),F(v)+F(h,1_000_000_000)
            y=F(7,10)*x1-F(2,5)*x2+F(g,500_000_000)+F(h,1_000_000_000)+noise
            weight=F(1) if kind=='unit' else F(1+g+h) if kind=='fw' else F(1,4)+F(g,4)+F(h,8)
            rows.append((g,h,u,v,F(noise),x1,x2,y,weight))
        # Every dummy normal equation and both finite normal equations vanish
        # exactly. Within controls are nonzero, orthogonal u/v, proving rank two.
        for col in range(2):
            for level in sorted({r[col] for r in rows}):
                assert sum(r[8]*r[4] for r in rows if r[col]==level)==0
                assert sum(r[8]*r[2] for r in rows if r[col]==level)==0
                assert sum(r[8]*r[3] for r in rows if r[col]==level)==0
        assert sum(r[8]*r[4]*r[5] for r in rows)==0
        assert sum(r[8]*r[4]*r[6] for r in rows)==0
        assert sum(r[8]*r[2]*r[3] for r in rows)==0
        assert all(r[8]>0 for r in rows)
        a=sum(r[8]*r[2]**2 for r in rows)
        b=sum(r[8]*r[3]**2 for r in rows)
        assert a*b>0
        records.append(dict(kind=kind,physical_rows=48,weight_sum=str(sum(r[8] for r in rows)),
                            within_gram_determinant=str(a*b),all_normal_equations_exact_zero=True,
                            coefficient_truth=['7/10','-2/5'],residual_truth='4*u*v'))
    return records


if __name__=='__main__':
    print(json.dumps(check(),indent=2))
