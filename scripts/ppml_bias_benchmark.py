#!/usr/bin/env python3
"""Matched-probe native/current-Mata/cached-Mata correction benchmark.

Each invocation measures one process/route. Pair invocations on the same host
in alternating order. Startup is separate from one cold and five warm calls.
Mata uses the frozen production row operators with added numerical diagnostics;
only its information-solve strategy differs between reference routes.
"""
from __future__ import annotations
import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path
import resource
import sys
import threading
import time
import numpy as np
import pandas as pd
from ppml_bias_oracle import philox_signs
from run_stata_tests import run_fixture, sha256_file


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--reference',type=Path,required=True)
    p.add_argument('--out',type=Path,required=True)
    p.add_argument('--rows',type=int,default=2880)
    p.add_argument('--levels',type=int,default=8)
    p.add_argument('--units',type=int,default=240)
    p.add_argument('--probes',type=int,default=256)
    p.add_argument('--threads',type=int,default=1)
    p.add_argument('--warm',type=int,default=5)
    p.add_argument('--engine',choices=['native','mata_current','mata_cached'],required=True)
    p.add_argument('--memory',choices=['auto','low','fast'],default='auto')
    p.add_argument('--refit-reference',action='store_true',
                   help='Include a fresh ordinary PPML fit in every Mata call for end-to-end comparisons')
    p.add_argument('--stata',required=True)
    p.add_argument('--source-manifest',type=Path,required=True)
    p.add_argument('--plugin',type=Path,required=True)
    args=p.parse_args();root=Path(__file__).resolve().parents[1];out=args.out.resolve()
    out.mkdir(parents=True,exist_ok=False)
    n,g,u=args.rows,args.levels,args.units
    rng=np.random.default_rng(282601)
    left=np.arange(n)%g;right=rng.integers(0,g,n)
    x,z=rng.normal(size=(2,n));unit=np.arange(n)%u+1
    eta=.2+.3*np.sin(left)-.2*np.cos(right)+.2*x-.1*z
    exposure=rng.exponential(np.exp(-eta))
    data=pd.DataFrame(dict(y=np.ones(n),g=left+1,h=right+1,x=x,z=z,unit=unit,exposure=exposure))
    data.to_stata(out/'input.dta',write_index=False,version=118,time_stamp=datetime(2026,9,26))
    started=time.perf_counter()
    signs=philox_signs(np.arange(1,u+1,dtype=float),0,args.probes,739391,17)
    pd.DataFrame(signs,columns=[f'p{j+1}' for j in range(args.probes)]).to_stata(out/'signs.dta',write_index=False,version=118,time_stamp=datetime(2026,9,26))
    probe_preparation_seconds=time.perf_counter()-started
    production=args.reference.read_text()
    start=production.index('real colvector ev__xb(');end=production.index('real colvector ev__clip(',start)
    (out/'operators.do').write_text('mata:\n'+production[start:end]+'end\n')
    command=f'feppml y x z, absorb(g h) exposure(exposure) keepsingletons threads({args.threads}) memory({args.memory})'
    correction=command+f' biascorrection(randomized) biasunit(unit) biasprobes({args.probes}) biasseed(739391) biasstream(17) biasfe(bf)'
    def phase(name):
        return f'e(feppml_runtime)[rownumb(e(feppml_runtime),"time_{name}"),1]'
    fitting='+'.join(phase(name) for name in ('weight_response_refresh','fe_projection','finite_solve','objective_line_search','strict_final_solve'))
    lines=['version 18.0','clear all','set more off',f'set processors {args.threads}',
           f'do "{out}/operators.do"',f'do "{root}/tests/stata/bias/mata_correction.do"',
           *([f'use "{out}/signs.dta", clear','mata: PB_signs=st_data(.,.)'] if args.engine!='native' else []),
           f'use "{out}/input.dta", clear','timer clear 8','timer on 8',f'quietly {command.replace("absorb(g h)","absorb(base_g=g base_h=h)")} mu(fit_mu)',
           'timer off 8','quietly timer list 8','local ordinary_seconds=r(t8)',
           'quietly summarize base_h if h==1, meanonly','quietly replace base_g=base_g+r(mean)',
           'quietly summarize base_h if h==1, meanonly','quietly replace base_h=base_h-r(mean)',
           'matrix ordinary_b=e(b)','matrix ordinary_v=e(V)',
           'generate double raw_d=0','generate double raw_c=0',
           'tempname receipt',
           f'postfile `receipt\' str16 engine int repeat double total double correction double preparation double fitting double ordinary double inference double recovery double publication double workspace double score using "{out}/timing.dta", replace']
    if args.engine.startswith('mata'):
        lines += ['timer clear 9','timer on 9',
          'mata: PB_g=st_data(.,"g"); PB_h=st_data(.,"h"):-1; PB_C=st_data(.,("x","z")); PB_y=st_data(.,"y"); PB_mu=st_data(.,"fit_mu"); PB_w=J(st_nobs(),1,1); PB_unit=st_data(.,"unit"); PB_rawg=st_data(.,"base_g"); PB_rawh=st_data(.,"base_h")',
          'timer off 9','quietly timer list 9','local mata_preparation=r(t9)']
    for repeat in range(args.warm+1):
        lines += ['timer clear 1','timer on 1']
        if args.engine=='native':
            lines += [f'quietly {correction}',
                'timer off 1','quietly timer list 1','local elapsed=r(t1)',
                'assert mreldif(ordinary_b,e(b))==0','assert mreldif(ordinary_v,e(V))==0',
                f'post `receipt\' ("native") ({repeat}) (`elapsed\') (e(bias_diagnostics)[10,1]) ({phase("preparation")}) ({fitting}) (`ordinary_seconds\') ({phase("inference")}) ({phase("fe_recovery")}) ({phase("publication")}) (e(bias_diagnostics)[11,1]) (e(bias_diagnostics)[8,1])']
            if repeat==args.warm:
                lines += ['generate double d_x=e(b_bias_d)[1,1]','generate double d_z=e(b_bias_d)[1,2]',
                          'generate double c_x=e(b_bias_c)[1,1]','generate double c_z=e(b_bias_c)[1,2]',
                          'generate double vd_x=e(bias_mcse_d)[1,1]^2','generate double vd_z=e(bias_mcse_d)[1,2]^2',
                          'generate double vc_x=e(bias_mcse_c)[1,1]^2','generate double vc_z=e(bias_mcse_c)[1,2]^2',
                          'generate double covdc_x=e(bias_cov_dc)[1,1]','generate double covdc_z=e(bias_cov_dc)[1,2]',
                          'format bf* d_x d_z c_x c_z vd_* vc_* covdc_* %24.17g',
                          f'export delimited bf* d_x d_z c_x c_z vd_* vc_* covdc_* using "{out}/coefficients.csv", replace datafmt']
            lines += ['drop bf_*']
        else:
            if args.refit_reference:
                lines += ['drop base_g base_h fit_mu', 'timer clear 3', 'timer on 3',
                          f'quietly {command.replace("absorb(g h)","absorb(base_g=g base_h=h)")} mu(fit_mu)',
                          'timer off 3','quietly timer list 3','local ordinary_seconds=r(t3)',
                          'timer clear 9','timer on 9',
                          'quietly summarize base_h if h==1, meanonly','quietly replace base_g=base_g+r(mean)',
                          'quietly summarize base_h if h==1, meanonly','quietly replace base_h=base_h-r(mean)',
                          'mata: PB_g=st_data(.,"g"); PB_h=st_data(.,"h"):-1; PB_C=st_data(.,("x","z")); PB_y=st_data(.,"y"); PB_mu=st_data(.,"fit_mu"); PB_w=J(st_nobs(),1,1); PB_unit=st_data(.,"unit"); PB_rawg=st_data(.,"base_g"); PB_rawh=st_data(.,"base_h")',
                          'timer off 9','quietly timer list 9','local mata_preparation=r(t9)',
                          'assert mreldif(ordinary_b,e(b))==0','assert mreldif(ordinary_v,e(V))==0']
            lines += ['timer clear 4','timer on 4',
                      f'mata: PB_result=pb_correction(PB_g,PB_h,PB_C,PB_y,PB_mu,PB_w,PB_unit,PB_signs,{args.probes},{int(args.engine=="mata_cached")})',
                      'timer off 4','quietly timer list 4','local elapsed=r(t4)',
                      'timer clear 2','timer on 2',
                      'mata: pb_publish(PB_result,PB_g,PB_h,PB_rawg,PB_rawh)',
                      'timer off 2','quietly timer list 2','local publication=r(t2)',
                      'timer off 1','quietly timer list 1','local total=r(t1)',
                      f'post `receipt\' ("{args.engine}") ({repeat}) (`total\') (`elapsed\') (`mata_preparation\'+{phase("preparation")}) ({fitting}) (`ordinary_seconds\') ({phase("inference")}) ({phase("fe_recovery")}) (`publication\'+{phase("publication")}) (.) (.)']
            if repeat==args.warm:
                # Full identified D/C plus covariance, in the same anchor map.
                lines += ['preserve','clear',f'set obs {2*g+1}',
                          'generate double d=.', 'generate double c=.',
                          'generate double vd=.', 'generate double vc=.', 'generate double covdc=.',
                          'mata: st_store(.,("d","c","vd","vc","covdc"),PB_result)',
                          'format * %24.17g',f'export delimited using "{out}/coefficients.csv", replace datafmt','restore']
            lines += ['drop bf_*']
    lines += ['postclose `receipt\'',f'use "{out}/timing.dta", clear', 'format total-score %24.17g',
              f'export delimited using "{out}/timing.csv", replace datafmt',
              'display "END_FIXTURE|bias_benchmark|rc=0"']
    fixture=out/'bias_benchmark.do';fixture.write_text('\n'.join(lines)+'\n')
    sys.path.insert(0,str(root/'benchmarks/two_fe_dense'))
    from route_development import foreign_processes
    monitor_path=root/'benchmarks/two_fe_dense/route_development.py'
    overlaps=[];monitor_errors=[];samples=[]
    def sample_processes():
        try:
            foreign=foreign_processes()
            samples.append(time.monotonic())
            if foreign:
                overlaps.append(dict(time=time.time(),processes=foreign))
        except Exception as error:
            monitor_errors.append(repr(error))
    sample_processes()
    if overlaps or monitor_errors:
        (out/'preflight-blocked.json').write_text(json.dumps(dict(overlaps=overlaps,errors=monitor_errors),indent=2)+'\n')
        raise SystemExit('Benchmark process isolation failed before measurement')
    stop=threading.Event()
    def watch():
        while not stop.wait(.05):
            sample_processes()
    watcher=threading.Thread(target=watch)
    started=time.perf_counter()
    watcher.start()
    try:
        result=run_fixture(args.stata,fixture,root,plugin=args.plugin,
                           plugin_sha256=sha256_file(args.plugin),log_dir=out/'logs',require_status=True)
    finally:
        stop.set();watcher.join()
    process_seconds=time.perf_counter()-started
    rss=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss*(1 if sys.platform=='darwin' else 1024)
    receipt=dict(schema=1,source=json.loads(args.source_manifest.read_text()),settings={k:str(v) if isinstance(v,Path) else v for k,v in vars(args).items()},
                 plugin_sha256=sha256_file(args.plugin),reference_sha256=sha256_file(args.reference),
                 script_sha256=sha256_file(Path(__file__)),
                 mata_sha256=sha256_file(root/"tests/stata/bias/mata_correction.do"),
                 input_sha256=sha256_file(out/'input.dta'),signs_sha256=sha256_file(out/'signs.dta'),
                 fixture_sha256=sha256_file(fixture),probe_preparation_seconds=probe_preparation_seconds,
                 monitor=dict(sha256=sha256_file(monitor_path),interval_seconds=.05,
                              successful_samples=len(samples),overlaps=overlaps,errors=monitor_errors),
                 timing_eligible=not overlaps and not monitor_errors and len(samples)>1,
                 process_seconds=process_seconds,process_peak_rss_bytes=rss,stata=result.metadata,
                 stata_rc=result.stata_rc,error=result.error,passed=not result.error and result.stata_rc==0)
    (out/'receipt.json').write_text(json.dumps(receipt,indent=2,sort_keys=True)+'\n')
    if not receipt['passed']:
        print(result.log_text[-14000:]);raise SystemExit(1)
    print(json.dumps(receipt,indent=2,sort_keys=True))

if __name__=='__main__':
    main()
