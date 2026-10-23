"""Small preparation witnesses; no serializer process, estimator or final data."""
from contextlib import ExitStack
import copy
import json
from pathlib import Path
import signal
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownership_qualification_cases as cases
from native_io import case_header, digest


def template(path, k, *, duplicate=False, candidate=1, solver='auto'):
    """Small canonical-shaped serializer output; numeric draws are replaced."""
    n, inputs = 80, k+4
    fields = dict(solver=solver, preconditioner='auto', memory='fast', threads='1',
                  term_count=str(k), absorb_term_count='2', include_intercept='1',
                  candidate_all='0', input_complete='1', keep_singletons='0',
                  vce='robust', dof='pairwise', account_cluster_nesting='0',
                  cluster_term_count='0', cluster_component_count='0',
                  model_major='2', model_minor='1', binding_count=str(inputs),
                  output_saved_channels='1:0|1:0')
    fields.update({'output_'+name: str(inputs+j+1) for j, name in enumerate(cases.routing_oracle.OUTPUTS)})
    tables = {
        'S': [[j,0] for j in range(1,k+3)],
        'T': [[j,j,j,1,1,0,0,0] for j in range(1,k+1)],
        'C': [[j,1,j,1,1,0,0,2,1,j,j,0,0] for j in range(1,k+1)],
        'A': [[1,1,1,0],[2,1,1,0]], 'AC': [[1,1,1,k+1],[2,1,1,k+2]],
        'BIND': [[1,1,0],[2,2,0], *[[j+2,3,j] for j in range(1,k+3)]],
        'CT': [[0,0]], 'CC': [[0,0,0]],
        'RB': [[0]*(k+1)], 'RV': [[0]*(k+1) for _ in range(k+1)],
        'RSTATUS': [[0]*4 for _ in range(k+1)],
        'RMARKB': [[0]*(k+1)], 'RMARKV': [[0]*(k+1) for _ in range(k+1)],
    }
    fields.update(dict(zip(('source_matrix','term_matrix','component_matrix',
                           'absorb_term_matrix','absorb_component_matrix','binding_matrix',
                           'cluster_term_matrix','cluster_component_matrix'),
                          ('S','T','C','A','AC','BIND','CT','CC'))))
    argv = [b'estimate-ols', *[(k+'='+v).encode() for k,v in fields.items()]]
    if duplicate:
        argv.append(b'solver=auto')
    encode = lambda v: struct.pack('>d', float(v)).hex().encode()+b'\n'
    with path.open('xb') as stream:
        stream.write(f'FEREG_REPLAY_V1\n{n} {inputs+6} {len(argv)} {len(tables)} 0\n'.encode())
        for arg in argv:
            stream.write(str(len(arg)).encode()+b'\n'+arg+b'\n')
        for j in range(inputs+6):
            stream.write(b'0\n'+encode(candidate if j == 0 else 0)*n)
        for name, values in tables.items():
            stream.write(f'{len(name)}\n{name}\n{len(values)} {len(values[0])}\n'.encode())
            stream.write(b''.join(encode(v) for row in values for v in row))


def serializer(command, **kwargs):
    """Only mocked subprocess: actual write_case framing/copy remains exercised."""
    template(Path(command[command.index('--output')+1]), int(command[command.index('--regressors')+1]))


