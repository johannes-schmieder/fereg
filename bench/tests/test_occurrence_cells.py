"""Occurrence discovery boundaries and exact parent-control coverage."""
import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import compact_cells as c
import test_compact_cells as old

class OccurrenceGate(unittest.TestCase):
    def evidence(self,gains=16,build=.0015,original=.0015/.35):
        p,s,m=old.MembershipDiscoveryGate.evidence();p['kind']=c.OCCURRENCE_KIND
        m.update(kind=c.OCCURRENCE_KIND+'_measurement',plan_sha256=c.routing.identity(p))
        for row in m['records']:
            # Map the old fixture's arm bank bijectively into the new four arms.
            rename={'cells_m':'cells_s','cells_m_lanes':'cells_m'}
            arm=row['arm'];new=rename.get(arm,arm);row['arm']=new
            row['label']=row['label'].replace(arm,new);row['science_sha256']=row['cell']+new
            index=next(i for i,x in enumerate(p['cells']) if x['id']==row['cell'])
            row['seconds']=.8 if new=='cells_s' and index<gains else 1.
            if row['phase']=='diagnostic':
                row['phase_diagnostics']['solver_trace']=[{'phase':'auto_kernel_route','preset':'routing_'+new,
                   'builder_seconds':str(build if new=='cells_s' else original)}]
        return p,s,m
    def gate(self,**kwargs):
        with patch.object(c,'validate'),patch.object(c,'validate_science'):
            return c.occurrence_gate(*self.evidence(**kwargs))
    def test_fixed_finalist_gain_count_and_inclusive_builder_boundaries(self):
        self.assertEqual(self.gate()['status'],'eligible')
        self.assertEqual(self.gate(gains=15)['status'],'ineligible')
        self.assertEqual(self.gate(build=.001500001)['status'],'ineligible')
        self.assertEqual(self.gate(original=.0015/.35-1e-8)['status'],'ineligible')
        self.assertEqual(self.gate()['finalist'],'cells_s')
    def test_affinity_is_enforced_before_science_and_both_old_m_arms_are_exact_controls(self):
        import os
        plan={'kind':c.OCCURRENCE_KIND,'native_affinity_cores':5}
        with patch.dict(os.environ,{'NSLOTS':'4'}):
            with self.assertRaisesRegex(ValueError,'slot count'):c.science_affinity(plan)
        with patch.dict(os.environ,{'NSLOTS':'5'}), patch.object(os,'sched_getaffinity',return_value={8,9,10,11,12},create=True):
            self.assertEqual(c.science_affinity(plan),[8,9,10,11,12])
        self.assertEqual(c.control_arms(plan),[*c.ARMS,'cells_m','cells_m_lanes'])
        with patch.object(c,'occurrence_parent',return_value={'plugin':'M'}):
            self.assertEqual(c.control_parent(plan,{'plugin':'original'},'cells_m')['plugin'],'M')
            self.assertEqual(c.control_parent(plan,{'plugin':'original'},'cells')['plugin'],'original')

if __name__=='__main__':unittest.main()
