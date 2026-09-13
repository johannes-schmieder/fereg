from pathlib import Path
import os
repo=Path(__file__).resolve().parents[4]
root=Path(os.environ['FEREG_LOCAL_BENCH_ROOT'])
source=(repo/'benchmarks/local_fixest_gap/collect_preparation.py').read_text()
old='else "profile" if "-profile-" in record["pair"] else'
new='else "profile" if ("-profile-" in record["pair"] or "-trace-" in record["pair"]) else'
assert source.count(old)==1
namespace={'__name__':'local_collector'}
exec(source.replace(old,new),namespace)
namespace['collect'](root,root/'portable-reviewed',pair_prefix='inference-onefe',
 science_marker='inference-science.json',exact_science=False,
 note='Nine unprofiled primary pairs, seven controls and one separate trace; all prescribed observations retained.',
 primary_cases=['interaction_iid','interaction_aw_robust','interaction_pw_cluster'])
