"""Finish the original three-FE control; preserve the mislabeled two-FE pair."""
import inspect
import compare
m=compare.ppml
source=inspect.getsource(m.pair)
old="cell['effects'] = ['g1'] if spec['design'] == 'one_fe' else ['g1', 'g2']"
assert source.count(old)==1
source=source.replace(old,"cell['effects'] = ['g1', 'g2', 'g3']")
exec(compile(source,'explicit_three_fe_pair','exec'),m.__dict__)
m.pair(dict(name='weak_three_fe',design='weak_d3',n=7680,k=10,threads=4),31415927,rotation=1,prefix='psd-control-correction-')
