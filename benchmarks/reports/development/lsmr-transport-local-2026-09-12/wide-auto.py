import argparse
from compare import ols
spec=dict(name="weak_wide_auto65",design="weak_d3",n=122880,k=65,threads=4,weight="[aw=w]",vce="robust")
p=argparse.ArgumentParser();p.add_argument("--part",choices=["pilot","primary"],required=True);a=p.parse_args()
for i,seed in enumerate([31415927] if a.part=="pilot" else [104729,8675309,20260819]):
 ols.pair(spec,seed,i,trace=a.part=="pilot",prefix="lsmr-transport-"+a.part+"-")
