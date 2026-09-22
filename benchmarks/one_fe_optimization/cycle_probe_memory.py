"""Budget descriptor construction in the isolated cycle-routing experiment."""
from pathlib import Path


def instrument(source):
    base = source / 'crates/fereg-core/src'
    def edit(relative, before, after):
        path = base / relative
        text = path.read_text()
        assert text.count(before) == 1, (relative, before)
        path.write_text(text.replace(before, after))

    path = base / 'memory.rs'
    path.write_text(path.read_text() + '\n#[path = "cycle_probe_memory_experiment.rs"]\nmod cycle_probe_memory_experiment;\npub(crate) use cycle_probe_memory_experiment::CycleProbeReservation;\n')
    (base / 'cycle_probe_memory_experiment.rs').write_bytes(Path(__file__).with_suffix('.rs').read_bytes())
    edit('fe/design.rs', "impl<'a> BorrowedInterceptFeDesign<'a> {", """impl<'a> BorrowedInterceptFeDesign<'a> {
    pub(crate) fn cycle_descriptor_bytes(&self) -> u128 {
        self.effects.capacity() as u128 * std::mem::size_of::<BorrowedInterceptEffect<'_>>() as u128
    }
""")
    edit('ols/prepared.rs', '    pub fn borrowed_intercept_fe_design(&self) -> Result<BorrowedInterceptFeDesign<\'_>> {', '''    pub(crate) fn cycle_borrowed_intercept_fe_design(&self, reservation: &mut crate::memory::CycleProbeReservation) -> Result<BorrowedInterceptFeDesign<'_>> {
        build_borrowed_intercept_fe_design_with_reservation(self, reservation)
    }

    pub fn borrowed_intercept_fe_design(&self) -> Result<BorrowedInterceptFeDesign<'_>> {''')
    for relative, trait in [('ols/prepared.rs', 'FeTopologyReservation'), ('fe/topology_adapter.rs', 'TopologyAdapterReservation')]:
        path = base / relative
        path.write_text(path.read_text() + f'''
impl {trait} for crate::memory::CycleProbeReservation {{
    fn reserve_vec<T>(&mut self, target: &mut Vec<T>, additional: usize, label: &'static str) -> Result<()> {{
        self.reserve(target, additional, label)
    }}
}}
''')
    path = base / 'fe/topology_adapter.rs'
    path.write_text(path.read_text() + '''
pub(crate) fn cycle_borrow_categorical_topology_design<'a>(
    nobs: usize, registry: &'a CategoricalTopologyRegistry, sources: &[CategoricalCodeView<'a>],
    effects: &[usize], reservation: &mut crate::memory::CycleProbeReservation,
) -> Result<BorrowedInterceptFeDesign<'a>> {
    borrow_categorical_topology_design_with_reservation(nobs, registry, sources, effects, reservation)
}
''')
    path = base / 'fe/mod.rs'
    path.write_text(path.read_text() + '\npub(crate) use topology_adapter::cycle_borrow_categorical_topology_design;\npub(crate) use borrowed_map::cycle_direct_lsmr_workspace_bytes;\n')
    edit('ols/prepared_fit_view.rs', '    fn fit_borrowed_cmg_design(&self) -> Result<BorrowedInterceptFeDesign<\'_>>;', '''    fn fit_borrowed_cmg_design(&self) -> Result<BorrowedInterceptFeDesign<'_>>;
    fn fit_cycle_probe_eligible(&self) -> bool;
    fn fit_cycle_probe_design(&self, reservation: &mut crate::memory::CycleProbeReservation) -> Result<BorrowedInterceptFeDesign<'_>>;
''')
    edit('ols/prepared_fit_view.rs', 'impl PreparedOlsFitView for PreparedOlsData {', '''impl PreparedOlsFitView for PreparedOlsData {
    fn fit_cycle_probe_eligible(&self) -> bool { self.fe_topology.effects().len() == 2 }
    fn fit_cycle_probe_design(&self, reservation: &mut crate::memory::CycleProbeReservation) -> Result<BorrowedInterceptFeDesign<'_>> {
        self.cycle_borrowed_intercept_fe_design(reservation)
    }
''')
    edit('ols/prepared_fit_view.rs', 'impl PreparedOlsFitView for PreparedCategoricalOlsData {', '''impl PreparedOlsFitView for PreparedCategoricalOlsData {
    fn fit_cycle_probe_eligible(&self) -> bool {
        self.effect_specs().len() == 2 && self.fit_supports_cmg_auto()
            && self.fe_topology().registry().sources().iter().all(|s| !s.partition().is_interaction())
    }
    fn fit_cycle_probe_design(&self, reservation: &mut crate::memory::CycleProbeReservation) -> Result<BorrowedInterceptFeDesign<'_>> {
        let topology = self.fe_topology();
        let mut sources = Vec::new();
        reservation.reserve(&mut sources, topology.compacted_sources().len(), "cycle probe source views")?;
        for source in topology.compacted_sources() { sources.push(source.view()); }
        crate::fe::cycle_borrow_categorical_topology_design(self.fit_sample().n_active(), topology.registry(),
            &sources, topology.effect_to_topology(), reservation)
    }
''')
    path = base / 'fe/borrowed_map.rs'
    path.write_text(path.read_text() + '''
// Match the existing direct-auto admission: initial certificate workspace,
// original norms, optional pilot products and the resolved LSMR envelope.
pub(crate) fn cycle_direct_lsmr_workspace_bytes(
    design: &BorrowedInterceptFeDesign<'_>, rhs: usize, config: super::AbsorbConfig,
) -> Result<u128> {
    if design.effects().len() != 2 { return Err(invalid_argument("cycle admission requires two effects")); }
    let levels = [design.effects()[0].n_levels(), design.effects()[1].n_levels()];
    let memory = estimate_fe_solver_memory(design.nobs(), rhs, &levels,
        config.lsmr.threads.resolved().max(rayon::current_num_threads()), config.lsmr.local_size)?;
    let coefficients = if config.retain_coefficients { memory.coefficient_batch_bytes } else { 0 };
    let pilot = BorrowedMapWorkspace::planned_payload_bytes(design, design.nobs(), rhs, rayon::current_num_threads())?
        .checked_add(coefficients).ok_or_else(|| allocation("cycle pilot memory overflows"))?;
    let preconditioner = resolve_borrowed_lsmr_preconditioner(2, design.total_term_slots(), config.lsmr.preconditioner);
    let lsmr = memory.lsmr_bytes(preconditioner).checked_add(coefficients)
        .and_then(|bytes| bytes.checked_add(rhs as u128 * 8))
        .ok_or_else(|| allocation("cycle LSMR memory overflows"))?;
    Ok(pilot.max(lsmr))
}
''')
    path = base / 'ols/prepared_fit.rs'
    text = path.read_text()
    assert text.count('mod tests {') == 1
    tests = Path(__file__).with_name('cycle_probe_memory_tests.rs').read_text()
    path.write_text(text.replace('mod tests {', 'mod tests {\n' + tests))
