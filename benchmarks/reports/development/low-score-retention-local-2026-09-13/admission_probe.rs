use fereg_core::memory::estimate_fe_solver_memory;
fn main() {
    let a: Vec<usize> = std::env::args().skip(1).map(|x| x.parse().unwrap()).collect();
    let m = estimate_fe_solver_memory(a[0], a[1], &a[3..], a[2], Some(4)).unwrap();
    println!("{{\"cg_bytes\":{},\"lsmr_off_bytes\":{},\"lsmr_diagonal_bytes\":{},\"lsmr_schwarz_bytes\":{},\"coefficient_batch_bytes\":{},\"topology_bytes\":{}}}", m.cg_bytes,m.lsmr_off_bytes,m.lsmr_diagonal_bytes,m.lsmr_schwarz_bytes,m.coefficient_batch_bytes,m.topology_bytes);
}
