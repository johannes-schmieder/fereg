import copy
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from common import HERE,cell_id,read_json,write_json
from light import groups
from continue_campaign import clean_accounting,oracle_certificates,production


class ContinuationTests(unittest.TestCase):
    def test_accounting_requires_exact_clean_tasks(self):
        r=dict(job_id='7',status='pass',missing_tasks=[],records=[dict(task_id=1,failed=0,exit_status=0,slots=28,jobnumber='7')])
        clean_accounting(r,7,[1])
        for changed in [dict(failed=1),dict(exit_status=1),dict(slots=4),dict(jobnumber='8')]:
            bad=copy.deepcopy(r);bad['records'][0].update(changed)
            with self.assertRaises(ValueError):clean_accounting(bad,7,[1])
        bad=copy.deepcopy(r);bad['records']*=2
        with self.assertRaises(ValueError):clean_accounting(bad,7,[1])
        with self.assertRaises(ValueError):clean_accounting(r,7,[1,2])

    def test_fresh_hessian_rank_and_scores_are_mandatory(self):
        factor=dict(rank=8,largest_discarded=1e-15,lower_rank_cutoff=1e-12,cutoff=1e-10,upper_rank_cutoff=1e-8,smallest_kept=.1)
        oracle=dict(status='pass',N=20,maximum_scaled_score=1e-13,certificates=dict(maximum_scaled_projection=1e-12,inverse_error=1e-12),
                    model_rank=8,fe_rank=6,factor_trace=[factor,copy.deepcopy(factor)],iteration_trace=[{}])
        cell=dict(model='ppml',n=20,k=2);oracle_certificates(oracle,cell)
        for field,value in [('maximum_scaled_score',1e-7),('maximum_scaled_score',float('nan')),('fe_rank',7),('N',19),('factor_trace',[factor])]:
            bad=copy.deepcopy(oracle);bad[field]=value
            with self.assertRaises(ValueError):oracle_certificates(bad,cell)
        bad=copy.deepcopy(oracle);bad['factor_trace'][0]['largest_discarded']=1e-9
        with self.assertRaises(ValueError):oracle_certificates(bad,cell)
        bad=copy.deepcopy(oracle);bad['certificates']['inverse_error']=1e-5
        with self.assertRaises(ValueError):oracle_certificates(bad,cell)

    def test_full_grid_retains_failed_pairs_and_only_excludes_completed_groups(self):
        manifest=read_json(HERE/'manifest-light-v2.json');registered=groups(manifest)
        pilot=[g for g in registered if g['repetition']==1 and (g['cell'].get('slopes') or
               (g['cell']['section']=='main' and g['cell']['k']==10 and g['cell']['threads']==28 and
                (g['cell']['design'],g['cell']['n']) in [('two_fe_dense',983040),('strong_d6',3932160)]))]
        self.assertEqual(len(pilot),12)
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);(root/'input').mkdir()
            write_json(root/'input/manifest.json',manifest)
            (root/'input/historical-runs.csv').write_bytes((HERE.parent/'reports/systematic-stopped-2026-09-11/runs.csv').read_bytes())
            done=[]
            for g in pilot:
                c=g['cell'];key=cell_id(c);done.append([key,1])
                p=root/'results'/key/'warm-1';write_json(p/'GROUP.COMPLETE.json',dict(cell=c,repetition=1,elapsed_seconds=100))
                write_json(p/'fixest/result.json',dict(cell=c,status='pass',measurement_fingerprint='test',seconds=1,arm='fixest'))
            slope=next(g['cell'] for g in pilot if g['cell']['model']=='ppml' and g['cell']['design']=='three_fe_sparse' and g['cell']['k']==10)
            gate=dict(completed_groups=done,measurement_fingerprint='test',forecast_only=[dict(cell=slope,arm='fixest_strict',seconds=322.253)])
            gate_path=root/'env/PILOT.ADJUDICATED.json';write_json(gate_path,gate)
            plan=production(root,gate_path,gate)
            pending=[g for b in plan['bundles'] for g in b['groups']]
            keys={(cell_id(g['cell']),g['repetition']) for g in pending}
            self.assertEqual(len(pending),556);self.assertEqual(len(keys),556)
            self.assertFalse(keys & {tuple(x) for x in done})
            self.assertEqual(keys|{tuple(x) for x in done},{(cell_id(g['cell']),g['repetition']) for g in registered})
            strict=next(g for g in pending if g['cell']==slope)
            self.assertEqual(strict['limits']['fixest_strict']['command'],1934)
            self.assertEqual(plan['wall_seconds'],43200)
            extra=next(g for g in registered if (cell_id(g['cell']),g['repetition']) in keys)
            c=extra['cell'];write_json(root/'results'/cell_id(c)/'extra/GROUP.COMPLETE.json',dict(cell=c,repetition=extra['repetition'],elapsed_seconds=1))
            with self.assertRaisesRegex(ValueError,'already exists'):production(root,gate_path,gate)


if __name__=='__main__':unittest.main()
