"""Interpret native return/convergence status without rewriting measured receipts."""
import copy


def audited_outcome(row):
    result = copy.deepcopy(row)
    result['recorded_status'] = row['status']
    converged = row.get('metadata', {}).get('converged')
    # Stata scalars deserialize as floats; `is False` misses e(converged)=0.
    if row['status'] == 'pass' and converged is not None and converged == 0:
        result['status'] = 'nonconvergence'
    if result['status'] == 'worker_failure' and row.get('metadata', {}).get('rc') == 430:
        result['failure_class'] = 'native_numerical_failure_r430'
    else:
        result['failure_class'] = result['status'] if result['status'] != 'pass' else None
    return result
