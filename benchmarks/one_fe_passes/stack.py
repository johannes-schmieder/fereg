"""Diagnostic-only relink: parallel validation plus accessor/tile variants.

The pthread tile adapter deliberately measures transport feasibility. Its scratch
and worker must move under Rust admission/team ownership before any adoption.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

REPO = Path(__file__).resolve().parents[2]


def main(root, accessor):
    root, accessor = root.resolve(), accessor.resolve()
    archive = root / 'build/aarch64-apple-darwin/release/libfereg_plugin.a'
    expected = json.loads((root / 'identity-final.json').read_text())['rust_archive']
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == expected
    probe = (REPO / 'benchmarks/one_fe_passes/pipeline_probe.c').read_text()
    helper = probe[probe.index('struct tile'):probe.index('static uint64_t hash_values')]
    a, b = helper.index('static double seconds'), helper.index('static void transpose')
    helper = helper[:a] + helper[b:]
    helper = helper.replace('static int read_values(int mode, int n, int k, double **columns, double *buffer)', 'static int pipeline_read(int mode, int n, int k, double **columns, double *buffer, const ST_int *variables, int first_observation)')
    a, b = helper.index('    if (!reader'), helper.index('    if (mode<2)')
    helper = helper[:a] + helper[b:]
    helper = helper.replace('reader(j+1,i+1,', 'reader(variables[j],first_observation+i,')
    helper = helper.replace('reader(j+1,first+i+1,', 'reader(variables[j],first_observation+first+i,')
    for name in ['fastfinal', 'pipelinefit']:
        source = root / name
        source.mkdir()
        for folder in ['plugin', 'stata', 'tests/mock_spi']:
            shutil.copytree(root / 'final' / folder, source / folder)
        for folder in ['dist', 'build']:
            (source / folder).mkdir()
        bridge = (accessor / 'fast/plugin/stata_bridge.c').read_text()
        if name == 'pipelinefit':
            bridge = bridge.replace('#include <string.h>', '#include <string.h>\n#include <stdlib.h>')
            anchor = bridge.index('ST_int fereg_spi_vdata_columns(')
            bridge = bridge[:anchor] + helper + '\n' + bridge[anchor:]
            marker = '    *missing_value = SV_missval;'
            assert bridge.count(marker) == 1
            bridge = bridge.replace(marker, marker + '''
    /* Experiment only: bounded owned transport buffers, joined before return.
       Production adoption requires Rust memory admission and worker policy. */
    if (variable_count >= 8 && variable_count <= 64 && observation_count >= 16384) {
        double *tiles = malloc(2 * 4096 * (size_t)variable_count * sizeof(double));
        if (tiles == NULL) return 909;
        rc = pipeline_read(7, observation_count, variable_count, columns, tiles,
                           variables, first_observation);
        free(tiles);
        return rc;
    }''')
        (source / 'plugin/stata_bridge.c').write_text(bridge)
        test = source / 'tests/mock_spi/actual_plugin_loader_test.c'
        test.write_text(test.read_text().replace('    plugin.safevdata = mock_vdata;', '    plugin.safevdata = mock_vdata;\n    plugin.vdata = mock_vdata;'))
        env = dict(os.environ, DEVELOPER_DIR='/Library/Developer/CommandLineTools')
        with (root / ('build-' + name + '.log')).open('w') as log:
            def run(command):
                subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
            for unit in ['stplugin', 'stata_bridge']:
                run(['/usr/bin/clang', '-std=c11', '-O3', '-fPIC', '-Wall', '-Wextra', '-Werror', '-DSYSTEM=APPLEMAC', '-target', 'arm64-apple-macos11', '-Iplugin', '-c', f'plugin/{unit}.c', '-o', f'build/{unit}.o'])
            run(['/usr/bin/clang', '-bundle', '-target', 'arm64-apple-macos11', '-Wl,-dead_strip', '-Wl,-exported_symbols_list,plugin/exports_macos.txt', 'build/stplugin.o', 'build/stata_bridge.o', str(archive), '-framework', 'Security', '-framework', 'CoreFoundation', '-framework', 'SystemConfiguration', '-liconv', '-lresolv', '-o', 'dist/fereg_macos.plugin'])
            run(['/usr/bin/codesign', '--force', '--sign', '-', 'dist/fereg_macos.plugin'])
            run(['bash', 'tests/mock_spi/run_actual_unix_plugin_test.sh', 'dist/fereg_macos.plugin'])
        (root / ('identity-' + name + '.json')).write_text(json.dumps(dict(plugin=hashlib.sha256((source / 'dist/fereg_macos.plugin').read_bytes()).hexdigest(), rust_archive=expected, bridge=hashlib.sha256((source / 'plugin/stata_bridge.c').read_bytes()).hexdigest(), abi='pass'), indent=2)+'\n')
        print('STACK_BUILD_PASS', name, flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('root', type=Path)
    p.add_argument('accessor', type=Path)
    a = p.parse_args()
    main(a.root, a.accessor)
