"""Small independent symmetric eigensolver for the bounded dense replay oracle."""
import math


def eigensystem(values):
    n = len(values)
    if not n or n > 64 or any(len(row) != n for row in values):
        raise ValueError('bounded square matrix required')
    a = [list(row) for row in values]
    q = [[float(i == j) for j in range(n)] for i in range(n)]
    scale = max(abs(x) for row in a for x in row)
    if any(not math.isfinite(x) for row in a for x in row):
        raise ValueError('nonfinite matrix')
    for _ in range(100 * n * n):
        largest, p, r = max((abs(a[i][j]), i, j) for i in range(n) for j in range(i + 1, n)) if n > 1 else (0., 0, 0)
        if largest <= max(1e-300, scale * 1e-15):
            break
        tau = (a[r][r] - a[p][p]) / (2 * a[p][r])
        t = math.copysign(1., tau) / (abs(tau) + math.hypot(1., tau))
        c = 1 / math.hypot(1., t)
        s = t * c
        app, arr, apr = a[p][p], a[r][r], a[p][r]
        a[p][p], a[r][r], a[p][r], a[r][p] = app - t * apr, arr + t * apr, 0., 0.
        for i in range(n):
            if i not in (p, r):
                x, y = a[i][p], a[i][r]
                a[i][p] = a[p][i] = c * x - s * y
                a[i][r] = a[r][i] = s * x + c * y
            x, y = q[i][p], q[i][r]
            q[i][p], q[i][r] = c * x - s * y, s * x + c * y
    else:
        raise ValueError('independent eigensolver did not converge')
    eigenvalues = [a[i][i] for i in range(n)]
    residual = max(abs(sum(values[i][k] * q[k][j] for k in range(n)) - q[i][j] * eigenvalues[j]) for i in range(n) for j in range(n))
    if residual > max(1e-300, 1e-12 * scale):
        raise ValueError('independent eigenpair residual failed')
    repaired = [[sum(q[i][k] * max(eigenvalues[k], 0.) * q[j][k] for k in range(n)) for j in range(n)] for i in range(n)]
    return eigenvalues, repaired
