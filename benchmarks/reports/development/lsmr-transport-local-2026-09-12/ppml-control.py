import ppml_preparation_compare as ppml
ppml.BASELINE="15bd21dd978d014bc477bac072e504b83c8f4dba"
spec=dict(name="weak_checkpoint",design="weak_d3",n=30720,k=10,threads=4,weight="[pw=w]",vce="cluster c1",fit_options="offset(off)")
ppml.pair(spec,15485863,1,trace=True,prefix="lsmr-transport-control-")
