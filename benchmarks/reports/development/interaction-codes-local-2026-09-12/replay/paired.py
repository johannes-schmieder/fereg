import argparse
import preparation_compare as ols
import ppml_preparation_compare as ppml
BASELINE='9c2bcc5348c3f25dda70ab1340c3cbbdd48a089c'
ols.BASELINE=ppml.BASELINE=BASELINE
PRIMARY=[
 ('ols',dict(name='ols_interacted_cluster',design='one_fe',n=983040,k=10,threads=4,weight='[pw=w]',vce='cluster c1#c2',missing=True)),
 ('ols',dict(name='ols_interacted_fe_cluster',design='one_fe',n=245760,k=50,threads=16,weight='[fw=w]',vce='cluster c1#c2',absorb='g1#c2',missing=True)),
 ('ppml',dict(name='ppml_interacted_fe_cluster',design='one_fe',n=122880,k=10,threads=4,weight='[fw=w]',vce='cluster c1#c2',absorb='g1#c2',fit_options='offset(off)',missing=True)),
]
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--part',choices=['pilot','primary','profile'],required=True);args=parser.parse_args()
 for j,(model,spec) in enumerate(PRIMARY):
  if args.part=='pilot' and j:continue
  for i,seed in enumerate([104729,8675309,20260819] if args.part=='primary' else [31415927]):
   (ols if model=='ols' else ppml).pair(spec,seed,j+i,trace=args.part!='primary',prefix='interaction-'+args.part+'-')
