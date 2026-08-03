#!/usr/bin/env python3
"""Aggregate lamscreen TSVs into lambda_classical per config, with sigmas."""
import glob, math, sys, collections

def load(pattern):
    rows = []
    for f in glob.glob(pattern):
        for line in open(f):
            p = line.rstrip('\n').split('\t')
            if not p or p[0] == 'tag':
                continue
            rows.append((p[0], int(p[1]), int(p[2]), p[8]))
    return rows

def stats(rows):
    v = [r[2] for r in rows]
    n = len(v)
    m = sum(v)/n
    sd = math.sqrt(sum((x-m)**2 for x in v)/(n-1))
    return n, m, sd, sd/math.sqrt(n), len({r[3] for r in rows})

def report(label, pattern, base=None):
    rows = load(pattern)
    if not rows:
        print(f"{label:22s}  (no data)"); return None
    n, m, sd, sem, nfp = stats(rows)
    extra = ""
    if base:
        bn, bm, bsd, bsem, _ = base
        d = m - bm
        dsem = math.sqrt(sem**2 + bsem**2)
        extra = f"   delta={d:+.3f} +/- {dsem:.3f}  ({abs(d)/dsem:.1f} sigma)"
    print(f"{label:22s}  n={n:4d}  lambda_c={m:6.3f}  sd={sd:5.3f}  sem={sem:.3f}  fp={nfp:4d}{extra}")
    return (n, m, sd, sem, nfp)

if __name__ == '__main__':
    specs = sys.argv[1:]
    base = None
    for i, s in enumerate(specs):
        label, pattern = s.split('=', 1)
        r = report(label, pattern, base if i else None)
        if i == 0:
            base = r
