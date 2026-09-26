"""Independent direct QR/SVD oracle for bounded positive-outcome slope pilots.

Eliminate the first partition with group Householder QR, then explicitly SVD
all remaining regressors. No fereg/fixest solver, rank or inference input is used.
"""
import time

import numpy as np
from scipy import linalg, sparse
from scipy.special import gammaln, xlogy


def indicators(values):
    _, codes = np.unique(values, return_inverse=True)
    return sparse.csr_matrix((np.ones(len(codes)), (np.arange(len(codes)), codes)))


class Design:
    def __init__(self, frame, cell):
        self.n, self.k = len(frame), cell['k']
        first, *others = cell['effects']
        self.order = np.argsort(frame[first].to_numpy(), kind='stable')
        self.inverse = np.argsort(self.order)
        frame = frame.iloc[self.order]
        self.ids = frame.obs_id.to_numpy()
        self.y = frame.y.to_numpy()
        _, counts = np.unique(frame[first], return_counts=True)
        # This diagnostic intentionally accepts only the captured balanced
        # first partition, rather than silently changing a sample or rank rule.
        if not np.all(counts == counts[0]):
            raise ValueError('first-partition group sizes must agree')
        self.m, self.g = int(counts[0]), len(counts)
        local = [np.ones(self.n)] + [frame[z].to_numpy() for z in cell.get('slopes', {}).get(first, [])]
        self.p = len(local)
        if self.m <= self.p:
            raise ValueError('no within-first-partition degrees of freedom')
        self.L = np.column_stack(local).reshape(self.g, self.m, self.p)
        X = frame[[f'x{i}' for i in range(1, self.k+1)]].to_numpy()
        parts = [sparse.csr_matrix(X)]
        for name in others:
            F = indicators(frame[name].to_numpy())
            parts.append(F)
            parts.extend(F.multiply(frame[z].to_numpy()[:, None]) for z in cell.get('slopes', {}).get(name, []))
        self.A = sparse.hstack(parts, format='csr')
        if self.n > 61440 or self.A.shape[1] > 2200:
            raise ValueError('explicit diagnostic exceeds declared size bound')
        rows = np.repeat(np.arange(self.n), self.p)
        columns = np.repeat(np.arange(self.g), self.m*self.p)*self.p + np.tile(np.arange(self.p), self.n)
        first_matrix = sparse.csr_matrix((self.L.ravel(), (rows, columns)), shape=(self.n, self.g*self.p))
        self.full = sparse.hstack([self.A, first_matrix], format='csr')
        self.trace = []

    def factor(self, weights):
        start = time.monotonic()
        if not np.isfinite(weights).all() or np.min(weights) <= 0:
            raise ValueError('weights must be finite and strictly positive')
        root = np.sqrt(weights).reshape(self.g, self.m)
        WL = root[:, :, None]*self.L
        singular = np.linalg.svd(WL, compute_uv=False)
        if np.any(singular[:, -1] <= 1e-12*singular[:, 0]):
            raise ValueError('first-partition rank is unresolved')
        Q, R = np.linalg.qr(WL, mode='complete')
        Q2 = Q[:, :, self.p:]
        rows_per_group = self.m-self.p
        C = np.empty((self.g*rows_per_group, self.A.shape[1]), order='F')
        for first in range(0, self.g, 256):
            last = min(first+256, self.g)
            block = self.A[first*self.m:last*self.m].toarray().reshape(last-first, self.m, -1)
            C[first*rows_per_group:last*rows_per_group] = np.matmul(
                Q2[first:last].transpose(0, 2, 1), root[first:last, :, None]*block).reshape(-1, C.shape[1])
        U, s, Vt = linalg.svd(C, full_matrices=False, check_finite=False, lapack_driver='gesdd')
        cutoff = max(C.shape)*np.finfo(float).eps*s[0]
        selected = s > cutoff
        # Vary the dimension-scaled threshold without dropping below floating
        # arithmetic noise in the small analytic controls. Discarded gauges
        # and retained directions must have a clear singular-value gap.
        lower_cutoff = max(cutoff/100, 32*np.finfo(float).eps*s[0])
        if np.count_nonzero(s > lower_cutoff) != selected.sum() or np.count_nonzero(s > cutoff*100) != selected.sum():
            raise ValueError('reduced explicit rank is threshold-sensitive')
        if np.max(abs(Vt[~selected, :self.k]), initial=0) > 1e-8:
            raise ValueError('finite coefficients are not independently identified')
        U, s_kept, Vt_kept = U[:, selected], s[selected], Vt[selected]
        total_rank = self.g*self.p + int(selected.sum())
        trace = dict(seconds=time.monotonic()-start, rank=total_rank, reduced_rank=int(selected.sum()),
                     first_rank=self.g*self.p, largest_singular=float(s[0]), smallest_kept=float(s_kept[-1]),
                     largest_discarded=float(np.max(s[~selected], initial=0)), cutoff=float(cutoff),
                     lower_rank_cutoff=float(lower_cutoff), upper_rank_cutoff=float(cutoff*100))
        self.trace.append(trace)
        return Factor(self, root, Q, R, U, s_kept, Vt_kept, trace)

    def score(self, residual, scale):
        score = np.asarray(self.full.T@residual).ravel()
        normalizer = np.maximum(1., np.asarray(abs(self.full).T@scale).ravel())
        return float(np.max(abs(score)/normalizer))


