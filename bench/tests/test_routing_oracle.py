"""Dense original-design witnesses for the independent sparse Schur oracle."""
import copy
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing_oracle as oracle
from native_io import digest


def fixture(disconnected=False, weighted=True):
    rng = np.random.default_rng(177)
    a, b, c = np.meshgrid(np.arange(5), np.arange(4), np.arange(3), indexing='ij')
    factors = [a.ravel(), b.ravel(), c.ravel()]
    if disconnected:
        factors = [np.r_[v, v + v.max() + 1] for v in factors]
    n = len(factors[0])
    x = rng.standard_normal((n, 3))
    y = x @ np.array([.7,-.2,.4]) + np.sin(factors[0]) + .3*factors[1] + rng.normal(size=n)*.2
    weights = np.exp(rng.uniform(-2,2,n)) if weighted else np.ones(n)
    return y, x, factors, weights


def dense(y, x, factors, w, sample, intercept):
    """A full observation-by-dummy SVD, independent of Schur/ground selection."""
    y, x, w = y[sample], x[sample], w[sample]
    w = w * len(w) / w.sum()
    dummy = np.column_stack([np.eye(len(np.unique(v[sample])))[np.unique(v[sample], return_inverse=True)[1]]
                             for v in factors])
    design = np.column_stack([x, dummy])
    weighted = design*np.sqrt(w)[:,None]
    inverse = np.linalg.pinv(weighted, rcond=1e-13)*np.sqrt(w)[None,:]
    full = inverse@y
    fitted = design@full
    residual = y-fitted
    k = x.shape[1]
    transform = np.zeros((k+int(intercept),len(full)))
    transform[:k,:k] = np.eye(k)
    if intercept:
        transform[-1,k:] = np.average(dummy,axis=0,weights=w)
    beta = transform@full
    influence = transform@inverse
    rank = np.linalg.matrix_rank(weighted)
    covariance = (influence*residual)@(influence*residual).T*len(y)/(len(y)-rank)
    return beta,covariance,fitted,residual,rank-k


