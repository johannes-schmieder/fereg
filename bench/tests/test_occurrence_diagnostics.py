import copy
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import occurrence_diagnostics as d

class DiagnosticContract(unittest.TestCase):
    def test_fixed_bounded_panel(self):
        self.assertEqual(d.CONTRACT['repetitions'],{'1':12,'4':24})
        self.assertEqual(d.CONTRACT['budget_s'],600)
        self.assertEqual(d.CONTRACT['extensions'],'none')
    def test_association_requires_slow_coverage_and_preserves_noisy_calls(self):
        row={'threads':4,'seconds':.02,'probe':{'wait_ns':2_000_000,'involuntary':0}}
        self.assertFalse(d.classify([row]*2)['association'])
        self.assertTrue(d.classify([row]*3)['association'])
        unlinked=copy.deepcopy(row);unlinked['probe']['wait_ns']=0
        result=d.classify([row]*4+[unlinked]);self.assertTrue(result['association']);self.assertEqual(result['slow_calls'],5)
        self.assertFalse(d.classify([row]*3+[unlinked]*2)['association'])
        unlinked['probe']['involuntary']=1
        self.assertTrue(d.classify([unlinked]*3)['association'])
    def test_fast_or_one_worker_does_not_change_affinity(self):
        rows=[{'threads':w,'seconds':seconds,'probe':{'wait_ns':10_000_000,'involuntary':5}}
              for w,seconds in [(1,.1),(4,.01)]]*5
        self.assertEqual(d.classify(rows)['affinity_cores'],4)
    def probe(self):
        rows=[]
        for scope,id_ in [('native',0),('absorb',1)]:
            for edge,v in [('start',0),('end',1)]:
                prefix=f'FEREG_SCHED|scope={scope}|id={id_}|edge={edge}|record='
                rows += [prefix+f'boundary|ns={v+1}|wall={v+1}|caller=1/task/1',
                         prefix+f'task|tid=1|run_ns={v*100}|wait_ns={v*50}|slices={v}|cpu=3|voluntary={v}|involuntary=0',
                         prefix+'overhead|read_ns=1']
        return '\n'.join(rows)
    def test_boundaries_and_absorption_deltas(self):
        got=d.parse_probe(self.probe());self.assertEqual(got['run_ns'],100);self.assertEqual(got['wait_ns'],50)
        with self.assertRaisesRegex(ValueError,'missing probe boundary'):
            d.parse_probe(self.probe().replace('edge=end|record=boundary','edge=absent|record=boundary'))
    def test_thread_set_or_counter_changes_fail(self):
        with self.assertRaisesRegex(ValueError,'task set changed'):
            d.parse_probe(self.probe().replace('scope=absorb|id=1|edge=end|record=task|tid=1','scope=absorb|id=1|edge=end|record=task|tid=2'))
        with self.assertRaisesRegex(ValueError,'went backwards'):
            d.parse_probe(self.probe().replace('run_ns=100','run_ns=-1'))

class DiagnosticExecution(unittest.TestCase):
    def test_complete_shutdown_validates_only_after_final_configuration(self):
        import tempfile,os
        from contextlib import ExitStack
        from unittest.mock import Mock,patch
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root=Path(directory); artifact=root/'artifact';artifact.write_text('a')
            cells=[{'id':f'easy-t{n}','family':'easy','threads':n,'metadata':{'path':'meta'},
                    'inputs':{a:{'path':'input'} for a in d.CONTRACT['arms']}} for n in [1,4]]
            parent={'kind':d.c.MEMBERSHIP_KIND,'plugin':d.c.ref(artifact),'harness':d.c.ref(artifact),'cells':cells}
            parent_path=root/'parent.json';d.c.save(parent_path,d.c.routing.seal(parent))
            stages=[];observer=Mock();observer.errors=[]
            observer.validate.side_effect=lambda *args:self.assertEqual(stages[-1],'after')
            def native(p,cell,arm,out,label,**kwargs):
                log=out/(label+'.log');log.write_text('probe')
                return {'cell':cell['id'],'arm':arm,'label':label,'log':str(log),'status':'executed'}
            for owner,name,kwargs in [
              (d.c,'checked',{'return_value':artifact}), (d.c,'tool_files',{'return_value':[]}),
              (d.c,'native',{'side_effect':native}),
              (d.c,'parse_run',{'return_value':({}, {'science_sha256':'same','seconds':.01},None)}),
              (d,'parse_probe',{'return_value':{'wait_ns':0,'involuntary':0}}),
              (d.c.scc.discovery,'host',{'return_value':{'affinity':list(range(28))}}),
              (d.c.scc,'configuration',{'side_effect':lambda out,host,stage:(stages.append(stage) or {},{})}),
              (d.c.scc,'check_configuration',{'return_value':'q'}),
              (d.c.scc,'check_scheduler_snapshot',{}),(d.c,'check_stable_configuration',{}),
              (d.c,'Observers',{'return_value':observer})]:stack.enter_context(patch.object(owner,name,**kwargs))
            original=d.c.read
            stack.enter_context(patch.object(d.c,'read',side_effect=lambda p:{} if p=='meta' else original(p)))
            stack.enter_context(patch.dict(os.environ,{'JOB_ID':'1','NSLOTS':'28'}))
            d.run(parent_path,artifact,root/'output',provenance=artifact)
            receipt=d.c.routing.unseal(original(root/'output/diagnostic.json'))
            self.assertEqual(receipt['status'],'passed',receipt['failure'])
            self.assertEqual(len(receipt['records']),72);self.assertEqual(len(receipt['controls']),4)
            observer.validate.assert_called_once();self.assertEqual(stages,['before','after'])

if __name__=='__main__':unittest.main()
