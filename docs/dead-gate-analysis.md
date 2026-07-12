# Candidate D: extending the structural-dead-gate skip tables, risk & opportunity

Read-only investigation. No code changed. Question: is there a *pure-win* extension of the
`TLM_*_SKIP_*` skip tables, removing genuinely-dead Toffolis at no width cost, or are the tables
already covering everything provably dead?

## Verdict up front

**No pure-win extension is accessible from static reading of the repo.** Two independent layers
already harvest deadness, CONSTPROP (a sound live dataflow pass, run to fixpoint every build) and
the baked structural skip tables (23 tables, thousands of entries, all enabled in the submission).
The existing table entries have **no in-repo derivation, proof, or generator**, they are opaque
baked numbers justified only by "eval still passes," which the project's own docs already flag as
insufficient. I cannot point to a single un-skipped Toffoli that is *provably* dead for all inputs;
the obvious candidate classes are already handled by parking/CONSTPROP. So candidate D as literally
stated (hand-add table entries) is **not a safe pure win**: you would be asserting "probably dead,"
the exact forbidden category. The only sound way to remove more is to *strengthen CONSTPROP* (§5),
whose additional yield is unknown but whose safety is structural.

## 1. The safety model, call-index bookkeeping, in full

**Counters.** Each skippable primitive has a private thread-local `Cell<usize>` counter, advanced by
a `next_*_call_index()` that reads-returns-then-increments **on function entry, once per
invocation, not per gate** (`mod.rs:161-169`):
```rust
fn next_call_index(counter) -> usize {
    counter.with(|index| { let current = index.get(); index.set(current + 1); current })
}
```
Counters exist per primitive: compare-direct / compare-cin (`comparator.rs:7-8`), FFG / cuccaro
(`arith.rs:14-21`, `next_cuccaro_call_index`), gidney (`gidney.rs:15`), fold (`fused.rs`), gcd
shifts / hyb / cout (`gcd.rs:9-10`, `mod.rs:122-123`).

**Reset.** All counters reset to 0 exactly once per build, in `load_schedule`
(`mod.rs:262-267`), which is called once at the top of `build_trailmix_ludicrous_ops`
(`mod.rs:360`), before any emission. There is no count-only pre-pass on the live trailmix path that
would double-advance them, so indices are deterministic per build.

**Keying.** A skip predicate maps `(call_index, bit)` to a boolean, in two forms:
- **Range tables** `&[(call, lo, hi)]`, skip if `call == call_index && (lo..=hi).contains(&bit)`
  (e.g. `comparator.rs:643`, `arith.rs`).
- **Exact-key tables** `&[u32]` of packed `(call & 0xffff) << 8 | (bit & 0xff)`, binary-searched
  (`gidney_key`, `gidney.rs:836`; `comparator.rs:176,636`).
- A few are keyed by `call_index` **only** (whole-call skips): `GIDNEY_THREAD_BOUNDARY_DEAD_CALLS`
  (`gidney.rs:266`), the erase-CCZ call lists (`gidney.rs:854,864`).

**Skip = gate never emitted.** The predicate guards emission: `if !…has_structurally_dead…(call_index, i) { circ.ccx(a[i], b[i], next); }` (e.g. `comparator.rs:688-693`,
`gidney.rs:1198-1202`). A skipped gate is never pushed into `circ.ops`.

**The linchpin property (why extension is index-safe).** Because the counter advances on function
entry and *not* per emitted gate, **suppressing a gate does not change any call index.** Adding an
entry to a table therefore removes exactly one gate and shifts nothing. So *extending a table is,
by construction, index-safe*, the danger of candidate D is **not** index perturbation; it is
solely whether the gate is truly dead.

**What DOES break the indices.** Any change that alters the **number or order of primitive
invocations** repoints every baked entry for that primitive downstream of the change. Because entry
`(call=C, bit=B)` means "the C-th invocation's bit B," if an earlier change adds/removes/reorders a
call to that primitive, index `C` now names a *different* emission site and the stale entry can
suppress a **live** gate, silent data corruption that only a full eval would catch. This is the
real landmine (see §4), and it is a landmine for *other* edits, not for table extension itself.

## 2. Where the existing "proof of death" comes from, nowhere in the repo

- **No derivation, no generator, no comments.** The files holding the tables (`gidney.rs`,
  `comparator.rs`, `arith.rs`, `fused.rs`, `gcd.rs`) contain **zero `//` comment lines**. There is
  no `build.rs`, no `scripts/`/`tools/`, no `.py`, and no test that regenerates the `(call,bit)`
  keys. They are baked-in numeric constants with no accompanying proof.
- **The only in-repo justification is "eval passes,"** which `docs/gcd-engine-study.md:286-289`
  already calls out as the unsafe way to establish deadness.
