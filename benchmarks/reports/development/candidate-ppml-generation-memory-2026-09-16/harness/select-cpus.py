from pathlib import Path
import os,sys,json
allowed=os.sched_getaffinity(0); wanted=4
for node in sorted(Path('/sys/devices/system/node').glob('node[0-9]*')):
    cpus=[];cores=set()
    for cpu in sorted(node.glob('cpu[0-9]*'),key=lambda p:int(p.name[3:])):
        i=int(cpu.name[3:])
        if i not in allowed:continue
        topology=Path('/sys/devices/system/cpu')/cpu.name/'topology'
        key=((topology/'physical_package_id').read_text().strip(),(topology/'core_id').read_text().strip())
        if key in cores:continue
        cpus.append(i);cores.add(key)
        if len(cpus)==wanted:
            Path('placement.json').write_text(json.dumps(dict(node=node.name,cpus=cpus,distinct_physical_cores=sorted(cores),allowed=sorted(allowed)),indent=2)+'\n')
            print(','.join(map(str,cpus)));sys.exit(0)
raise SystemExit('No single NUMA node with enough allowed physical cores')