class RoutingOracle(unittest.TestCase):
    def test_unit_iid_and_cr1_match_full_dummy_svd_sandwich(self):
        for disconnected in [False, True]:
            for intercept in [False, True]:
                y, x, factors, w = fixture(disconnected, weighted=False)
                # Full original design and normalized-constant transformation;
                # no oracle projection, bread, cluster sums or DoF helpers.
                dummy = np.column_stack([np.eye(len(np.unique(f)))[np.unique(f, return_inverse=True)[1]] for f in factors])
                design = np.column_stack([x, dummy])
                inverse = np.linalg.pinv(design, rcond=1e-13)
                residual = y-design@(inverse@y)
                transform = np.zeros((x.shape[1]+int(intercept), design.shape[1]))
                transform[:x.shape[1], :x.shape[1]] = np.eye(x.shape[1])
                if intercept:
                    transform[-1, x.shape[1]:] = dummy.mean(axis=0)
                influence = transform@inverse
                df = len(y)-np.linalg.matrix_rank(design)
                for vce in ['iid', 'cluster']:
                    groups = np.unique(factors[0])
                    if vce == 'iid':
                        covariance = influence@influence.T*(residual@residual/df)
                    else:
                        sums = np.stack([influence[:, factors[0] == g]@residual[factors[0] == g] for g in groups])
                        covariance = sums.T@sums*(len(y)-1)/df*len(groups)/(len(groups)-1)
                    ref, arrays = oracle.solve(y, x, factors, w, include_intercept=intercept,
                                               vce=vce, clusters=factors[0] if vce == 'cluster' else None)
                    np.testing.assert_allclose(ref['beta'], transform@inverse@y, rtol=0, atol=3e-13)
                    np.testing.assert_allclose(ref['covariance'], covariance, rtol=2e-11, atol=3e-14)
                    np.testing.assert_allclose(arrays['residual'], residual, rtol=0, atol=3e-13)
                    self.assertEqual(ref['dof_rows']['11'], df)
                    self.assertEqual(ref['dof_rows']['9'], min(df, len(groups)-1) if vce == 'cluster' else df)
                    self.assertGreaterEqual(ref['covariance_psd']['minimum_eigenvalue'], -ref['covariance_psd']['tolerance'])
                    if vce == 'cluster':
                        self.assertEqual(ref['cluster_count'], len(groups))

    def test_new_vce_refuses_unsupported_weights_codes_and_degenerate_clusters(self):
        y, x, f, w = fixture(weighted=False)
        for vce, weights, clusters in [('iid', w*2, None), ('cluster', w*2, f[0]),
                                      ('cluster', w, None), ('robust', w, f[0]),
                                      ('bad', w, None), ('cluster', w, np.zeros(len(y))),
                                      ('cluster', w, np.full(len(y), np.nan))]:
            with self.subTest(vce=vce), self.assertRaises(ValueError):
                oracle.solve(y, x, f, weights, vce=vce, clusters=clusters)
        old, a = oracle.solve(y, x, f, w)
        explicit, b = oracle.solve(y, x, f, w, vce='robust')
        self.assertEqual(old, explicit)
        for name in a:
            np.testing.assert_array_equal(a[name], b[name])

    def test_cluster_reference_checks_covariance_metadata_and_reported_dof(self):
        y, x, factors, w = fixture(weighted=False)
        ref, arrays = oracle.solve(y,x,factors,w,vce='cluster',clusters=factors[0])
        def table(value):
            value = np.atleast_2d(value)
            return dict(rows=value.shape[0],columns=value.shape[1],
                        bits=[struct.pack('>d',float(v)).hex() for v in value.ravel()])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); descriptor = root/'oracle.json'; npz = root/'oracle.npz'
            np.savez_compressed(npz,**arrays)
            outputs = dict(zip(oracle.OUTPUTS,range(6)))
            ref.update(case_sha256='a'*64,body_sha256='b'*64,columns=6,outputs=outputs,
                       arrays=dict(file=npz.name,sha256=digest(npz)))
            descriptor.write_text(json.dumps(ref))
            loaded, _ = oracle.load_reference(descriptor,'a'*64,'b'*64)
            dof = np.zeros((20,2))
            for key,value in ref['dof_rows'].items():
                dof[int(key)] = [1,value]
            status = np.zeros((4,4)); status[:,1] = [1,1,1,5]
            raw = dict(matrices=dict(RB=table(ref['beta']),RV=table(ref['covariance']),
                RDOF=table(dof),RFE=table(ref['fe_rows']),RSTATUS=table(status),
                RCLUSTER=table([[1,1,5,0]]),RSUBSET=table([[1,1,1,5]])))
            columns = root/'columns'
            np.asarray([arrays['sample'],arrays['residual'],arrays['drop_reason'],arrays['xb'],arrays['fitted'],arrays['absorbed']],dtype=float).tofile(columns)
            meta = dict(rows=len(y),columns=6,outputs=outputs)
            oracle.check_reference(loaded,raw,columns,meta)
            for name,index in [('RCLUSTER',2),('RSUBSET',3),('RDOF',19),('RV',0)]:
                changed = copy.deepcopy(raw)
                changed['matrices'][name]['bits'][index] = struct.pack('>d',123.).hex()
                with self.subTest(table=name), self.assertRaises(ValueError):
                    oracle.check_reference(loaded,changed,columns,meta)
            broken = copy.deepcopy(ref)
            broken['covariance_psd']['tolerance'] *= 2
            descriptor.write_text(json.dumps(broken))
            with self.assertRaisesRegex(ValueError,'PSD'):
                oracle.load_reference(descriptor,'a'*64,'b'*64)

    def test_direct_schur_matches_full_weighted_dummy_svd_and_normalized_constant(self):
        for disconnected in [False,True]:
            for intercept in [False,True]:
                y,x,f,w = fixture(disconnected)
                target, arrays = oracle.solve(y,x,f,w,include_intercept=intercept)
                beta,cov,fitted,residual,rank = dense(y,x,f,w,arrays['sample'],intercept)
                np.testing.assert_allclose(target['beta'],beta,rtol=0,atol=2e-13)
                np.testing.assert_allclose(target['covariance'],cov,rtol=2e-12,atol=2e-14)
                np.testing.assert_allclose(arrays['fitted'],fitted,rtol=0,atol=2e-13)
                np.testing.assert_allclose(arrays['residual'],residual,rtol=0,atol=2e-13)
                self.assertEqual(target['true_fe_rank'],rank)
                self.assertEqual(target['pairwise_fe_rank'],rank)
                self.assertEqual(target['certificate']['known_nullity'],2*(1+int(disconnected)))

    def test_singleton_fixed_point_and_keepsingletons_preserve_original_rows(self):
        y,x,f,w = fixture()
        # First appended row loses its private level. Its partner then becomes a
        # singleton in another partition, so the independent second round matters.
        f = [np.r_[f[0],5,0],np.r_[f[1],4,4],np.r_[f[2],0,0]]
        y = np.r_[y,1.,2.];x=np.vstack([x,[[1.,2.,3.],[3.,2.,1.]]]);w=np.r_[w,1.,2.]
        target, arrays = oracle.solve(y,x,f,w)
        np.testing.assert_array_equal(arrays['sample'],np.r_[np.ones(len(y)-2,dtype=bool),False,False])
        np.testing.assert_array_equal(arrays['drop_reason'][-2:],[8,8])
        kept, all_rows = oracle.solve(y,x,f,w,keep_singletons=True)
        self.assertTrue(all_rows['sample'].all())
        self.assertEqual(kept['retained_rows'],len(y))
        for ref,out in [(target,arrays),(kept,all_rows)]:
            beta,cov,_,_,rank = dense(y,x,f,w,out['sample'],True)
            np.testing.assert_allclose(ref['beta'],beta,atol=3e-13,rtol=0)
            np.testing.assert_allclose(ref['covariance'],cov,atol=3e-14,rtol=1e-11)
            self.assertEqual(ref['true_fe_rank'],rank)

    def test_extra_multiway_alias_fails_instead_of_selecting_a_new_rank(self):
        y,x,f,w = fixture()
        with self.assertRaisesRegex(ValueError,'nonpositive|ambiguous'):
            oracle.solve(y,x,[f[0],f[1],f[1]],w)

    def test_weighted_nested_fe_and_absorbed_finite_combinations_fail_closed(self):
        y,x,f,w = fixture()
        # A constant third partition connects both nested retained levels.
        # The one ungrounded Schur entry is exactly zero. These uneven weights
        # leave a positive floating remnant whose relative scalar spectrum
        # alone would pass; the absolute formation-error margin must reject it.
        nested=np.repeat(np.arange(12),5)
        nested_weights=np.exp(np.random.default_rng(1).uniform(-2,2,len(y)))
        with self.assertRaisesRegex(ValueError,'grounded FE Schur: ambiguous'):
            oracle.solve(y,x,[nested,nested//6,np.zeros(len(y))],nested_weights,keep_singletons=True)
        for combination in [False,True]:
            changed=x.copy()
            absorbed=np.sin(f[0])+.3*f[1]-.2*f[2]
            changed[:,1]=absorbed + (changed[:,0] if combination else 0.)
            with self.assertRaisesRegex(ValueError,'nonpositive|ambiguous'):
                oracle.solve(y,changed,f,w)

    def test_common_weight_scale_and_original_row_permutation(self):
        y,x,f,w = fixture()
        base, a = oracle.solve(y,x,f,w)
        order = np.random.default_rng(9).permutation(len(y))
        changed,b = oracle.solve(y[order],x[order],[v[order] for v in f],w[order]*1e80)
        np.testing.assert_allclose(base['beta'],changed['beta'],atol=2e-13,rtol=0)
        np.testing.assert_allclose(base['covariance'],changed['covariance'],atol=2e-14,rtol=2e-12)
        np.testing.assert_allclose(a['fitted'][order],b['fitted'],atol=2e-13,rtol=0)

    def test_extreme_finite_scale_and_nonfinite_moments_fail_closed(self):
        y,x,f,w = fixture()
        # These inputs remain finite and their individual squared norms fit,
        # but the finite-design certificate's product of norms overflows.
        with np.errstate(over='ignore',invalid='ignore'):
            with self.assertRaisesRegex(ValueError,'nonfinite.*finite-design moment normalization'):
                oracle.solve(y*1e100,x*1e100,f,w)
            for value in [np.nan,np.inf]:
                residual=np.ones((len(y),1));residual[0,0]=value
                with self.assertRaisesRegex(ValueError,'nonfinite'):
                    oracle._moments(f,residual,w,y[:,None])
            with self.assertRaisesRegex(ValueError,'nonfinite original-design norm'):
                oracle._moments(f,y[:,None],w,y[:,None]*1e200)

    def test_resource_and_conditioning_refusals_do_not_return_partial_target(self):
        y,x,f,w = fixture()
        from unittest.mock import patch
        with patch.object(oracle,'MAX_RETAINED',2):
            with self.assertRaisesRegex(ValueError,'dimension'):
                oracle.solve(y,x,f,w)
        x[:,1]=x[:,0]
        with self.assertRaisesRegex(ValueError,'ambiguous'):
            oracle.solve(y,x,f,w)

    def test_canonical_parser_checks_actual_bindings_and_does_not_change_input(self):
        y,x,f,w = fixture()
        n,k,q = len(y),x.shape[1],len(f)
        inputs = k+q+3
        fields = dict(term_count=str(k),absorb_term_count=str(q),vce='robust',dof='pairwise',
            account_cluster_nesting='0',cluster_term_count='0',cluster_component_count='0',
            model_major='2',model_minor='1',keep_singletons='0',include_intercept='1',threads='1',
            output_saved_channels='|'.join(['1:0']*q))
        fields.update({'output_'+key:str(inputs+j+1) for j,key in enumerate(oracle.OUTPUTS)})
        tables = {
            'source_matrix':[[j,0] for j in range(1,k+q+1)],
            'term_matrix':[[j,j,j,1,1,0,0,0] for j in range(1,k+1)],
            'component_matrix':[[j,1,j,1,1,0,0,2,1,j,j,0,0] for j in range(1,k+1)],
            'absorb_term_matrix':[[j,1,1,0] for j in range(1,q+1)],
            'absorb_component_matrix':[[j,1,1,k+j] for j in range(1,q+1)],
            'binding_matrix':[[1,1,0],[2,2,0],*[[j+2,3,j] for j in range(1,k+q+1)],[inputs,6,0]],
        }
        fields.update({key:key for key in tables})
        def write(path):
            arguments=['estimate-ols',*[key+'='+value for key,value in fields.items()]]
            with path.open('wb') as stream:
                stream.write(f'FEREG_REPLAY_V1\n{n} {inputs+6} {len(arguments)} {len(tables)} 0\n'.encode())
                for arg in arguments:stream.write(f'{len(arg)}\n{arg}\n'.encode())
                def values(column):
                    stream.write(''.join(struct.pack('>d',float(v)).hex()+'\n' for v in np.asarray(column).ravel()).encode())
                for column in [np.ones(n),y,*x.T,*f,w,*[np.zeros(n)]*6]:
                    stream.write(b'0\n');values(column)
                for key,table in tables.items():
                    array=np.asarray(table);stream.write(f'{len(key)}\n{key}\n{array.shape[0]} {array.shape[1]}\n'.encode());values(array)
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'a-test.case';write(path);before=digest(path)
            parsed=oracle.read_case(path)
            np.testing.assert_array_equal(parsed['x'],x)
            np.testing.assert_array_equal(parsed['weights'],w)
            self.assertEqual(parsed['case_sha256'],before)
            self.assertEqual(digest(path),before)
            tables['term_matrix'][0][5]=1
            write(path)
            with self.assertRaisesRegex(ValueError,'binding'):
                oracle.read_case(path)

    def test_bound_reference_checks_all_numeric_and_discrete_outputs(self):
        y,x,f,w = fixture()
        ref, arrays = oracle.solve(y,x,f,w)
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);p=root/'oracle.json';npz=root/'oracle.npz'
            np.savez_compressed(npz,**arrays)
            outputs=dict(zip(oracle.OUTPUTS,range(6)))
            ref.update(case_sha256='a'*64,body_sha256='b'*64,columns=6,outputs=outputs,
                       arrays={'file':npz.name,'sha256':digest(npz)},producer_sha256=digest(Path(oracle.__file__)),parser_sha256=digest(oracle.PARSER_SOURCE))
            p.write_text(json.dumps(ref))
            loaded,bound=oracle.load_reference(p,'a'*64,'b'*64)
            self.assertEqual(set(bound), {p.resolve(), npz.resolve()})
            historical = {**ref, 'producer_sha256': 'c' * 64, 'parser_sha256': 'd' * 64}
            p.write_text(json.dumps(historical))
            old, _ = oracle.load_reference(p, 'a' * 64, 'b' * 64)
            self.assertEqual(old['producer_sha256'], historical['producer_sha256'])
            for case_digest, body_digest in [('a' * 64, 'c' * 64), (None, 'b' * 64), ('a' * 64, None)]:
                with self.subTest(case=case_digest, body=body_digest), self.assertRaisesRegex(ValueError, 'identity'):
                    oracle.load_reference(p, case_digest, body_digest)
            for key in ['case_sha256', 'body_sha256']:
                invalid = copy.deepcopy(ref)
                invalid.pop(key)
                p.write_text(json.dumps(invalid))
                with self.subTest(missing=key), self.assertRaisesRegex(ValueError, 'identity'):
                    oracle.load_reference(p, 'a' * 64, 'b' * 64)
            p.write_text(json.dumps(ref))
            def table(values):
                v=np.atleast_2d(values)
                return dict(rows=v.shape[0],columns=v.shape[1],bits=[struct.pack('>d',float(x)).hex() for x in v.ravel()])
            dof=np.zeros((20,2))
            for k,v in ref['dof_rows'].items():dof[int(k)]=[1,v]
            status=np.zeros((4,4));status[:,1]=[1,1,1,5]
            raw={'matrices':{'RB':table(ref['beta']),'RV':table(ref['covariance']),
                 'RFE':table(ref['fe_rows']),'RDOF':table(dof),'RSTATUS':table(status)}}
            data=np.array([arrays['sample'],arrays['residual'],arrays['drop_reason'],arrays['xb'],arrays['fitted'],arrays['absorbed']],dtype=float)
            binary=root/'columns';data.tofile(binary)
            meta=dict(rows=len(y),columns=6,outputs=outputs)
            oracle.check_reference(loaded,raw,binary,meta)
            bad=copy.deepcopy(raw);bad['matrices']['RB']['bits'][0]=struct.pack('>d',123.).hex()
            with self.assertRaisesRegex(ValueError,'coefficients'):
                oracle.check_reference(loaded,bad,binary,meta)
            bad=copy.deepcopy(raw);bad['matrices']['RFE']['bits'][3]=struct.pack('>d',0.).hex()
            with self.assertRaisesRegex(ValueError,'ranks'):
                oracle.check_reference(loaded,bad,binary,meta)
            data[0,0]=0;data.tofile(binary)
            with self.assertRaisesRegex(ValueError,'sample'):
                oracle.check_reference(loaded,raw,binary,meta)
            with self.assertRaisesRegex(ValueError,'identity'):
                oracle.load_reference(p,'c'*64,'b'*64)
            for key,value in [('projection_moment',1e-5),('known_nullity',0)]:
                invalid=copy.deepcopy(ref);invalid['certificate'][key]=value
                p.write_text(json.dumps(invalid))
                with self.assertRaisesRegex(ValueError,'certificate'):
                    oracle.load_reference(p,'a'*64,'b'*64)
            invalid=copy.deepcopy(ref)
            invalid['certificate']['schur_spectrum']['minimum_eigenvalue']=0.
            p.write_text(json.dumps(invalid))
            with self.assertRaisesRegex(ValueError,'spectrum'):
                oracle.load_reference(p,'a'*64,'b'*64)
            p.write_text(json.dumps(ref))
            npz.rename(root / 'missing.npz')
            with self.assertRaises(FileNotFoundError):
                oracle.load_reference(p, 'a' * 64, 'b' * 64)
            (root / 'missing.npz').rename(npz)
            npz.write_bytes(npz.read_bytes()+b'changed')
            with self.assertRaisesRegex(ValueError,'arrays changed'):
                oracle.load_reference(p,'a'*64,'b'*64)


if __name__ == '__main__':
    unittest.main()
