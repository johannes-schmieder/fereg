"""Captured-source experiment; does not admit shared serial routing changes."""


def replace_once(text, before, after):
    assert text.count(before) == 1, 'Serial LSMR experiment anchor changed: ' + before
    return text.replace(before, after)


def instrument(source):
    path = source / 'crates/fereg-core/src/memory.rs'
    text = path.read_text()
    text = replace_once(text,
        '    if rhs <= 64 || rhs <= threads.saturating_mul(4) {',
        '''    // Isolated experiment: transport and admission use this same bound.
    if threads == 1 && rhs > 1 {
        return 1;
    }
    if rhs <= 64 || rhs <= threads.saturating_mul(4) {''')
    text = replace_once(text, 'assert_eq!(four.krylov_bytes, 4 * one.krylov_bytes);',
                        'assert_eq!(four.krylov_bytes, one.krylov_bytes);')
    text = replace_once(text, 'assert_eq!(lsmr_rhs_window(rhs, threads), rhs);',
                        'assert_eq!(lsmr_rhs_window(rhs, threads), if threads == 1 { 1 } else { rhs });')
    path.write_text(text)

    path = source / 'crates/fereg-core/src/fe/lsmr_batch.rs'
    text = path.read_text()
    text = replace_once(text,
        '                        let oracle = solver.solve_batch(&rhs, &options).unwrap();',
        '''                        for width in [1, 2, 12, 52, 67] {
                        let rhs = &rhs[..width];
                        let oracle = solver.solve_batch(&rhs, &options).unwrap();''')
    text = replace_once(text,
        '                        assert_eq!((result.n_dofs, result.n_obs), (oracle.n_dofs, oracle.n_obs));',
        '''                        assert_eq!((result.n_dofs, result.n_obs), (oracle.n_dofs, oracle.n_obs));
                        }''')
    path.write_text(text)
