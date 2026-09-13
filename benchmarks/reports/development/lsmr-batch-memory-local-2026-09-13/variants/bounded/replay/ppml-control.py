import ppml_preparation_compare as ppml
ppml.BASELINE="ac47b1907c7bc7bc8ee78de85621b0cc96546a4c"
spec=dict(name="weak_checkpoint",design="weak_d3",n=30720,k=10,threads=4,weight="[pw=w]",vce="cluster c1",fit_options="offset(off)")
for index,seed in enumerate([104729,8675309,20260819]):
 ppml.pair(spec,seed,index,trace=True,prefix="joint-model-control-")