- **The one sound proof engine in the repo is CONSTPROP** (`constprop.rs`), and it is *separate*
  from the tables:
  - A live constant/affine dataflow pass: every qubit/bit starts `Zero`, declared inputs set
    `Unknown` (`constprop.rs:99-106`); it symbolically executes the op stream and **drops any
    Toffoli with a provably-`Zero` control** (`DropZeroCtrl`, `constprop.rs:124-131`), folds
    constant-`One` controls to CX/X (`constprop.rs:132-156`), cancels adjacent inverse pairs
    (`constprop.rs:594,882`), and applies affine complement/equal reasoning (`constprop.rs:294,934`).
  - It **iterates to a fixpoint** (breaks when a full pass transforms nothing, `constprop.rs:997-999`,
    capped `CONSTPROP_MAX_ITERS`).
  - It is re-derived every build and can be *empirically* re-verified (`CONSTPROP_VERIFY` re-simulates
    against secp256k1 and **panics** on any unsound transform, `constprop.rs:844-909`).
- **CONSTPROP runs AFTER the tables, on survivors only** (`build_trailmix_ludicrous_ops`: emit
  everything → `constprop::run(ops, …)` at `mod.rs:499`). So the two are **disjoint by
  construction**: the tables remove gates CONSTPROP never sees; CONSTPROP removes gates the tables
  didn't. On the live build CONSTPROP removes just **269** Toffolis (fixpoint at iter 3:
  `dropped=166, folded_cx=20, inverse_pairs=37, aff_drop=6, aff_fold=3`), i.e. ~0.019%, the
  locally-provable residue after the tables is nearly exhausted.

**So: CONSTPROP = deadness *proven live and re-verifiable*; the static tables = deadness *asserted
as unproven baked numbers*, keyed to fragile emission-order indices.**

## 3. Are there provably-dead gates the tables miss? Not identifiable by inspection

I could not find a single un-skipped Toffoli that is *provably* dead for all inputs. The reasoning
for why the obvious candidate classes are already covered:

- **Controls on provably-constant qubits** (the classic dead/foldable case): already removed *twice
  over*. Structurally, the engine **frees** known-value qubits rather than using them as controls,
  `park_known_one(u[0])` / `park_known_zero(v[0])` `zero_and_free` the parked bit (`gcd.rs:212,233`),
  so a known-0/known-1 qubit is not even present as a control. Anything that slips through is caught
  by CONSTPROP's `DropZeroCtrl`/`FoldCx`/`FoldX` to fixpoint. There is no residue here to harvest.
- **Structural deadness from GCD bit-growth invariants** (a carry/remainder limb that can't matter
  at a given step): this is exactly what the baked tables already encode, and they are *extensive*
  and *fully enabled* (§Appendix): e.g. `COMPARE_STRUCTURAL_DEAD_TOP_RANGES` (396 ranges) +
  `COMPARE_DIRECT_REMAINDER_KEYS` (513 keys), `GIDNEY_THREAD_SUM_REMAINDER_KEYS` (810 keys),
  `FUSED_CLEAN_FOLD_DEAD_RANGES` (492 ranges). Finding *more* of this class requires re-running the
  (absent) structural analysis that produced them; it is not derivable by reading the code.
- **A hypothetical un-skipped provably-dead gate** would have to be simultaneously (a) beyond
  CONSTPROP's constant/affine lattice and (b) missing from the baked structural tables. I can
  hypothesize such gates exist (the tables are unlikely to be provably *complete*), but I cannot
  *prove* any specific one dead from the source, which is the whole bar. **Estimated safely-harvestable
  Toffolis from static inspection: 0.** Any number above that would be a guess.

## 4. The safe-change boundary, concretely

| edit | index effect | safe? |
|---|---|---|
| Add entries to an existing skip table (no code motion) | none, suppressing gates doesn't move counters (§1 linchpin) | **index-safe**; risk is *only* the deadness claim |
| Remove/disable a table | none | safe (just re-adds gates) |
| Change a schedule width that alters chunk count (`GCD_SUB_K`, `HYB_V`, `FOLD_SCHED`, `CMP_K`, `APPLY_COUT_K`) | **shifts** gidney/compare/fold/cout call counts → repoints all downstream baked entries | **dangerous**, silently drops live gates |
| Reorder phases, add/remove a primitive call, refactor emit order | **shifts** the affected primitive's counter | **dangerous** |
| Strengthen CONSTPROP (lattice/affine/word-level) | none, CONSTPROP works on the op stream, index-agnostic | **safe & sound** (§5) |

The crucial asymmetry: hand-adding a table entry is index-safe but *unprovable*; the code changes
that would be *worth* optimizing (widths, reordering) are exactly the ones that **invalidate the
existing tables**. So the tables are simultaneously (a) safe to append to and (b) a hazard that
constrains every other edit near the inner loop.

