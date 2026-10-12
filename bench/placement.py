"""Linux physical-core placement shared by both campaign estimator arms."""
from pathlib import Path
import os
import subprocess


def socket_ordered_cpus(cpuinfo, allowed):
    sockets = {}
    seen = set()
    for block in cpuinfo.split('\n\n'):
        values = dict(line.split(':', 1) for line in block.splitlines() if ':' in line)
        values = {k.strip(): v.strip() for k, v in values.items()}
        if 'processor' not in values:
            continue
        cpu = int(values['processor'])
        if cpu not in allowed:
            continue
        if 'physical id' not in values or 'core id' not in values:
            raise RuntimeError('physical CPU topology unavailable')
        socket = int(values['physical id'])
        key = (socket, int(values['core id']))
        if key not in seen:
            sockets.setdefault(socket, []).append(cpu)
            seen.add(key)
    # Start with the largest allowed socket: every prefix that fits on any
    # socket then fits on this one, including partially restricted allocations.
    order = sorted(sockets, key=lambda s: (-len(sockets[s]), s))
    return [cpu for socket in order for cpu in sorted(sockets[socket])]


def placement_metadata(cpus, sys_cpu=Path('/sys/devices/system/cpu')):
    if not cpus or len(cpus) != len(set(cpus)) or not set(cpus) <= os.sched_getaffinity(0):
        raise RuntimeError('invalid or unavailable campaign CPU mask')
    nodes = set()
    sockets = set()
    for cpu in cpus:
        root = sys_cpu / f'cpu{cpu}'
        selected = [int(p.name[4:]) for p in root.glob('node[0-9]*')]
        if len(selected) != 1:
            raise RuntimeError(f'NUMA node unavailable for CPU {cpu}')
        nodes.update(selected)
        sockets.add(int((root / 'topology/physical_package_id').read_text()))
    return dict(cpus=list(cpus), mask=','.join(map(str, cpus)),
                sockets=sorted(sockets), memory_nodes=sorted(nodes),
                lscpu=subprocess.check_output(['lscpu'], text=True),
                thp=Path('/sys/kernel/mm/transparent_hugepage/enabled').read_text().strip())


def bound_command(command, placement):
    nodes = ','.join(map(str, placement['memory_nodes']))
    return ['numactl', f"--physcpubind={placement['mask']}", f'--membind={nodes}', '--', *command]
