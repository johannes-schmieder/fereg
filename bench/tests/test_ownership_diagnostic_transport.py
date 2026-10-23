"""Zero-native transport recovery: bit preservation and a real 1+79 runner path."""
import copy
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
import ownership_diagnostic_transport as t
import ownership_qualification_diagnostics as d
import qualify as q
import qualify_ownership as own
import routing
import test_ownership_qualification_diagnostics as original


class Transport(unittest.TestCase):
    def test_words_roundtrip_signed_zero_nan_payload_and_shape_failures(self):
        raw={'columns':[['0000000000000000','8000000000000000','7ff8000000000123',
                         'ffffffffffffffff','0000000000000001']]}
        arrays=t.inline_words(raw,dict(columns=1,rows=5))
        self.assertEqual(arrays[0].tobytes(),b''.join(int(v,16).to_bytes(8,'little') for v in raw['columns'][0]))
        for value in ('0','xyz','3ff00000000000000',1,1.0,None):
            bad=copy.deepcopy(raw);bad['columns'][0][0]=value
            with self.assertRaises(ValueError):t.inline_words(bad,dict(columns=1,rows=5))
        for meta in (dict(columns=2,rows=5),dict(columns=1,rows=4)):
            with self.assertRaises(ValueError):t.inline_words(raw,meta)

    def test_manifest_equality_allows_only_relocated_paths_and_harness(self):
        a=dict(harness='old',files={'old':'1'*64,'input':'2'*64},cases=[dict(input='input')],seed=7)
        b=dict(harness='new',files={'new':'3'*64,'moved':'2'*64},cases=[dict(input='moved')],seed=7)
        self.assertEqual(t.normalized_manifest(a),t.normalized_manifest(b))
        for edit in ('seed','input','extra'):
            bad=copy.deepcopy(b)
            if edit=='seed':bad['seed']=8
            elif edit=='input':bad['files']['moved']='4'*64
            else:bad['files']['extra']='5'*64
            self.assertNotEqual(t.normalized_manifest(a),t.normalized_manifest(bad))

    def test_timer_is_upper_bound_and_host_is_exact(self):
        self.assertEqual(t.timing(dict(started_monotonic_ns=1,finished_monotonic_ns=500000001)),.5)
        for start,end in ((0,1),(2,1),(1,120000000002),(1.,2)):
            with self.assertRaises(ValueError):t.timing(dict(started_monotonic_ns=start,finished_monotonic_ns=end))
        old=dict(host=dict(host=dict(hostname='scc-uf3')))
        t.same_host(dict(host=dict(hostname='scc-uf3.scc.bu.edu')),old)
        with self.assertRaises(ValueError):t.same_host(dict(host=dict(hostname='scc-uf4')),old)


