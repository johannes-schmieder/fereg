"""Exact offline arithmetic for docs/two_fe_auto_policy.md.

Pure, uncalibrated tooling: no label discovery, matching, support inference or
measurement. Callers supply complete matched rows and retain rejected evidence.
The encoded evaluator matches model.rs on the supported 64-bit platforms.
Integer/Fraction labels are exact repeat means; binary floats are rejected.
"""
from collections import Counter
from dataclasses import dataclass, replace
from fractions import Fraction
from itertools import combinations

SCALE = 1 << 32
U64_MAX = (1 << 64) - 1
U128_MAX = (1 << 128) - 1
I128_MIN = -(1 << 127)
I128_MAX = (1 << 127) - 1


class Unknown(ValueError):
    """Encoded runtime would defer: invalid input, overflow or unpriced future."""


class Unsupported(ValueError):
    """Calibration component has missing labels, failed rank or failed bounds."""


def _integer(value, maximum=U128_MAX):
    if type(value) is not int or not 0 <= value <= maximum:
        raise Unknown("invalid unsigned integer or overflow")
    return value


def _signed(value):
    if type(value) is not int or not I128_MIN <= value <= I128_MAX:
        raise Unknown("signed overflow")
    return value


def _as_signed(value):
    return _signed(_integer(value))


def _add(a, b):
    return _integer(_integer(a) + _integer(b))


def _mul(a, b):
    return _integer(_integer(a) * _integer(b))


def ceil_div(value, divisor):
    _integer(value)
    if _integer(divisor) == 0:
        raise Unknown("zero divisor")
    quotient, remainder = divmod(value, divisor)
    return _add(quotient, int(remainder != 0))


def down(coefficient, exposure):
    return _mul(_integer(coefficient, U64_MAX), exposure) // SCALE


def up(coefficient, exposure):
    return ceil_div(_mul(_integer(coefficient, U64_MAX), exposure), SCALE)


@dataclass(frozen=True)
class Structure:
    n: int
    p: int
    q: int
    d: int


@dataclass(frozen=True)
class Execution:
    r: int
    stride: int
    workers: int


@dataclass(frozen=True)
class Work:
    H: int
    D: int

    def validate(self, width):
        _integer(width, U64_MAX)
        _integer(self.H)
        _integer(self.D)
        if width == 0 or self.D < self.H or self.D > _mul(self.H, width):
            raise Unknown("incompatible shared/live work")


def history_work(width, actions, shared_steps, live_steps, original_width):
    """Only compatible completed regressor actions, including zero-step ones."""
    _integer(width, U64_MAX)
    for value in (actions, shared_steps, live_steps, original_width):
        _integer(value)
    if (width == 0 or actions == 0 or original_width != _mul(actions, width)
            or live_steps < shared_steps or live_steps > _mul(shared_steps, width)):
        raise Unknown("incompatible raw history")
    work = Work(ceil_div(shared_steps, actions),
                ceil_div(_mul(width, live_steps), original_width))
    work.validate(width)
    return work


@dataclass(frozen=True)
class FutureGroup:
    width: int
    stride: int
    actions: int
    work: Work | None


def grouped_future(remaining_columns, block, stride, include_final, *,
                   history_width=None, history=None):
    """Full blocks, tail and final; caller owns demand/key/support compatibility."""
    for value in (remaining_columns, block, stride):
        _integer(value, U64_MAX)
    if block == 0 or stride == 0 or type(include_final) is not bool:
        raise Unknown("invalid future schedule")
    if (history_width is None) != (history is None):
        raise Unknown("incomplete history")
    if history is not None:
        history.validate(history_width)
    groups = []
    full, tail = divmod(remaining_columns, block)
    for width, actions in ((block, full), (tail, int(tail > 0))):
        if actions:
            if width > stride:
                raise Unknown("future width exceeds stride")
            groups.append(FutureGroup(width, stride, actions,
                                      history if width == history_width else None))
    if include_final:
        groups.append(FutureGroup(1, stride, 1, None))
    return tuple(groups)


@dataclass(frozen=True)
class Coefficients:
    i_s: int
    i_p: int
    b_s: int
    b_p: int
    a_0: int
    r_0: int
    t_0: int
    t_1: int
    kappa_f: int

    def __post_init__(self):
        for value in vars(self).values():
            _integer(value, U64_MAX)


@dataclass(frozen=True)
class Exposure:
    O: int
    V: int
    Z: int
    S: int
    P: int


