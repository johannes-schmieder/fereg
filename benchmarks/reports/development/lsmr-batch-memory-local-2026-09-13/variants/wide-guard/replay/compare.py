import ppml_preparation_compare as ppml
import preparation_compare as ols
from pathlib import Path
from measure_with_solver_trace import measure
ppml.BASELINE=ols.BASELINE="ac47b1907c7bc7bc8ee78de85621b0cc96546a4c"
ppml.measure=measure
exec((Path(__file__).parent/'ppml-compare-science.py').read_text(),ppml.__dict__)
for k in [17,65]:
 for index,seed in enumerate([104729,15485863,32452843]):
  ppml.pair(dict(name=f"checkpoint_k{k}",design="weak_d3",n=30720,k=k,threads=4,weight="[pw=w]",vce="cluster c1",fit_options="offset(off)"),seed,index,trace=True,prefix="wide-guard-")
for index,seed in enumerate([104729,8675309,20260819]):
 ols.pair(dict(name="wide_weak",design="weak_d3",n=122880,k=65,threads=4,weight="[aw=w]",vce="robust"),seed,index,trace=True,prefix="wide-guard-")
