from pathlib import Path
import hashlib
import ppml_preparation_compare as ppml
from common import write_json
ppml.BASELINE="ac47b1907c7bc7bc8ee78de85621b0cc96546a4c"
exec((Path(__file__).parent/'ppml-compare-science.py').read_text(),ppml.__dict__)
original_measure=ppml.measure
def measured(*args,**kwargs):
 if args[1].get('same_artifact'):
  kwargs['source']=ppml.BASE/'baseline'
  kwargs['identity']=dict(kwargs['identity'],plugin_sha256=hashlib.sha256((kwargs['source']/'dist/fereg_macos.plugin').read_bytes()).hexdigest(),same_artifact=True)
 return original_measure(*args,**kwargs)
ppml.measure=measured
for repeat in range(2):
 for index,seed in enumerate([15485863,67867967,49979687]):
  order=[True,False] if (index+repeat)%2==0 else [False,True]
  for same in order:
   spec=dict(name='checkpoint_aa' if same else 'checkpoint_ab',design='weak_d3',n=30720,k=17,threads=4,weight='[pw=w]',vce='cluster c1',fit_options='offset(off)',same_artifact=same)
   paths=ppml.pair(spec,seed,index+repeat,trace=False,prefix=f'ordinary-r{repeat}-')
   if same:
    a=ppml.named_runtime(paths['baseline']);b=ppml.named_runtime(paths['candidate'])
    assert a['modeled_selected_bytes']==b['modeled_selected_bytes']
    write_json(paths['candidate']/'same-artifact.json',dict(plugin_sha256=hashlib.sha256((ppml.BASE/'baseline/dist/fereg_macos.plugin').read_bytes()).hexdigest(),model_exact=True))
