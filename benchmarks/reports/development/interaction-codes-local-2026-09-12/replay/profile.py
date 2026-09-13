from paired import ols,ppml,PRIMARY
for j,(model,spec) in enumerate(PRIMARY[1:]):
 (ols if model=='ols' else ppml).pair(spec,32452843,j,trace=True,prefix='interaction-profile-trace-')
