#!/usr/bin/env python3
"""Reuse the historical synthetic and audited Veneto inputs, without reading rows.

The allocated fit validates file and decoded-column hashes before estimation.
"""
import argparse,json
from pathlib import Path
import campaign

def prepare(root, previous):
    destination=root/'input'
    inventory=json.loads((previous/'synthetic/input/frozen-inputs.json').read_text())
    cells=[];smoke=[];origins={}
    for model in ['ols','ppml']:
        origin=previous/('veneto-'+model)
        old=json.loads((origin/'manifest.json').read_text())
        assert old['cells'] and all(c['model']==model and c['design']=='veneto-long-lcs' for c in old['cells'])
        origins[model]=dict(root=str(origin),manifest_sha256=campaign.sha256(origin/'manifest.json'))
        for original in old['cells']:
            c=dict(original,repetitions=3);c.pop('arms');c['arms']=campaign.arms(c);cells.append(c)
        c=dict(cells[-1],id='veneto-smoke-'+model,threads=4,save_fe=True,section='veneto-smoke',repetitions=1)
        path,=list((origin/'smoke-data').glob(model+'-veneto-long-lcs-n*.dta'))
        meta=json.loads(path.with_suffix('.json').read_text());c['rows']=meta['rows']
        assert c['rows']<=1200
        c.pop('arms');c['arms']=campaign.arms(c);smoke.append(c)
        if model=='ols':
            # Multi-effect areg cannot save FE; exercise its actual requested path.
            unsaved=dict(c,id=c['id']+'-unsaved',save_fe=False);unsaved.pop('arms')
            unsaved['arms']=['fereg','fereg-strict','areg','areg-strict'];smoke.append(unsaved)
        for cell in [cells[-1],c]:
            folder='smoke-data' if cell['section']=='veneto-smoke' else 'data'
            path=origin/folder/(campaign.dataset_key(cell)+'.dta')
            meta=json.loads(path.with_suffix('.json').read_text())
            assert path.is_file() and path.with_suffix('.components.npz').is_file()
            assert meta['rows']==cell['rows']
            key=campaign.dataset_key(cell)
            inventory['datasets'][key]=meta
            inventory.setdefault('dataset_paths',{})[key]=str(path)
    inventory['additional_input_origins']=origins
    for name,data in [('frozen-inputs.json',inventory),('additional-cells.json',cells),('additional-smoke-cells.json',smoke)]:
        if (destination/name).exists():raise ValueError('refusing to overwrite prepared input: '+name)
        campaign.write_json(destination/name,data)
    print('REVISION_INPUTS_PREPARED synthetic_datasets='+str(len(inventory['datasets'])-4)+' panel_cells='+str(len(cells)))

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path);p.add_argument('previous',type=Path)
    a=p.parse_args();prepare(a.root,a.previous)
