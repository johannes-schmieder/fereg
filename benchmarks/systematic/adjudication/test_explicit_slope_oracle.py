import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np
import pandas as pd
from scipy import sparse

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from oracle import oracle as sparse_oracle
from explicit_slope_oracle import Design, oracle


def case(model='ols'):
    rng = np.random.default_rng(412786)
    n = 200
    frame = pd.DataFrame(dict(obs_id=np.arange(n)+1, g1=np.repeat(np.arange(n//5),5),
                              g2=rng.integers(0,9,n), g3=rng.integers(0,4,n)))
    for name in ['x1','x2','z1','z2','z3','z4']:
        frame[name] = rng.normal(size=n)
    eta = .2*frame.x1-.1*frame.x2+.1*np.sin(frame.g1)+.1*frame.z1*np.cos(frame.g1)
    frame['y'] = eta+rng.normal(size=n) if model=='ols' else rng.poisson(np.exp(eta))+1
    cell=dict(model=model,k=2,effects=['g1','g2','g3'],slopes={'g1':['z1','z2'],'g2':['z3','z4']},section='main')
    return frame,cell


class ExplicitSlopeTests(unittest.TestCase):
    def test_weighted_projection_against_full_dense_svd(self):
        frame,cell=case();d=Design(frame,cell)
        weights=np.exp(np.linspace(-2,2,len(frame)))
        factor=d.factor(weights);b,pred=factor.solve(d.y)
        full=d.full.toarray();root=np.sqrt(weights)
        expected,_,rank,_=np.linalg.lstsq(full*root[:,None],d.y*root,rcond=None)
        self.assertEqual(factor.trace['rank'],rank)
        np.testing.assert_allclose(pred,full@expected,rtol=1e-10,atol=1e-10)
        np.testing.assert_allclose(b[:2],expected[:2],rtol=1e-10,atol=1e-10)
        bread,Z,_=factor.inference()
        nuisance=full[:,2:];within=full[:,:2]-nuisance@np.linalg.lstsq(root[:,None]*nuisance,root[:,None]*full[:,:2],rcond=None)[0]
        np.testing.assert_allclose(Z,within,rtol=1e-9,atol=1e-10)
        np.testing.assert_allclose(bread,np.linalg.inv(within.T@(weights[:,None]*within)),rtol=1e-10,atol=1e-10)

    def test_ols_and_poisson_against_sparse_full_design(self):
        for model in ['ols','ppml']:
            frame,cell=case(model)
            with tempfile.TemporaryDirectory() as folder:
                path=Path(folder)/'input.dta';frame.to_stata(path,write_index=False,version=118)
                expected=sparse_oracle(path,cell)
            actual=oracle(frame,cell)
            self.assertEqual(actual['fe_rank'],expected['fe_rank'])
            np.testing.assert_allclose(actual['coefficients'],expected['coefficients'],rtol=1e-8,atol=1e-9)
            np.testing.assert_allclose(actual['covariance'],expected['covariance'],rtol=1e-8,atol=1e-9)
            objective='rss' if model=='ols' else 'deviance'
            self.assertAlmostEqual(actual[objective],expected[objective],places=8)

    def test_row_permutation(self):
        frame,cell=case();a=oracle(frame,cell);b=oracle(frame.sample(frac=1,random_state=57),cell)
        np.testing.assert_allclose(a['coefficients'],b['coefficients'],atol=1e-11)
        np.testing.assert_allclose(a['covariance'],b['covariance'],atol=1e-11)

    def test_invalid_first_geometry_fails_closed(self):
        frame,cell=case();frame['z2']=frame.z1
        with self.assertRaisesRegex(ValueError,'first-partition rank'):
            Design(frame,cell).factor(np.ones(len(frame)))
        frame,cell=case()
        with self.assertRaisesRegex(ValueError,'group sizes'):
            Design(frame.iloc[:-1],cell)

    def test_separation_is_not_silently_ignored(self):
        frame,cell=case('ppml');frame.loc[0,'y']=0
        with self.assertRaisesRegex(ValueError,'separation'):
            oracle(frame,cell)


if __name__=='__main__':unittest.main()
