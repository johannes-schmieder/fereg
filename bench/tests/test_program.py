"""Mapped gaps in the second-program executable benchmark contract."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare import timing_envelope, paired_envelopes, acceptance_envelopes, summarize
from generate import generate, sha256
from run import load_suite, selected_cells, thread_counts


class ProgramTests(unittest.TestCase):
    def setUp(self):
        self.suite = load_suite(Path(__file__).resolve().parents[1] / 'program-v1.json')

    def test_declared_core_is_the_complete_supported_interaction_cube(self):
        cells = selected_cells(self.suite, 'development-mac')
        core = [c for c in cells if c.get('split') == 'development']
        for model in ['ols', 'ppml']:
            for effects in range(4):
                actual = {(c['output_mode'], c['vce']) for c in core if c['model'] == model and c['effects'] == effects}
                outputs = ['none', 'residual'] if effects == 0 else ['none', 'residual', 'fe', 'combined']
                vces = ['iid', 'robust', 'cluster', 'multiway'] if model == 'ols' else ['robust', 'cluster', 'multiway']
                self.assertEqual(actual, {(o,v) for o in outputs for v in vces})
        self.assertEqual(len(core), 154)
        self.assertTrue(all(thread_counts(self.suite, 'development-mac', c) == [1,2,4,7,8,9,15,16] for c in core))
        self.assertTrue(all(Path(__file__).resolve().parents[2].joinpath(p).is_file() for p in self.suite['fixture_witnesses']))

    def test_holdout_and_development_shapes_and_combinations_are_separate(self):
        dev = selected_cells(self.suite, 'development-mac')
        hold = selected_cells(self.suite, 'holdout-mac')
        self.assertEqual(len(hold), 98)
        self.assertFalse({c['id'] for c in dev} & {c['id'] for c in hold})
        self.assertEqual({c['seed'] for c in hold}, {819217})
        self.assertTrue(all(c['rows'] == 137219 for c in hold))

    def test_generator_supports_no_fe_and_preserves_inputs_across_output_vce_cube(self):
        import pandas as pd
        cell = {**selected_cells(self.suite, 'baseline-pilot')[0], 'rows': 400}
        with tempfile.TemporaryDirectory() as temp:
            a, b = Path(temp)/'a.dta', Path(temp)/'b.dta'
            generate(cell, 739391, a)
            generate({**cell, 'vce':'multiway', 'output_mode':'residual'}, 739391, b)
            self.assertEqual(sha256(a), sha256(b))
            frame = pd.read_stata(a)
            self.assertNotIn('g1', frame)
            self.assertTrue({'cluster','cluster2','biasunit'} <= set(frame))
            generate({**cell, 'effects':2, 'topology':'disconnected'}, 739391, b)
            frame = pd.read_stata(b)
            # No level of either effect crosses the constructed components.
            for group in ['g1','g2']:
                self.assertTrue(frame.assign(component=(frame.obs_id-1)%3).groupby(group).component.nunique().eq(1).all())

    def test_uncertainty_and_missing_measurements_cannot_pass_an_envelope(self):
        p = self.suite['protocol']
        n = p['repetitions']
        self.assertEqual(timing_envelope([.1]*n, [.1]*n, p)['verdict'], 'within')
        self.assertEqual(timing_envelope([.1]*n, [.12]*n, p)['verdict'], 'outside')
        self.assertEqual(timing_envelope([.06,.08,.12,.14]*(n//4), [.1]*n, p)['verdict'], 'unresolved')
        self.assertEqual(timing_envelope([.1]*(n-1), [.1]*n, p)['verdict'], 'unavailable')

    def test_paired_checker_rejects_missing_reversed_and_mismatched_blocks(self):
        import copy,json
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            (root/'coefficients.csv').write_text('name,value\nx1,1\n')
            (root/'sample.csv').write_text('obs_id\n1\n')
            native={'plugin_sha256':'p','frontend_sha256':'f'}
            template={'paired_artifacts':{role:{'native_identity':native,'source_sha':'s'} for role in ['parent','candidate']},
                'suite_sha256':'m','tooling_identity':{'t':'h'},'datasets':{'c':'d'},'platform':'darwin','tier':'pilot','environment':{'ref':'h'},'timing_identity':{'method':'monotonic-command-v1','plugin_sha256':'clock'},
                'protocol':{**self.suite['protocol'],'repetitions':5},'expected_groups':[['c',1,'fereg',role] for role in ['parent','candidate']]}
            paths=[]
            for reverse in [False,True]:
                c=copy.deepcopy(template);c.update(reverse_order=reverse,records=[],complete=True,identities_verified=True)
                for role in ['parent','candidate']:
                    c['records'].append(dict(cell='c',artifact_role=role,reference=True,status='ok',directory=str(root)))
                for repetition in ['warmup',1,2,3,4,5]:
                    roles=['parent','candidate']
                    if reverse ^ (isinstance(repetition,int) and repetition%2==0):roles.reverse()
                    for role in roles:
                        c['records'].append(dict(cell='c',threads=1,arm='fereg',artifact_role=role,reference=False,
                            repetition=repetition,status='ok',directory=str(root),metadata={'seconds':'.1','clock_rc':'0','clock_resolution_seconds':'1e-9'},timing_identity=c['timing_identity'],**native))
                path=root/f'{reverse}.json';path.write_text(json.dumps(c));paths.append(path)
            self.assertTrue(acceptance_envelopes(paths,paths,'p','f',True)['passed'])
            for pair in [paths[:1],paths[::-1],[paths[0],paths[0]]]:
                with self.assertRaises(ValueError):paired_envelopes(pair)
            original=paths[1].read_text()
            for change in ['dataset','missing','order','artifact','unterminated','empty']:
                c=json.loads(original)
                if change=='dataset':c['datasets']['c']='other'
                if change=='missing':c['records'].pop()
                if change=='order':c['records'][2:4]=c['records'][2:4][::-1]
                if change=='artifact':c['records'][-1]['plugin_sha256']='other'
                if change=='unterminated':c.pop('complete')
                if change=='empty':c['expected_groups']=[]
                paths[1].write_text(json.dumps(c))
                with self.assertRaises(ValueError):paired_envelopes(paths)
            paths[1].write_text(original)
            with self.assertRaises(ValueError):acceptance_envelopes(paths,paths,'wrong','f')
            with self.assertRaises(ValueError):acceptance_envelopes(paths,paths,'p','wrong')

    def test_full_mac_cube_summary_fits_raw_bound_and_cannot_be_committed_as_large_receipt(self):
        import contextlib,io,json
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            (root/'coefficients.csv').write_text('name,value\nx1,1\n')
            (root/'sample.csv').write_text('obs_id\n1\n')
            campaign=dict(source_sha='s'*40,source_dirty=False,platform='darwin',tier='holdout-mac',partial=False,
                comparison='engine',engine='next',solver='auto',program='v1',protocol={**self.suite['protocol'],'repetitions':5},records=[],expected_groups=[])
            cells=selected_cells(self.suite,'holdout-mac')
            for cell in cells:
                campaign['records'].append(dict(cell=cell['id'],reference=True,status='ok',directory=str(root)))
                for threads in thread_counts(self.suite,'holdout-mac',cell):
                    campaign['expected_groups'].append([cell['id'],threads,'fereg'])
                    for repetition in ['warmup',1,2,3,4,5]:
                        campaign['records'].append(dict(cell=cell['id'],threads=threads,arm='fereg',reference=False,repetition=repetition,status='ok',directory=str(root),metadata={'seconds':'.1'}))
            path=root/'results.json';path.write_text(json.dumps(campaign))
            with contextlib.redirect_stdout(io.StringIO()):rows=summarize(path)
            self.assertEqual(len(rows),98*8)
            self.assertGreater((root/'summary.csv').stat().st_size,100000)
            with self.assertRaisesRegex(ValueError,'compact'):
                summarize(path,Path(__file__).resolve().parents[1]/'history/uncreated-large-test.csv')


if __name__ == '__main__':
    unittest.main()
