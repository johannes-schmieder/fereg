"""GLFixedEffectModels.jl and reghdfejl arms: schedule, pins, conventions and environment layers."""
import json
import os
from pathlib import Path
import sys
import tempfile
import tomllib
import unittest

import numpy as np
from scipy.special import gammaln, xlogy

HERE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HERE))
import campaign
from campaign import POLICY, absorbed_rank, arms, configurations, fe_span_rank, make_dataset
from campaign_oracle import oracle


class ComparatorArms(unittest.TestCase):
    def test_schedule(self):
        for cell in configurations()+configurations(True):
            selected = arms(cell)
            ppml_fe = cell['model'] == 'ppml' and cell['effects'] > 0
            self.assertEqual('glfixedeffects' in selected, ppml_fe, cell['id'])
            self.assertEqual('glfixedeffects-strict' in selected, ppml_fe and 'fixest-strict' in selected, cell['id'])
            self.assertFalse({'julia', 'julia-strict'} & set(selected) and cell['model'] == 'ppml')
            # reghdfejl follows reghdfe/ppmlhdfe; its nonlinear route needs absorbed
            # effects and cannot save them.
            stata = 'reghdfe' in selected or 'ppmlhdfe' in selected
            self.assertEqual('reghdfejl' in selected,
                             stata and (cell['model'] == 'ols' or (cell['effects'] > 0 and not cell['save_fe'])), cell['id'])
            self.assertTrue(all(arm in campaign.PACKING_SECONDS for arm in selected))
        from campaign_report import ARM_NAMES, STATA_ARMS
        self.assertTrue({a for c in configurations() for a in arms(c)} <= set(ARM_NAMES))
        self.assertIn('reghdfejl', STATA_ARMS)
        self.assertIn('reghdfejl', campaign.STATA_REFERENCES)
        for arm in ['reghdfejl', 'glfixedeffects', 'glfixedeffects-strict']:
            self.assertEqual(campaign.command_timeout(arm), 3600)

    def test_pins_agree(self):
        pins = json.loads((HERE/'reghdfejl/pins.json').read_text())
        bridge = POLICY['reghdfejl']
        self.assertEqual((bridge['commit'], bridge['bridge_commit'], bridge['julia'], bridge['FixedEffectModels']),
                         (pins['reghdfejl_commit'], pins['bridge_commit'], pins['julia_version'],
                          pins['fixedeffectmodels_version']))
        for name in ['reghdfejl.ado', 'reghdfejl_manifest.toml']:
            self.assertIn(pins['reghdfejl_commit'], pins['files'][name]['url'])
        self.assertIn(pins['bridge_commit'], pins['files']['jl.ado']['url'])
        manifest = tomllib.loads((HERE/'julia/ppml/Manifest.toml').read_text())
        project = tomllib.loads((HERE/'julia/ppml/Project.toml').read_text())
        version = lambda name: manifest['deps'][name][0]['version']
        ppml = POLICY['julia_ppml']
        self.assertEqual(manifest['julia_version'], ppml['julia'])
        self.assertEqual(project['compat']['GLFixedEffectModels'], '='+ppml['GLFixedEffectModels'])
        self.assertEqual((version('GLFixedEffectModels'), version('FixedEffectModels')),
                         (ppml['GLFixedEffectModels'], ppml['FixedEffectModels']))
        # The OLS arm keeps FixedEffectModels 2.1.0; GLFixedEffectModels 0.5.6 requires 1.x.
        ols = tomllib.loads((HERE/'julia/Manifest.toml').read_text())
        self.assertEqual(ols['deps']['FixedEffectModels'][0]['version'], POLICY['julia']['FixedEffectModels'])
        self.assertNotEqual(ppml['FixedEffectModels'], POLICY['julia']['FixedEffectModels'])
        self.assertEqual(set(POLICY['julia_ppml_strict']), {'center_tol', 'dev_tol'})

    def test_level_counts_map_to_the_fe_span(self):
        cell = next(c for c in configurations(True) if c['model'] == 'ppml' and c['design'] == 'akm-clustered')
        with tempfile.TemporaryDirectory() as directory:
            frame, _ = make_dataset(cell, Path(directory)/'input.dta')
        levels = frame.g1.nunique()+frame.g2.nunique()
        span = oracle(frame, cell)['pairwise_rank']
        self.assertEqual(fe_span_rank(frame, 2), span)
        for arm in ['glfixedeffects', 'glfixedeffects-strict', 'reghdfejl']:
            for vce in ['robust', 'cluster']:
                metadata = dict(df_a=str(levels), arm=arm, native_arm='0', covariance_policy=f'ppml-{vce}-K.adj=FALSE')
                self.assertEqual(absorbed_rank(metadata, dict(cell, vce=vce), frame), span)
            with self.assertRaisesRegex(ValueError, 'level count'):
                absorbed_rank(dict(metadata, df_a=str(levels-1)), cell, frame)
        selected = np.ones(len(frame), dtype=bool)
        selected[frame.g1 == frame.g1.iloc[0]] = False
        metadata['df_a'] = str(frame.g1[selected].nunique()+frame.g2[selected].nunique())
        self.assertEqual(absorbed_rank(metadata, cell, frame, selected), fe_span_rank(frame[selected], 2))

    def test_poisson_likelihood_from_the_deviance(self):
        # Both arms report -deviance/2 + sum(y log y - y - loggamma(y+1)).
        rng = np.random.default_rng(7)
        y = np.where(rng.random(500) < .3, 0., rng.gamma(2., 1.5, 500))
        mu = rng.gamma(3., 1., 500)
        deviance = 2*np.sum(xlogy(y, y/mu)-(y-mu))
        converted = -deviance/2+np.sum(xlogy(y, y)-y-gammaln(y+1))
        self.assertAlmostEqual(converted, np.sum(xlogy(y, mu)-mu-gammaln(y+1)), delta=1e-9*abs(converted))


