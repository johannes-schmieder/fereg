import sys
from pathlib import Path
from final_compare import BASE,invoke,baseline_compare
from run import compare
for threads in [4,16]:
 for design in ['strong_d3','strong_d6']:
  for rep,seed in enumerate([104729,8675309,20260819]):
   arms=[('vector','fereg',BASE/'vector-prototype'),('fixest','fixest',BASE/'baseline')]
   if rep%2: arms.reverse()
   paths={name:invoke('confirm-'+name,design,983040,50,threads,seed,arm,source) for name,arm,source in arms}
   print('FIXEST',compare(paths['vector'],paths['fixest']),flush=True)
   baseline=BASE/'runs'/f'repeat-prototype-{design}-n983040-k50-t{threads}-s{seed}-fereg'
   print('SCHUR',baseline_compare(paths['vector'],baseline),flush=True)
