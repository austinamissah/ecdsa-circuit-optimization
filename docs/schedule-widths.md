# GCD per-iteration width schedules: SCHED_J2 and GAP_J2

Data extraction from `src/point_add/trailmix_ludicrous/schedule.rs`. Pure data + consumption
sites; no changes proposed. All values below are the exact table contents as of extraction.

## 1. Lengths and structure

| symbol | value / length | source |
|---|---|---|
| `ITERS` | `258` | `schedule.rs:4` |
| `JUMP` | `2` (asserted in gcd.rs) | `schedule.rs:2` |
| `PAD` | `20` | `schedule.rs:6` |
| `SCHED_J2` | **258 entries** (`&[u16]`) | `schedule.rs:8` |
| `GAP_J2` | **258 entries** (`&[u16]`) | `schedule.rs:24` |

**Both tables are length 258 = `ITERS`, indexed directly by iteration `i ∈ 0..258`.** They are
**not** the 1032-length structure — that length belongs to the *other* schedule tables
(`GCD_SUB_K`, `GCD_BRANCH`, both `[…;1032]` = 4×258, one entry per iteration-step across the 4
sweeps). `SCHED_J2`/`GAP_J2` are per-iteration (258), and the same index `i` is reused unchanged in
both the forward and the reverse sweep (see §3), so all 4 sweeps read the same 258-long schedule.

## 2. Full table contents

Index shown in `[]` at the start of each row, 16 values per row.

### SCHED_J2 (258 values)
```
[  0] 256 256 256 256 256 256 256 256 256 256 256 255 254 253 252 251
[ 16] 250 249 248 247 246 245 244 243 242 241 240 239 238 237 236 235
[ 32] 234 233 232 231 230 229 228 227 226 225 224 223 222 221 220 219
[ 48] 218 217 216 215 214 213 212 211 210 209 208 207 206 205 204 203
[ 64] 202 201 200 199 198 197 196 195 194 193 192 191 190 189 188 187
[ 80] 186 185 184 183 182 181 180 179 178 177 176 175 174 173 173 172
[ 96] 170 169 168 167 166 166 164 164 163 162 160 160 159 157 157 156
[112] 155 154 153 152 151 149 148 147 146 145 145 144 143 141 141 140
[128] 139 138 137 136 135 134 133 131 130 129 128 127 126 126 125 124
[144] 122 122 120 119 118 117 116 115 114 113 112 111 110 109 108 107
[160] 106 105 104 103 102 101 100  99  98  97  96  95  94  93  92  91
[176]  90  89  88  87  86  85  84  83  82  81  80  79  78  77  76  75
[192]  74  73  72  71  70  69  68  67  66  65  64  63  62  61  60  59
[208]  58  57  56  55  54  53  52  51  50  49  48  47  46  45  44  43
[224]  42  41  40  39  38  37  36  35  34  33  32  31  30  29  26  25
[240]  24  23  22  21  21  20  20  19  18  17  17  16  16  16  14  14
[256]  12  11
```

### GAP_J2 (258 values)
```
[  0]  23  25  25  26  27  29  29  30  32  32  34  34  34  34  34  34
[ 16]  35  35  34  35  35  35  34  36  36  35  35  36  35  35  37  35
[ 32]  36  36  36  36  37  36  37  37  37  36  37  37  37  37  38  37
[ 48]  38  37  38  38  38  38  38  38  39  39  39  39  39  39  39  39
[ 64]  39  39  39  39  40  40  40  40  40  40  41  41  42  41  40  41
[ 80]  41  42  42  43  41  41  42  42  42  42  42  42  43  42  44  43
[ 96]  42  42  44  43  43  44  44  45  44  45  44  45  45  44  46  45
[112]  46  46  46  46  46  45  45  46  45  46  47  46  46  46  46  46
[128]  46  46  47  47  47  47  46  47  47  46  46  47  46  48  47  48
[144]  47  48  47  47  48  47  48  48  47  48  48  48  48  48  49  49
[160]  48  50  49  49  49  49  49  50  49  49  50  50  50  51  50  50
[176]  51  50  50  51  51  51  51  51  51  51  52  53  52  52  52  52
[192]  52  52  53  52  52  52  54  53  53  53  52  54  53  54  54  54
[208]  54  54  54  53  52  51  50  49  48  47  46  46  45  44  43  42
[224]  41  40  39  38  37  36  35  34  33  32  31  30  29  28  27  26
[240]  25  24  23  22  22  21  21  20  19  18  18  17  17  17  15  15
[256]  13  12
```