## 5. Honest verdict, effectively saturated; the only sound "more" is stronger CONSTPROP

Candidate D mirrors the width result (`docs/apply-swap-analysis.md`, `docs/gcd-engine-study.md`): the
low-hanging deadness is already gone, via two layers that between them cover the provable cases,
CONSTPROP (sound, live, to fixpoint, ~269 removed) and the baked structural tables (23 tables, all
enabled). There is no pure-win table extension identifiable by inspection, because the proof-of-death
for any *new* entry does not exist in the repo and cannot be reconstructed by reading the code; a
green eval on 9,024 sampled inputs is not a proof for all inputs.

The one genuinely safe *and* sound version of the idea is **not** editing tables by hand but
**strengthening CONSTPROP** so it *re-proves* more deadness each build:
- It is index-agnostic (operates on the finished op stream), so it is immune to the emission-order
  fragility that makes the baked tables dangerous.
- It is re-derived per build and empirically re-verifiable (`CONSTPROP_VERIFY`), so its removals are
  trustworthy by construction.
- Its yield beyond the current 269 is **unknown**, the extensive baked tables suggest the structural
  deadness they target is largely captured, so a stronger CONSTPROP might find little. But it is the
  only avenue that converts "probably dead" into "provably dead" automatically and safely.

If the goal is a *provable* pure win, the work item is "extend CONSTPROP's reasoning (e.g. model the
GCD register bit-growth bound so it can prove high-limb carries constant-zero) and measure the extra
`dropped=` it reports," not "append rows to the skip tables." The latter cannot be done safely from
what the repo contains.

---

## Appendix, complete skip-table inventory (live engine)

All keyed by emission-order `call_index`. "Enabled" = set on the live `build()` path
(`mod.rs:1886-1914`). Range = `(call,lo,hi)`; Key = packed `(call<<8|bit)`; Call = whole-invocation.

**arith.rs**
- FFG hybrid-carry: `FFG_DEAD_HYBRID_CARRY_RANGES` (arith.rs:95, 94) + `FFG_TOP29_REMAINDER_KEYS` (arith.rs:227, 63); pred `ffg_call_has_structurally_dead_hybrid_carry` (arith.rs:192); flags `TLM_FFG_SKIP_STRUCTURAL_DEAD_CALLS`, `TLM_FFG_SKIP_EXACT_TOP29_REMAINDER`, inline TOP31/TOP30/INVERSE_MOD_SUB_TOP29, all **enabled**; suppresses ccx arith.rs:1185.
- Cuccaro carry: inline `match call_index` (arith.rs:400-420, no table); pred arith.rs:396; flag `TLM_CUCCARO_SKIP_STRUCTURAL_DEAD_CALLS` **enabled**; suppresses ccx arith.rs:483/496.
- Const-chunk: `CONST_CHUNK_DEAD_RANGES` (arith.rs:236, 111) + `CONST_CHUNK_REMAINDER_KEYS` (arith.rs:350, 284); pred arith.rs:381; flags `TLM_CONST_CHUNK_SKIP_STRUCTURAL_DEAD_CALLS`/`_EXACT_REMAINDER` **enabled**; suppresses ccx arith.rs:970.
- Add-const: inline `call==0 && (bit==55||bit>=57)` (arith.rs:427, no table); flag `TLM_ADD_CONST_SKIP_STRUCTURAL_DEAD_CARRIES` **enabled**; suppresses ccx arith.rs:1983.

**comparator.rs**
- Compare-direct top: `COMPARE_STRUCTURAL_DEAD_TOP_RANGES` (comparator.rs:186, 396) + `COMPARE_DIRECT_REMAINDER_KEYS` (comparator.rs:585, 513); pred comparator.rs:631; flags `TLM_COMPARE_SKIP_STRUCTURAL_DEAD_CALLS`/`_EXACT_REMAINDER` **enabled**; suppresses ccx comparator.rs:692.
- Compare-cin: `COMPARE_CIN_STRUCTURAL_DEAD_RANGES` (comparator.rs:32, 116) + `COMPARE_CIN_REMAINDER_KEYS` (comparator.rs:151, 180); pred comparator.rs:171; flags `…STRUCTURAL_DEAD_CALLS`/`TLM_COMPARE_SKIP_EXACT_CIN_REMAINDER` **enabled**; suppresses ccx comparator.rs:823.

