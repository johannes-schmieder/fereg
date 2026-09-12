import compare
spec=dict(name='low_fw_cluster',design='one_fe',n=122880,k=10,threads=4,weight='[fw=w]',vce='cluster c1',fit_options='memory(low)',missing=True)
paths=compare.pair(spec,104729,trace=True,prefix='low-pilot-')
for name in ['b','V','fereg_fit','fereg_certificate']:
 assert (paths['baseline']/(name+'.csv')).read_bytes()==(paths['candidate']/(name+'.csv')).read_bytes(),name