class BoundAmendment(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)
        self.schedule=[dict(cell='ownership-nonmovers-t1',family='ownership-nonmovers',workers=1,arm='incumbent',repeat=0)]
        old_sources={'ownership_qualification_diagnostics':t.OLD_DIAGNOSTIC_SHA,'qualify':'f'*64}
        new_sources={**old_sources,'ownership_qualification_diagnostics':'d'*64,'ownership_diagnostic_transport':'e'*64}
        old_manifest=dict(harness='old-harness',files={'old-harness':'1'*64,'input':'3'*64},
                          cases=[dict(input='input')],execution_sources={'qualify':'f'*64})
        self.manifest={**old_manifest,'harness':'new-harness','files':{'new-harness':'2'*64,'input':'3'*64}}
        q.write(self.root/'qualification.json',old_manifest)
        self.prior=dict(schema=1,kind=d.KIND,platform='Darwin',schedule=self.schedule,
                        qualification_sha256=q.digest(self.root/'qualification.json'),qualification_identity=routing.identity(old_manifest),
                        candidate_sha256='a'*64,incumbent_sha256='b'*64,request='cells_deferred_both',
                        native_calls=80,timeout_s=120,budget_s=2400,retries=0,execution_sources=old_sources)
        q.write(self.root/'plan.json',self.prior)
        q.write(self.root/'raw.json',{'columns':[['0000000000000000']]})
        (self.root/'raw.log').write_text('trace only\n');(self.root/'request.case').write_bytes(b'bound request')
        (self.root/'host').mkdir();(self.root/'host/host-observations.jsonl').write_text('bound observation')
        call=dict(self.schedule[0],status='failed',result_sha256=q.digest(self.root/'raw.json'),
                  log_sha256=q.digest(self.root/'raw.log'),request_sha256=q.digest(self.root/'request.case'),
                  started_monotonic_ns=1,finished_monotonic_ns=500000001)
        self.old=dict(status='failed',native_calls_started=1,calls=[call],cells={},retries=0,exclusions=[],elapsed_s=1.2,
                      failures=[dict(type='ValueError',message='native harness must write binary columns')],
                      plan_sha256=q.digest(self.root/'plan.json'),qualification_sha256=self.prior['qualification_sha256'],
                      schedule=self.schedule,platform='Darwin',candidate_sha256='a'*64,incumbent_sha256='b'*64)
        q.write(self.root/'results.json',self.old)
        compiled={n:q.digest(q.ROOT/n) for n in ('plugin/stplugin.h','tests/mock_spi/actual_plugin_loader_test.c',
                                                'tests/mock_spi/replay.h','tests/mock_spi/replay_io.h')}
        self.old_build=dict(sha256='1'*64,native_calls=0,max_rows=1000000,max_cells=10000000,
                            source_sha256={k:v for k,v in compiled.items() if k!='tests/mock_spi/replay.h'})
        self.new_build=dict(self.old_build,sha256='2'*64,source_sha256=compiled,
                            command=['cc','-std=c11','-O2','-Wall','-Wextra','-Wpedantic','-Wconversion','-Wshadow',
                                     '-Wstrict-prototypes','-Wmissing-prototypes','-Werror','-D_POSIX_C_SOURCE=200809L',
                                     '-DREPLAY_TIMING','-DSYSTEM=APPLEMAC','-DREPLAY_MAX_ROWS=1000000',
                                     '-DREPLAY_MAX_CELLS=10000000ULL','-I<repo>/plugin',
                                     '<repo>/tests/mock_spi/actual_plugin_loader_test.c','-lc','-o','<output>/actual_plugin_loader_test'],
                            platform='Darwin',compiler='bound compiler',preprocessor_macros=dict(REPLAY_MAX_ROWS='1000000',
                            REPLAY_MAX_CELLS='10000000ULL',REPLAY_TIMING='1',_POSIX_C_SOURCE='200809L'))
        q.write(self.root/'old-build.json',self.old_build);q.write(self.root/'new-build.json',self.new_build)
        paths=dict(previous_plan='plan.json',previous_results='results.json',previous_qualification='qualification.json',
                   old_build='old-build.json',new_build='new-build.json',raw_head='raw.json',raw_log='raw.log',
                   request='request.case',observations='host/host-observations.jsonl')
        self.plan=dict(self.prior,execution_sources=new_sources,transport_repair=dict(kind=t.KIND,
                      files={k:dict(path=v,sha256=q.digest(self.root/v)) for k,v in paths.items()},
                      attempt_seconds=1.3,charged_seconds=2,reused_calls=1,new_calls=79,total_calls=80))
        self.host=patch.object(d,'verify_host',return_value={});self.host.start();self.addCleanup(self.host.stop)

    def validate(self):
        return t.validate(self.plan,self.manifest,self.root/'new-plan.json')

    def replace(self,key,value):
        bound=self.plan['transport_repair']['files'][key];q.write(self.root/bound['path'],value)
        bound['sha256']=q.digest(self.root/bound['path'])

    def test_bound_cohort_and_narrow_manifest_delta(self):
        self.validate()
        saved=copy.deepcopy(self.plan)
        for key,value in (('new_calls',80),('reused_calls',0),('charged_seconds',1),('attempt_seconds',.5)):
            self.plan=copy.deepcopy(saved);self.plan['transport_repair'][key]=value
            with self.assertRaises(ValueError):self.validate()
        self.plan=saved
        for key in ('platform','request','candidate_sha256','schedule'):
            altered=copy.deepcopy(self.prior)
            altered[key]='wrong' if key!='schedule' else []
            self.replace('previous_plan',altered)
            with self.assertRaises(ValueError):self.validate()
        self.replace('previous_plan',self.prior)
        self.manifest['cases'][0]['model']='different'
        with self.assertRaises(ValueError):self.validate()

    def test_raw_hash_source_flag_and_success_relabelling_rejected(self):
        self.validate()
        for key,change in (('status','complete'),('native_calls_started',2),('exclusions',['slow'])):
            bad=copy.deepcopy(self.old);bad[key]=change;self.replace('previous_results',bad)
            with self.assertRaises(ValueError):self.validate()
        self.replace('previous_results',self.old)
        for change in ('flag','cap','source','extra_flag','macro'):
            bad=copy.deepcopy(self.new_build)
            if change=='flag':bad['command'].remove('-DREPLAY_TIMING')
            elif change=='cap':bad['max_rows']=1000001
            elif change=='extra_flag':bad['command'].insert(1,'-ffast-math')
            elif change=='macro':bad['preprocessor_macros']['REPLAY_TIMING']='0'
            else:bad['source_sha256']['tests/mock_spi/replay.h']='0'*64
            self.replace('new_build',bad)
            with self.assertRaises(ValueError):self.validate()
        self.replace('new_build',self.new_build)
        (self.root/'raw.log').write_text('NATIVE_SECONDS=0.1\n')
        with self.assertRaises(ValueError):self.validate()
        self.plan['transport_repair']['files']['raw_log']['sha256']=q.digest(self.root/'raw.log')
        self.old['calls'][0]['log_sha256']=q.digest(self.root/'raw.log');self.replace('previous_results',self.old)
        with self.assertRaises(ValueError):self.validate()

    def test_linux_failed_terminal_accounting_is_bound_and_not_success(self):
        self.plan['platform']='Linux';self.prior['platform']='Linux';self.old['platform']='Linux'
        self.old['host']=dict(host=dict(hostname='scc-uf3',job_id='123',task_id='0',pe='mpi_28_tasks_per_node_u'))
        self.replace('previous_plan',self.prior)
        self.old['plan_sha256']=q.digest(self.root/'plan.json');self.replace('previous_results',self.old)
        build=copy.deepcopy(self.new_build);build['platform']='Linux'
        build['command']=[{'-DSYSTEM=APPLEMAC':'-DSYSTEM=OPUNIX','-lc':'-ldl'}.get(v,v) for v in build['command']]
        self.replace('new_build',build)
        text='jobnumber 123\ntaskid undefined\nhostname scc-uf3.scc.bu.edu\nproject welfgr\nfailed 0\nexit_status 1\nend_time completed\nslots 28\ngranted_pe mpi_28_tasks_per_node_u\nru_wallclock 34\n'
        (self.root/'accounting.txt').write_text(text);(self.root/'job.log').write_text('parser failed\n')
        for name,file in (('accounting','accounting.txt'),('job_log','job.log')):
            self.plan['transport_repair']['files'][name]=dict(path=file,sha256=q.digest(self.root/file))
        self.validate()
        for bad in (text.replace('exit_status 1','exit_status 0'),text.replace('welfgr','other'),
                    text.replace('scc-uf3','scc-uf4'),text.replace('taskid undefined','taskid 1'),
                    text.replace('slots 28','slots 4'),text.replace('end_time completed\n',''),
                    text+'jobnumber 123\n'):
            (self.root/'accounting.txt').write_text(bad)
            self.plan['transport_repair']['files']['accounting']['sha256']=q.digest(self.root/'accounting.txt')
            with self.assertRaises((ValueError,RuntimeError)):self.validate()


