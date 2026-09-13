import argparse
import preparation_compare as ols
original_measure=ols.measure
ols.measure=lambda *a,**kw:original_measure(*a,solver=a[1].get("solver","auto"),**kw)
from common import sha256
ols.BASELINE="15bd21dd978d014bc477bac072e504b83c8f4dba"
PRIMARY=[
 dict(name="weak_aw_cluster",design="weak_d3",n=30720,k=10,threads=4,weight="[aw=w]",vce="cluster c1"),
 dict(name="weak_wide_lsmr",design="weak_d3",n=122880,k=50,threads=4,weight="[aw=w]",vce="robust",solver="lsmr",fit_options="preconditioner(schwarz)"),
]
CONTROLS=[
 dict(name="small_narrow",design="weak_d3",n=7680,k=1,threads=1,weight="[aw=w]",vce="robust"),
 dict(name="retained_low",design="weak_d3",n=30720,k=10,threads=4,weight="[fw=w]",vce="cluster c1",solver="lsmr",fit_options="memory(low) preconditioner(schwarz)"),
 dict(name="saved",design="weak_d3",n=7680,k=10,threads=4,weight="[aw=w]",absorb="saved_g=g1 g2",solver="lsmr",fit_options="residuals(saved_e) preconditioner(schwarz)"),
 dict(name="strong_schur",design="two_fe_dense",n=61440,k=50,threads=4,weight="[aw=w]"),
 dict(name="complete_iid",design="one_fe",n=983040,k=50,threads=16),
]
if __name__=="__main__":
 p=argparse.ArgumentParser();p.add_argument("--part",choices=["pilot","primary","controls"],required=True);a=p.parse_args()
 for j,spec in enumerate(CONTROLS if a.part=="controls" else PRIMARY):
  for i,seed in enumerate([104729,8675309,20260819] if a.part=="primary" else [31415927]):
   ols.pair(spec,seed,j+i,trace=a.part=="pilot",prefix="lsmr-transport-"+a.part+"-")
