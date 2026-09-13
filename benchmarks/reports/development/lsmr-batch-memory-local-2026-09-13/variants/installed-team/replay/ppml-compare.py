import ppml_preparation_compare as ppml
from pathlib import Path
ppml.BASELINE="ac47b1907c7bc7bc8ee78de85621b0cc96546a4c"
exec((Path(__file__).parent/'ppml-compare-science.py').read_text(),ppml.__dict__)
spec=dict(name="weak_checkpoint",design="weak_d3",n=30720,k=10,threads=4,weight="[pw=w]",vce="cluster c1",fit_options="offset(off)")
for repeat in range(2):
 for index,seed in enumerate([104729,15485863,32452843]):
  ppml.pair(spec,seed,index+repeat,trace=True,prefix=f"installed-team-r{repeat}-")
