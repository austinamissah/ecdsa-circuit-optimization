#!/usr/bin/env python3
"""Emit the markdown tables for docs/lambda-levers.md from the raw TSVs."""
import glob, math, statistics as st

SHIPPED_SCORE = 1487590242

def lam(pat):
    v = []
    for f in glob.glob(pat):
        for line in open(f):
            p = line.rstrip('\n').split('\t')
            if p[0] != 'tag':
                v.append((int(p[2]), p[8]))
    if not v: return None
    c = [x[0] for x in v]; n = len(c); m = st.mean(c); sd = st.stdev(c)
    return dict(n=n, m=m, sd=sd, sem=sd/math.sqrt(n), fp=len({x[1] for x in v}))

def pre(path, keyfn):
    d = {}
    for i, line in enumerate(open(path)):
        if i == 0: continue
        p = line.rstrip('\n').split('\t')
        d[keyfn(p)] = p
    return d

def delta(a, b):
    """a - b with combined sem and sigma."""
    d = a['m'] - b['m']; s = math.sqrt(a['sem']**2 + b['sem']**2)
    return d, s, abs(d)/s

TAIL = {258:5.228, 259:2.453, 260:1.114, 261:0.483, 262:0.200, 265:0.014}
r = (TAIL[265]/TAIL[262])**(1/3)
for k in (263,264): TAIL[k] = TAIL[262]*r**(k-262)
for k in (266,267): TAIL[k] = TAIL[265]*r**(k-265)

ip = pre('iters/preflight.tsv', lambda p: (p[0], p[1]))
print("### ITERS ladder, deep strip off\n")
print("| ITERS | λ_classical | sem | tail-curve prediction | avgT | peak q | score | vs shipped |")
print("|---|---|---|---|---|---|---|---|")
base = lam('iters/L_261_0_*.tsv')
for it in (259, 261, 262, 264, 267):
    L = lam(f'iters/L_{it}_0_*.tsv')
    if not L: continue
    p = ip.get((str(it), '0'))
    pred = base['m'] - TAIL[261] + TAIL[it]
    sc = int(p[5]); q = p[3]; avgt = float(p[4])
    print(f"| {it}{' **(shipped)**' if it==261 else ''} | {L['m']:.3f} | ±{L['sem']:.3f} | {pred:.3f} | "
          f"{avgt:,.0f} | {q} | {sc:,} | {100*(sc/SHIPPED_SCORE-1):+.2f}% |")

print("\n### Deep strip (SUB4_APPLY_STRIP), ITERS=261\n")
on = lam('iters/L_261_1_*.tsv'); off = lam('iters/L_261_0_*.tsv')
d, s, sig = delta(off, on)
p_on = ip.get(('261','1')); p_off = ip.get(('261','0'))
print("| strip | λ_classical | sem | avgT | peak q | score |")
print("|---|---|---|---|---|---|")
for nm, L, p in (("on (shipped)", on, p_on), ("off", off, p_off)):
    print(f"| {nm} | {L['m']:.3f} | ±{L['sem']:.3f} | {float(p[4]):,.0f} | {p[3]} | {int(p[5]):,} |")
print(f"\nΔλ = {d:+.3f} ± {s:.3f} ({sig:.1f}σ); Δscore = {100*(int(p_off[5])/int(p_on[5])-1):+.2f}%")

try:
    ep = pre('env/preflight.tsv', lambda p: p[0])
    print("\n### Env-knob arms, ITERS=261, deep strip off\n")
    print("| arm | knob | λ_classical | sem | Δλ vs base | avgT | peak q | score | Δscore | % score per λ-unit |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    bp = ep['base261s0']; bs = int(bp[5])
    print(f"| baseline |, | {base['m']:.3f} | ±{base['sem']:.3f} |, | {float(bp[4]):,.0f} | {bp[3]} | {bs:,} |, |, |")
    for name, p in ep.items():
        if name == 'base261s0': continue
        L = lam(f'env/L_{name}_*.tsv')
        if not L: 
            print(f"| {name} | `{p[1]}` | (skipped) | | | {float(p[4]):,.0f} | {p[3]} | {int(p[5]):,} | {100*(int(p[5])/bs-1):+.2f}% | |")
            continue
        d, s, sig = delta(L, base)
        ds = 100*(int(p[5])/bs-1)
        rate = f"{ds/abs(d):.2f}" if abs(d) > 1e-9 else "-"
        print(f"| {name} | `{p[1]}` | {L['m']:.3f} | ±{L['sem']:.3f} | {d:+.3f} ± {s:.3f} ({sig:.1f}σ) | "
              f"{float(p[4]):,.0f} | {p[3]} | {int(p[5]):,} | {ds:+.2f}% | {rate} |")
except (FileNotFoundError, KeyError) as e:
    print(f"\n(env arms not ready: {e})")
