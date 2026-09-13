import ppml_preparation_compare as ppml
import preparation_compare as ols
from pathlib import Path
from measure_with_solver_trace import measure
ppml.BASELINE=ols.BASELINE="ac47b1907c7bc7bc8ee78de85621b0cc96546a4c"
ppml.measure=measure
exec((Path(__file__).parent/'ppml-compare-science.py').read_text(),ppml.__dict__)
for index,seed in enumerate([15485863,67867967,49979687]):
 ppml.pair(dict(name="checkpoint_k17",design="weak_d3",n=30720,k=17,threads=4,weight="[pw=w]",vce="cluster c1",fit_options="offset(off)"),seed,index,trace=True,prefix="wide-guard-recheck-")
original=ols.measure
ols.measure=lambda *a,**kw:original(*a,solver=a[1].get('solver','auto'),**kw)
cases=[
 dict(name="weak_narrow",design="weak_d3",n=30720,k=10,threads=4,weight="[aw=w]",vce="cluster c1"),
 dict(name="small_narrow",design="weak_d3",n=7680,k=1,threads=1,weight="[aw=w]",vce="robust"),
 dict(name="strong_schur",design="two_fe_dense",n=61440,k=50,threads=4,weight="[aw=w]"),
 dict(name="complete_iid",design="one_fe",n=983040,k=50,threads=16),
 dict(name="low_one_fe",design="one_fe",n=30720,k=10,threads=4,weight="[fw=w]",vce="cluster c1",fit_options="memory(low)"),
 dict(name="generic_lsmr",design="weak_d3",n=30720,k=10,threads=4,weight="[aw=w]",vce="robust",solver="lsmr",fit_options="preconditioner(schwarz)")]
for index,spec in enumerate(cases):
 ols.pair(spec,31415927,index,trace=True,prefix="wide-guard-control-")
