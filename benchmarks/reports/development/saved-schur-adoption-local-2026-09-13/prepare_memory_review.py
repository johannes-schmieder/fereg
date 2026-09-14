import os
from pathlib import Path
import shutil,json,hashlib,difflib
root=Path(__file__).resolve().parent;src=root/'candidate';dst=root/'memory-diagnostic'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((root/'built-identities.json').read_text())
assert all(sha(src/p)==h for p,h in ids['candidate_inputs'].items())
shutil.copytree(src,dst)
p=dst/'crates/fereg-core/src/lib.rs'
p.write_text(p.read_text()+'''
// Isolated allocation diagnostic only; never part of the adoption candidate.
pub static OLS_ALLOCATION_AUDIT_STAGE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub struct OlsAllocationAuditGuard(usize);
impl OlsAllocationAuditGuard {
    pub fn enter(stage: usize) -> Self { Self(OLS_ALLOCATION_AUDIT_STAGE.swap(stage, std::sync::atomic::Ordering::Relaxed)) }
}
impl Drop for OlsAllocationAuditGuard {
    fn drop(&mut self) { OLS_ALLOCATION_AUDIT_STAGE.store(self.0, std::sync::atomic::Ordering::Relaxed); }
}
''')
p=dst/'crates/fereg-core/src/ols/prepared_fit.rs';s=p.read_text()
needle=') -> Result<CertifiedBorrowedMap> {\n    if initial {';assert s.count(needle)==1
s=s.replace(needle,') -> Result<CertifiedBorrowedMap> {\n    let _audit_stage = crate::OlsAllocationAuditGuard::enter(if initial { 1 } else if final_residual { 2 } else { 3 });\n    eprintln!("OLS_ALLOCATION_BUDGET|initial={initial}|final={final_residual}|rhs={}|budget={:?}", values.ncols(), config.solver_memory_budget);\n    if initial {')
needle='fn build_fit_fe_absorber_legacy';i=s.index(needle);j=s.index(') -> Result<FitFeAbsorber<\'a>> {',i)+len(') -> Result<FitFeAbsorber<\'a>> {');s=s[:j]+'\n    let _audit_stage = crate::OlsAllocationAuditGuard::enter(4);'+s[j:];p.write_text(s)
p=dst/'crates/fereg-core/src/ols/prepared_categorical_data.rs';s=p.read_text();i=s.index('fn recover_absorbed_effects_fast_with_weights_inner');j=s.index(') -> Result<RecoveredCategoricalEffects> {',i)+len(') -> Result<RecoveredCategoricalEffects> {');s=s[:j]+'\n        let _audit_stage = crate::OlsAllocationAuditGuard::enter(5);'+s[j:]
needle='        let fits = |bytes| memory_budget.is_none_or(|budget| budget.fits(bytes));';assert s.count(needle)==1;s=s.replace(needle,'        eprintln!("OLS_RECOVERY_BUDGET|retained={}|budget={memory_budget:?}|model={recovery_memory:?}", retained.is_some());\n'+needle);p.write_text(s)
p=dst/'crates/fereg-plugin/src/ffi.rs';s=p.read_text();needle='    record_allocation_on(&ALLOCATION_CURRENT_BYTES, &ALLOCATION_PEAK_BYTES, bytes);';assert s.count(needle)==1;s=s.replace(needle,needle+'''
    let audit_stage = fereg_core::OLS_ALLOCATION_AUDIT_STAGE.load(Ordering::Relaxed);
    OLS_AUDIT_PEAKS[audit_stage].fetch_max(ALLOCATION_CURRENT_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
''');s+='''
static OLS_AUDIT_PEAKS: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];
pub(crate) struct OlsAllocationAudit(u64);
impl OlsAllocationAudit {
    pub(crate) fn begin() -> Self {
        for peak in &OLS_AUDIT_PEAKS { peak.store(0, Ordering::Relaxed); }
        Self(ALLOCATION_CURRENT_BYTES.load(Ordering::Relaxed))
    }
}
impl Drop for OlsAllocationAudit {
    fn drop(&mut self) {
        for (stage, peak) in OLS_AUDIT_PEAKS.iter().enumerate() {
            eprintln!("OLS_ALLOCATION_AUDIT|stage={stage}|baseline={}|absolute_peak={}|delta={}", self.0, peak.load(Ordering::Relaxed), peak.load(Ordering::Relaxed).saturating_sub(self.0));
        }
    }
}
''';p.write_text(s)
p=dst/'crates/fereg-plugin/src/estimate.rs';s=p.read_text();needle='pub fn estimate_ols_test(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {';assert s.count(needle)==1;s=s.replace(needle,needle+'\n    let _allocation_audit = crate::ffi::OlsAllocationAudit::begin();');p.write_text(s)
changed=[p for p,h in ids['candidate_inputs'].items() if sha(dst/p)!=h]
(root/'memory-diagnostic.patch').write_text(''.join(''.join(difflib.unified_diff((src/p).read_text().splitlines(True),(dst/p).read_text().splitlines(True),fromfile='a/'+p,tofile='b/'+p)) for p in changed))
(root/'memory-diagnostic-identities.json').write_text(json.dumps({'parent_plugin':ids['candidate_plugin'],'inputs':{p:sha(dst/p) for p in ids['candidate_inputs']},'changed_inputs':changed,'patch_sha256':sha(root/'memory-diagnostic.patch'),'stages':{'0':'other caller work','1':'initial absorption','2':'final residual absorption','3':'other absorption','4':'legacy solver setup','5':'saved-effect recovery'},'purpose':'Whole live Rust allocation by active scope; diagnostic only, excluded from timing aggregates.'},indent=2)+'\n')
s=(root/'run_build.py').read_text().replace("root/'candidate'","root/'memory-diagnostic'").replace("source.name=='candidate'","source.name=='memory-diagnostic'").replace("'prebuild-identities.json'","'memory-diagnostic-identities.json'").replace("ids['candidate_inputs']","ids['inputs']").replace("'build-isolated.log'","'build-memory-diagnostic.log'").replace("'built-identities.json'","'memory-diagnostic-built.json'")
(root/'build_memory_review.py').write_text(s)
print('Diagnostic prepared',changed)