## 3. What each entry physically controls

There is exactly one consumer of each table, in `gcd.rs`, and it appears once per sweep
(forward + reverse), both using the same index `i`:

### `SCHED_J2[i]` → `current_n` — the live bit-width of the `u` and `v` GCD registers
```
gcd.rs:753  (forward)   let current_n = (SCHED_J2[i] as usize).max(1);
gcd.rs:999  (reverse)   let current_n = (SCHED_J2[i] as usize).max(1);
```
`current_n` is the physical width of both working registers at iteration `i`, and therefore the
operand width of every arithmetic op that iteration:
- **Register sizing:** forward pops-and-frees the high qubits of `u`/`v` down to `current_n`
  (`while u.len() > current_n { … zero_and_free }`, `gcd.rs:754-761`); reverse grows them back up
  (`while u.len() < current_n { push alloc_qubit }`, `gcd.rs:1000-1005`).
- **Right-shifts** operate on `v[..current_n]` (`gcd.rs:768,770,775`).
- **Comparator operands** are `v[..current_n]` / `u[..current_n]` (`gcd.rs:791-792`).
- **Compare-and-swap** loop runs `for j in 1..current_n` (`gcd.rs:799`).
- **Controlled modular subtract** (the body adder) runs on `u[..current_n]` / `v[..current_n]`
  (`gcd.rs:827-828`).

So `SCHED_J2[i]` scales the Toffoli cost of the shift, compare, and body steps at iteration `i`.

### `GAP_J2[i]` → `cmp_eff` — the truncated operand width of the swap-decision comparator
```
gcd.rs:763  (forward)   let cmp_eff = (GAP_J2[i] as usize).min(current_n).max(1);
gcd.rs:1006 (reverse)   let cmp_eff = (GAP_J2[i] as usize).min(current_n).max(1);
```
`cmp_eff` is passed as the truncation width to `controlled_swap_decision_v_lt_u`
(`gcd.rs:788-795`, forward) and to `swap_decision_uncompute_vented` (`gcd.rs:1129-1137`, reverse).
Inside `controlled_swap_decision_lt_truncated` (`comparator.rs:768`) it selects the **top `cmp_eff`
limbs** of `u`/`v` for the `v < u` comparison that produces the swap-decision bit. So `GAP_J2[i]`
controls **how many high limbs the comparator looks at** — i.e. the comparator's operand width,
independent of (but clamped to) the full register width.

**Clamp note:** `cmp_eff = min(GAP_J2[i], current_n)`. Whenever `GAP_J2[i] > SCHED_J2[i]` the raw
`GAP_J2` value is capped to `current_n` and the extra is inert. This happens on exactly **20
iterations: i = 238…257** (the tail, where `SCHED_J2` has dropped below `GAP_J2`). On all other
iterations `cmp_eff == GAP_J2[i]`.

## 4. Per-iteration table

Columns: `i`, `SCHED_J2[i]` (= `current_n`, register width), `GAP_J2[i]` (raw comparator width),
`cmp_eff = min(GAP_J2[i], SCHED_J2[i])` (effective comparator width after the clamp). Split into two
side-by-side halves: left is `i = 0…128`, right is `i = 129…257`.

