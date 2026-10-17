from pathlib import Path
import argparse,subprocess,hashlib,json,tarfile,io
parser=argparse.ArgumentParser(description='Prepare a separate instrumented copy of fixed parent 2dfc0e86; never edits the current runtime.')
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
root=Path(__file__).resolve().parents[1]; out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
paths=['Cargo.toml','Cargo.lock','.cargo','crates','vendor','plugin','scripts/audit_binary.py','tests/mock_spi']
data=subprocess.check_output(['git','-C',str(root),'archive','2dfc0e86','--',*paths])
with tarfile.open(fileobj=io.BytesIO(data)) as archive: archive.extractall(out,filter='data')
probe=(root/'bench/scheduler_probe.rs').read_text()
(out/'crates/fereg-core/src/scheduler_probe.rs').write_text(probe)
p=out/'crates/fereg-core/src/diagnostics.rs';p.write_text(p.read_text()+'\n#[path="scheduler_probe.rs"]\nmod scheduler_probe;\npub use scheduler_probe::SchedulerProbe;\n')
p=out/'crates/fereg-core/src/fe/borrowed_map.rs';s=p.read_text();needle='    ) -> Result<CertifiedBorrowedMap> {\n        validate_config(config)?;';assert s.count(needle)==1;s=s.replace(needle,'    ) -> Result<CertifiedBorrowedMap> {\n        let _probe = crate::diagnostics::SchedulerProbe::new("absorb");\n        validate_config(config)?;',1);p.write_text(s)
p=out/'crates/fereg-plugin/src/ffi.rs';s=p.read_text();needle='    let outcome = catch_unwind(AssertUnwindSafe(|| {\n        pin_plugin_module()?;';assert s.count(needle)==1;s=s.replace(needle,'    let _probe = fereg_core::diagnostics::SchedulerProbe::new("native");\n'+needle,1);p.write_text(s)
manifest={str(p.relative_to(out)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.rglob('*')) if p.is_file()}
out.with_name(out.name+'-manifest.json').write_text(json.dumps(manifest,sort_keys=True,indent=2)+'\n')
