#!/usr/bin/env python3
"""Known-answer test: re-mined delta-0 table vs the shipped 12,543 / 3,923."""
import re, sys

def load(path):
    s = open(path).read()
    dead, down = {}, {}
    for m in re.finditer(r'^    \((\d+), (\d+), (\d+), (\d+), (\d+), (\d+), (\d+)\),$', s, re.M):
        k,c2,c1,t,cc,o,tot = map(int, m.groups())
        dead[(k,c2,c1,t,cc,o)] = tot
    for m in re.finditer(r'^    \((\d+), (\d+), (\d+), (\d+), (\d+), (\d+), (\d+), (\d+)\),$', s, re.M):
        k,c2,c1,t,cc,o,tot,act = map(int, m.groups())
        down[(k,c2,c1,t,cc,o)] = (tot, act)
    return dead, down

sd, sw = load(sys.argv[1])   # shipped
md, mw = load(sys.argv[2])   # re-mined
print(f"shipped : {len(sd):6d} dead  {len(sw):6d} downgrade")
print(f"re-mined: {len(md):6d} dead  {len(mw):6d} downgrade")
print(f"delta   : {len(md)-len(sd):+6d} dead  {len(mw)-len(sw):+6d} downgrade\n")

# A shipped key is "live" against the current stream iff its recorded occupancy
# matches what the re-mine measured for the same tuple (the re-mine computed
# occupancy on the current stream). Build tuple->occ from every re-mined key.
occ = {}
for (k,c2,c1,t,cc,o), tot in md.items(): occ[(k,c2,c1,t,cc)] = tot
for (k,c2,c1,t,cc,o), (tot,_) in mw.items(): occ[(k,c2,c1,t,cc)] = tot

def live(keys, getter):
    out = set()
    for key, v in keys.items():
        tot = getter(v)
        tup = key[:5]
        if tup in occ and occ[tup] == tot: out.add(key)
    return out

sd_live = live(sd, lambda v: v)
sw_live = live(sw, lambda v: v[0])
print(f"shipped keys whose tuple occupancy matches the current stream:")
print(f"  dead {len(sd_live)}/{len(sd)}   downgrade {len(sw_live)}/{len(sw)}")

# THE test: every shipped key that addresses a real gate should also be
# certified by a SHALLOWER census. Anything missing = the re-mine saw it FIRE.
mine_any = set(md) | set(mw)
miss_d = sd_live - mine_any
miss_w = sw_live - set(mw) - set(md)
print(f"\nshipped-dead not certified by the re-mine      : {len(miss_d)}")
print(f"shipped-downgrade not certified by the re-mine : {len(miss_w)}")
print(f"  (a nonzero count here means the re-mine OBSERVED THOSE GATES FIRE,")
print(f"   i.e. the shipped census and this one disagree on a live gate)")

# direction check on the ones we do share
both = sd_live & set(md)
print(f"\nshipped-dead also dead in the re-mine : {len(both)}")
reclass = sd_live & set(mw)
print(f"shipped-dead demoted to downgrade     : {len(reclass)}")
extra_d = set(md) - sd_live - set(sw_live)
print(f"re-mine dead NOT in the shipped table : {len(extra_d)}   <- recoverable score")
