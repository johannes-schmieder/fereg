"""Exact rational oracle for cancellation, independent of the Rust arithmetic."""
import csv
from fractions import Fraction as F
from pathlib import Path
import struct


def bits(x):
    return f'{struct.unpack(">Q", struct.pack(">d", x))[0]:016x}'


cases = [
    ('ordinary', 4.25, 0.5, 1.5),
    ('veneto_scale', 4.1340027, 0.63, 1104.4971840386115),
    ('negative_response', -4.1340027, -0.63, -1104.4971840386115),
    ('combined', 4.1340027, 0.0, 1104.4971840386115),
    ('large_terms', 1.0, 0.0, 1e200),
    ('small_terms', 1e-200, 1e-201, 1e-199),
    ('subnormal', float.fromhex('0x0.0000000000003p-1022'), 0.0,
     float.fromhex('0x0.0000000000001p-1022')),
    ('near_max', 1e306, 0.0, 8e307),
]
rows=[]
for label,z,f,r in cases:
    # Correctly round the exact absorbed predictor once, without the Rust
    # implementation's intermediate subtraction or reconstruction operations.
    a=float(F(z)-F(f)-F(r))
    for corrupted in [False, True]:
        supplied = a + max(abs(a),abs(z),1.0)*1e-8 if corrupted else a
        exact_error=abs(F(f)+F(supplied)+F(r)-F(z))
        rows.append([label+('_corrupt' if corrupted else ''), bits(z),bits(f),bits(supplied),bits(r),
                     bits(float(exact_error)),int(not corrupted)])
path=Path(__file__).with_name('reconstruction.csv')
with path.open('w',newline='') as output:
    w=csv.writer(output,lineterminator='\n')
    w.writerow(['case','response','finite','absorbed','residual','exact_discrepancy','pass'])
    w.writerows(rows)