class ComparatorEnvironment(unittest.TestCase):
    def test_private_layer_links_frozen_trees_and_copies_caches(self):
        import campaign_environment as environment
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root/'origin'
            files = ['julia-ppml/Project.toml', 'julia-ppml-depot/packages/A/x.jl', 'julia-ppml-depot/artifacts/a',
                     'julia-ppml-depot/compiled/v1.11/A/a.ji', 'reghdfejl/references/reghdfejl.ado',
                     f'reghdfejl/{environment.RUNTIME}/bin/julia', 'reghdfejl/depot/packages/B/y.jl',
                     'reghdfejl/depot/artifacts/b', 'reghdfejl/depot/environments/reghdfejl/Project.toml',
                     'reghdfejl/depot/compiled/v1.12/B/b.ji', *environment.VERSIONS]
            for name in files:
                (source/name).parent.mkdir(parents=True, exist_ok=True)
                (source/name).write_text(name)
            os.utime(source/'reghdfejl/references/reghdfejl.ado', ns=(10**18, 10**18))
            target = root/'run'
            target.mkdir()
            bridge = environment.layer(source, target)
            for name in environment.VERSIONS:
                (target/name).write_text(name)
            self.assertTrue((target/'julia-ppml-depot/packages').is_symlink())
            self.assertTrue((bridge/'depot/packages').is_symlink() and (bridge/environment.RUNTIME).is_symlink())
            self.assertFalse((bridge/'depot/compiled').is_symlink() or (bridge/'references').is_symlink())
            self.assertEqual((bridge/'references/reghdfejl.ado').stat().st_mtime_ns, 10**18)
            self.assertIn(f'+{environment.PINS["julia_version"]}', (bridge/'bin/julia').read_text())
            listed = {str(p.relative_to(target)) for p in environment.frozen_files(target)}
            self.assertIn('reghdfejl/depot/packages/B/y.jl', listed)
            self.assertFalse(any('/compiled/' in name for name in listed))


class CampaignStaging(unittest.TestCase):
    def test_stage_records_inputs_and_hardware_launcher(self):
        import hashlib
        import io
        import tarfile
        from campaign_submit import stage
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root/'source.tar'
            with tarfile.open(archive, 'w') as tar:
                for name in ['bench/scc/setup.sh', 'bench/scc/job.sh', 'bench/campaign.json']:
                    data = (HERE.parent/name).read_bytes()
                    info = tarfile.TarInfo(name)
                    info.size = len(data)
                    tar.addfile(info, io.BytesIO(data))
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            (root/'source.json').write_text(json.dumps(dict(commit='c'*40, dirty=False, archive_sha256=digest)))
            (root/'frozen.json').write_text('{}')
            (root/'policy.json').write_text('{"bundles": 1}')
            run = root/'granite'
            stage(run, archive, root/'source.json', root/'frozen.json', root/'policy.json', 'prefix', 32, 'omp32', '6737P')
            job = (run/'job.sh').read_text()
            self.assertIn('#$ -pe omp32 32\n', job)
            self.assertIn('#$ -l num_proc=32,cpu_type=6737P\n', job)
            listed = dict(reversed(line.split('  ', 1)) for line in (run/'input/SHA256SUMS').read_text().splitlines())
            self.assertEqual(set(listed), {'input/source.tar', 'input/source.json', 'input/frozen-inputs.json',
                                           'input/policy.json', 'setup.sh', 'job.sh'})
            self.assertTrue(all(hashlib.sha256((run/name).read_bytes()).hexdigest() == value for name, value in listed.items()))
            context = json.loads((run/'context.json').read_text())
            self.assertEqual(context['resources']['production']['cpu_type'], '6737P')
            self.assertEqual(context['launcher_sha256']['job.sh'], listed['job.sh'])
            with self.assertRaises(FileExistsError):
                stage(run, archive, root/'source.json', root/'frozen.json', None, 'prefix', 28, 'omp28')


class CandidatePlugin(unittest.TestCase):
    def test_setup_admits_only_a_candidate_with_this_frontend(self):
        import hashlib
        import subprocess
        text = (HERE/'scc/setup.sh').read_text()
        start = text.index('CAMPAIGN_PLUGIN:-build}" == candidate')
        snippet = text[text.index("<<'PY'\n", start)+7:text.index('\nPY\n', start)]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, source = root/'package', root/'source'
            for tree in [package/'dist', package/'stata', source/'dist', source/'stata']:
                tree.mkdir(parents=True)
            (package/'dist/fereg_linux.plugin').write_bytes(b'candidate')
            (package/'dist/fereg_linux.plugin.sha256').write_text(
                hashlib.sha256(b'candidate').hexdigest()+'  fereg_linux.plugin\n')
            (source/'dist/fereg_linux.plugin').write_bytes(b'stale')
            for tree in [package, source]:
                (tree/'stata/fereg.ado').write_text('program fereg\nend\n')
            (package/'stata/fereg.sthlp').write_text('newer help')
            run = lambda: subprocess.run([sys.executable, '-c', snippet, str(package), str(source)],
                                         capture_output=True, text=True)
            result = run()
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((source/'dist/fereg_linux.plugin').read_bytes(), b'candidate')
            (source/'dist/fereg_linux.plugin').write_bytes(b'stale')
            (source/'stata/fereg.ado').write_text('program fereg\n  version 19\nend\n')
            self.assertNotEqual(run().returncode, 0)
            self.assertEqual((source/'dist/fereg_linux.plugin').read_bytes(), b'stale')


if __name__ == '__main__':
    unittest.main()
