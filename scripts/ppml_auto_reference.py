"""Prospective reference-row validation, not retrospective campaign acceptance."""
import csv
import hashlib
import math
from pathlib import Path

from ppml_solver_development import _science_issues


def compare_reference_fit(candidate, reference, controls, poisson_constant, science):
    """Apply no-worse objective policy only after independently supplied identity.

    The Poisson constant must come from the original candidate audit; a caller
    cannot derive it from the reference pair being checked. This helper grants
    neither a numerical exception nor source/sample provenance acceptance.
    """
    if not math.isfinite(poisson_constant):
        raise ValueError('nonfinite independently supplied Poisson constant')
    rt, at = science['likelihood_rtol'], science['likelihood_atol']
    if not all(math.isfinite(v) and v >= 0 for v in (rt, at)):
        raise ValueError('invalid objective comparison envelope')
    for fit in (candidate, reference):
        ll, dev = fit['ll'], fit['deviance']
        if (not all(math.isfinite(v) for v in (ll, dev)) or dev < 0
                or not math.isfinite(dev+2*ll)
                or abs(dev+2*ll-poisson_constant) > at+rt*abs(poisson_constant)):
            raise ValueError('invalid or Poisson-inconsistent reported objective')
    notes = set()
    issues = _science_issues(candidate, reference, controls, science, 'ppmlhdfe', notes)
    issues = [issue for issue in issues if issue not in ('ppmlhdfe_ll', 'ppmlhdfe_deviance')]
    for key, direction in (('ll', -1), ('deviance', 1)):
        c, r = candidate[key], reference[key]
        if direction*(c-r) > at+rt*max(abs(c), abs(r)):
            issues.append('ppmlhdfe_'+key)
    return {'campaign_admission': False, 'comparison_pass': not issues,
            'issues': issues, 'dof_notes': sorted(notes)}


def read_reference_rows(path, controls):
    """Keep complete successful fit records; never turn short V into disagreement.

    Callers must independently bind the original input, reference pin and fit
    execution. A completed row inside a failed campaign does not pass that
    campaign or supply an accepted timing observation.
    """
    if type(controls) is not int or controls < 0:
        raise ValueError('invalid control count')
    path = Path(path)
    rows, rejected = [], []
    with path.open() as stream:
        reader = csv.DictReader(stream, delimiter='\t')
        fields = reader.fieldnames or []
        required = {'arm', 'repetition', 'rc', 'N', 'll', 'deviance', 'converged',
                    'rank', 'df_a', 'df_resid', 'b_names', 'b_values', 'v_names',
                    'v_values', 'reference_acceleration', 'reference_transform'}
        if len(fields) != len(set(fields)) or not required.issubset(fields):
            raise ValueError('invalid reference table header')
        for number, raw in enumerate(reader, 1):
            try:
                if None in raw or any(value is None for value in raw.values()):
                    raise ValueError('incomplete record')
                names = {key: raw[key].split() for key in ('b_names', 'v_names')}
                values = {key: list(map(float, raw[key].split())) for key in ('b_values', 'v_values')}
                width = len(names['v_names'])
                if (width == 0 or names['b_names'] != names['v_names']
                        or len(set(names['b_names'])) != width
                        or len(values['b_values']) != width
                        or width < controls or len(values['v_values']) != width*width
                        or not all(math.isfinite(v) for vector in values.values() for v in vector)):
                    raise ValueError('incomplete or nonfinite scientific matrix')
                expected = [f'x{i+1}' for i in range(controls)]
                if any(names[key][:controls] != expected for key in names):
                    raise ValueError('control identity mismatch')
                if (raw['arm'] not in ('ppmlhdfe', 'ppmlhdfe_strict')
                        or int(raw['rc']) != 0 or float(raw['converged']) != 1
                        or raw['reference_acceleration'] != 'conjugate_gradient'
                        or raw['reference_transform'] != 'symmetric_kaczmarz'):
                    raise ValueError('unsuccessful or unexpected reference route')
                scalars = {key: float(raw[key]) for key in ('N', 'll', 'deviance', 'rank', 'df_a')}
                if (not all(math.isfinite(v) for v in scalars.values())
                        or scalars['N'] <= 0 or scalars['deviance'] < 0):
                    raise ValueError('invalid reference scalar')
                residual_dof = None if raw['df_resid'].strip() == '.' else float(raw['df_resid'])
                if residual_dof is not None and not math.isfinite(residual_dof):
                    raise ValueError('nonfinite residual DoF')
                rows.append({'row': number, 'raw': raw,
                             'fit': {**scalars, **names, **values, 'df_resid': residual_dof,
                                     'sample_n': scalars['N']}})
            except (ValueError, OverflowError) as error:
                rejected.append({'row': number, 'reason': str(error)})
    return {'campaign_admission': False, 'table_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'complete_fits': rows, 'rejected_records': rejected,
            'reference_available': bool(rows)}
