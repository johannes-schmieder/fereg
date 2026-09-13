"""Protocol-authorized rejection branch: no additional primary seeds."""
from compare import CONTROLS, PRIMARY, pair
# Small/slope and low/saved exercise affected categorical preparation; weak
# ordinary OLS and explicit three-FE PPML protect graph paths. Chosen before
# these control timings; these observations cannot qualify the failed primaries.
for i in [2,3,4,7]:
 model,spec=CONTROLS[i]
 pair(model,spec,31415927,i,prefix='control-')
for i,(model,spec) in enumerate(PRIMARY[1:]):
 pair(model,spec,67280421,i,True,prefix='profile-')
