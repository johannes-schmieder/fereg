def compare(paths):
    a, b = paths['candidate'], paths['baseline']
    ra, rb = read_json(a / 'result.json'), read_json(b / 'result.json')
    assert ra['status'] == rb['status'] == 'pass'
    assert ra['sample_sha256'] == rb['sample_sha256']
    runtime = {arm: named_runtime(path) for arm, path in paths.items()}
    for name in ('b', 'V', 'feppml_status', 'feppml_fit', 'feppml_dof', 'feppml_fe',
                 'feppml_cluster', 'feppml_subset', 'feppml_separation', 'feppml_inference',
                 'feppml_drop_counts', 'feppml_separation_sequence', 'feppml_certificate', 'feppml_receipt'):
        assert (a / f'{name}.csv').read_bytes() == (b / f'{name}.csv').read_bytes(), name
    solver = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 16, 17, 18, 19, 20, 21]
    assert np.array_equal(matrix(a, 'feppml_solver')[:, solver], matrix(b, 'feppml_solver')[:, solver])
    for key in ('outer_iterations', 'inner_iterations', 'step_halvings', 'memory_requested',
                'memory_selected', 'linear_route', 'fallback_mask', 'fallback_count',
                'threads_requested', 'threads_effective'):
        assert runtime['candidate'][key] == runtime['baseline'][key], key
    for path in paths.values():
        assert matrix(path, 'feppml_certificate')[-1, 0] == 1
        assert read_json(path / 'result.json')['metadata']['converged'] == 1
    if (a / 'saved_outputs.csv').exists():
        assert (a / 'saved_outputs.csv').read_bytes() == (b / 'saved_outputs.csv').read_bytes()
    write_json(a / 'exact-science.json', dict(sample_b_V_status_dof_weight_cluster_separation_exact=True,
               certificates_pass=True, solver_decisions_exact=True, saved_outputs_exact=True))
    write_json(a / 'paired-runtime.json', runtime)