class Continuation(unittest.TestCase):
    setUp=original.Runner.setUp
    execute=original.Runner.execute

    def prefix(self):
        directory=self.f.root/'failed';folder=directory/self.plan['schedule'][0]['cell'];folder.mkdir(parents=True)
        source=q.resolve(self.f.manifest,self.manifest['cases'][0]['input'])
        request=folder/'incumbent.case';own.prepare(source,request,'auto',1)
        target=folder/'call-00.json';log=target.with_suffix('.log')
        self.execute(['fake',str(request),str(target)],log,120)
        raw=q.read(target)
        raw['columns']=[[format(int(v),'016x') for v in col] for col in self.f.data.view(np.uint64)]
        q.write(target,raw);target.with_suffix('.json.columns').unlink()
        log.write_text(log.read_text().replace('NATIVE_SECONDS=0.010000\n',''))
        call=dict(self.plan['schedule'][0],status='failed',target=str(target.relative_to(directory)),
              log=str(log.relative_to(directory)),result_sha256=q.digest(target),log_sha256=q.digest(log),
              request_sha256=q.digest(request),started_monotonic_ns=10,finished_monotonic_ns=500000010)
        old=dict(status='failed',calls=[call],host=dict(host=dict(hostname='synthetic')))
        summary=directory/'results.json';q.write(summary,old)
        paths=dict(raw_head=target,raw_log=log,request=request,previous_results=summary)
        self.plan['transport_repair']=dict(charged_seconds=2)
        self.prior=patch.object(t,'validate',return_value=(paths,old,{}));self.prior.start();self.addCleanup(self.prior.stop)
        return paths,old

    def test_exact_one_reused_seventy_nine_new_and_mixed_proof(self):
        paths,old=self.prefix();original_hash=q.digest(paths['previous_results'])
        result=d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=original.Monitor)
        self.assertEqual(len(self.f.calls)-1,79)
        self.assertEqual((result['native_calls_started'],result['native_calls_reused'],result['native_calls_executed_here']),(80,1,79))
        self.assertEqual([{k:c[k] for k in ('cell','family','workers','arm','repeat')} for c in result['calls']],self.plan['schedule'])
        first=result['calls'][0]
        self.assertEqual(first['timing_field'],t.TIMER);self.assertIsNone(first['maxrss_bytes'])
        self.assertEqual(first['columns_sha256'],hashlib.sha256(self.f.data.tobytes()).hexdigest())
        self.assertEqual(first['fingerprint'],result['calls'][1]['fingerprint'])
        self.assertTrue(all(c['timing_field']=='NATIVE_SECONDS' for c in result['calls'][1:]))
        self.assertEqual(result['elapsed_s'],2+result['continuation_elapsed_s'])
        proof=d.prove(self.plan_path,self.output/'results.json',self.output/'proof.json')
        self.assertEqual(proof['status'],'passed');self.assertEqual(proof['native_calls_executed_here'],79)
        self.assertEqual(q.digest(paths['previous_results']),original_hash)
        for field,value in (('seconds',.6),('maxrss_bytes',0),('timing_field','NATIVE_SECONDS')):
            changed=copy.deepcopy(result);changed['calls'][0][field]=value;q.write(self.output/'results.json',changed)
            with self.assertRaises(ValueError):d.prove(self.plan_path,self.output/'results.json',self.output/'bad-proof.json')
        self.assertEqual(len(self.f.calls),80)

    def test_invalid_import_stops_before_any_new_native_and_preserves_failure(self):
        paths,_=self.prefix();raw=q.read(paths['raw_head']);raw['columns'][0][0]='bad';q.write(paths['raw_head'],raw)
        with self.assertRaises(ValueError):d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=original.Monitor)
        self.assertEqual(len(self.f.calls),1)
        result=q.read(self.output/'results.json');self.assertEqual(result['status'],'failed')
        self.assertEqual(q.read(paths['previous_results'])['status'],'failed')

    def test_nonauto_prefix_fails_before_any_new_call(self):
        import struct
        paths,_=self.prefix();raw=q.read(paths['raw_head'])
        raw['matrices']['RSOLVER']['bits'][0]=struct.pack('>d',99.).hex()
        q.write(paths['raw_head'],raw)
        with self.assertRaises(ValueError):d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=original.Monitor)
        self.assertEqual(len(self.f.calls),1)

    def test_wrong_host_and_oracle_failure_start_zero_new_calls(self):
        paths,old=self.prefix();old['host']['host']['hostname']='different'
        with self.assertRaises(ValueError):d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=original.Monitor)
        self.assertEqual(len(self.f.calls),1)
        with patch.object(q.science,'check_oracle',side_effect=ValueError('independent witness')):
            with self.assertRaises(ValueError):d.run(self.plan_path,self.output.with_name('oracle-failed'),execute=self.execute,monitor_factory=original.Monitor)
        self.assertEqual(len(self.f.calls),1)


if __name__=='__main__':unittest.main()
