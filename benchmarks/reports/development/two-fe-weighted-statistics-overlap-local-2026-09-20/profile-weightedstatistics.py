from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
for label, extra in [
 ('weighted-statistics-detail-shuffled',['--shuffled','--seed','31415927']),
 ('weighted-statistics-detail-strong6',['--design','strong_d6','--n','245760'])]:
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--label',label,'--variants','weightedstatsprofile','weightedstatisticsprofile','--widths','6','--cases','iid','--weight-kind','aw','--weight-design','moderate','--repeats','1','--trace']+extra,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('WEIGHTED_STATISTICS_DETAIL_COMPLETE',flush=True)
