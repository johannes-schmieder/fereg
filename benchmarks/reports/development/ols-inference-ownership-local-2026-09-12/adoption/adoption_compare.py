import compare
CASES=[
 dict(name='no_fe_eligible',design='one_fe',n=245760,k=50,threads=16,vce='robust',effects=[]),
 dict(name='width65_eligible',design='one_fe',n=98304,k=65,threads=4,vce='robust'),
 dict(name='interaction_multiway_eligible',design='one_fe',n=245760,k=50,threads=16,vce='cluster c1 c2',absorb='g1#c2',missing=True),
]
for j,spec in enumerate(CASES):compare.pair(spec,67280421,j,prefix='adoption-')
