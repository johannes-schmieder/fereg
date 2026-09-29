"""Create a reproducible real VWH panel; raw and row-level files stay ignored."""
import argparse
import hashlib
import json
import shutil
import zipfile
import zlib
from pathlib import Path

import numpy as np
import pandas as pd

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(8 << 20), b''):
            h.update(block)
    return h.hexdigest()


def main():
    p = argparse.ArgumentParser()
    p.add_argument('archive', type=Path)
    p.add_argument('--first-year', type=int, default=1999)
    args = p.parse_args()
    work = ROOT / 'work'
    raw = work / 'raw'
    raw.mkdir(exist_ok=True)
    receipt = {'archive_sha256': sha(args.archive), 'first_year': args.first_year,
               'last_year': 2001, 'sample_definition': 'VWH benchmark adaptation, not KSS replication',
               'counts': {}, 'raw_members': {}}
    with zipfile.ZipFile(args.archive) as z:
        for name in ['contr.dta', 'anagr.dta', 'azien.dta']:
            member = next(n for n in z.namelist() if n.endswith('/' + name))
            target = raw / name
            if not target.exists():
                with z.open(member) as src, target.open('wb') as dst:
                    shutil.copyfileobj(src, dst, 8 << 20)
            crc = 0
            with target.open('rb') as stream:
                for block in iter(lambda: stream.read(8 << 20), b''):
                    crc = zlib.crc32(block, crc)
            assert target.stat().st_size == z.getinfo(member).file_size and crc == z.getinfo(member).CRC, 'Cached raw data differ from archive'
            receipt['raw_members'][name] = sha(target)
    columns = ['cod_pgr', 'matr_az', 'anno', 'gior_r', 'retrib03']
    parts = []
    total = 0
    for chunk in pd.read_stata(raw / 'contr.dta', columns=columns,
                               convert_categoricals=False, chunksize=1_000_000):
        total += len(chunk)
        parts.append(chunk.loc[chunk.anno.between(args.first_year, 2001)].copy())
    d = pd.concat(parts, ignore_index=True)
    del parts
    receipt['counts']['raw_spells'] = total
    receipt['counts']['selected_year_spells'] = len(d)
    d = d.dropna(subset=['cod_pgr', 'matr_az', 'anno'])
    d = d.drop_duplicates(columns)
    # Stable, declared tie-break: highest pay, then smallest firm ID, then most days.
    d = d.sort_values(['cod_pgr', 'anno', 'retrib03', 'matr_az', 'gior_r'],
                      ascending=[True, True, False, True, False], na_position='last')
    d = d.drop_duplicates(['cod_pgr', 'anno'])
    receipt['counts']['dominant_person_years'] = len(d)
    workers = pd.read_stata(raw / 'anagr.dta', columns=['cod_pgr', 'anno_n'], convert_categoricals=False)
    firms = pd.read_stata(raw / 'azien.dta', columns=['matr_az', 'prov'], convert_categoricals=False)
    assert workers.cod_pgr.is_unique and firms.matr_az.is_unique
    d = d.merge(workers, on='cod_pgr', validate='many_to_one').merge(firms, on='matr_az', validate='many_to_one')
    d['age'] = d.anno - d.anno_n
    d = d[d.prov.isin(['BL', 'PD', 'RO', 'TV', 'VE', 'VR', 'VI']) & d.age.between(19, 63)]
    d = d[(d.gior_r > 0) & (d.retrib03 > 0)].copy()
    d['pay'] = d.retrib03.astype('float64') / d.gior_r
    d = d[np.isfinite(d.pay)].copy()
    receipt['counts']['geography_age_positive_pay'] = len(d)
    # Shared fixed-point singleton removal prevents different native sample policies.
    iterations = 0
    while True:
        keep = (d.groupby('cod_pgr').cod_pgr.transform('size') > 1) & (d.groupby('matr_az').matr_az.transform('size') > 1)
        if keep.all():
            break
        d = d.loc[keep].copy()
        iterations += 1
    d = d.sort_values(['cod_pgr', 'anno', 'matr_az']).reset_index(drop=True)
    assert d.matr_az.max() < 2**53
    out = pd.DataFrame({'obs_id': np.arange(1, len(d)+1, dtype=np.int32),
                        'g1': pd.factorize(d.cod_pgr, sort=True)[0].astype(np.int32)+1,
                        'g2': pd.factorize(d.matr_az, sort=True)[0].astype(np.int32)+1,
                        'x1': (d.age.astype(float)/10)**2,
                        'x2': (d.age.astype(float)/10)**3})
    for j, year in enumerate(range(args.first_year+1, 2002), 3):
        out[f'x{j}'] = (d.anno == year).astype('float64')
    receipt.update(N=len(d), workers=int(out.g1.nunique()), firms=int(out.g2.nunique()),
                   singleton_iterations=iterations, k=len(out.columns)-3,
                   controls='age squared /100, age cubed /1000, year indicators (first year omitted)',
                   ppml_outcome='daily remuneration in delivered units; positive continuous outcome',
                   no_kss_restrictions=['wage-level or wage-change trimming', 'public-sector exclusions',
                                        'sex/nationality restrictions', 'leave-out connected-set selection'])
    for model in ['ols', 'ppml']:
        out['y'] = np.log(d.pay) if model == 'ols' else d.pay.to_numpy()
        path = work / f'{model}.dta'
        out.to_stata(path, write_index=False, version=118)
        with (work / f'{model}.bin').open('wb') as stream:
            for column in out:
                stream.write(np.asarray(out[column], dtype='<f8').tobytes())
        hashes = {c: hashlib.sha256(np.asarray(out[c], dtype='<f8').tobytes()).hexdigest() for c in out}
        (work / f'{model}.json').write_text(json.dumps({'column_sha256': hashes, 'columns': list(out), 'n':len(out)}, indent=2))
        receipt[model+'_dta_sha256'] = sha(path)
    (ROOT / 'results/sample.json').write_text(json.dumps(receipt, indent=2)+'\n')
    print(json.dumps(receipt, indent=2), flush=True)


if __name__ == '__main__':
    main()
