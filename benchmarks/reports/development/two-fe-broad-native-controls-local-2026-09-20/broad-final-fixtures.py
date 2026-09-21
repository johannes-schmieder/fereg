from pathlib import Path
import sys
sys.path.insert(0, str(Path.cwd()))
from benchmarks.stata_input_probe.fixtures import main
main(Path('target/two-fe-broad-20260920'), 'usefulpilot', [
    'development/ols_narrow_weighted_schur.do',
    'development/ols_native_input.do',
], label='-broad')