def small_columns(k, seed, paired=False):
    rng = np.random.default_rng(seed)
    a, b, repeat = np.meshgrid(np.arange(5), np.arange(4), np.arange(8), indexing='ij')
    a, b = a.ravel().astype(float), b.ravel().astype(float)
    n = len(a)
    if paired:
        def pair():
            z = rng.integers(-16, 17, n//2)/8
            return np.column_stack((z, -z)).ravel()
        x = [pair() for _ in range(k)]
        y = sum((j+1)*v/8 for j,v in enumerate(x))+pair()/64
    else:
        x = [rng.normal(size=n) for _ in range(k)]
        y = sum((j+1)*v/8 for j,v in enumerate(x))+.2*a-.3*b+rng.normal(size=n)*.1
    return [y, *x, a, b]


class QualificationInputs(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.generator = self.root/'serializer'
        self.generator.write_bytes(b'never executed; synthetic serializer identity')

    def small_panel(self, output, **patches):
        specs = {f: cases.specification(f) for f in cases.FAMILIES}
        base = small_columns(16, 91)
        values = {f: cases.boundary_columns(base, f) if f in cases.BOUNDARY else
                  small_columns(cases.WIDTHS[f], i+13, paired=f==cases.FAMILIES[2])
                  for i,f in enumerate(cases.FAMILIES)}
        for f,spec in specs.items():
            spec.update(rows=len(values[f][0]), workers=5, firms=4)
        with ExitStack() as stack:
            stack.enter_context(patch.object(cases.routing_cases, 'scratch', side_effect=lambda p:p.resolve()))
            stack.enter_context(patch.object(cases, 'source_hashes', return_value={'synthetic': 'a'*64}))
            stack.enter_context(patch.object(cases, 'specification', side_effect=lambda f:copy.deepcopy(specs[f])))
            stack.enter_context(patch.object(cases, 'generated_columns', return_value=iter(values.items())))
            stack.enter_context(patch.object(cases.generators.subprocess, 'run', side_effect=serializer))
            for name,value in patches.items():
                stack.enter_context(patch.object(cases, name, **value))
            return cases.prepare(output, generator=self.generator)

    def test_frozen_shapes_seeds_streams_and_allowances(self):
        expected = [(20000,1000,100,20,2,0), (64000,1000,100,64,2,1),
                    (20000,1000,100,20,4,2), (262143,4096,100,64,16,3),
                    (262144,4096,100,64,16,3), (262145,4096,100,64,16,3),
                    (600000,5000,100,30,2,6), (480000,6000,100,80,2,7)]
        for family,(n,w,f,t,k,offset) in zip(cases.FAMILIES, expected):
            spec = cases.specification(family)
            self.assertEqual(tuple(spec[key] for key in ('rows','workers','firms','periods','regressors')), (n,w,f,t,k))
            self.assertEqual(spec['derived_seed'], 2026106302+offset)
            self.assertEqual(spec['oracle_memory_budget_bytes'], (2 if family in cases.BOUNDARY else 1)<<30)
            self.assertEqual(spec['include_intercept'], family != 'ownership-inactive-zero')
        self.assertEqual([cases.ROWS[f]*16*8 for f in cases.BOUNDARY], [(32<<20)-128, 32<<20, (32<<20)+128])
        self.assertTrue(all(cases.ROWS[f]*18*8 > 32<<20 for f in cases.BOUNDARY))
        with self.assertRaisesRegex(ValueError, 'unknown'):
            cases.specification('weak')

    def test_dispatch_reuses_one_boundary_draw_and_existing_nonmover_transform(self):
        # Tiny x/y columns suffice for dispatch; only the 20,000-code nonmover
        # array is materialized to exercise its exact historical transformation.
        calls = []
        def fake(kind,w,f,t,seed):
            calls.append((kind,w,f,t,seed))
            n = 20000 if kind == 'long' and w == 1000 else 12
            return [np.arange(n, dtype=float)+j/8 for j in range(5)]
        with patch.object(cases.generators,'synthetic',side_effect=fake), \
             patch.object(cases.generators,'additional_case',side_effect=fake), \
             patch.object(cases.generators,'high_degree_case',side_effect=fake):
            generated = dict(cases.generated_columns())
        self.assertEqual(calls, [('long',1000,100,20,2026106302), ('d60-stress',1000,100,64,2026106303),
            ('orthogonal',1000,100,20,2026106304), ('wide',4096,100,64,2026106305),
            ('long',5000,100,30,2026106308), ('weak',6000,100,80,2026106309)])
        np.testing.assert_array_equal(generated[cases.FAMILIES[0]][-1][2000:], np.repeat(np.arange(100,1000)%100,20))
        np.testing.assert_array_equal(generated[cases.FAMILIES[6]][0], np.repeat(np.arange(12),4))
        for low,mid,high in zip(*(generated[f] for f in cases.BOUNDARY)):
            self.assertEqual(low.tobytes(),mid[:-1].tobytes())
            self.assertEqual(high.tobytes(),mid.tobytes()+mid[-1:].tobytes())

    def test_boundary_preserves_signed_zero_bits_and_rejects_unknown_variant(self):
        base = [np.array([1., -0., 0., -0.]), np.array([2., 4., 8., 16.])]
        snapshots = [a.tobytes() for a in base]
        for low,mid,high,raw in zip(cases.boundary_columns(base,cases.BOUNDARY[0]),
                                  cases.boundary_columns(base,cases.BOUNDARY[1]),
                                  cases.boundary_columns(base,cases.BOUNDARY[2]), snapshots):
            self.assertEqual(mid.tobytes(),raw)
            self.assertEqual(low.tobytes(),raw[:-8])
            self.assertEqual(high.tobytes(),raw+raw[-8:])
        with self.assertRaises(ValueError): cases.boundary_columns(base,'other')

    def test_real_framing_and_real_small_oracles_cover_all_eight_entries(self):
        output = self.root/'panel'
        result = self.small_panel(output)
        self.assertEqual(result['native_calls'],0)
        self.assertEqual(result['reference_timeout_s'],120)
        self.assertEqual([r['id'] for r in result['cases']],list(cases.FAMILIES))
        self.assertEqual(json.loads((output/'panel.json').read_text()),result)
        for row in result['cases']:
            path = output/row['input']
            meta = json.loads((output/row['metadata']).read_text())
            with path.open('rb') as stream:
                shape, argv = case_header(stream)
            fields = dict(a.decode().split('=',1) for a in argv[1:])
            self.assertEqual((fields['candidate_all'],fields['input_complete']),('1','0'))
            self.assertEqual((fields['solver'],fields['preconditioner'],fields['memory'],fields['vce']),('auto','auto','fast','robust'))
            self.assertEqual(meta['seed'],2026106302)
            self.assertEqual(meta['case_sha256'],digest(path))
            self.assertEqual(meta['independent_reference_summary']['retained_rows'],shape[0])
            self.assertTrue(meta['independent_reference_summary']['full_finite_rank'])
            if row['family'] == 'ownership-inactive-zero':
                self.assertEqual(row['oracle'],{'kind':'inactive'})
                self.assertEqual(fields['include_intercept'],'0')
            else:
                ref = json.loads((output/row['oracle']['path']).read_text())
                self.assertEqual(ref['retained_rows'],shape[0])
                self.assertEqual(ref['certificate'].get('memory_budget_bytes',1<<30),
                                 (2 if row['family'] in cases.BOUNDARY else 1)<<30)
        for name,sha in result['files'].items(): self.assertEqual(digest(output/name),sha)
        self.assertFalse(list(output.glob('*.serialized.case')))
        # Exact row relation also holds in serialized output columns and tables.
        streams = [(output/(f+'.case')).open('rb') for f in cases.BOUNDARY]
        try:
            headers = [case_header(s) for s in streams]
            self.assertEqual(headers[0][1],headers[1][1]);self.assertEqual(headers[1][1],headers[2][1])
            for _ in range(headers[0][0][1]):
                self.assertEqual([s.readline() for s in streams],[b'0\n']*3)
                lo,mid,hi = [s.read(h[0][0]*17) for s,h in zip(streams,headers)]
                self.assertEqual(lo,mid[:-17]);self.assertEqual(hi,mid+mid[-17:])
            self.assertEqual(streams[0].read(),streams[1].read())
        finally:
            for s in streams:s.close()

    def test_hint_edit_is_only_header_change_and_duplicate_or_solver_rejected(self):
        source = self.root/'source.case';template(source,2)
        target = self.root/'target.case'
        before = digest(source)
        changes = cases.frame_hints(source,target)
        self.assertEqual(changes,{'candidate_all':{'before':'0','after':'1'},'input_complete':{'before':'1','after':'0'}})
        self.assertEqual(digest(source),before)
        with source.open('rb') as a,target.open('rb') as b:
            self.assertEqual(case_header(a)[0],case_header(b)[0]);self.assertEqual(a.read(),b.read())
        with self.assertRaises(FileExistsError):cases.frame_hints(source,target)
        for name,kwargs in [('duplicate',{'duplicate':True}),('solver',{'solver':'map'})]:
            bad=self.root/(name+'.case');template(bad,2,**kwargs)
            with self.assertRaises(ValueError):cases.frame_hints(bad,self.root/(name+'-target.case'))

    def test_wrong_shapes_nonfinite_and_lost_levels_fail(self):
        values = small_columns(2,11)
        spec = dict(rows=160,regressors=2,workers=5,firms=4)
        cases.validate_columns(values,spec)
        for changed in [values[:-1], [v[:-1] for v in values], [*values[:-1],np.zeros(160)]]:
            with self.assertRaises(ValueError):cases.validate_columns(changed,spec)
        values[0][0]=np.nan
        with self.assertRaises(ValueError):cases.validate_columns(values,spec)

    def test_false_candidate_bitmap_rejected_by_actual_reader(self):
        source=self.root/'selected.case';template(source,2,candidate=0)
        target=self.root/'hint.case';cases.frame_hints(source,target)
        with self.assertRaisesRegex(ValueError,'candidate selection'):
            cases.routing_oracle.read_case(target)

    def test_failure_retains_partial_files_no_resume_or_substitution(self):
        output=self.root/'failed'
        with self.assertRaisesRegex(ValueError,'independent failure'):
            self.small_panel(output,reference={'side_effect':ValueError('independent failure')})
        self.assertFalse((output/'panel.json').exists())
        self.assertEqual(len(list(output.glob('*.serialized.case'))),1)
        with self.assertRaisesRegex(ValueError,'already exists'):self.small_panel(output)

    def test_source_drift_does_not_publish_panel(self):
        output=self.root/'drift'
        with self.assertRaisesRegex(ValueError,'drift'):
            self.small_panel(output,source_hashes={'side_effect':[{'a':'1'},{'a':'2'}]})
        self.assertFalse((output/'panel.json').exists())
        self.assertEqual(len(list(output.glob('*.oracle.json'))),7)

    def test_alarm_is_fixed_and_failure_restores_handler_without_sleep(self):
        handler = None
        def remember(_sig, callback):
            nonlocal handler
            handler = callback
        with patch.object(cases.signal,'getitimer',return_value=(0.,0.)), \
             patch.object(cases.signal,'getsignal',return_value=signal.SIG_DFL), \
             patch.object(cases.signal,'signal',side_effect=remember), \
             patch.object(cases.signal,'alarm') as alarm:
            with self.assertRaisesRegex(TimeoutError,'120-second'):
                with cases.reference_deadline():handler(None,None)
            self.assertEqual(alarm.call_args_list[0].args,(120,))
            self.assertEqual(alarm.call_args_list[-1].args,(0,))
            self.assertEqual(handler,signal.SIG_DFL)
        with patch.object(cases.signal,'getitimer',return_value=(1.,0.)):
            with self.assertRaisesRegex(ValueError,'active alarm'):
                with cases.reference_deadline():self.fail('must reject existing timer')


if __name__ == '__main__':
    unittest.main()
