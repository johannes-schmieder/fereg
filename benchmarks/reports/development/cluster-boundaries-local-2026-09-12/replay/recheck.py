from controls import CONTROLS
from paired import ppml
spec=next(s for model,s in CONTROLS if s['name']=='ppml_two_way')
if __name__=='__main__':
 for i,seed in enumerate([32452843,49979687,67867967]):
  ppml.pair(spec,seed,i,trace=True,prefix='boundaries-recheck-')