| i | SCHED_J2 | GAP_J2 | cmp_eff | | i | SCHED_J2 | GAP_J2 | cmp_eff |
|--:|--:|--:|--:|---|--:|--:|--:|--:|
| 0 | 256 | 23 | 23 | | 129 | 138 | 46 | 46 |
| 1 | 256 | 25 | 25 | | 130 | 137 | 47 | 47 |
| 2 | 256 | 25 | 25 | | 131 | 136 | 47 | 47 |
| 3 | 256 | 26 | 26 | | 132 | 135 | 47 | 47 |
| 4 | 256 | 27 | 27 | | 133 | 134 | 47 | 47 |
| 5 | 256 | 29 | 29 | | 134 | 133 | 46 | 46 |
| 6 | 256 | 29 | 29 | | 135 | 131 | 47 | 47 |
| 7 | 256 | 30 | 30 | | 136 | 130 | 47 | 47 |
| 8 | 256 | 32 | 32 | | 137 | 129 | 46 | 46 |
| 9 | 256 | 32 | 32 | | 138 | 128 | 46 | 46 |
| 10 | 256 | 34 | 34 | | 139 | 127 | 47 | 47 |
| 11 | 255 | 34 | 34 | | 140 | 126 | 46 | 46 |
| 12 | 254 | 34 | 34 | | 141 | 126 | 48 | 48 |
| 13 | 253 | 34 | 34 | | 142 | 125 | 47 | 47 |
| 14 | 252 | 34 | 34 | | 143 | 124 | 48 | 48 |
| 15 | 251 | 34 | 34 | | 144 | 122 | 47 | 47 |
| 16 | 250 | 35 | 35 | | 145 | 122 | 48 | 48 |
| 17 | 249 | 35 | 35 | | 146 | 120 | 47 | 47 |
| 18 | 248 | 34 | 34 | | 147 | 119 | 47 | 47 |
| 19 | 247 | 35 | 35 | | 148 | 118 | 48 | 48 |
| 20 | 246 | 35 | 35 | | 149 | 117 | 47 | 47 |
| 21 | 245 | 35 | 35 | | 150 | 116 | 48 | 48 |
| 22 | 244 | 34 | 34 | | 151 | 115 | 48 | 48 |
| 23 | 243 | 36 | 36 | | 152 | 114 | 47 | 47 |
| 24 | 242 | 36 | 36 | | 153 | 113 | 48 | 48 |
| 25 | 241 | 35 | 35 | | 154 | 112 | 48 | 48 |
| 26 | 240 | 35 | 35 | | 155 | 111 | 48 | 48 |
| 27 | 239 | 36 | 36 | | 156 | 110 | 48 | 48 |
| 28 | 238 | 35 | 35 | | 157 | 109 | 48 | 48 |
| 29 | 237 | 35 | 35 | | 158 | 108 | 49 | 49 |
| 30 | 236 | 37 | 37 | | 159 | 107 | 49 | 49 |
| 31 | 235 | 35 | 35 | | 160 | 106 | 48 | 48 |
| 32 | 234 | 36 | 36 | | 161 | 105 | 50 | 50 |
| 33 | 233 | 36 | 36 | | 162 | 104 | 49 | 49 |
| 34 | 232 | 36 | 36 | | 163 | 103 | 49 | 49 |
| 35 | 231 | 36 | 36 | | 164 | 102 | 49 | 49 |
| 36 | 230 | 37 | 37 | | 165 | 101 | 49 | 49 |
| 37 | 229 | 36 | 36 | | 166 | 100 | 49 | 49 |
| 38 | 228 | 37 | 37 | | 167 | 99 | 50 | 50 |
| 39 | 227 | 37 | 37 | | 168 | 98 | 49 | 49 |
| 40 | 226 | 37 | 37 | | 169 | 97 | 49 | 49 |
| 41 | 225 | 36 | 36 | | 170 | 96 | 50 | 50 |
| 42 | 224 | 37 | 37 | | 171 | 95 | 50 | 50 |
| 43 | 223 | 37 | 37 | | 172 | 94 | 50 | 50 |
| 44 | 222 | 37 | 37 | | 173 | 93 | 51 | 51 |
| 45 | 221 | 37 | 37 | | 174 | 92 | 50 | 50 |
| 46 | 220 | 38 | 38 | | 175 | 91 | 50 | 50 |
| 47 | 219 | 37 | 37 | | 176 | 90 | 51 | 51 |
| 48 | 218 | 38 | 38 | | 177 | 89 | 50 | 50 |
| 49 | 217 | 37 | 37 | | 178 | 88 | 50 | 50 |
| 50 | 216 | 38 | 38 | | 179 | 87 | 51 | 51 |
| 51 | 215 | 38 | 38 | | 180 | 86 | 51 | 51 |
| 52 | 214 | 38 | 38 | | 181 | 85 | 51 | 51 |
| 53 | 213 | 38 | 38 | | 182 | 84 | 51 | 51 |
| 54 | 212 | 38 | 38 | | 183 | 83 | 51 | 51 |
| 55 | 211 | 38 | 38 | | 184 | 82 | 51 | 51 |
| 56 | 210 | 39 | 39 | | 185 | 81 | 51 | 51 |
| 57 | 209 | 39 | 39 | | 186 | 80 | 52 | 52 |
| 58 | 208 | 39 | 39 | | 187 | 79 | 53 | 53 |
| 59 | 207 | 39 | 39 | | 188 | 78 | 52 | 52 |
| 60 | 206 | 39 | 39 | | 189 | 77 | 52 | 52 |
| 61 | 205 | 39 | 39 | | 190 | 76 | 52 | 52 |
| 62 | 204 | 39 | 39 | | 191 | 75 | 52 | 52 |
| 63 | 203 | 39 | 39 | | 192 | 74 | 52 | 52 |
| 64 | 202 | 39 | 39 | | 193 | 73 | 52 | 52 |
| 65 | 201 | 39 | 39 | | 194 | 72 | 53 | 53 |
| 66 | 200 | 39 | 39 | | 195 | 71 | 52 | 52 |
| 67 | 199 | 39 | 39 | | 196 | 70 | 52 | 52 |
| 68 | 198 | 40 | 40 | | 197 | 69 | 52 | 52 |
| 69 | 197 | 40 | 40 | | 198 | 68 | 54 | 54 |
| 70 | 196 | 40 | 40 | | 199 | 67 | 53 | 53 |
| 71 | 195 | 40 | 40 | | 200 | 66 | 53 | 53 |
| 72 | 194 | 40 | 40 | | 201 | 65 | 53 | 53 |
| 73 | 193 | 40 | 40 | | 202 | 64 | 52 | 52 |
| 74 | 192 | 41 | 41 | | 203 | 63 | 54 | 54 |
| 75 | 191 | 41 | 41 | | 204 | 62 | 53 | 53 |
| 76 | 190 | 42 | 42 | | 205 | 61 | 54 | 54 |
| 77 | 189 | 41 | 41 | | 206 | 60 | 54 | 54 |
| 78 | 188 | 40 | 40 | | 207 | 59 | 54 | 54 |
| 79 | 187 | 41 | 41 | | 208 | 58 | 54 | 54 |
| 80 | 186 | 41 | 41 | | 209 | 57 | 54 | 54 |
| 81 | 185 | 42 | 42 | | 210 | 56 | 54 | 54 |
| 82 | 184 | 42 | 42 | | 211 | 55 | 53 | 53 |
| 83 | 183 | 43 | 43 | | 212 | 54 | 52 | 52 |
| 84 | 182 | 41 | 41 | | 213 | 53 | 51 | 51 |
| 85 | 181 | 41 | 41 | | 214 | 52 | 50 | 50 |
| 86 | 180 | 42 | 42 | | 215 | 51 | 49 | 49 |
| 87 | 179 | 42 | 42 | | 216 | 50 | 48 | 48 |
| 88 | 178 | 42 | 42 | | 217 | 49 | 47 | 47 |
| 89 | 177 | 42 | 42 | | 218 | 48 | 46 | 46 |
| 90 | 176 | 42 | 42 | | 219 | 47 | 46 | 46 |
| 91 | 175 | 42 | 42 | | 220 | 46 | 45 | 45 |
| 92 | 174 | 43 | 43 | | 221 | 45 | 44 | 44 |
| 93 | 173 | 42 | 42 | | 222 | 44 | 43 | 43 |
| 94 | 173 | 44 | 44 | | 223 | 43 | 42 | 42 |
| 95 | 172 | 43 | 43 | | 224 | 42 | 41 | 41 |
| 96 | 170 | 42 | 42 | | 225 | 41 | 40 | 40 |
| 97 | 169 | 42 | 42 | | 226 | 40 | 39 | 39 |
| 98 | 168 | 44 | 44 | | 227 | 39 | 38 | 38 |
| 99 | 167 | 43 | 43 | | 228 | 38 | 37 | 37 |
| 100 | 166 | 43 | 43 | | 229 | 37 | 36 | 36 |
| 101 | 166 | 44 | 44 | | 230 | 36 | 35 | 35 |
| 102 | 164 | 44 | 44 | | 231 | 35 | 34 | 34 |
| 103 | 164 | 45 | 45 | | 232 | 34 | 33 | 33 |
| 104 | 163 | 44 | 44 | | 233 | 33 | 32 | 32 |
| 105 | 162 | 45 | 45 | | 234 | 32 | 31 | 31 |
| 106 | 160 | 44 | 44 | | 235 | 31 | 30 | 30 |
| 107 | 160 | 45 | 45 | | 236 | 30 | 29 | 29 |
| 108 | 159 | 45 | 45 | | 237 | 29 | 28 | 28 |
| 109 | 157 | 44 | 44 | | 238 | 26 | 27 | 26 |
| 110 | 157 | 46 | 46 | | 239 | 25 | 26 | 25 |
| 111 | 156 | 45 | 45 | | 240 | 24 | 25 | 24 |
| 112 | 155 | 46 | 46 | | 241 | 23 | 24 | 23 |
| 113 | 154 | 46 | 46 | | 242 | 22 | 23 | 22 |
| 114 | 153 | 46 | 46 | | 243 | 21 | 22 | 21 |
| 115 | 152 | 46 | 46 | | 244 | 21 | 22 | 21 |
| 116 | 151 | 46 | 46 | | 245 | 20 | 21 | 20 |
| 117 | 149 | 45 | 45 | | 246 | 20 | 21 | 20 |
| 118 | 148 | 45 | 45 | | 247 | 19 | 20 | 19 |
| 119 | 147 | 46 | 46 | | 248 | 18 | 19 | 18 |
| 120 | 146 | 45 | 45 | | 249 | 17 | 18 | 17 |
| 121 | 145 | 46 | 46 | | 250 | 17 | 18 | 17 |
| 122 | 145 | 47 | 47 | | 251 | 16 | 17 | 16 |
| 123 | 144 | 46 | 46 | | 252 | 16 | 17 | 16 |
| 124 | 143 | 46 | 46 | | 253 | 16 | 17 | 16 |
| 125 | 141 | 46 | 46 | | 254 | 14 | 15 | 14 |
| 126 | 141 | 46 | 46 | | 255 | 14 | 15 | 14 |
| 127 | 140 | 46 | 46 | | 256 | 12 | 13 | 12 |
| 128 | 139 | 46 | 46 | | 257 | 11 | 12 | 11 |

