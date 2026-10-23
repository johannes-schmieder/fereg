"""Synthetic framing/provenance witnesses; no native calls or panel sampling."""
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownership_moments_cases as cases
import routing_oracle as oracle
from native_io import digest


def source_case(path, weighted=False, candidate=1):
    rng = np.random.default_rng(43)
    a, b, repeat = np.meshgrid(np.arange(5), np.arange(4), np.arange(6), indexing='ij')
    factors = [a.ravel(), b.ravel()]
    n, k, q = len(factors[0]), 2, 2
    x = rng.normal(size=(n, k))
    y = x@np.array([.8, -.3])+factors[0]*.2-factors[1]*.7+rng.normal(size=n)*.1
    inputs = 2+k+q+int(weighted)
    fields = dict(term_count=str(k), absorb_term_count=str(q), vce='robust', dof='pairwise',
                  account_cluster_nesting='0', cluster_term_count='0', cluster_component_count='0',
                  model_major='2', model_minor='1', keep_singletons='0', include_intercept='1', threads='1',
                  candidate_all='0', input_complete='0', memory='fast', absorb_labels='1:a|1:b', cluster_labels='.',
                  output_saved_channels='1:0|1:0')
    fields.update({'output_'+key: str(inputs+j+1) for j, key in enumerate(oracle.OUTPUTS)})
    tables = {
        'source_matrix': [[j, 0] for j in range(1, k+q+1)],
        'term_matrix': [[j,j,j,1,1,0,0,0] for j in range(1,k+1)],
        'component_matrix': [[j,1,j,1,1,0,0,2,1,j,j,0,0] for j in range(1,k+1)],
        'absorb_term_matrix': [[j,1,1,0] for j in range(1,q+1)],
        'absorb_component_matrix': [[j,1,1,k+j] for j in range(1,q+1)],
        'binding_matrix': [[1,1,0],[2,2,0], *[[j+2,3,j] for j in range(1,k+q+1)],
                           *([[inputs,6,0]] if weighted else [])],
        'cluster_term_matrix': [[0,0]], 'cluster_component_matrix': [[0,0,0]],
    }
    fields.update({key:key for key in tables})
    columns = [np.full(n, candidate), y, *x.T, *factors,
               *([np.exp(rng.uniform(-1,1,n))] if weighted else []), *[np.zeros(n)]*6]
    args = ['estimate-ols', *[key+'='+value for key,value in fields.items()]]
    with path.open('wb') as stream:
        stream.write(f'FEREG_REPLAY_V1\n{n} {len(columns)} {len(args)} {len(tables)} 0\n'.encode())
        for arg in args:
            stream.write(f'{len(arg)}\n{arg}\n'.encode())
        for column in columns:
            stream.write(b'0\n'+''.join(struct.pack('>d',float(v)).hex()+'\n' for v in column).encode())
        for key,values in tables.items():
            rows, cols, payload = cases._table(values)
            stream.write(f'{len(key)}\n{key}\n{rows} {cols}\n'.encode()+payload)


class MomentCases(unittest.TestCase):
    def test_amendment_preserves_numeric_bytes_and_only_declared_model_edits(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root/'source.case'
            source_case(source)
            before = digest(source)
            frame = cases._frame(source)
            for vce in ['robust', 'iid', 'cluster']:
                target = root/(vce+'.case')
                meta = cases.amend_case(source, target, vce, 'fast')
                after = cases._frame(target)
                self.assertEqual(frame[2], after[2])
                self.assertEqual(digest(source), before)
                changed = {name for name in frame[3] if frame[3][name] != after[3][name]}
                self.assertEqual(changed, {'cluster_term_matrix','cluster_component_matrix'} if vce == 'cluster' else set())
                self.assertEqual(meta['candidate_all_verified'], True)
                self.assertFalse(meta['input_complete'])
                parsed = oracle.read_case(target)
                self.assertEqual(parsed['vce'], vce)
                if vce == 'cluster':
                    np.testing.assert_array_equal(parsed['clusters'], parsed['factors'][0])
                else:
                    self.assertEqual(meta['body_sha256'], meta['original_body_sha256'])
                with self.assertRaises(FileExistsError):
                    cases.amend_case(source, target, vce, 'fast')

    def test_false_candidate_hint_weighted_cluster_and_wrong_source_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, weighted, candidate, vce in [('selected',False,0,'robust'),
                                                   ('weighted',True,1,'cluster'),
                                                   ('weighted-iid',True,1,'iid')]:
                source = root/(name+'.case')
                source_case(source, weighted, candidate)
                with self.assertRaises(ValueError):
                    cases.amend_case(source, root/(name+'-amended.case'), vce, 'fast')
            source = root/'valid.case'
            source_case(source)
            for vce, memory in [('multiway','fast'), ('robust','auto')]:
                with self.assertRaises(ValueError):
                    cases.amend_case(source, root/'bad.case', vce, memory)

    def test_cluster_model_binding_is_not_inferred_from_label(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root/'source.case'; source_case(source)
            target = root/'cluster.case'; cases.amend_case(source, target, 'cluster', 'fast')
            frame = cases._frame(target)
            # Rewrite only the cluster source from first to second FE. The narrow
            # reader must refuse even though both are legitimate cluster models.
            old = cases._table([[1,1,3]])[2]
            new = cases._table([[1,1,4]])[2]
            self.assertEqual(frame[3]['cluster_component_matrix'][2], old)
            content = target.read_bytes()
            marker = b'cluster_component_matrix\n1 3\n'+old
            self.assertEqual(content.count(marker), 1)
            target.write_bytes(content.replace(marker, b'cluster_component_matrix\n1 3\n'+new))
            with self.assertRaisesRegex(ValueError, 'binding'):
                oracle.read_case(target)

    def test_prepare_exact_coverage_independent_references_and_origin_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); original = root/'original'; original.mkdir()
            entries, files = [], {}
            for name in ['weak','wide','weight-dynamic']:
                path = original/(name+'.case'); source_case(path, weighted=name == 'weight-dynamic')
                entries.append(dict(id=name,input=path.name));files[path.name]=digest(path)
            panel = dict(kind='ownership-cells-development-v1-inputs',seed=cases.SEED,cases=entries,files=files)
            (original/'panel.json').write_text(json.dumps(panel))
            with patch.object(cases.routing_cases, 'scratch', side_effect=lambda p:p):
                result = cases.prepare_inputs(original, root/'out')
                self.assertEqual(len(result['cases']), 7)
                self.assertEqual(len(result['settings']), 12)
                self.assertEqual(sum(s['eligible'] for s in result['settings']), 8)
                self.assertEqual([s['cell'] for s in result['settings']][:3], ['weak-robust-t1','weak-robust-t2','weak-robust-t4'])
                self.assertEqual(result['native_calls'], 0)
                for item in result['cases']:
                    meta = json.loads((root/'out'/item['metadata']).read_text())
                    ref, bindings = oracle.load_reference(root/'out'/item['oracle']['path'], meta['case_sha256'],meta['body_sha256'])
                    self.assertEqual(ref['status'], 'passed')
                    for path, expected in bindings.items():
                        self.assertEqual(result['files'][path.name], expected)
                # The original manifest binding must reject changed bytes.
                (original/'weak.case').write_bytes((original/'weak.case').read_bytes()+b'\n')
                with self.assertRaisesRegex(ValueError, 'binding'):
                    cases.prepare_inputs(original, root/'bad')


if __name__ == '__main__':
    unittest.main()
