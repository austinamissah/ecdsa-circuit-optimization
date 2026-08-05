#!/usr/bin/env python3
"""Re-derive lambda_total from a committed harness sweep TSV.

This is the analysis side of `lambda-sweep-driver*.sh`. The driver produces the
TSV; this turns the TSV into the published figures. It reads only the committed
data, so it needs no build, no circuit and no network.

    python3 docs/data/analyze-sweep.py docs/data/lambda-sweep-6909d15.tsv --seed 20260804

Not to be confused with `tools/lam-screen/drivers/analyze.py`, which aggregates
*lam-screen* TSVs (a different schema) and reports lambda_classical only. That
screen covers the classical channel, so it cannot produce lambda_total at all.

Estimator, matching `lambda-measurement.md`:

    classical ~ Pois(l_c + l_both),  phase ~ Pois(l_p + l_both),
    independent components, so Cov(c, p) = l_both and

    lambda_total = mean_c + mean_p - Cov(c, p)

Some shots fail on both channels at once, so adding the two means double-counts
them; the covariance measures exactly that overlap. Covariance and sd use the
sample convention (ddof=1). The CI is a percentile bootstrap over whole rows,
resampled with replacement, using linear interpolation between order statistics
(the numpy default).

Control rows are excluded from every statistic. They are repeat runs of the
head's own shipped nonce and must all come back 0/0/0; if any does not, the
sweep is void and this script says so.
"""
import argparse
import csv
import math
import random
import sys
from collections import Counter


def load(path):
    with open(path, newline="") as fh:
        rows = list(csv.DictReader(fh, delimiter="\t"))
    need = {"nonce", "classical", "phase", "ancilla"}
    missing = need - set(rows[0] if rows else {})
    if missing:
        sys.exit(
            f"{path}: missing column(s) {sorted(missing)}.\n"
            "This script reads harness sweep TSVs (block/nonce/exit/classical/phase/"
            "ancilla/avgT/md5).\nFor lam-screen TSVs use tools/lam-screen/drivers/analyze.py."
        )
    return rows


def percentile(sorted_vals, q):
    """Linear-interpolated percentile, the numpy default convention."""
    if not sorted_vals:
        return float("nan")
    idx = q * (len(sorted_vals) - 1)
    lo = int(idx)
    frac = idx - lo
    if lo + 1 >= len(sorted_vals):
        return sorted_vals[lo]
    return sorted_vals[lo] * (1 - frac) + sorted_vals[lo + 1] * frac


def moments(vals):
    n = len(vals)
    mean = sum(vals) / n
    var = sum((v - mean) ** 2 for v in vals) / (n - 1)
    sd = math.sqrt(var)
    return n, mean, sd, sd / math.sqrt(n), var / mean if mean else float("nan")


def covariance(xs, ys):
    n = len(xs)
    mx = sum(xs) / n
    my = sum(ys) / n
    return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / (n - 1)


def lambda_total(pairs):
    cs = [c for c, _ in pairs]
    ps = [p for _, p in pairs]
    return sum(cs) / len(cs) + sum(ps) / len(ps) - covariance(cs, ps)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("tsv", help="e.g. docs/data/lambda-sweep-6909d15.tsv")
    ap.add_argument("--seed", type=int, default=20260804,
                    help="bootstrap seed (default 20260804, the seed lambda-6909d15.md records)")
    ap.add_argument("--resamples", type=int, default=4000)
    ap.add_argument("--control", type=int, default=None,
                    help="shipped nonce; default is whichever nonce appears more than once")
    args = ap.parse_args()

    rows = load(args.tsv)

    control = args.control
    if control is None:
        repeated = [n for n, k in Counter(int(r["nonce"]) for r in rows).items() if k > 1]
        if len(repeated) == 1:
            control = repeated[0]
        elif len(repeated) > 1:
            sys.exit(f"ambiguous control: nonces {repeated} each appear more than once; pass --control")

    controls = [r for r in rows if control is not None and int(r["nonce"]) == control]
    trials = [r for r in rows if control is None or int(r["nonce"]) != control]

    print(f"file        : {args.tsv}")
    print(f"rows        : {len(rows)} total, {len(controls)} control, {len(trials)} counted")

    if controls:
        dirty = [r for r in controls
                 if (int(r["classical"]), int(r["phase"]), int(r["ancilla"])) != (0, 0, 0)]
        verdict = "all 0/0/0" if not dirty else f"{len(dirty)} NOT 0/0/0 -- SWEEP IS VOID"
        print(f"control     : nonce {control}, {verdict}")
        if dirty:
            sys.exit("refusing to report statistics on a void sweep")
    else:
        print("control     : none identified (statistics cover every row)")

    cs = [int(r["classical"]) for r in trials]
    ps = [int(r["phase"]) for r in trials]
    an = [int(r["ancilla"]) for r in trials]

    print()
    print("channel     mean       sd      sem   var/mean   range")
    for name, v in (("classical", cs), ("phase", ps), ("ancilla", an)):
        if not any(v):
            print(f"{name:<10}  {0.0:6.3f}   {0.0:6.3f}      n/a        n/a   0")
            continue
        n, mean, sd, sem, vm = moments(v)
        print(f"{name:<10}  {mean:6.3f}   {sd:6.3f}   +/-{sem:.3f}     {vm:6.3f}   {min(v)} to {max(v)}")

    n = len(trials)
    mc = sum(cs) / n
    mp = sum(ps) / n
    cov = covariance(cs, ps)
    total = mc + mp - cov

    pairs = list(zip(cs, ps))
    rng = random.Random(args.seed)
    boot = sorted(lambda_total([pairs[rng.randrange(n)] for _ in range(n)])
                  for _ in range(args.resamples))
    lo, hi = percentile(boot, 0.025), percentile(boot, 0.975)

    p_clean = math.exp(-total)
    sd_c = math.sqrt(sum((v - mc) ** 2 for v in cs) / (n - 1))
    sd_p = math.sqrt(sum((v - mp) ** 2 for v in ps) / (n - 1))
    rho = cov / (sd_c * sd_p)

    print()
    print(f"lambda_total      : {total:.3f}   95% CI {lo:.3f} to {hi:.3f}"
          f"  ({args.resamples} resamples, seed {args.seed})")
    print(f"P(clean)          : {p_clean:.3g}")
    print(f"trials per seed   : {1 / p_clean:.3g}")
    print(f"decomposition     : classical_only {mc - cov:.3f}, both {cov:.3f}, phase_only {mp - cov:.3f}")
    print(f"Pearson rho(c,p)  : {rho:.3f}")
    print(f"bounds            : max(means) {max(mc, mp):.3f}, sum(means) {mc + mp:.3f}")
    print()
    print(f"zero-classical {sum(1 for v in cs if v == 0)}/{n} · "
          f"zero-phase {sum(1 for v in ps if v == 0)}/{n} · "
          f"zero-both {sum(1 for c, p in pairs if c == 0 and p == 0)}/{n}")


if __name__ == "__main__":
    main()
