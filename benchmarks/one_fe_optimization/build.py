"""Isolated, source-hashed arm64 builds for the one-FE development campaign."""
import argparse
import datetime
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile

REPO = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(root, name, revision, overlays, profile_preparation=False, profile_completion=False, profile_breadth=False, profile_statistics=False, profile_slopes=False, profile_inference=False, profile_recovery=False, profile_schur=False, schur_narrow_cap=None, serial_lsmr_window=False, direct_ols_lsmr=False, cycle_ols_lsmr=False, profile_lsmr_transport=False, profile_ppml_session=False, threaded_lsmr_window=False, profile_low_qr=False, profile_serial_sandwich=False, profile_owned_fe=False):
    assert not (direct_ols_lsmr and cycle_ols_lsmr), 'Choose one isolated routing experiment.'
    root = root.resolve()
    source = root / name
    source.mkdir(parents=True, exist_ok=False)
    commit = subprocess.check_output(["/opt/homebrew/bin/git", "rev-parse", revision], cwd=REPO, text=True).strip()
    archive = subprocess.check_output(["/opt/homebrew/bin/git", "archive", commit,
        "Cargo.toml", "Cargo.lock", "crates", "plugin", "stata", "tests", "scripts", "references.lock.toml"], cwd=REPO)
    with tarfile.open(fileobj=io.BytesIO(archive)) as bundle:
        bundle.extractall(source, filter="data")
    for relative in overlays:
        path = source / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes((REPO / relative).read_bytes())
    if serial_lsmr_window:
        from serial_lsmr_window import instrument
        instrument(source)
    if threaded_lsmr_window:
        assert not serial_lsmr_window, 'Choose one isolated window experiment.'
        import importlib.util
        spec = importlib.util.spec_from_file_location("threaded_window", REPO / "benchmarks/next_subsystem/threaded_lsmr_window.py")
        probes = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probes)
        probes.instrument(source)
    if direct_ols_lsmr:
        from direct_ols_lsmr import instrument
        instrument(source)
    if cycle_ols_lsmr:
        from cycle_ols_lsmr import instrument
        instrument(source)
    if schur_narrow_cap is not None:
        assert 0 < schur_narrow_cap <= 64
        path = source / 'crates/fereg-core/src/fe/borrowed_map.rs'
        original = 'try_absorb_ols_schur_narrow_at_cap(design, weights, values, config, OLS_SCHUR_MAX_ITERATIONS)'
        text = path.read_text()
        assert text.count(original) == 1, 'Narrow OLS entry point changed; inspect before instrumenting.'
        path.write_text(text.replace(original,
            f'try_absorb_ols_schur_narrow_at_cap(design, weights, values, config, {schur_narrow_cap})'))
    if profile_preparation:
        from prepare_profile import instrument
        instrument(source)
    if profile_completion:
        from completion_profile import instrument
        instrument(source)
    if profile_breadth:
        import importlib.util
        spec = importlib.util.spec_from_file_location("breadth_probes", REPO / "benchmarks/one_fe_breadth/profile.py")
        probes = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probes)
        probes.instrument(source)
    if profile_statistics:
        from statistics_profile import instrument
        instrument(source)
    if profile_slopes:
        import importlib.util
        spec = importlib.util.spec_from_file_location("slope_probes", REPO / "benchmarks/one_fe_profile/slope_instrument.py")
        probes = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probes)
        probes.instrument(source)
    if profile_inference:
        import importlib.util
        spec = importlib.util.spec_from_file_location("inference_probes", REPO / "benchmarks/one_fe_profile/inference_instrument.py")
        probes = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probes)
        probes.instrument(source)
    if profile_recovery:
        import importlib.util
        spec = importlib.util.spec_from_file_location("recovery_probes", REPO / "benchmarks/one_fe_profile/recovery_instrument.py")
        probes = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probes)
        probes.instrument(source)
    if profile_schur:
        import importlib.util
        spec = importlib.util.spec_from_file_location("schur_probes", REPO / "benchmarks/one_fe_profile/schur_instrument.py")
        probes = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probes)
        probes.instrument(source)
    if profile_lsmr_transport or profile_ppml_session or profile_low_qr or profile_serial_sandwich or profile_owned_fe:
        import importlib.util
        spec = importlib.util.spec_from_file_location("lsmr_transport_probes", REPO / "benchmarks/next_subsystem/profile.py")
        probes = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probes)
        if profile_lsmr_transport:
            probes.instrument(source)
        if profile_ppml_session:
            probes.instrument_ppml(source)
        if profile_low_qr:
            probes.instrument_low_qr(source)
        if profile_owned_fe:
            probes.instrument_owned_fe(source)
        if profile_serial_sandwich:
            probes.instrument_serial_sandwich(source)
    hashes = {str(p.relative_to(source)): sha(p) for p in sorted(source.rglob("*")) if p.is_file()}
    target = root / ("cargo-" + name)
    env = dict(os.environ, DEVELOPER_DIR="/Library/Developer/CommandLineTools",
               CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(target),
               FEREG_GIT_COMMIT=commit[:8], FEREG_BUILD_DATE=os.environ.get("FEREG_BUILD_DATE", datetime.datetime.now(datetime.timezone.utc).date().isoformat()))
    (source / "target/native").mkdir(parents=True)
    (source / "dist").mkdir()
    with (root / ("build-" + name + ".log")).open("w") as log:
        def run(command):
            subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        run(["cargo", "build", "--locked", "--offline", "--release", "-p", "fereg-plugin", "--target", "aarch64-apple-darwin"])
        for unit in ("stplugin", "stata_bridge"):
            run(["clang", "-std=c11", "-O3", "-fPIC", "-Wall", "-Wextra", "-Werror", "-DSYSTEM=APPLEMAC",
                 "-target", "arm64-apple-macos11", "-Iplugin", "-c", f"plugin/{unit}.c", "-o", f"target/native/{unit}.o"])
        library = target / "aarch64-apple-darwin/release/libfereg_plugin.a"
        run(["clang", "-bundle", "-target", "arm64-apple-macos11", "-Wl,-dead_strip",
             "-Wl,-exported_symbols_list,plugin/exports_macos.txt", "target/native/stplugin.o", "target/native/stata_bridge.o",
             str(library), "-framework", "Security", "-framework", "CoreFoundation", "-framework", "SystemConfiguration",
             "-liconv", "-lresolv", "-o", "dist/fereg_macos.plugin"])
        run(["codesign", "--force", "--sign", "-", "dist/fereg_macos.plugin"])
        run(["bash", "tests/mock_spi/run_actual_unix_plugin_test.sh", "dist/fereg_macos.plugin"])
    identity = dict(base_commit=commit, overlays=overlays, source=hashes, plugin=sha(source / "dist/fereg_macos.plugin"),
                    rust_archive=sha(library), abi="pass", target="aarch64-apple-darwin")
    if schur_narrow_cap is not None:
        identity['experiment'] = dict(schur_narrow_continuation_cap=schur_narrow_cap,
            scope='Narrow OLS entry point only; shared and wide Schur caps are unchanged.')
    if serial_lsmr_window:
        identity.setdefault('experiment', {})['serial_lsmr_window'] = dict(
            active_rhs=1, scope='Captured-source generic serial transport/admission experiment; no production or shared-routing acceptance.',
            instrument_sha256=sha(REPO / 'benchmarks/one_fe_optimization/serial_lsmr_window.py'))
    if threaded_lsmr_window:
        identity.setdefault('experiment', {})['threaded_lsmr_window'] = dict(
            scope='Captured-source medium threaded RHS transport/admission experiment; no production acceptance.',
            instrument_sha256=sha(REPO / 'benchmarks/next_subsystem/threaded_lsmr_window.py'))
    if direct_ols_lsmr:
        identity.setdefault('experiment', {})['direct_ols_lsmr'] = dict(
            scope='Captured-source public fast two-intercept OLS diagnostic; automatic LSMR tolerances and admission unchanged.',
            instrument_sha256=sha(REPO / 'benchmarks/one_fe_optimization/direct_ols_lsmr.py'))
    if cycle_ols_lsmr:
        identity.setdefault('experiment', {})['cycle_ols_lsmr'] = dict(
            scope='Captured-source exact-cycle routing experiment; production routing unchanged.',
            instrument_sha256=sha(REPO / 'benchmarks/one_fe_optimization/cycle_ols_lsmr.py'),
            detector_sha256=sha(REPO / 'benchmarks/one_fe_optimization/cycle_ols_lsmr.rs'))
    if profile_preparation:
        identity.setdefault("diagnostics", []).append("preparation_phases")
    if profile_completion:
        identity.setdefault("diagnostics", []).append("completion_phases")
    if profile_breadth:
        identity.setdefault("diagnostics", []).append("breadth_phases")
    if profile_statistics:
        identity.setdefault("diagnostics", []).append("weighted_statistics_phases")
    if profile_slopes:
        identity.setdefault("diagnostics", []).append("slope_phases")
    if profile_inference:
        identity.setdefault("diagnostics", []).append("inference_phases")
    if profile_recovery:
        identity.setdefault("diagnostics", []).append("recovery_fallbacks")
    if profile_schur:
        identity.setdefault("diagnostics", []).append("schur_pilot_steps")
    if profile_lsmr_transport:
        identity.setdefault("diagnostics", []).append("lsmr_transport")
    if profile_ppml_session:
        identity.setdefault("diagnostics", []).append("ppml_session")
    if profile_low_qr:
        identity.setdefault("diagnostics", []).append("low_qr_phases")
    if profile_owned_fe:
        identity.setdefault("diagnostics", []).append("owned_fe_phases")
    if profile_serial_sandwich:
        identity.setdefault("diagnostics", []).append("serial_sandwich_phases")
    (root / ("identity-" + name + ".json")).write_text(json.dumps(identity, indent=2) + "\n")
    print("BUILD_PASS", name, identity["plugin"], flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("name")
    p.add_argument("--revision", default="HEAD")
    p.add_argument("--overlay", nargs="*", default=[])
    p.add_argument("--profile-preparation", action="store_true")
    p.add_argument("--profile-completion", action="store_true")
    p.add_argument("--profile-breadth", action="store_true")
    p.add_argument("--profile-statistics", action="store_true")
    p.add_argument("--profile-slopes", action="store_true")
    p.add_argument("--profile-inference", action="store_true")
    p.add_argument("--profile-recovery", action="store_true")
    p.add_argument("--profile-schur", action="store_true")
    p.add_argument("--profile-lsmr-transport", action="store_true")
    p.add_argument("--profile-ppml-session", action="store_true")
    p.add_argument("--profile-low-qr", action="store_true")
    p.add_argument("--profile-serial-sandwich", action="store_true")
    p.add_argument("--profile-owned-fe", action="store_true")
    p.add_argument("--schur-narrow-cap", type=int, choices=range(1, 65),
                   help="isolated narrow OLS cap experiment; never modifies the checkout")
    p.add_argument("--serial-lsmr-window", action="store_true",
                   help="isolated serial RHS-window experiment; never modifies production source")
    p.add_argument("--threaded-lsmr-window", action="store_true",
                   help="isolated medium threaded RHS-window experiment; never modifies production source")
    p.add_argument("--direct-ols-lsmr", action="store_true",
                   help="isolated automatic-LSMR route diagnostic; never modifies production source")
    p.add_argument("--cycle-ols-lsmr", action="store_true",
                   help="isolated exact-cycle routing experiment; never modifies production source")
    a = p.parse_args()
    build(a.root, a.name, a.revision, a.overlay, a.profile_preparation, a.profile_completion, a.profile_breadth, a.profile_statistics, a.profile_slopes, a.profile_inference, a.profile_recovery, a.profile_schur, a.schur_narrow_cap, a.serial_lsmr_window, a.direct_ols_lsmr, a.cycle_ols_lsmr, a.profile_lsmr_transport, a.profile_ppml_session, a.threaded_lsmr_window, a.profile_low_qr, a.profile_serial_sandwich, a.profile_owned_fe)