## 5. Shape, in words (describing the actual numbers only)

### SCHED_J2 — starts at 256, monotonically non-increasing to 11
- **Non-increasing across the whole table** (verified: `SCHED_J2[i] ≥ SCHED_J2[i+1]` for all `i`).
  Range is 256 (start) down to 11 (end); never rises.
- **Flat plateau at the top:** `i = 0…10` are all `256` (11 entries).
- **Long unit-decrement region:** from `i = 10` the value falls by exactly 1 per step for a long
  run — `256→255→…→173` — the largest contiguous unit-step run is `i ≈ 10…93` (≈83 steps), and a
  second long unit-step run spans `i ≈ 146…237` (≈91 steps: `120→…→29`).
- **A middle region with occasional 2-drops and repeats:** roughly `i ≈ 93…145` the decrement is no
  longer pure unit steps — it mixes single-decrements with **repeats** (consecutive equal values at
  i = 93, 100, 102, 106, 109, 121, 125, 140, 144, e.g. `173,173` and `166,166`) and **drops of 2**
  (at i = 95, 101, 105, 108, 116, 124, 134, 143, 145, e.g. `172→170`, `166→164`). Net effect: it
  still trends down but a little faster than one-per-step through this band.
- **Accelerated tail:** near the end the drops enlarge — a drop of 3 at `i = 237` (`29→26`) and
  drops of 2 at i = 253 and 255 — plus repeats (`21,21`; `20,20`; `17,17`; `16,16,16`; `14,14`)
  before ending `12, 11`.

