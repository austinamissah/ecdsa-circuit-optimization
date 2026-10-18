import sys, glob, collections
import os; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__))); import lf_model as m
c = collections.Counter()
by_site = collections.Counter()
for path in sorted(glob.glob(sys.argv[1] + "/shots/*.tsv")):
    for line in open(path):
        f = line.split()
        if len(f) != 7 or not f[0].isdigit(): continue
        tx, ox, x3 = (int(v, 16) for v in f[2:5])
        o = m.shot(tx, ox, x3)
        sim = "failed" if f[5] == "1" or f[6] == "1" else "clean"
        c[(o, sim)] += 1
        if o != "ok":
            d, mu = m.shot_detail(tx, ox, x3)
            by_site[(f"div={d}", f"mul={mu}", sim, "wrong" if f[5] == "1" else ("phase" if f[6] == "1" else "-"))] += 1
for k in sorted(c): print(k, c[k])
print()
for k in sorted(by_site, key=str): print(k, by_site[k])
