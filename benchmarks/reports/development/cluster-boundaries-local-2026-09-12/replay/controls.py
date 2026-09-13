from paired import ols,ppml
CONTROLS=[
 ('ols',dict(name='ols_balanced',design='one_fe',n=983040,k=10,threads=4,weight='[pw=w]',vce='cluster c1')),
 ('ols',dict(name='ols_small_narrow',design='one_fe',n=7680,k=1,threads=1,weight='[aw=w]',vce='cluster c1',skew=True)),
 ('ols',dict(name='ols_weak',design='weak_d3',n=30720,k=10,threads=4,weight='[aw=w]',vce='cluster c1',skew=True)),
 ('ols',dict(name='ols_two_way',design='one_fe',n=122880,k=10,threads=16,weight='[fw=w]',vce='cluster c1 c2',skew=True,missing=True)),
 ('ols',dict(name='ols_low_saved',design='one_fe',n=30720,k=10,threads=4,weight='[aw=w]',vce='cluster c1',skew=True,absorb='saved_g=g1',fit_options='memory(low) residuals(saved_e)')),
 ('ppml',dict(name='ppml_balanced',design='one_fe',n=122880,k=10,threads=4,weight='[fw=w]',vce='cluster c1',fit_options='offset(off)')),
 ('ppml',dict(name='ppml_weak',design='weak_d3',n=30720,k=10,threads=4,weight='[pw=w]',vce='cluster c1',skew=True,fit_options='offset(off)')),
 ('ppml',dict(name='ppml_two_way',design='one_fe',n=122880,k=10,threads=16,weight='[pw=w]',vce='cluster c1 c2',skew=True,fit_options='offset(off)',missing=True)),
 ('ppml',dict(name='ppml_low_saved',design='one_fe',n=7680,k=3,threads=4,weight='[fw=w]',vce='cluster c1',skew=True,absorb='saved_g=g1',fit_options='memory(low) offset(off) mu(saved_mu) dropreason(saved_drop)',zero_groups=True)),
]
if __name__=='__main__':
 for j,(model,spec) in enumerate(CONTROLS):
  (ols if model=='ols' else ppml).pair(spec,15485863,j,prefix='boundaries-control-')
