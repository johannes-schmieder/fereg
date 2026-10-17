// Diagnostic-copy instrumentation only. Never linked into the scored artifact.
// Procfs reads perturb timings: these observations diagnose scheduler association.
use std::sync::{atomic::{AtomicUsize, Ordering}, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub struct SchedulerProbe {
    label: &'static str,
    id: usize,
    enabled: bool,
}
impl SchedulerProbe {
    pub fn new(label: &'static str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let value = Self {label, id: NEXT.fetch_add(1, Ordering::Relaxed),
                          enabled: super::solver_trace_enabled()};
        value.snapshot("start");
        value
    }
    fn snapshot(&self, edge: &str) {
        if !self.enabled { return; }
        static CLOCK: OnceLock<Instant> = OnceLock::new();
        let began = Instant::now();
        let ns = CLOCK.get_or_init(Instant::now).elapsed().as_nanos();
        let wall = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |x|x.as_nanos());
        let current = std::fs::read_link("/proc/thread-self").map(|p|p.display().to_string()).unwrap_or_else(|_|"unavailable".into());
        let process = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
        let fields: Vec<_> = process.rsplit_once(") ").map_or("",|(_,s)|s).split_whitespace().collect();
        let user = fields.get(11).copied().unwrap_or("unavailable");
        let sys = fields.get(12).copied().unwrap_or("unavailable");
        eprintln!("FEREG_SCHED|scope={}|id={}|edge={edge}|record=boundary|ns={ns}|wall={wall}|caller={current}|process_user_ticks={user}|process_system_ticks={sys}",self.label,self.id);
        let mut tasks: Vec<_> = std::fs::read_dir("/proc/self/task").into_iter().flatten()
            .filter_map(|entry|entry.ok()).filter_map(|entry|entry.file_name().to_str()?.parse::<usize>().ok()).collect();
        tasks.sort_unstable();
        for tid in tasks {
            let root = format!("/proc/self/task/{tid}");
            let sched = std::fs::read_to_string(format!("{root}/schedstat")).unwrap_or_default();
            let sched: Vec<_> = sched.split_whitespace().collect();
            let stat = std::fs::read_to_string(format!("{root}/stat")).unwrap_or_default();
            let stat: Vec<_> = stat.rsplit_once(") ").map_or("",|(_,s)|s).split_whitespace().collect();
            let status = std::fs::read_to_string(format!("{root}/status")).unwrap_or_default();
            let counter = |name: &str| status.lines().find_map(|line|line.strip_prefix(name)).map(str::trim).unwrap_or("unavailable");
            eprintln!("FEREG_SCHED|scope={}|id={}|edge={edge}|record=task|tid={tid}|run_ns={}|wait_ns={}|slices={}|cpu={}|voluntary={}|involuntary={}",
                self.label,self.id,sched.first().unwrap_or(&"unavailable"),sched.get(1).unwrap_or(&"unavailable"),
                sched.get(2).unwrap_or(&"unavailable"),stat.get(36).unwrap_or(&"unavailable"),
                counter("voluntary_ctxt_switches:"),counter("nonvoluntary_ctxt_switches:"));
        }
        for cpu in std::env::var("FEREG_SCHED_CPUS").unwrap_or_default().split(',').filter_map(|v|v.parse::<usize>().ok()) {
            let frequency=std::fs::read_to_string(format!("/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_cur_freq")).unwrap_or_else(|_|"unavailable".into());
            eprintln!("FEREG_SCHED|scope={}|id={}|edge={edge}|record=frequency|cpu={cpu}|khz={}",self.label,self.id,frequency.trim());
        }
        eprintln!("FEREG_SCHED|scope={}|id={}|edge={edge}|record=overhead|read_ns={}",self.label,self.id,began.elapsed().as_nanos());
    }
}
impl Drop for SchedulerProbe {
    fn drop(&mut self) { self.snapshot("end"); }
}
