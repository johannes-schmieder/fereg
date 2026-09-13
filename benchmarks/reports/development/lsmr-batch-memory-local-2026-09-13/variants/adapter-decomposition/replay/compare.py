import ppml_preparation_compare as ppml
from pathlib import Path
from measure_with_solver_trace import measure
ppml.BASELINE="ac47b1907c7bc7bc8ee78de85621b0cc96546a4c"
ppml.measure=measure
exec((Path(__file__).parent/'ppml-compare-science.py').read_text(),ppml.__dict__)
for repeat in range(2):
 for index,seed in enumerate([15485863,67867967,49979687]):
  ppml.pair(dict(name="checkpoint_k17",design="weak_d3",n=30720,k=17,threads=4,weight="[pw=w]",vce="cluster c1",fit_options="offset(off)"),seed,index+repeat,trace=True,prefix=f"adapter-decomposition-r{repeat}-")
