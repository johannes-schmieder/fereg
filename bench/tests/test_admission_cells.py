"""S2 gates preserve failures, exact S prerequisites and the fixed finalist."""
import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import compact_cells as c
import test_occurrence_cells as previous

class AdmissionGate(unittest.TestCase):
    def evidence(self, accepted=.001, refused=.0006, gains=16):
        p,s,m=previous.OccurrenceGate().evidence(gains=gains)
        p['kind']=c.ADMISSION_KIND
        m.update(kind=c.ADMISSION_KIND+'_measurement',plan_sha256=c.routing.identity(p))
        for row in m['records']:
            arm=row['arm'];new={'cells_m':'cells_s','cells_s':'cells_s2'}.get(arm,arm)
            row.update(arm=new,label=row['label'].replace(arm,new))
            row['science_sha256']=row['cell']+('cells_s' if new=='cells_s2' else new)
            index=next(i for i,x in enumerate(p['cells']) if x['id']==row['cell'])
            if new in ['cells_s','cells_s2']:row['seconds']=.8 if index<gains else 1.
            if row['phase']=='diagnostic':
                refuse=row['cell'].startswith('hubs-') and new=='cells_s2'
                seconds=refused if refuse else accepted if new=='cells_s2' else .001 if new=='cells_s' else .004
                traces=[{'phase':'auto_kernel_route','preset':'routing_'+new,'builder_seconds':str(seconds),
                         'installed':'0' if refuse else '1','refusal':'Some(Unprofitable { cells: 800, limit: 770 })' if refuse else 'None'}]
                if refuse:
                    traces.extend([{'phase':'auto_cells_refusal','cells':'800','limit':'770'},
                                   {'phase':'auto_cells_s2','used_operator':'0'}])
                row['phase_diagnostics']['solver_trace']=traces
        return p,s,m
    def gate(self,evidence=None,**kwargs):
        with patch.object(c,'validate'),patch.object(c,'validate_science'):
            return c.admission_gate(*(evidence or self.evidence(**kwargs)))
    def test_single_finalist_gain_requirement_and_both_builder_regimes(self):
        g=self.gate();self.assertEqual(g['status'],'eligible');self.assertEqual(g['finalist'],'cells_s2')
        self.assertEqual(len(g['comparisons_vs_s']),24)
        self.assertEqual(sum(r['outcome']=='refused' for r in g['builder_comparisons']),2)
        self.assertEqual(self.gate(gains=15)['status'],'ineligible')
        self.assertEqual(self.gate(accepted=.001100001)['status'],'ineligible')
        self.assertEqual(self.gate(refused=.000600001)['status'],'ineligible')
        self.assertEqual(self.gate(accepted=.0011)['status'],'eligible')
    def test_missing_rows_bad_proof_or_repeated_refused_build_cannot_pass(self):
        p,s,m=self.evidence();m['records'].pop()
        with self.assertRaises(ValueError):self.gate((p,s,m))
        p,s,m=self.evidence();row=next(r for r in m['records'] if r['phase']=='diagnostic' and r['arm']=='cells_s2' and r['cell']=='hubs-t1')
        row['phase_diagnostics']['solver_trace'][1]['cells']='770'
        with self.assertRaisesRegex(ValueError,'proof'):self.gate((p,s,m))
        row['phase_diagnostics']['solver_trace'][1]['cells']='800'
        row['phase_diagnostics']['solver_trace'].append(copy.deepcopy(row['phase_diagnostics']['solver_trace'][0]))
        with self.assertRaisesRegex(ValueError,'one build'):self.gate((p,s,m))
    def test_timing_host_cannot_have_consistent_s2_s_science_drift(self):
        p,s,m=self.evidence()
        for row in m['records']:
            if row['cell']=='weak-t1' and row['arm']=='cells_s2':
                row['science_sha256']='consistent-but-different-s2'
        with self.assertRaisesRegex(ValueError,'measurement host'):self.gate((p,s,m))
    def test_s_regression_is_not_hidden_by_auto_gain(self):
        p,s,m=self.evidence()
        for r in m['records']:
            if r['arm']=='cells_s' and r['cell']=='weak-t1':r['seconds']=.5
        g=self.gate((p,s,m));self.assertEqual(g['regressions_vs_s'],1);self.assertEqual(g['status'],'ineligible')
    def test_all_nine_prior_control_arms_and_four_cpu_science_are_frozen(self):
        import os
        plan={'kind':c.ADMISSION_KIND,'native_affinity_cores':4}
        self.assertEqual(len(c.control_arms(plan)),9)
        self.assertTrue({'cells_s','cells_deferred','cells_v2_df','cells_m_lanes'}<=set(c.control_arms(plan)))
        with patch.dict(os.environ,{'NSLOTS':'3'}):
            with self.assertRaisesRegex(ValueError,'slot count'):c.science_affinity(plan)
        with patch.dict(os.environ,{'NSLOTS':'4'}),patch.object(os,'sched_getaffinity',return_value={3,4,5,6},create=True):
            self.assertEqual(c.science_affinity(plan),[3,4,5,6])
    def test_science_rejects_s2_s_fingerprint_difference(self):
        p,_,_=self.evidence();rows=[];fingerprints={}
        for cell in p['cells']:
            for arm in c.ADMISSION_ARMS:
                key=cell['id']+'/'+arm;fingerprints[key]=key
                rows.extend(dict(cell=cell['id'],arm=arm,status='executed',science_sha256=key,resources={}) for _ in range(2))
        science=dict(kind=c.ADMISSION_KIND+'_science',status='passed',smoke=False,
                     plan_sha256=c.routing.identity(p),fingerprints=fingerprints,records=rows,files=[])
        with patch.object(c,'validate_membership_science'):
            with self.assertRaisesRegex(ValueError,'S2 changed S science'):c.validate_science(p,science)

if __name__=='__main__':unittest.main()