**fused.rs**
- Clean-fold: `FUSED_CLEAN_FOLD_DEAD_RANGES` (fused.rs:151, 492) + `FUSED_CLEAN_FOLD_REMAINDER_KEYS` (fused.rs:791, 289); pred fused.rs:883; flags `TLM_FUSED_SKIP_STRUCTURAL_DEAD_CARRIES`, `TLM_FUSED_SKIP_EXACT_FOLD_REMAINDER`, `TLM_FUSED_CLEAN_FOLD_SKIP_TOP31` **enabled**; suppresses ccx fused.rs:1025.
- Chunk-fold: `FUSED_CHUNK_FOLD_DEAD_RANGES` (fused.rs:646, 100) + `FUSED_CHUNK_FOLD_REMAINDER_KEYS` (fused.rs:820, 230); pred fused.rs:890; **enabled**; suppresses ccx fused.rs:1214.
- Dirty-fold: `FUSED_DIRTY_FOLD_DEAD_RANGES` (fused.rs:749, 27); pred fused.rs:896; flag `…_DIRTY_FOLD` **enabled**; suppresses ccx fused.rs:1581.
- Clean-window: `FUSED_CLEAN_WINDOW_DEAD_RANGES` (fused.rs:779, 9); pred fused.rs:903; flag `…_CLEAN_WINDOW` **enabled**; suppresses ccx fused.rs:1646/1663.
- Boundary-zero: `FUSED_BOUNDARY_ZERO_REMAINDER_KEYS` (fused.rs:858, 158); pred fused.rs:878; flag `TLM_FUSED_SKIP_EXACT_BOUNDARY_ZERO` **enabled**; suppresses ccx fused.rs:1309.
- C-double shift0 (cswap): inline (fused.rs:1870, no table); flag `TLM_FUSED_SKIP_STRUCTURAL_DEAD_SHIFT0` **enabled**; suppresses cswap fused.rs:1871/1932.

**gidney.rs**
- Threaded FWD: `GIDNEY_THREAD_FWD_DEAD_RANGES` (gidney.rs:54, 88) + `GIDNEY_THREAD_FWD_REMAINDER_KEYS` (gidney.rs:677, 541); pred gidney.rs:913; flags `TLM_GIDNEY_SKIP_STRUCTURAL_DEAD_CALLS`/`_EXACT_REMAINDER` **enabled**; suppresses ccx gidney.rs:1202/1211.
- Threaded SUM: `GIDNEY_THREAD_SUM_DEAD_RANGES` (gidney.rs:145, 118) + `GIDNEY_THREAD_SUM_REMAINDER_KEYS` (gidney.rs:725, 810); pred gidney.rs:950; **enabled**; suppresses ccx gidney.rs:1237/1266.
- Threaded BOUNDARY (cout): `GIDNEY_THREAD_BOUNDARY_DEAD_CALLS` (gidney.rs:266, 408) + residual (gidney.rs:840, 11); pred gidney.rs:939; flags `…STRUCTURAL_DEAD_CALLS`/`TLM_GIDNEY_SKIP_SMALL_RESIDUAL_DEAD` **enabled**; suppresses ccx gidney.rs:1222.
- Erase CCZ (uncapped): `GIDNEY_ERASE_CCZ_REMAINDER_CALLS` (gidney.rs:854, 98) + residual (gidney.rs:844, 10); pred gidney.rs:885; flag `TLM_GIDNEY_SKIP_EXACT_ERASE_ALL_CCZ` **enabled**; suppresses ccz gidney.rs:1321.
- Erase CCZ (capped): `GIDNEY_ERASE_CAPPED_CCZ_REMAINDER_CALLS` (gidney.rs:864, 248) + residual (gidney.rs:848, 35); pred gidney.rs:899; **enabled**; suppresses ccz gidney.rs:1380.
- **NOT enabled:** `TLM_GIDNEY_SKIP_TOP2_THREAD`, `TLM_GIDNEY_SKIP_FULLVENT_TOP2`, `TLM_SQUARE_SHIFTED_FFG_PREFIX_SKIP`.

**gcd.rs** (cswap/shift, not in the four core files but on the same hot path)
- Forward/reverse cswap: `GCD_REVERSE_CSWAP_DEAD_RANGES` (gcd.rs:269), `GCD_FORWARD/REVERSE_CSWAP_REMAINDER_KEYS` (gcd.rs:415,428); flags `TLM_GCD_SKIP_STRUCTURAL_DEAD_CSWAPS`/`_EXACT_FORWARD_CSWAPS`/`_REVERSE_DIAGONAL_EDGE` **enabled**.
- Shifts: `GCD_SHIFT_DEAD_RANGES` (gcd.rs:483), `GCD_RIGHT/LEFT_SHIFT_REMAINDER_KEYS` (gcd.rs:561,606); flags `TLM_GCD_SKIP_STRUCTURAL_DEAD_SHIFTS`/`_EXACT_SHIFT_REMAINDER` **enabled**.
