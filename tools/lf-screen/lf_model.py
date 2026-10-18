"""Classical reference model of the Leapfrog rail walk (fast path, recipe at upstream 0c3a075, 138 ticks).

Mirrors fwd_tick_fast / fast_choice / yp8_logic / t0_m_on / seed_half_rails in src/point_add/leapfrog.rs at the
integer level: two's-complement rails at the per-step widths of the pinned table
(pack_t0hat_tail0_138_steps_k770_lw2.txt, selected by LF_G10_TABLE=770), wrap-around adds, and a failure flag
whenever a resize drops a bit that is not a sign extension (the circuit would free a dirty wire) or the walk ends
off park (rails not +-1).

Choice rule per tick (the recipe in install_recipe):
  tick 0 (LF_T0_FREE): no window; equal-sign veto with the sign test m = !s1 & [top 12 bits of |R1| > C],
      C = top bits of p/5 (s0 = 0) or p/3 (s0 = 1).
  ticks 1..R-2 (LF_YP8=22 with LF_YP8_LATE, YP8_VETO=1): window compare Y = [|T2| > 2|B|] on L-bit windows at the
      yp8 anchors; q = E & (Y ^ not_flipped); veto deep at v = 2 when q, at v = 3 when q & added.
  tick R-1 (LF_TAIL_FORCED=0): no forced steps; the LF_W1 rule of fast_choice, [|T| > 4|B|].
"""
import os

P = 2**256 - 2**32 - 977
N = 256
HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.environ.get("LF_DATA", os.path.join(HERE, "..", "..", "src", "point_add", "leapfrog_data"))
STEPS_FILE = os.environ.get("LF_STEPS", "pack_t0hat_tail0_138_steps_k770_lw2.txt")


def _rows(name):
    with open(os.path.join(DATA, name)) as f:
        return [l.split() for l in f if l.strip() and not l.startswith("#")]


STEPS = [tuple(int(x) for x in r) for r in _rows(STEPS_FILE)]
with open(os.path.join(DATA, "yp8_L22_138_anch.txt")) as f:
    ANCH = [int(v) for v in f.read().replace("\n", ",").split(",") if v.strip()]
R = len(STEPS)
L_BASE = 22
# LF_YP8_LATE as the recipe leaves it (the last of its three settings wins)
LATE = [(1, 13), (6, 17), (11, 22), (90, 19), (100, 17), (112, 15), (118, 13), (124, 11)]
T0_KH = 12


def win_k(t):
    """w1_kt: window length L - 2 from the last LATE entry at or before t."""
    l = L_BASE
    for frm, ln in LATE:
        if t >= frm:
            l = ln
    return l - 2


def anchor(t):
    return ANCH[min(t, len(ANCH) - 1)]


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


def window_gt(a, b, t, s):
    """[win(a) > 2^s win(b)] on the K+2-bit windows at tick t (w1_window / w1_chain_s)."""
    k = win_k(t)
    h = max(anchor(t) - k, 0)
    m = (1 << (k + 2)) - 1
    return ((mag(a) >> h) & m) > (((mag(b) >> h) & m) << s)


def seed(d):
    """seed_half_rails: R0 = d (odd) or d - p (even d); R1 = (R0 + s p) / 2, s = +1 iff R0 == 3 mod 4."""
    w0 = max(STEPS[0][0], N + 4)
    r0 = d if d & 1 else d - P
    r1 = (r0 + P) // 2 if (r0 & 3) == 3 else (r0 - P) // 2
    assert r1 & 1 and fits(r0, w0) and fits(r1, w0)
    return [r0, r1], w0


def t0_m(r1, s0, s1):
    """t0_m_on: m = !s1 & [V > C], V = bits 244..255 of |R1| (ones' complement against bit 256)."""
    v = (mag(r1) >> (N - T0_KH)) & ((1 << T0_KH) - 1)
    c = 0x555 if s0 else 0x333
    return int((not s1) and v > c)


def walk(d, trace=False):
    """Returns (ok, fail_tick, where, letters). Letter = forced signs + (s3, k1, k2)."""
    r, _ = seed(d)
    letters = []
    try:
        for t in range(R):
            w, n0, n1, n2, nb = STEPS[t]
            wn = STEPS[t + 1][0] if t + 1 < R else w
            ti, si = t % 2, 1 - t % 2
            T, B = r[ti], r[si]
            tail = t == R - 1
            yp8 = t > 0 and not tail
            let = []
            T1 = None
            for nk in (() if tail else (n0, n1)):
                T = resize(T, nk, t, f"T@n{len(let)}")
                B = resize(B, nk, t, f"B@n{len(let)}")
                s = ((T >> 1) ^ (B >> 1)) & 1
                T1, sgn1 = T, s
                T = wrap(T - B if s else T + B, nk) >> 1
                let.append(s)
            T = resize(T, n2, t, "T@n2")
            B = resize(B, n2, t, "B@n2")
            if yp8:
                # yp8_logic reads the source through a view at the target's width (n1 - 1, at least 6)
                Bv = wrap(B, max(n1 - 1, 6))
            else:
                Bv = B
            dq = ((T >> 1) ^ (Bv >> 1)) & 1
            D = T + B if dq else T - B
            assert D & 3 == 0
            v = (D & -D).bit_length() - 1 if D else 99
            eq = ((T < 0) ^ (Bv < 0) ^ dq) == 1
            if t == 0:
                veto = v == 2 and (1 ^ t0_m(B, let[0], let[1]) ^ dq) == 1
            elif yp8:
                added = (T1 < 0) == ((B < 0) ^ bool(sgn1))
                nf = (T < 0) == (T1 < 0)
                q = eq and (window_gt(T, Bv, t, 1) ^ nf)
                veto = q and (v == 2 or (v == 3 and added))
            else:
                veto = v == 2 and eq and not window_gt(T, B, t, 2)
            dsel = v <= 4 and not veto
            s3 = int(dsel) ^ dq
            e = v - 1 if dsel else 0
            k2, k1 = e >> 1, e & 1
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
        return False, R, "nopark1", letters
    return True, None, None, letters


def shot(tx, ox, x3):
    """Outcome of one shot: 'ok' or 'fail' (certain).

    At 58229f8 a divide walk with R0 parked and R1 not was a coin (a random phase, caught about half the time).
    Here every walk failure is a wrong output: the tail batch lends both parked rails' wires (SL_N2), so an
    unparked R1 no longer leaves only a phase. Measured 5 of 5 divide nopark1 shots wrong on 14 nonces."""
    okd, _, _, _ = walk((tx - ox) % P)
    okm, _, _, _ = walk((ox - x3) % P)
    return "ok" if okd and okm else "fail"


def shot_detail(tx, ox, x3):
    """(divide outcome, multiply outcome), each None or the failure site, for validating the classes."""
    okd, td, wd, _ = walk((tx - ox) % P)
    okm, tm, wm, _ = walk((ox - x3) % P)
    return (None if okd else wd), (None if okm else wm)


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