class Factor:
    def __init__(self, design, root, Q, R, U, s, Vt, trace):
        self.d, self.root, self.Q, self.R = design, root, Q, R
        self.U, self.s, self.Vt, self.trace = U, s, Vt, trace

    def solve(self, response):
        d = self.d
        weighted = self.root*response.reshape(d.g, d.m)
        transformed = np.matmul(self.Q.transpose(0, 2, 1), weighted[:, :, None])[:, :, 0]
        other = self.Vt.T@((self.U.T@transformed[:, d.p:].ravel())/self.s)
        first_rhs = transformed[:, :d.p] - np.matmul(
            self.Q[:, :, :d.p].transpose(0, 2, 1),
            (self.root*(d.A@other).reshape(d.g, d.m))[:, :, None])[:, :, 0]
        first = np.linalg.solve(self.R[:, :d.p], first_rhs[:, :, None])[:, :, 0]
        coefficients = np.r_[other, first.ravel()]
        prediction = np.asarray(d.full@coefficients).ravel()
        return coefficients, prediction

    def inference(self):
        d = self.d
        finite_inverse_factor = self.Vt[:, :d.k].T/self.s
        bread = finite_inverse_factor@finite_inverse_factor.T
        reduced_Z = self.U@(finite_inverse_factor.T@np.linalg.inv(bread))
        weighted_Z = np.matmul(self.Q[:, :, d.p:], reduced_Z.reshape(d.g, d.m-d.p, d.k)).reshape(d.n, d.k)
        Z = weighted_Z/self.root.ravel()[:, None]
        # Check nuisance orthogonality and the independently reconstructed
        # finite Hessian, rather than trusting the SVD extraction formula alone.
        weighted_Z = Z*self.root.ravel()[:, None]**2
        nuisance = d.full[:, d.k:]
        moment = np.asarray(nuisance.T@weighted_Z)
        denominator = np.maximum(1., np.asarray(abs(nuisance).T@abs(weighted_Z)))
        orthogonality = float(np.max(abs(moment)/denominator))
        hessian = Z.T@weighted_Z
        inverse_error = float(np.max(abs(hessian@bread-np.eye(d.k))))
        if orthogonality > 1e-9 or inverse_error > 1e-9:
            raise ValueError('independent final-Hessian certificate failed')
        return bread, Z, dict(maximum_scaled_projection=orthogonality, inverse_error=inverse_error)


def oracle(frame, cell):
    start = time.monotonic()
    d = Design(frame, cell)
    y = d.y
    iterations = []
    if cell['model'] == 'ols':
        factor = d.factor(np.ones(d.n))
        coefficients, eta = factor.solve(y)
        residual = y-eta
        score = d.score(residual, abs(y)+abs(eta))
        if score > 1e-10:
            raise ValueError('independent OLS score certificate failed')
        bread, Z, certificates = factor.inference()
        df_r = d.n-factor.trace['rank']
        if df_r <= 0:
            raise ValueError('nonpositive independent residual DoF')
        rss = float(residual@residual)
        covariance = bread*rss/df_r
        extra = dict(rss=rss, df_r=df_r, maximum_scaled_score=score)
    else:
        if not np.all(y > 0):
            raise ValueError('this diagnostic excludes separation; all outcomes must be positive')
        coefficients = np.zeros(d.full.shape[1])
        coefficients[d.A.shape[1]::d.p] = np.log(y.mean())
        eta = np.asarray(d.full@coefficients).ravel()
        for iteration in range(50):
            mu = np.exp(eta)
            factor = d.factor(mu)
            increment, direction = factor.solve((y-mu)/mu)
            objective = float(np.sum(mu-y*eta))
            step = 1.
            while step > 2**-30:
                trial = eta+step*direction
                if np.isfinite(trial).all() and np.max(trial) < 700 and np.sum(np.exp(trial)-y*trial) <= objective+1e-11*max(1,abs(objective)):
                    break
                step *= .5
            if step <= 2**-30:
                raise ValueError('independent Poisson line search failed')
            coefficients += step*increment
            eta = np.asarray(d.full@coefficients).ravel()
            mu = np.exp(eta)
            score = d.score(y-mu, y+mu)
            iterations.append(dict(iteration=iteration+1, score=score, step=step))
            print('ORACLE_ITERATION', cell['model'], cell['k'], iterations[-1], flush=True)
            if score < 1e-11:
                break
        else:
            raise ValueError('independent Poisson iteration limit')
        # Fresh final-weight Hessian; the final Newton increment used old weights.
        factor = d.factor(mu)
        bread, Z, certificates = factor.inference()
        meat = Z.T@((y-mu)[:, None]**2*Z)
        covariance = bread@meat@bread*d.n/(d.n-1)
        extra = dict(deviance=float(2*np.sum(xlogy(y,y/mu)-y+mu)),
                     ll=float(np.sum(y*eta-mu-gammaln(y+1))), maximum_scaled_score=score)
    return dict(schema='explicit-group-qr-reduced-svd-v1', status='pass', N=d.n,
                sample_sha256=__import__('hashlib').sha256(b''.join(int(x).to_bytes(8,'little') for x in d.ids[d.inverse])).hexdigest(),
                coefficient_names=[f'x{i}' for i in range(1,d.k+1)], coefficients=coefficients[:d.k].tolist(),
                covariance=covariance.tolist(), bread=bread.tolist(), model_rank=factor.trace['rank'],
                fe_rank=factor.trace['rank']-d.k, certificates=certificates,
                factor_trace=d.trace, iteration_trace=iterations, elapsed_seconds=time.monotonic()-start, **extra)
