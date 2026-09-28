"""Small graph check and scalar scratch arithmetic; no estimator or fitting."""
import json


def components(firms, disconnected=False):
    # Independently connect firm pairs through each degree-two worker. This
    # contracts worker vertices without changing connected-component counts.
    graph = [set() for _ in range(firms)]
    for layer in range(40):
        for firm in range(firms):
            if disconnected:
                half = firms // 2
                other = (firm // half) * half + (firm % half + 1 + layer % (half // 4 - 1)) % half
            else:
                other = (firm + 1 + layer % (firms // 4 - 1)) % firms
            graph[firm].add(other)
            graph[other].add(firm)
    unseen = set(range(firms))
    count = 0
    while unseen:
        stack = [unseen.pop()]
        count += 1
        while stack:
            for neighbor in graph[stack.pop()]:
                if neighbor in unseen:
                    unseen.remove(neighbor)
                    stack.append(neighbor)
    return count


def main():
    checks = []
    for firms in (16, 32, 64, 128):
        for disconnected in (False, True):
            observed = components(firms, disconnected)
            assert observed == (2 if disconnected else 1)
            checks.append(dict(firms=firms, workers=40 * firms, disconnected=disconnected,
                               components=observed))
    n, threads, rhs = 3_932_160, 4, 11
    maximum, minimum = n // 2, n // 80
    levels = maximum + minimum
    ordinary_banks = threads + levels // 10_002
    def apply(local_values, banks):
        return 8 * (levels + local_values) * banks + 512 * banks + 256 * threads
    old = apply(6 * maximum + 4, ordinary_banks)
    imbalance = apply(2 * maximum + 4 * minimum + 4, ordinary_banks)
    # One component is invoked once per apply, so it cannot borrow a second
    # buffer from that apply's own WorkerBufferStack, even under nested Rayon.
    connected = apply(2 * maximum + 4 * minimum + 4, 1)
    result = dict(scope='source proof and scalar apply subset, not runtime or full admission',
                  graph_checks=checks, n=n, threads=threads, rhs=rhs,
                  levels=levels, maximum_levels=maximum, minimum_levels=minimum,
                  topology_blind_bank_bound=ordinary_banks,
                  connected_graph_bank_bound=1,
                  scratch_subset={name:dict(per_apply_bytes=value,
                                           four_cached_plus_live_rhs_bytes=(4 + rhs) * value,
                                           four_cached_plus_live_rhs_gib=(4 + rhs) * value / 2**30)
                                  for name,value in [('current',old),('queued_imbalance',imbalance),
                                                     ('connected_graph_proof',connected)]})
    print(json.dumps(result,indent=2,sort_keys=True))


if __name__ == '__main__':
    main()
