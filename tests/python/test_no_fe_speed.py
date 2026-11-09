"""Reject incomplete or changed no-FE benchmark evidence before timing."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

import numpy as np

ROOT=Path(__file__).resolve().parents[2]
sys.path[:0]=[str(ROOT/'bench'),str(ROOT/'scripts')]
import no_fe_speed as target


class NoFeScienceTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)
        self.config={'data_dir':str(self.root),'arms':dict.fromkeys(['incumbent','candidate','regress']),
                     'widths':[1], 'threads':[1,4]}
        ref={'b':[.5,1.], 'V':[[1e-6,0.],[0.,1e-6]], 'rss':999998., 'N':1000000,
             'rank':2,'df_r':999998}
        (self.root/'reference.json').write_text(json.dumps({'1':ref}))
        values=[*ref['b'],*np.asarray(ref['V']).flatten(order='F'),ref['rss'],ref['N'],ref['rank'],ref['df_r']]
        self.paths=[]
        for arm in self.config['arms']:
            p=self.root/f'verify-{arm}.txt'
            p.write_text(''.join(f'{arm} 0 1 {t} {r} '+' '.join(map(str,values))+'\n'
                                 for t in (1,4) for r in (0,1)))
            self.paths.append(p)

    def test_complete_exact_science(self):
        self.assertEqual(target.check_science(self.config,self.paths)['observations'],12)

    def test_missing_repeat_fails(self):
        p=self.paths[0]
        p.write_text('\n'.join(p.read_text().splitlines()[:-1])+'\n')
        with self.assertRaisesRegex(ValueError,'incomplete scientific observations'):
            target.check_science(self.config,self.paths)

    def test_missing_timed_science_is_not_covered_by_preflight(self):
        p=self.root/'block-1-candidate.txt';p.write_text('')
        with self.assertRaisesRegex(ValueError,'incomplete scientific observations'):
            target.check_science(self.config,[*self.paths,p])

    def test_duplicate_repeat_fails(self):
        p=self.paths[0];s=p.read_text();p.write_text(s+s.splitlines()[0]+'\n')
        with self.assertRaisesRegex(ValueError,'duplicate scientific observation'):
            target.check_science(self.config,self.paths)

    def test_small_data_fails(self):
        p=self.root/'reference.json';d=json.loads(p.read_text());d['1']['N']=999;p.write_text(json.dumps(d))
        with self.assertRaisesRegex(ValueError,'one million'):
            target.check_science(self.config,self.paths)

    def test_trace_must_execute_structural_constant(self):
        p=self.root/'trace'
        lines=[]
        for t in (1,4):
            lines.append(f'FEREG_SOLVER_TRACE|phase=no_fe_benchmark|k=1|threads={t}')
            for _ in (0,1):
                lines += ['FEREG_PHASE|map_wrapper|rhs=3|rows=1000000|norms_count=1|initial_certificate_count=1|sweep_count=0',
                          'FEREG_SOLVER_TRACE|phase=input_digest|digest=1234',
                          'FEREG_SOLVER_TRACE|phase=no_fe_result|iterations=1|n_hdfe=0|solver=direct']
        p.write_text('\n'.join(lines)+'\n')
        self.assertEqual(len(target.check_trace(self.config,p)),2)
        p.write_text(p.read_text().replace('iterations=1','iterations=0'))
        with self.assertRaisesRegex(ValueError,'did not execute'):
            target.check_trace(self.config,p)

    def test_direct_scope_allows_only_covariance_and_rss(self):
        self.config.update(optimization='direct', numerical_scope=target.DIRECT_SCOPE)
        p=self.paths[1]
        rows=[line.split() for line in p.read_text().splitlines()]
        for row in rows:
            row[7]=str(np.nextafter(float(row[7]),np.inf))
        p.write_text(''.join(' '.join(row)+'\n' for row in rows))
        science=target.check_science(self.config,self.paths)
        self.assertFalse(science['exact_candidate_incumbent'])
        self.assertTrue(science['exact_unaffected_fields'])
        for row in rows:
            row[5]=str(np.nextafter(float(row[5]),np.inf))
        p.write_text(''.join(' '.join(row)+'\n' for row in rows))
        with self.assertRaisesRegex(ValueError,'unaffected science changed'):
            target.check_science(self.config,self.paths)

    def test_direct_trace_requires_projection_and_derived_residual(self):
        self.config.update(optimization='direct', numerical_scope=target.DIRECT_SCOPE)
        p=self.root/'verify-candidate.trace'
        lines=[]
        for t in (1,4):
            lines.append(f'FEREG_SOLVER_TRACE|phase=no_fe_benchmark|k=1|threads={t}')
            for _ in (0,1):
                lines += ['FEREG_PHASE|structural_direct|rhs=3|rows=1000000|raw_statistics_count=1|center_count=1|residual_norms_count=1|certificate_count=1',
                          'FEREG_SOLVER_TRACE|phase=input_digest|digest=1234',
                          'FEREG_SOLVER_TRACE|phase=no_fe_result|iterations=1|n_hdfe=0|solver=direct|residual_stop=5|residual_iterations=0']
        p.write_text('\n'.join(lines)+'\n')
        self.assertEqual(len(target.check_trace(self.config,p)),2)
        p.write_text(p.read_text().replace('residual_stop=5','residual_stop=2'))
        with self.assertRaisesRegex(ValueError,'did not execute'):
            target.check_trace(self.config,p)

    def test_centered_scope_allows_science_but_preserves_decisions(self):
        self.config.update(optimization='centered', numerical_scope=target.CENTERED_SCOPE)
        p=self.paths[1]
        rows=[line.split() for line in p.read_text().splitlines()]
        for row in rows:
            row[5]=str(np.nextafter(float(row[5]),np.inf))
        p.write_text(''.join(' '.join(row)+'\n' for row in rows))
        science=target.check_science(self.config,self.paths)
        self.assertFalse(science['exact_candidate_incumbent'])
        self.assertTrue(science['cross_worker_scientific_agreement'])
        for row in rows:
            row[-2]='1'
        p.write_text(''.join(' '.join(row)+'\n' for row in rows))
        with self.assertRaisesRegex(ValueError,'sample/rank/degrees'):
            target.check_science(self.config,self.paths)

    def test_centered_trace_requires_actual_products_and_factor(self):
        self.config.update(optimization='centered', numerical_scope=target.CENTERED_SCOPE)
        p=self.root/'verify-candidate.trace'
        lines=[]
        for t in (1,4):
            lines.append(f'FEREG_SOLVER_TRACE|phase=no_fe_benchmark|k=1|threads={t}')
            for _ in (0,1):
                lines += ['FEREG_PHASE|centered_projection|columns=3|rows=1000000',
                          'FEREG_PHASE|centered_moments|columns=1|rows=1000000',
                          'FEREG_SOLVER_TRACE|phase=finite_factor|method=cholesky',
                          'FEREG_SOLVER_TRACE|phase=input_digest|digest=1234',
                          'FEREG_SOLVER_TRACE|phase=no_fe_result|iterations=1|n_hdfe=0|solver=direct|residual_stop=5|residual_iterations=0']
        p.write_text('\n'.join(lines)+'\n')
        self.assertEqual(len(target.check_trace(self.config,p)),2)
        p.write_text(p.read_text().replace('centered_moments','unused_route'))
        with self.assertRaisesRegex(ValueError,'finite path did not execute'):
            target.check_trace(self.config,p)

    def test_declared_packed_handoff_requires_actual_reuse(self):
        self.config.update(optimization='centered', numerical_scope=target.CENTERED_SCOPE, require_packed_loading=True)
        p=self.root/'verify-candidate.trace'
        lines=[]
        for t in (1,4):
            reuse=int(t>1)
            lines.append(f'FEREG_SOLVER_TRACE|phase=no_fe_benchmark|k=1|threads={t}')
            for _ in (0,1):
                lines += ['FEREG_PHASE|centered_projection|columns=3|rows=1000000',
                          'FEREG_PHASE|centered_moments|columns=1|rows=1000000',
                          f'FEREG_PHASE|dense_raw|loaded_rows={reuse}|reused_rows={reuse}|reused_centered_summary={reuse}',
                          f'FEREG_SOLVER_TRACE|phase=input_layout|packed_rhs={str(t>1).lower()}',
                          'FEREG_SOLVER_TRACE|phase=finite_factor|method=cholesky',
                          'FEREG_SOLVER_TRACE|phase=input_digest|digest=1234',
                          'FEREG_SOLVER_TRACE|phase=no_fe_result|iterations=1|n_hdfe=0|solver=direct|residual_stop=5|residual_iterations=0']
        p.write_text('\n'.join(lines)+'\n')
        self.assertEqual(len(target.check_trace(self.config,p)),2)
        p.write_text(p.read_text().replace('reused_centered_summary=1','reused_centered_summary=0'))
        with self.assertRaisesRegex(ValueError,'handoff did not execute'):
            target.check_trace(self.config,p)
