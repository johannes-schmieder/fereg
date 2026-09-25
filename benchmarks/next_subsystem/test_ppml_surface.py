"""Boundary checks for the additive PPML comparison surface."""
import tempfile
import hashlib
import json
import unittest
from pathlib import Path

import numpy as np
import pandas as pd
from scipy.special import gammaln,xlogy

from audit import compare
from ppml_surface import configure,stage
from ppml_audit import design,public_state,explicit_poisson,means,output_state
from surface import columns


class PpmlSurfaceTests(unittest.TestCase):
    def test_rejects_unmatched_outputs_weights_and_inference(self):
        cell=dict(model='ppml',effects=['g1','g2'],k=2)
        for options in [dict(outputs='none'),dict(weight='[fw=g1]'),dict(vce='iid'),
                        dict(vce='cluster unknown'),dict(vce='cluster g1 g1'),dict(terms='time_series'),dict(terms='product'),
                        dict(outputs='all',slope_partitions={'g1':['z1']})]:
            with self.assertRaises(ValueError):configure(dict(cell,**options))
        c=configure(dict(cell,slope_partitions={'g1':['z1']}))
        self.assertEqual(c['effects_r'],'g1[z1] + g2')
        self.assertEqual(c['outputs'],'mu')

    def test_generated_cluster_and_saved_axes_are_explicit(self):
        c=configure(dict(model='ppml',effects=['g1','g2'],k=2,terms='factor_cross',
                         factor_levels=3,cross_levels=2,vce='cluster g2 cl1',outputs='all'))
        self.assertEqual(c['clusters'],['g2','cl1'])
        self.assertEqual(c['coefficient_names'],['x1','x2','2.f','3.f','2.h','2.f#2.h','3.f#2.h'])
        self.assertEqual(c['saved_absorb'],'saved_g1=g1 saved_g2=g2')
        self.assertEqual(len(c['output_names']),6)

    def test_saved_mean_alignment_requires_the_certified_row_hash(self):
        ids=np.array([1,3],dtype=np.int64)
        digest=hashlib.sha256(b''.join(int(x).to_bytes(8,'little') for x in ids)).hexdigest()
        with tempfile.TemporaryDirectory() as d:
            path=Path(d)
            (path/'result.json').write_text(json.dumps(dict(sample_sha256=digest)))
            np.array([2.,np.nan,5.],dtype='<f8').tofile(path/'means.bin')
            np.testing.assert_array_equal(means(path,dict(sha256=digest),3,ids),[2.,5.])
            with self.assertRaisesRegex(ValueError,'different estimation sample'):
                means(path,dict(sha256=digest),3,np.array([1,2]))

    def test_stock_repeats_keep_objective_and_covariance_checks(self):
        with tempfile.TemporaryDirectory() as d:
            paths=[Path(d)/name for name in ['first','second']]
            for path in paths:
                path.mkdir()
                (path/'result.json').write_text(json.dumps(dict(status='pass',sample_sha256='same',metadata=dict(N=10,loglik=-7.,deviance=2.))))
                (path/'coefficients.csv').write_text('name,value\nx1,0.2\n')
                (path/'covariance.csv').write_text('row,column,value\nx1,x1,0.03\n')
            self.assertTrue(compare(*paths,'ppml')['passed'])
            (paths[1]/'covariance.csv').write_text('row,column,value\nx1,x1,0.04\n')
            self.assertFalse(compare(*paths,'ppml')['passed'])

    def test_staged_workers_request_matching_public_means(self):
        root=Path(__file__).resolve().parents[1]/'local_fixest_gap'
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)
            for name in ['measure.py','stata_worker.do','r_worker.R']:(p/name).write_bytes((root/name).read_bytes())
            stage(p);compile((p/'measure.py').read_text(),'staged-measure','exec')
            self.assertIn('mu(saved_mu)',(p/'stata_worker.do').read_text())
            self.assertIn("memory(`memory')",(p/'stata_worker.do').read_text())
            self.assertIn('predict(fit, type="response")',(p/'r_worker.R').read_text())
            self.assertIn('means.bin',(p/'r_worker.R').read_text())
            self.assertIn('fixef.tol=1e-11',(p/'r_worker.R').read_text())
            self.assertIn('G.df="min"',(p/'r_worker.R').read_text())

    def test_explicit_slope_optimum_and_corrupted_mean_diagnostics(self):
        n=120;r=np.arange(n);frame=pd.DataFrame(dict(g1=r%4,z1=np.sin(r*.7)))
        F=design(frame,dict(effects=['g1'],slope_partitions={'g1':['z1']}))
        X=np.cos(r*.3)[:,None];beta=np.array([.2]);mu=np.exp(.2*X[:,0]+.1*(r%4)+.1*frame.z1.to_numpy())
        y=mu.copy();m=dict(ll=float(np.sum(y*np.log(mu)-mu-gammaln(y+1))),deviance=0.)
        self.assertTrue(public_state(y,X,F,beta,mu,m)['passed'])
        fit=explicit_poisson(y,X,F)
        np.testing.assert_allclose(fit['beta'],beta,atol=1e-10,rtol=1e-10)
        np.testing.assert_allclose(fit['mu'],mu,atol=1e-10,rtol=1e-10)
        changed=mu.copy();changed[0]*=1.01
        self.assertFalse(public_state(y,X,F,beta,changed,m)['passed'])
        self.assertEqual(public_state(y,X,F,beta,changed,m)['model_space'],'indeterminate')
        shifted=mu*1.01
        self.assertFalse(public_state(y,X,F,beta,shifted,m)['stationarity_pass'])
        self.assertEqual(public_state(y,X,F,beta,shifted,m)['model_space'],'feasible')

    def test_clustered_generated_oracle_matches_full_design_influence(self):
        r=np.arange(240);rng=np.random.default_rng(814)
        frame=pd.DataFrame(dict(g1=r%5,g2=(r//5)%4,x1=rng.normal(size=len(r)),
                                x2=rng.normal(size=len(r)),f=r%3+1))
        cell=configure(dict(model='ppml',effects=['g1','g2'],k=2,terms='factor',factor_levels=3))
        X=columns(frame,cell);F=design(frame,cell);D=np.column_stack([X,F.toarray()])
        beta=np.array([.2,-.1,.05,.08]);eta=X@beta+.04*frame.g1-.03*frame.g2
        mu=np.exp(eta.to_numpy());raw=rng.normal(size=len(r))
        # Construct a nonzero score-orthogonal residual: this is the known
        # Poisson optimum, not a fit supplied by either estimator.
        residual=raw-D@np.linalg.lstsq(D,raw,rcond=None)[0]
        residual*=.2*mu.min()/abs(residual).max();y=mu+residual
        clusters=np.column_stack([r%7,(r//7)%11])
        H=D.T@(mu[:,None]*D)
        influence=(D@np.linalg.pinv(H)[:,:X.shape[1]])*residual[:,None]
        meats=[]
        for keys in [list(clusters[:,0]),list(clusters[:,1]),list(map(tuple,clusters))]:
            groups={}
            for key,row in zip(keys,influence):groups[key]=groups.get(key,np.zeros(X.shape[1]))+row
            meats.append(sum(np.outer(row,row) for row in groups.values()))
        expected=(meats[0]+meats[1]-meats[2])*7/6
        values,vectors=np.linalg.eigh(expected)
        expected=(vectors*np.maximum(values,0))@vectors.T
        fit=explicit_poisson(y,X,F,clusters)
        np.testing.assert_allclose(fit['beta'],beta,atol=1e-10,rtol=1e-10)
        np.testing.assert_allclose(fit['mu'],mu,atol=1e-10,rtol=1e-10)
        np.testing.assert_allclose(fit['V'],expected,atol=1e-12,rtol=1e-8)
        self.assertEqual(fit['df_r'],6)

    def test_saved_channels_reject_corruption_even_when_the_sum_is_unchanged(self):
        r=np.arange(60);frame=pd.DataFrame(dict(g1=r%3,g2=r%5))
        X=np.sin(r)[:,None];beta=np.array([.2]);xb=X@beta
        a=.1*frame.g1.to_numpy();b=-.03*frame.g2.to_numpy();eta=xb+a+b;mu=np.exp(eta);y=mu+.1
        values=np.column_stack([xb,a+b,eta,y-mu,a,b]);cell=dict(effects=['g1','g2'])
        self.assertTrue(output_state(values,y,X,beta,mu,frame,cell)['passed'])
        values[0,4]+=.01;values[0,5]-=.01
        result=output_state(values,y,X,beta,mu,frame,cell)
        self.assertTrue(result['checks']['channel_sum']['passed'])
        self.assertFalse(result['checks']['g1_span']['passed'])
        self.assertFalse(result['passed'])


if __name__=='__main__':unittest.main()
