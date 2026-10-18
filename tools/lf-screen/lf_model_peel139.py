"""Classical reference model of the Leapfrog rail walk at upstream 58229f8 (139 ticks, W1 rule). Superseded by lf_model.py.

Mirrors fwd_tick_fast / fast_choice / seed_half_rails in src/point_add/leapfrog.rs at the integer level:
two's-complement rails at the per-step widths of env_w1_peel139_steps.txt, wrap-around adds, and a
failure flag whenever a resize drops a bit that is not a sign extension (the circuit would free a dirty
wire) or the walk ends off park (rails not +-1).
"""
import os, sys

P = 2**256 - 2**32 - 977
N = 256
HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.environ.get("LF_DATA", os.path.join(HERE, "..", "..", "src", "point_add", "leapfrog_data"))


def _rows(name):
    with open(os.path.join(DATA, name)) as f:
        return [l.split() for l in f if l.strip() and not l.startswith("#")]


STEPS = [tuple(int(x) for x in r) for r in _rows("env_w1_peel139_steps.txt")]
ENV = [int(r[0]) for r in _rows("env_w1_peel139.txt")]
with open(os.path.join(DATA, "anch_w1.txt")) as f:
    ANCH = [int(v) for v in f.read().replace("\n", ",").split(",") if v.strip()]
K = 24
R = len(STEPS)


def fits(v, n):
    return -(1 << (n - 1)) <= v < (1 << (n - 1))


def wrap(v, n):
    m = 1 << n
    v %= m
    return v - m if v >= m >> 1 else v


def mag(v):
    # ones'-complement magnitude: the window bits after conjugating with the sign wire
    return v if v >= 0 else ~v


class Fail(Exception):
    def __init__(self, tick, where):
        self.tick, self.where = tick, where


def resize(v, n, t, where):
    if not fits(v, n):
        raise Fail(t, where)
    return v


def w1(T, B, t):
    h = max(ANCH[min(t, len(ANCH) - 1)] - K, 0)
    m = (1 << (K + 2)) - 1
    a = (mag(T) >> h) & m
    b = (mag(B) >> h) & m
    return a > 4 * b


def seed(d):
    """seed_half_rails: R0 = d (odd) or d - p (even d); R1 = (R0 + s p) / 2, s = +1 iff R0 == 3 mod 4."""
    w0 = max(ENV[0], N + 4)
    r0 = d if d & 1 else d - P
    r1 = (r0 + P) // 2 if (r0 & 3) == 3 else (r0 - P) // 2
    assert r1 & 1 and fits(r0, w0) and fits(r1, w0)
    return [r0, r1], w0


def walk(d, trace=False):
    """Returns (ok, fail_tick, where, letters). Letter = (s0, s1, s3, k1, k2)."""
    r, _ = seed(d)
    letters = []
    try:
        for t in range(R):
            w, n0, n1, n2, nb = STEPS[t]
            wn = STEPS[t + 1][0] if t + 1 < R else w
            ti, si = t % 2, 1 - t % 2
            T, B = r[ti], r[si]
            let = []
            for nk in (n0, n1):
                T = resize(T, nk, t, f"T@n{len(let)}")
                B = resize(B, nk, t, f"B@n{len(let)}")
                s = ((T >> 1) ^ (B >> 1)) & 1
                T = wrap(T - B if s else T + B, nk) >> 1
                let.append(s)
            T = resize(T, n2, t, "T@n2")
            B = resize(B, n2, t, "B@n2")
            dq = ((T >> 1) ^ (B >> 1)) & 1
            D = T + B if dq else T - B
            assert D & 3 == 0
            z2, z3, z4 = (D >> 2) & 1, (D >> 3) & 1, (D >> 4) & 1
            a2 = not (z2 or z3 or z4)
            eq = ((T < 0) ^ (B < 0) ^ dq) == 1
            g = z2 and eq
            W = w1(T, B, t) if g else False
            dsel = (not a2) and not (g and not W)
            s3 = int(dsel) ^ dq
            k2 = int(dsel and not z2)
            k1 = int(dsel) ^ (k2 & z3)
            e = k1 + 2 * k2
            T = wrap(T - B if s3 else T + B, n2) >> 1
            T = resize(T, nb, t, "T@nb")
            assert T & ((1 << e) - 1) == 0, (t, e)
            T >>= e
            assert T & 1, t
            let += [s3, k1, k2]
            letters.append(tuple(let))
            T = resize(T, wn, t, "T@wn")
            B = resize(B, wn, t, "B@wn")
            r[ti], r[si] = T, B
            if trace:
                print(t, w, T.bit_length(), B.bit_length(), let)
    except Fail as f:
        return False, f.tick, f.where, letters
    if abs(r[0]) != 1:
        return False, R, "nopark", letters
    if abs(r[1]) != 1:
        # R0 parked, R1 not: the divide's quotient is still right and only its spare payload register is freed
        # dirty, which the simulator turns into a random phase (caught about half the time). Multiply: fatal.
        return False, R, "nopark1", letters
    return True, None, None, letters


def shot(tx, ox, x3):
    """Outcome of one shot: 'ok', 'fail' (certain), or 'coin' (fails with probability about 1/2)."""
    okd, _, wd, _ = walk((tx - ox) % P)
    okm, _, wm, _ = walk((ox - x3) % P)
    if (not okm) or (not okd and wd != "nopark1"):
        return "fail"
    return "coin" if not okd else "ok"


if __name__ == "__main__":
    import random
    rng = random.Random(int(os.environ.get("SEED", "1")))
    n = int(os.environ.get("NWALK", "2000"))
    fails = {}
    for _ in range(n):
        ok, t, where, _ = walk(rng.randrange(1, P))
        if not ok:
            fails[where] = fails.get(where, 0) + 1
    print(n, sum(fails.values()), fails)
