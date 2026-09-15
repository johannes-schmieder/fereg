from pathlib import Path
import argparse,hashlib,json,os,time
import numpy as np
import pandas as pd
import pyarrow.dataset as ds
p=argparse.ArgumentParser();p.add_argument('--root',type=Path,required=True);a=p.parse_args();root=a.root
spec=json.loads((root/'sample-authorization.json').read_text());source=root/'input/source.parquet';out=root/'input/sample.dta';assert not out.exists();assert source.stat().st_size==spec['source_bytes']
start=time.time();digest=hashlib.sha256()
with source.open('rb') as f:
 for block in iter(lambda:f.read(8*1024**2),b''):digest.update(block)
assert digest.hexdigest()==spec['source_sha256'],'transferred input checksum mismatch'
threshold=(1<<64)//20;mask64=(1<<64)-1;seed=spec['seed']
def worker_hash(ids):
 x=ids.astype(np.uint64)^np.uint64(seed)
 with np.errstate(over='ignore'):
  x=x+np.uint64(0x9e3779b97f4a7c15);x=(x^(x>>np.uint64(30)))*np.uint64(0xbf58476d1ce4e5b9);x=(x^(x>>np.uint64(27)))*np.uint64(0x94d049bb133111eb);x=x^(x>>np.uint64(31))
 return x
def scalar_hash(i):
 x=((i^seed)+0x9e3779b97f4a7c15)&mask64;x=((x^(x>>30))*0xbf58476d1ce4e5b9)&mask64;x=((x^(x>>27))*0x94d049bb133111eb)&mask64;return x^(x>>31)
probe=np.array([0,1,19,20,20260915,202444946,2**31-1,1],dtype=np.int64)
assert worker_hash(probe).tolist()==[scalar_hash(int(i)) for i in probe]
columns=['persid','betnr','year','logrwage2','age','edyrs'];batches=[];seen_workers=set();window_n=0;selected_n=0;selection_hash=hashlib.sha256();full_extremes=0;selected_extremes=0
scanner=ds.dataset(source,format='parquet').scanner(columns=columns,filter=(ds.field('year')>=2001)&(ds.field('year')<=2009),batch_size=262144,use_threads=False)
for batch in scanner.to_batches():
 frame=batch.to_pandas();ids=frame['persid'].to_numpy(dtype=np.int64);assert (ids>=0).all() and (ids<=2**31-1).all()
 seen_workers.update(ids.tolist());keep=worker_hash(ids)<np.uint64(threshold)
 original_rows=np.arange(window_n+1,window_n+len(frame)+1,dtype=np.int64)
 window_n+=len(frame);full_extremes+=int((frame['logrwage2']>20).sum())
 chosen=frame.loc[keep].copy();chosen['full_window_row']=original_rows[keep];selection_hash.update(chosen['full_window_row'].to_numpy(dtype='<i8').tobytes());selected_extremes+=int((chosen['logrwage2']>20).sum());selected_n+=len(chosen)
 if len(chosen):batches.append(chosen)
assert window_n==91045290
full_workers=len(seen_workers);del seen_workers
frame=pd.concat(batches,ignore_index=True);del batches
assert len(frame)==selected_n and frame['full_window_row'].is_monotonic_increasing
assert not frame.duplicated(['persid','year']).any()
assert frame[['persid','betnr','year','logrwage2','age']].notna().all().all()
assert frame['edyrs'].dropna().isin([10,13,16,18]).all()
selected_workers=frame['persid'].nunique();assert .048<selected_workers/full_workers<.052
frame.insert(0,'obs_id',np.arange(1,len(frame)+1,dtype=np.int32))
partial=out.with_suffix('.partial.dta');frame.to_stata(partial,write_index=False,version=118)
check=pd.read_stata(partial,convert_categoricals=False)
assert check.columns.equals(frame.columns)
for col in frame:assert np.array_equal(check[col].to_numpy(),frame[col].to_numpy(),equal_nan=True),col
digest=hashlib.sha256()
with partial.open('rb') as f:
 for block in iter(lambda:f.read(8*1024**2),b''):digest.update(block)
partial.rename(out)
receipt=dict(source_sha256=spec['source_sha256'],sample_sha256=digest.hexdigest(),sample_bytes=out.stat().st_size,window_rows=window_n,window_workers=full_workers,sample_rows=selected_n,sample_workers=int(selected_workers),worker_fraction=selected_workers/full_workers,row_fraction=selected_n/window_n,sampling_unit='worker',seed=seed,rule='splitmix64(persid xor seed) < floor(2^64/20)',full_window_row_sha256=selection_hash.hexdigest(),window_logwage_above_20=full_extremes,sample_logwage_above_20=selected_extremes,all_selected_worker_years=True,external_singleton_pruning=False,roundtrip_passed=True,elapsed_seconds=time.time()-start,job_id=os.environ['JOB_ID'],host=os.uname().nodename)
(root/'input/sample-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt),flush=True);print('GERMAN_WORKER_SAMPLE_COMPLETE',flush=True)
