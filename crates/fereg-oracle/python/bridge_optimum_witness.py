"""Graph-only optimum witnesses, independent of production FE solvers.

Collapse observations with the same worker/firm pair into one graph edge. If
that edge is a bridge, its observation indicator belongs to the FE column space:
shift worker effects by +1 and firm effects by -1 on one side of the cut. Extra
regressors or FEs cannot remove this direction. At an OLS or Poisson optimum,
the cell's weighted residual sum must vanish. A one-observation bridge thus has
fitted value exactly equal to its outcome, even in a much larger model.

A multirow bridge only identifies the conditional intercept update; it does not
independently identify the within-cell contrasts or the other coefficients.
"""
import numpy as np
from scipy.sparse import coo_matrix


def bridges(vertex_count, left, right):
    """Return bridge flags and a DFS forest for a simple undirected graph.

    Edges must already be unique, with no loops. Discovery/subtree-end indices,
    parents and parent-edge indices let callers check claimed cuts independently.
    """
    left = np.asarray(left, dtype=np.int64)
    right = np.asarray(right, dtype=np.int64)
    if (left.ndim != 1 or right.shape != left.shape or vertex_count < 0 or
            np.any(left == right) or np.any(left < 0) or np.any(right < 0) or
            np.any(left >= vertex_count) or np.any(right >= vertex_count)):
        raise ValueError('invalid undirected graph')
    edge_count = len(left)
    ids = np.arange(edge_count, dtype=np.int64) + 1
    graph = coo_matrix((np.r_[ids, ids],
                        (np.r_[left, right], np.r_[right, left])),
                       shape=(vertex_count, vertex_count)).tocsr()
    if graph.nnz != 2 * edge_count:
        raise ValueError('collapse repeated worker/firm cells first')
    seen = np.full(vertex_count, -1, dtype=np.int64)
    low = seen.copy()
    parent = seen.copy()
    parent_edge = seen.copy()
    cursor = graph.indptr[:-1].copy()
    finish = np.empty(vertex_count, dtype=np.int64)
    result = np.zeros(edge_count, dtype=bool)
    clock = 0
    for root in range(vertex_count):
        if seen[root] >= 0:
            continue
        seen[root] = low[root] = clock
        clock += 1
        stack = [root]
        while stack:
            vertex = stack[-1]
            if cursor[vertex] < graph.indptr[vertex + 1]:
                position = cursor[vertex]
                cursor[vertex] += 1
                neighbor = int(graph.indices[position])
                edge = int(graph.data[position]) - 1
                if edge == parent_edge[vertex]:
                    continue
                if seen[neighbor] < 0:
                    parent[neighbor] = vertex
                    parent_edge[neighbor] = edge
                    seen[neighbor] = low[neighbor] = clock
                    clock += 1
                    stack.append(neighbor)
                else:
                    low[vertex] = min(low[vertex], seen[neighbor])
            else:
                stack.pop()
                finish[vertex] = clock
                ancestor = parent[vertex]
                if ancestor >= 0:
                    result[parent_edge[vertex]] = low[vertex] > seen[ancestor]
                    low[ancestor] = min(low[ancestor], low[vertex])
    return result, seen, finish, parent, parent_edge