def exposures(structure, execution):
    for value in vars(structure).values():
        if _integer(value, U64_MAX) == 0:
            raise Unknown("zero structure dimension")
    for value in vars(execution).values():
        _integer(value, U64_MAX)
    n, p, q, d = structure.n, structure.p, structure.q, structure.d
    r, s, workers = execution.r, execution.stride, execution.workers
    if r == 0 or s < r or workers == 0:
        raise Unknown("invalid execution")
    o = _add(_mul(_mul(10, n), _add(r, 1)), _mul(_mul(4, p), r))
    v = _add(_add(_add(_add(_mul(_mul(64, q), r),
                            _mul(_add(p, _mul(3, q)), s)), _mul(8, p)),
                   _mul(12, q)), _mul(64, r))
    z = _add(q, _mul(4, n))
    return Exposure(o, v, z, _mul(z, _add(1, z.bit_length())),
                    _mul(n, _add(1, d.bit_length())))


def iteration(coefficients, exposure, workers):
    if _integer(workers, U64_MAX) == 0:
        raise Unknown("zero workers")
    return _add(down(coefficients.i_s, exposure.V),
                down(coefficients.i_p, exposure.O) // workers)


def continuation(coefficients, j, startup, application, work):
    return _add(up(coefficients.t_0, _add(j, startup)),
                up(coefficients.t_1, _add(_mul(work.H, j), _mul(application, work.D))))


@dataclass(frozen=True)
class Prices:
    incumbent: int
    setup: int
    application: int
    restart: int
    startup: int
    continuation: int
    future_actions: int
    future_reserve: int
    future_gain: int
    net_gain: int
    build: bool


def strict_build(net_gain, upfront):
    return _signed(_signed(net_gain) * 4) > _signed(_as_signed(upfront) * 5)


def evaluate(coefficients, structure, execution, work, future=()):
    """Same checked expression/rounding order as Rust; never saturates."""
    if work.H == 0 or len(future) > 3:
        raise Unknown("zero current work or too many future groups")
    work.validate(execution.r)
    e = exposures(structure, execution)
    j = iteration(coefficients, e, execution.workers)
    incumbent = _mul(work.H, j)
    setup = _add(up(coefficients.b_s, e.S),
                 ceil_div(up(coefficients.b_p, e.P), execution.workers))
    application = up(coefficients.a_0, e.Z)
    restart = up(coefficients.r_0, _add(e.O, e.V))
    startup = _mul(execution.r, application)
    current = continuation(coefficients, j, startup, application, work)
    actions, gain = 0, 0
    for group in future:
        _integer(group.width, U64_MAX)
        _integer(group.stride, U64_MAX)
        _integer(group.actions)
        if group.actions == 0 or group.width == 0 or group.stride < group.width:
            raise Unknown("invalid future group")
        actions = _add(actions, group.actions)
        startup_f = _mul(group.width, application)
        if group.work is None:
            delta = -_as_signed(startup_f)
        else:
            group.work.validate(group.width)
            future_e = exposures(structure, Execution(group.width, group.stride, execution.workers))
            future_j = iteration(coefficients, future_e, execution.workers)
            future_c = continuation(coefficients, future_j, startup_f, application, group.work)
            delta = _signed(_as_signed(_mul(group.work.H, future_j))
                            - _as_signed(_add(startup_f, future_c)))
        discounted = delta // 2 if delta > 0 else delta
        gain = _signed(gain + _signed(discounted * _as_signed(group.actions)))
    if actions > 0 and application == 0:
        raise Unknown("unpriced future")
    reserve = up(coefficients.kappa_f, _mul(application, actions))
    gain = _signed(gain - _as_signed(reserve))
    net = _signed(_signed(_as_signed(incumbent) - _as_signed(current)) + gain)
    upfront = _add(_add(setup, restart), startup)
    return Prices(incumbent, setup, application, restart, startup, current,
                  actions, reserve, gain, net, strict_build(net, upfront))


def _rational(value, *, signed=False):
    if type(value) not in (int, Fraction):
        raise Unsupported("labels/features must be exact integers or Fractions")
    value = Fraction(value)
    if not signed and value < 0:
        raise Unsupported("negative component label/feature")
    return value


def paired_mean(first_ns, second_ns):
    """The two recorded durations, in integer ns; no float timer conversion."""
    return Fraction(_integer(first_ns) + _integer(second_ns), 2)


def quantize(value, *, upper):
    if type(upper) is not bool:
        raise Unsupported("quantization direction must be explicit boolean")
    value = _rational(value)
    scaled = value * SCALE
    raw = scaled.numerator // scaled.denominator
    if upper and scaled.denominator != 1:
        raw += 1
    if raw > U64_MAX:
        raise Unsupported("Q32.32 coefficient is not representable")
    return raw


@dataclass(frozen=True)
class FitPoint:
    features: tuple
    label: Fraction | int
    cell: object


def fit_envelope(points, *, upper):
    """Exact nonnegative 1-/2-variable LP, equal total objective weight per cell.

    Every row remains a constraint, including zero rows. Enumerates all vertices;
    full rank is global. Lexicographically smallest coefficients break ties.
    Returned rational coefficients have NOT yet been encoded.
    """
    if type(upper) is not bool:
        raise Unsupported("envelope direction must be explicit boolean")
    points = tuple(points)
    if not points:
        raise Unsupported("missing component labels")
    dimension = len(points[0].features)
    if dimension not in (1, 2):
        raise Unsupported("only one/two-coefficient frozen fits are supported")
    rows = []
    counts = Counter()
    for point in points:
        if len(point.features) != dimension or point.cell is None:
            raise Unsupported("incomplete feature/cell identity")
        try:
            counts[point.cell] += 1
        except TypeError as error:
            raise Unsupported("cell identity must be hashable") from error
        rows.append((tuple(_rational(x) for x in point.features),
                     _rational(point.label) * Fraction(11 if upper else 9, 10), point.cell))
    if dimension == 1:
        full_rank = any(x[0] != 0 for x, _, _ in rows)
    else:
        full_rank = any(a[0] * b[1] != a[1] * b[0]
                        for (a, _, _), (b, _, _) in combinations(rows, 2))
    if not full_rank:
        raise Unsupported("rank-deficient actual feature matrix")
    objective = tuple(sum((x[column] / counts[cell] for x, _, cell in rows), Fraction())
                      for column in range(dimension))
    # Identical boundaries need only one intersection, but all rows/weights above
    # and all feasibility constraints below remain authoritative.
    boundaries = tuple(dict.fromkeys((x, y) for x, y, _ in rows))

    def vertices():
        yield (Fraction(0),) * dimension
        for x, y in boundaries:
            for column in range(dimension):
                if x[column]:
                    value = [Fraction(0)] * dimension
                    value[column] = y / x[column]
                    yield tuple(value)
        if dimension == 2:
            for (a, y), (b, z) in combinations(boundaries, 2):
                determinant = a[0] * b[1] - a[1] * b[0]
                if determinant:
                    yield ((y * b[1] - a[1] * z) / determinant,
                           (a[0] * z - y * b[0]) / determinant)

    best = None
    for candidate in vertices():
        if min(candidate) < 0:
            continue
        feasible = True
        for x, target, _ in rows:
            prediction = sum(a * b for a, b in zip(x, candidate))
            if (prediction < target if upper else prediction > target):
                feasible = False
                break
        if feasible:
            value = sum(a * b for a, b in zip(objective, candidate))
            key = (value if upper else -value, candidate)
            if best is None or key < best:
                best = key
    if best is None:
        raise Unsupported("infeasible component envelope")
    return best[1]


@dataclass(frozen=True)
class LabelRow:
    """One already matched pair mean; no implicit defaults or discarded labels.

    cell is input×mode×workers×platform (seed belongs to input). Parent parser
    owns state fingerprints, censoring, timer disjointness and support coverage.
    Future work must be the information available at the intervention, not the
    realized suffix. Only observed_suffix_gain may be None, for an empty future.
    """
    cell: object
    structure: Structure
    execution: Execution
    work: Work
    future: tuple
    incumbent_ns: Fraction | int
    setup_ns: Fraction | int
    startup_ns: Fraction | int
    restart_ns: Fraction | int
    continuation_ns: Fraction | int
    observed_suffix_gain: Fraction | int | None


@dataclass(frozen=True)
class FitResult:
    coefficients: Coefficients
    rational: dict
    rows: int
    cells: int
    suffix_rows: int


def fit(rows):
    """I → B/A/R → C → reserve; quantize once, encoded recheck, or unsupported.

    This does not grant runtime support or certify calibration/qualification.
    Incomplete rows are errors, never silently omitted or replaced.
    """
    rows = tuple(rows)
    if not rows:
        raise Unsupported("no matched calibration rows")
    rational = {}
    # Internal partial state only; never returned before every required fit and
    # recheck succeeds. No calibrated coefficient defaults exist in this API.
    coefficients = Coefficients(*(0 for _ in range(9)))
    try:
        exposure = [exposures(row.structure, row.execution) for row in rows]
        for row in rows:
            row.work.validate(row.execution.r)
            if row.work.H == 0:
                raise Unsupported("current-action fit requires a live forecast")
            for name in ("incumbent_ns", "setup_ns", "startup_ns", "restart_ns", "continuation_ns"):
                _rational(getattr(row, name))
            if row.future:
                _rational(row.observed_suffix_gain, signed=True)
            elif row.observed_suffix_gain is not None and _rational(row.observed_suffix_gain, signed=True) != 0:
                raise Unsupported("nonzero suffix label without scheduled future")

        def stage(names, features, label, upper):
            nonlocal coefficients
            values = fit_envelope([FitPoint(tuple(feature), getattr(row, label), row.cell)
                                   for row, feature in zip(rows, features)], upper=upper)
            rational.update(zip(names, values))
            coefficients = replace(coefficients, **{
                name: quantize(value, upper=upper) for name, value in zip(names, values)})

        def check(label, predictions, upper):
            margin = Fraction(11 if upper else 9, 10)
            for row, prediction in zip(rows, predictions):
                target = margin * _rational(getattr(row, label))
                if (prediction < target if upper else prediction > target):
                    raise Unsupported(f"encoded {label} inequality failed")

        stage(("i_s", "i_p"), [(row.work.H * e.V,
              Fraction(row.work.H * e.O, row.execution.workers))
              for row, e in zip(rows, exposure)], "incumbent_ns", False)
        js = [iteration(coefficients, e, row.execution.workers) for row, e in zip(rows, exposure)]
        check("incumbent_ns", [_mul(row.work.H, j) for row, j in zip(rows, js)], False)
        stage(("b_s", "b_p"), [(e.S, Fraction(e.P, row.execution.workers))
              for row, e in zip(rows, exposure)], "setup_ns", True)
        check("setup_ns", [_add(up(coefficients.b_s, e.S),
              ceil_div(up(coefficients.b_p, e.P), row.execution.workers))
              for row, e in zip(rows, exposure)], True)
        stage(("a_0",), [(row.execution.r * e.Z,) for row, e in zip(rows, exposure)],
              "startup_ns", True)
        applications = [up(coefficients.a_0, e.Z) for e in exposure]
        startups = [_mul(row.execution.r, a) for row, a in zip(rows, applications)]
        check("startup_ns", startups, True)
        stage(("r_0",), [(_add(e.O, e.V),) for e in exposure], "restart_ns", True)
        check("restart_ns", [up(coefficients.r_0, _add(e.O, e.V)) for e in exposure], True)
        stage(("t_0", "t_1"), [(_add(j, startup), _add(_mul(row.work.H, j), _mul(a, row.work.D)))
              for row, j, a, startup in zip(rows, js, applications, startups)], "continuation_ns", True)
        check("continuation_ns", [continuation(coefficients, j, startup, a, row.work)
              for row, j, a, startup in zip(rows, js, applications, startups)], True)
        reserve = Fraction(0)
        suffix_rows = 0
        for row in rows:
            prices = evaluate(coefficients, row.structure, row.execution, row.work, row.future)
            if prices.future_actions:
                suffix_rows += 1
                observed = _rational(row.observed_suffix_gain, signed=True)
                denominator = _mul(prices.application, prices.future_actions)
                reserve = max(reserve, (prices.future_gain - observed + abs(observed) / 10) / denominator)
        rational["kappa_f"] = reserve
        coefficients = replace(coefficients, kappa_f=quantize(reserve, upper=True))
        for row in rows:
            prices = evaluate(coefficients, row.structure, row.execution, row.work, row.future)
            if prices.future_actions:
                observed = _rational(row.observed_suffix_gain, signed=True)
                if prices.future_gain > observed - abs(observed) / 10:
                    raise Unsupported("encoded future-gain inequality failed")
        return FitResult(coefficients, rational, len(rows), len({row.cell for row in rows}), suffix_rows)
    except Unknown as error:
        raise Unsupported(f"encoded arithmetic: {error}") from error
