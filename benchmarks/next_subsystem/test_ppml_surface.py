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
from ppml_audit import design,public_state,explicit_poisson,means


class PpmlSurfaceTests(unittest.TestCase):
    def test_rejects_unmatched_outputs_weights_and_inference(self):
        cell=dict(model='ppml',effects=['g1','g2'],k=2)
        for options in [dict(outputs='all'),dict(weight='[fw=g1]'),dict(vce='cluster g1'),dict(terms='factor')]:
            with self.assertRaises(ValueError):configure(dict(cell,**options))
        c=configure(dict(cell,slope_partitions={'g1':['z1']}))
        self.assertEqual(c['effects_r'],'g1[z1] + g2')
        self.assertEqual(c['outputs'],'mu')

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


if __name__=='__main__':unittest.main()
