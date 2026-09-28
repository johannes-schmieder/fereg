"""Independent component-size enumeration for the proposed unsigned bound."""
import json


def main():
    components = frames = 0
    for a in range(1, 9):
        for b in range(1, 9):
            for c in range(9):
                counts = [a, b, c]
                maximum, total = max(counts), sum(counts)
                bound = 2 * maximum + 4 * min(maximum, total - maximum) + 4
                assert bound <= 6 * maximum + 4
                if maximum <= total - maximum:
                    assert bound == 6 * maximum + 4
                frames += 1
                for left in range(3):
                    for right in range(left + 1, 3):
                        for q in range(1, counts[left] + 1):
                            for r in range(1, counts[right] + 1):
                                # Two upstream buffers: local input, reduced
                                # kept block and up to two ground entries.
                                observed = 2 * (q + r + min(q, r) + 2)
                                assert observed <= bound
                                components += 1
    levels, maximum, threads, rhs = 2_015_232, 1_966_080, 4, 11
    banks = threads + levels // 10_002
    def apply(local):
        return 8 * (levels + local) * banks + 512 * banks + 256 * threads
    old = apply(6 * maximum + 4) * (4 + rhs)
    candidate = apply(2 * maximum + 4 * min(maximum, levels - maximum) + 4) * (4 + rhs)
    print(json.dumps(dict(frames=frames, component_shapes=components,
        all_component_bounds_passed=True, old_apply_subset_bytes=old,
        candidate_apply_subset_bytes=candidate, full_admission_proved=False,
        rust_tests_run=False, numerical_fits_run=False), indent=2))


if __name__ == '__main__':
    main()
