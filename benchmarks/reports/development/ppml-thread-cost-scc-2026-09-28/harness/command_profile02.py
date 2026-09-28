"""Command-interval CPU sampler used only by this isolated diagnostic runner."""
from pathlib import Path
import subprocess, signal, time, json

def start(pid, output):
    stderr=(output/'perf.stderr').open('w')
    argv=['perf','record','-F','997','-e','cycles:u','--call-graph','dwarf,8192','-o',str(output/'perf.data'),'-p',str(pid)]
    child=subprocess.Popen(argv,stdout=subprocess.DEVNULL,stderr=stderr)
    attached=time.monotonic()
    time.sleep(.05)
    if child.poll() is not None:
        stderr.close()
        raise RuntimeError('perf attach failed: '+(output/'perf.stderr').read_text())
    return child,stderr,dict(pid=pid,argv=argv,attached_monotonic=attached,released_monotonic=time.monotonic())

def stop(state, output):
    child,stderr,receipt=state
    requested_stop = child.poll() is None
    receipt["requested_stop_monotonic"] = time.monotonic()
    receipt["requested_sigint"] = requested_stop
    if requested_stop:child.send_signal(signal.SIGINT)
    child.wait(timeout=30);stderr.close()
    receipt.update(stopped_monotonic=time.monotonic(),process_rc=child.returncode,diagnostic_only=True)
    (output/'perf-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    allowed = (0, -signal.SIGINT, 128+signal.SIGINT) if requested_stop else (0,)
    if child.returncode not in allowed:raise RuntimeError('perf failed: '+(output/'perf.stderr').read_text())
    if not (output/'perf.data').is_file() or (output/'perf.data').stat().st_size == 0:
        raise RuntimeError('perf emitted no sample file')