Summary: an 11-wide plateau at 256, then a predominantly one-bit-per-iteration decline (with a
mildly-faster textured band around i≈93–145 and a slightly accelerated final ~20 iterations),
bottoming at 11.

### GAP_J2 — rises from 23 to a peak of 54, then falls to 12 (NOT monotonic)
- **Not monotonic.** It **rises** over roughly `i = 0…198`, from `23` to its maximum `54` (peak at
  `i = 198`), then **falls** over `i = 198…257` back down to `12`.
- **Rise region (`i ≈ 0…198`):** upward overall but noisy — it climbs in small increments with
  frequent ±1 wobble (160 non-decreasing steps vs 38 small dips). It moves through the low-20s at
  the start, into the 30s by `i ≈ 10–30`, the 40s by `i ≈ 100`, and the low-50s / 54 near the peak.
- **Fall region (`i ≈ 198…257`):** essentially a clean monotone descent — nearly every step is
  non-increasing (57 of 59 steps down), from `54` at the peak to `12` at the end (`54→53→…→13→12`,
  with small flats like `22,22`, `18,18`, `17,17,17`, `15,15`).
- **Interaction with the clamp:** because `cmp_eff = min(GAP_J2, SCHED_J2)` and `SCHED_J2` is
  falling while `GAP_J2` is still elevated in the tail, on `i = 238…257` the raw `GAP_J2` exceeds
  the register width and is clamped down to `current_n`; on those iterations the comparator simply
  uses the full (small) register width. Everywhere else (`i = 0…237`) `cmp_eff` equals `GAP_J2`
  exactly.
