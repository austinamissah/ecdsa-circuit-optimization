//! Classical convergence pre-filter for the ping-pong Fiat-Shamir nonce search.
//!
//! The 96-op X tail at the end of the op stream is an identity: it changes no
//! executed gate, only the Fiat-Shamir seed, and therefore the 9,024 test
//! inputs. A nonce is valid only if the particular draw it induces happens to
//! hit no failure. Screening one nonce with `eval_circuit` costs ~20 s, which
//! is far too slow to search a space where roughly one draw in 4 x 10^5 is
//! clean.
//!
//! This tool reproduces that condition in classical arithmetic, at tens of
//! microseconds per shot instead of ~530 us, and aborts at the first failing
//! shot. A doomed nonce dies in milliseconds rather than 20 s.
//!
//! # The walk, and why it is exact
//!
//! * The walk state is a pair of two's-complement values `(u, v)` starting at
//!   `(p, lift(denominator))`, where `lift` maps an even denominator to the
//!   congruent negative representative `a - p` so both values are odd.
//! * Each round adds or subtracts `source` into `target` (the choice is
//!   `bit1(target) ^ bit1(source)`, which is exactly the rule that keeps the
//!   result odd after the shift), wrapping at the round's scheduled width, then
//!   arithmetic-shifts `target` right by one.
//! * `shrink_to` drops the top wire of each register by XORing bit `w-1` into
//!   bit `w` and freeing it. That free is clean only if the value sign-extends
//!   into `w` bits. If it does not, the freed qubit is dirty, which the
//!   simulator reports. So a width violation is a hard, exactly-checkable
//!   failure, not an approximation.
//! * The walk must end at `(+/-1, +/-1)`: the terminal passenger loan frees
//!   every bit below the sign as a sign copy and bit 0 as a constant 1.
//!
//! # The replay, and which truncation causes which failure
//!
//! The walk records a sign tape; the replay consumes it once to build the
//! coefficient. Reading the gates settles something the failure counts alone
//! cannot. The chunk boundary carries and the overflow flag are erased by
//! `cmp_lt_phase_conditioned`, a *phase*-conditioned correction, so when its
//! truncated 22-bit comparison disagrees with the true carry the shot takes a
//! phase error, not a wrong value. Those cannot produce a classical mismatch.
//!
//! The one channel that can is the pseudo-Mersenne fold. It adds one of
//! `{-f, 0, +f, +2f}` into the low `REPLAY_FOLD_WINDOW = 54` bits and discards
//! the carry out of bit 53, so whenever that carry was needed the value is
//! wrong by 2^54. Both traversals are modeled: the fused halving cell for the
//! divide, the fused doubling cell for the multiply, plus `mod_halve_pm`,
//! `mod_double_pm` and the round-one seeds, whose `csub`/`cadd` chains truncate
//! at `min(n-2, highest_set_bit(c) + window) + 1`.
//!
//! In both cells the true carry out of fold position 0 works out to exactly the
//! quantity the circuit injects as `first_carry`, which is what makes the fold
//! equivalent to a plain windowed add.
//!
//! # What this does and does not buy
//!
//! Measured on the shipped 698/696 config, the model catches about 65% of
//! classical failures with no false positives: the shipped nonce, which is
//! 9,024/9,024 clean, reports zero escapes across both traversals.
//!
//! That is worth a great deal on eval overhead, taking full `eval_circuit` runs
//! per clean nonce from roughly 87,000 to 830. It does **not** make a grind
//! viable on one workstation, and no better model would. P(clean draw) is about
//! e^-19.5, so a clean nonce costs ~2.9 x 10^8 draws whatever the filter does.
//! The bottleneck is raw draw throughput, not filter quality.
//!
//! Build (it deliberately does not live in `src/`, so it can never be swept
//! into a submission by `editablePaths`):
//!
//!   cp tools/pp-screen/screen.rs src/bin/pp_screen.rs
//!   cargo build --release --bin pp_screen
//!   rm src/bin/pp_screen.rs
//!
//! Usage:
//!   pp_screen --ops ops.bin --from <n> --count <k> [--threads T] [--rounds R]
//!             [--rounds-mul M] [--out survivors.txt] [--verbose] [--envelope]
//!
//! Always drive it through `grind.sh`, which rebuilds ops.bin for the config
//! being screened. The seed is a hash of the whole op stream, so screening one
//! depth against another depth's ops.bin silently produces noise.
//!
//! The op stream is read once; the SHAKE256 state is cloned after absorbing
//! everything but the last 96 ops, so re-seeding a nonce costs 96 records.

use alloy_primitives::U256;
use quantum_ecc::weierstrass_elliptic_curve::WeierstrassEllipticCurve;
use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// ─── op stream framing (mirrors eval_circuit's loader) ─────────────────────

const MAGIC: &[u8; 8] = b"QECCOPSZ";
const ZSTD_WINDOW_LOG_MAX: u32 = 27;
const OP_BYTES: usize = 7 * 8;
/// Bytes absorbed per op: the kind discriminant, then six u64 fields.
const HASH_BYTES: usize = 1 + 6 * 8;
const TAIL_OPS: usize = 96;
const NUM_TESTS: usize = 9024;

// ─── walk geometry (mirrors pingpong_div.rs) ───────────────────────────────

const N: usize = 256;
const VALUE_WIDTH: usize = N + 3;
// The width schedule is READ FROM THE BUILDER, never recomputed here.
//
// It has moved three times: a sampled table, then a greedy table in its own
// file, then back to the embedded table with a compressing rescale switched on
// by default and a sparse repair set switched off. An earlier version of this
// tool hard-coded one of those and stopped compiling the moment upstream
// deleted the file. Worse than not compiling would have been still compiling
// against a stale table, which produces confident nonsense.
//
// `tools/pp-screen/instrument.py` adds a dump to the builder; `grind.sh` runs
// it. The `#width` rows are the resolved widths, with rescale, repair, bias and
// the round-0 special case already applied, so this is a pure lookup.
static SCHEDULE: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
static DUMPED_ROUNDS: std::sync::OnceLock<(usize, usize)> = std::sync::OnceLock::new();
/// Resolved truncation windows: (fold_div, fold_mul, endpoint, chunk_cmp, flag_cmp).
/// Read from the dump for the same reason as the schedule: these are set by
/// `set_default_env` in mod.rs, not by the literals in `pingpong_div.rs`, and the
/// two traversals no longer share a fold window.
static WINDOWS: std::sync::OnceLock<(usize, usize, usize, usize, usize)> =
    std::sync::OnceLock::new();

fn win() -> (usize, usize, usize, usize, usize) {
    *WINDOWS.get().expect("windows not loaded; pass --geometry")
}

fn value_width(round: usize) -> usize {
    let table = SCHEDULE.get().expect("width schedule not loaded; pass --geometry");
    table.get(round).map_or(8, |w| (*w as usize).clamp(8, VALUE_WIDTH))
}

/// Load the resolved geometry the builder dumped under `PP_GEOMETRY`.
fn load_geometry(path: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("{path}: {e} (run grind.sh, or build with PP_GEOMETRY set)"))?;
    let mut widths: Vec<(usize, u16)> = Vec::new();
    let mut rounds: Option<(usize, usize)> = None;
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.first().copied() {
            Some("#rounds") if f.len() >= 3 => {
                rounds = Some((
                    f[1].parse().map_err(|_| "bad #rounds")?,
                    f[2].parse().map_err(|_| "bad #rounds")?,
                ));
            }
            Some("#windows") if f.len() >= 6 => {
                let g = |i: usize| f[i].parse::<usize>().map_err(|_| "bad #windows");
                let _ = WINDOWS.set((g(1)?, g(2)?, g(3)?, g(4)?, g(5)?));
            }
            Some("#width") if f.len() >= 3 => {
                widths.push((
                    f[1].parse().map_err(|_| "bad #width round")?,
                    f[2].parse().map_err(|_| "bad #width value")?,
                ));
            }
            _ => {}
        }
    }
    let (div, mul) = rounds.ok_or("geometry file has no #rounds line")?;
    if WINDOWS.get().is_none() {
        return Err("geometry file has no #windows line; re-run instrument.py".into());
    }
    if widths.is_empty() {
        return Err("geometry file has no #width rows".into());
    }
    let mut table = vec![0u16; widths.iter().map(|(r, _)| *r).max().unwrap() + 1];
    for (r, w) in widths {
        table[r] = w;
    }
    if table.len() < div.max(mul) {
        return Err(format!(
            "geometry covers {} rounds but the walk runs {}",
            table.len(),
            div.max(mul)
        ));
    }
    let _ = SCHEDULE.set(table);
    let _ = DUMPED_ROUNDS.set((div, mul));
    Ok(())
}

// ─── 320-bit two's-complement arithmetic ───────────────────────────────────
//
// The envelope is 259 bits, so five limbs carry every intermediate with room
// for the sign. Values are always held sign-extended to the full 320 bits.

const LIMBS: usize = 5;
type W = [u64; LIMBS];

fn w_zero() -> W {
    [0; LIMBS]
}

fn from_u256(x: U256) -> W {
    let l = x.into_limbs();
    [l[0], l[1], l[2], l[3], 0]
}

fn is_neg(a: &W) -> bool {
    a[LIMBS - 1] >> 63 == 1
}

fn bit(a: &W, i: usize) -> u64 {
    (a[i / 64] >> (i % 64)) & 1
}

fn add(a: &W, b: &W) -> W {
    let mut out = w_zero();
    let mut carry = 0u64;
    for i in 0..LIMBS {
        let (s1, c1) = a[i].overflowing_add(b[i]);
        let (s2, c2) = s1.overflowing_add(carry);
        out[i] = s2;
        carry = (c1 as u64) | (c2 as u64);
    }
    out
}

fn neg(a: &W) -> W {
    let mut inv = w_zero();
    for i in 0..LIMBS {
        inv[i] = !a[i];
    }
    let mut one = w_zero();
    one[0] = 1;
    add(&inv, &one)
}

fn sub(a: &W, b: &W) -> W {
    add(a, &neg(b))
}

/// Arithmetic shift right by one, on a value already sign-extended to 320 bits.
fn sar1(a: &W) -> W {
    let mut out = w_zero();
    for i in 0..LIMBS - 1 {
        out[i] = (a[i] >> 1) | (a[i + 1] << 63);
    }
    out[LIMBS - 1] = ((a[LIMBS - 1] as i64) >> 1) as u64;
    out
}

/// Does `a` sign-extend into `w` bits, i.e. is every bit from `w-1` up equal to
/// the sign? This is exactly the condition for `shrink_to`'s frees to be clean.
fn fits_signed(a: &W, w: usize) -> bool {
    debug_assert!(w >= 1 && w <= 320);
    let sign = if is_neg(a) { u64::MAX } else { 0 };
    // Every bit at index >= w-1 must equal the sign bit. Compare a limb at a
    // time: the limb holding bit w-1 is masked to just its high part.
    let start = w - 1;
    let (first, off) = (start / 64, start % 64);
    if a[first] >> off != sign >> off {
        return false;
    }
    for i in first + 1..LIMBS {
        if a[i] != sign {
            return false;
        }
    }
    true
}

/// Truncate to `w` bits and sign-extend back out to 320. Mirrors the circuit's
/// wrapping add at the round's scheduled width.
fn wrap_signed(a: &W, w: usize) -> W {
    if w >= 320 {
        return *a;
    }
    let mut out = w_zero();
    // Keep the low w bits.
    for i in 0..LIMBS {
        let lo = i * 64;
        if lo >= w {
            break;
        }
        let keep = (w - lo).min(64);
        out[i] = if keep == 64 { a[i] } else { a[i] & ((1u64 << keep) - 1) };
    }
    // Sign-extend from bit w-1.
    if bit(&out, w - 1) == 1 {
        for i in 0..LIMBS {
            let lo = i * 64;
            if lo + 64 <= w {
                continue;
            }
            if lo >= w {
                out[i] = u64::MAX;
            } else {
                out[i] |= !((1u64 << (w - lo)) - 1);
            }
        }
    }
    out
}

fn is_pm_one(a: &W) -> bool {
    let mut one = w_zero();
    one[0] = 1;
    *a == one || *a == neg(&one)
}

// ─── the walk ──────────────────────────────────────────────────────────────

/// As [`walk_ok`], but also returns the sign tape the walk records. The replay
/// consumes that tape, so the two halves of the model share it.
fn walk_tape(p: &W, denominator: U256, rounds: usize) -> Option<Vec<bool>> {
    walk_tape_signs(p, denominator, rounds).map(|(t, _, _)| t)
}

/// As `walk_tape`, and also the terminal signs of u and v. The multiply replay
/// seeds both registers from the numerator conditionally negated by those.
fn walk_tape_signs(p: &W, denominator: U256, rounds: usize) -> Option<(Vec<bool>, bool, bool)> {
    let a = from_u256(denominator);
    let mut v = if bit(&a, 0) == 1 { a } else { sub(&a, p) };
    let mut u = *p;
    let mut tape = Vec::with_capacity(rounds);

    for r in 0..rounds {
        let w = value_width(r);
        if !fits_signed(&u, w) || !fits_signed(&v, w) {
            return None;
        }
        let even = r % 2 == 0;
        let (src, tgt) = if even { (u, v) } else { (v, u) };
        let sign = bit(&tgt, 1) ^ bit(&src, 1);
        tape.push(sign == 1);
        let s = if sign == 1 { sub(&tgt, &src) } else { add(&tgt, &src) };
        let shifted = sar1(&wrap_signed(&s, w));
        if even {
            v = shifted;
        } else {
            u = shifted;
        }
    }
    if is_pm_one(&u) && is_pm_one(&v) {
        Some((tape, is_neg(&u), is_neg(&v)))
    } else {
        None
    }
}

/// Run the ping-pong walk on `(p, lift(denominator))` for `rounds` rounds.
/// Returns true iff every scheduled shrink is clean and the walk terminates at
/// `(+/-1, +/-1)`, which is exactly the condition the circuit needs.
fn walk_ok(p: &W, denominator: U256, rounds: usize) -> bool {
    let a = from_u256(denominator);
    // Ping-pong's signed recurrence requires both values odd. An even
    // denominator lifts to the congruent negative representative a - p.
    let mut v = if bit(&a, 0) == 1 { a } else { sub(&a, p) };
    let mut u = *p;

    for r in 0..rounds {
        let w = value_width(r);
        if !fits_signed(&u, w) || !fits_signed(&v, w) {
            return false;
        }
        // Even rounds add u into v; odd rounds add v into u.
        let even = r % 2 == 0;
        let (src, tgt) = if even { (u, v) } else { (v, u) };
        let sign = bit(&tgt, 1) ^ bit(&src, 1);
        let s = if sign == 1 { sub(&tgt, &src) } else { add(&tgt, &src) };
        let shifted = sar1(&wrap_signed(&s, w));
        if even {
            v = shifted;
        } else {
            u = shifted;
        }
    }

    is_pm_one(&u) && is_pm_one(&v)
}

// ─── secp256k1 field and group ─────────────────────────────────────────────
//
// The harness curve does one `inv_mod` per point operation, which is ~770
// inversions per shot and completely dominates screening. Everything here
// exists to avoid that: a pseudo-Mersenne field, Jacobian point arithmetic so a
// scalar multiplication needs no inversion at all, a fixed-base window table so
// it needs no doublings either, and Montgomery batch inversion so a whole block
// of shots shares one inversion. Validated against the harness curve in
// `selftest`.

/// Field element, four 64-bit limbs, little-endian, always fully reduced.
type Fe = [u64; 4];

const P: Fe = [0xFFFF_FFFE_FFFF_FC2F, u64::MAX, u64::MAX, u64::MAX];
/// 2^256 mod p = 2^32 + 977.
const FOLD: u64 = 0x1_0000_03D1;

fn fe_zero() -> Fe {
    [0; 4]
}

fn fe_is_zero(a: &Fe) -> bool {
    *a == [0; 4]
}

fn fe_ge_p(a: &Fe) -> bool {
    for i in (0..4).rev() {
        if a[i] != P[i] {
            return a[i] > P[i];
        }
    }
    true
}

fn fe_sub_p(a: &Fe) -> Fe {
    let mut out = fe_zero();
    let mut borrow = 0u64;
    for i in 0..4 {
        let (d1, b1) = a[i].overflowing_sub(P[i]);
        let (d2, b2) = d1.overflowing_sub(borrow);
        out[i] = d2;
        borrow = (b1 as u64) | (b2 as u64);
    }
    out
}

fn fe_add(a: &Fe, b: &Fe) -> Fe {
    let mut out = fe_zero();
    let mut carry = 0u128;
    for i in 0..4 {
        let cur = a[i] as u128 + b[i] as u128 + carry;
        out[i] = cur as u64;
        carry = cur >> 64;
    }
    // A carry out of 256 bits folds back in as FOLD.
    if carry != 0 {
        let mut c = FOLD as u128;
        for limb in out.iter_mut() {
            let cur = *limb as u128 + (c & 0xFFFF_FFFF_FFFF_FFFF);
            *limb = cur as u64;
            c = (c >> 64) + (cur >> 64);
        }
    }
    if fe_ge_p(&out) {
        out = fe_sub_p(&out);
    }
    out
}

fn fe_sub(a: &Fe, b: &Fe) -> Fe {
    let mut out = fe_zero();
    let mut borrow = 0u64;
    for i in 0..4 {
        let (d1, b1) = a[i].overflowing_sub(b[i]);
        let (d2, b2) = d1.overflowing_sub(borrow);
        out[i] = d2;
        borrow = (b1 as u64) | (b2 as u64);
    }
    if borrow != 0 {
        // Add p back.
        let mut carry = 0u128;
        for i in 0..4 {
            let cur = out[i] as u128 + P[i] as u128 + carry;
            out[i] = cur as u64;
            carry = cur >> 64;
        }
    }
    out
}

/// Reduce a 512-bit product modulo p using 2^256 = 2^32 + 977.
fn fe_reduce(t: &[u64; 8]) -> Fe {
    // First fold: lo + hi * FOLD, which fits in five limbs.
    let mut r = [0u64; 5];
    let mut carry = 0u128;
    for i in 0..4 {
        let cur = t[i] as u128 + (t[4 + i] as u128) * (FOLD as u128) + carry;
        r[i] = cur as u64;
        carry = cur >> 64;
    }
    r[4] = carry as u64;

    // Second fold: the top limb is small, so its product with FOLD spans two
    // limbs and one more pass suffices.
    let prod = (r[4] as u128) * (FOLD as u128);
    let padd = [prod as u64, (prod >> 64) as u64, 0u64, 0u64];
    let mut out = fe_zero();
    let mut carry = 0u128;
    for i in 0..4 {
        let cur = r[i] as u128 + padd[i] as u128 + carry;
        out[i] = cur as u64;
        carry = cur >> 64;
    }
    if carry != 0 {
        let mut c = FOLD as u128;
        for limb in out.iter_mut() {
            let cur = *limb as u128 + (c & 0xFFFF_FFFF_FFFF_FFFF);
            *limb = cur as u64;
            c = (c >> 64) + (cur >> 64);
        }
    }
    while fe_ge_p(&out) {
        out = fe_sub_p(&out);
    }
    out
}

fn fe_mul(a: &Fe, b: &Fe) -> Fe {
    let mut t = [0u64; 8];
    for i in 0..4 {
        let mut carry = 0u128;
        for j in 0..4 {
            let cur = t[i + j] as u128 + (a[i] as u128) * (b[j] as u128) + carry;
            t[i + j] = cur as u64;
            carry = cur >> 64;
        }
        t[i + 4] = carry as u64;
    }
    fe_reduce(&t)
}

fn fe_sqr(a: &Fe) -> Fe {
    fe_mul(a, a)
}

/// a^(p-2) mod p. Only used once per block thanks to batch inversion.
fn fe_inv(a: &Fe) -> Fe {
    // p - 2 = 2^256 - 2^32 - 979.
    let exp: Fe = {
        let mut e = P;
        // Subtract 2.
        let (d, b) = e[0].overflowing_sub(2);
        e[0] = d;
        if b {
            let mut i = 1;
            while i < 4 {
                let (d2, b2) = e[i].overflowing_sub(1);
                e[i] = d2;
                if !b2 {
                    break;
                }
                i += 1;
            }
        }
        e
    };
    let mut result: Fe = [1, 0, 0, 0];
    let mut base = *a;
    for i in 0..256 {
        if (exp[i / 64] >> (i % 64)) & 1 == 1 {
            result = fe_mul(&result, &base);
        }
        base = fe_sqr(&base);
    }
    result
}

fn fe_from_u256(x: U256) -> Fe {
    let l = x.into_limbs();
    [l[0], l[1], l[2], l[3]]
}

fn fe_to_u256(a: &Fe) -> U256 {
    U256::from_limbs(*a)
}

/// Jacobian point: (X : Y : Z) represents (X/Z^2, Y/Z^3). Z = 0 is infinity.
#[derive(Clone, Copy)]
struct Jac {
    x: Fe,
    y: Fe,
    z: Fe,
}

impl Jac {
    fn infinity() -> Self {
        Jac { x: [1, 0, 0, 0], y: [1, 0, 0, 0], z: fe_zero() }
    }
    fn is_infinity(&self) -> bool {
        fe_is_zero(&self.z)
    }
}

/// Affine point.
#[derive(Clone, Copy, PartialEq)]
struct Aff {
    x: Fe,
    y: Fe,
    infinity: bool,
}

/// Jacobian += affine, for a = 0. Standard madd-2007-bl, with the doubling and
/// infinity cases handled explicitly because the fixed-base table can hit them.
fn jac_add_affine(p: &Jac, q: &Aff) -> Jac {
    if q.infinity {
        return *p;
    }
    if p.is_infinity() {
        return Jac { x: q.x, y: q.y, z: [1, 0, 0, 0] };
    }
    let z1z1 = fe_sqr(&p.z);
    let u2 = fe_mul(&q.x, &z1z1);
    let s2 = fe_mul(&fe_mul(&q.y, &z1z1), &p.z);
    let h = fe_sub(&u2, &p.x);
    let r = fe_sub(&s2, &p.y);
    if fe_is_zero(&h) {
        if fe_is_zero(&r) {
            return jac_double(p);
        }
        return Jac::infinity();
    }
    let hh = fe_sqr(&h);
    let i = fe_add(&hh, &hh);
    let i = fe_add(&i, &i);
    let j = fe_mul(&h, &i);
    let rr = fe_add(&r, &r);
    let v = fe_mul(&p.x, &i);
    let x3 = fe_sub(&fe_sub(&fe_sqr(&rr), &j), &fe_add(&v, &v));
    let y3 = fe_sub(
        &fe_mul(&rr, &fe_sub(&v, &x3)),
        &fe_add(&fe_mul(&p.y, &j), &fe_mul(&p.y, &j)),
    );
    let z3 = fe_mul(&p.z, &fe_add(&h, &h));
    Jac { x: x3, y: y3, z: z3 }
}

/// dbl-2009-l, valid for a = 0.
fn jac_double(p: &Jac) -> Jac {
    if p.is_infinity() {
        return *p;
    }
    let a = fe_sqr(&p.x);
    let b = fe_sqr(&p.y);
    let c = fe_sqr(&b);
    let t = fe_add(&p.x, &b);
    let d = fe_sub(&fe_sub(&fe_sqr(&t), &a), &c);
    let d = fe_add(&d, &d);
    let e = fe_add(&fe_add(&a, &a), &a);
    let f = fe_sqr(&e);
    let x3 = fe_sub(&f, &fe_add(&d, &d));
    let c8 = {
        let c2 = fe_add(&c, &c);
        let c4 = fe_add(&c2, &c2);
        fe_add(&c4, &c4)
    };
    let y3 = fe_sub(&fe_mul(&e, &fe_sub(&d, &x3)), &c8);
    let z3 = fe_mul(&fe_add(&p.y, &p.y), &p.z);
    Jac { x: x3, y: y3, z: z3 }
}

/// Fixed-base table: `rows[i][d]` is `d * 256^i * G`, so a scalar multiplication
/// is 32 mixed additions and no doublings.
struct FixedBase {
    rows: Vec<[Aff; 256]>,
}

impl FixedBase {
    fn new(gx: Fe, gy: Fe) -> Self {
        let g = Aff { x: gx, y: gy, infinity: false };
        let mut rows = Vec::with_capacity(32);
        let mut base = g;
        for _ in 0..32 {
            let mut row = [Aff { x: fe_zero(), y: fe_zero(), infinity: true }; 256];
            let mut acc = Jac::infinity();
            // Accumulate d * base for d = 1..255, converting each to affine.
            let mut jacs = Vec::with_capacity(256);
            for _ in 0..256 {
                jacs.push(acc);
                acc = jac_add_affine(&acc, &base);
            }
            let affs = batch_to_affine(&jacs);
            row[..256].copy_from_slice(&affs[..256]);
            rows.push(row);
            // base *= 256
            let mut j = Jac { x: base.x, y: base.y, z: [1, 0, 0, 0] };
            for _ in 0..8 {
                j = jac_double(&j);
            }
            base = batch_to_affine(&[j])[0];
        }
        FixedBase { rows }
    }

    fn mul(&self, k: &Fe) -> Jac {
        let mut acc = Jac::infinity();
        for i in 0..32 {
            let byte = ((k[i / 8] >> ((i % 8) * 8)) & 0xFF) as usize;
            if byte != 0 {
                acc = jac_add_affine(&acc, &self.rows[i][byte]);
            }
        }
        acc
    }
}

/// Montgomery batch inversion: one field inversion for the whole slice.
fn batch_invert(vals: &[Fe]) -> Vec<Fe> {
    let mut prefix = Vec::with_capacity(vals.len() + 1);
    prefix.push([1u64, 0, 0, 0]);
    for v in vals {
        let last = *prefix.last().unwrap();
        // Zeros are passed through; they cannot be inverted and the caller
        // treats them as the infinity case.
        prefix.push(if fe_is_zero(v) { last } else { fe_mul(&last, v) });
    }
    let mut acc = fe_inv(prefix.last().unwrap());
    let mut out = vec![fe_zero(); vals.len()];
    for i in (0..vals.len()).rev() {
        if fe_is_zero(&vals[i]) {
            out[i] = fe_zero();
            continue;
        }
        out[i] = fe_mul(&acc, &prefix[i]);
        acc = fe_mul(&acc, &vals[i]);
    }
    out
}

fn batch_to_affine(pts: &[Jac]) -> Vec<Aff> {
    let zs: Vec<Fe> = pts.iter().map(|p| p.z).collect();
    let inv = batch_invert(&zs);
    pts.iter()
        .zip(inv.iter())
        .map(|(p, zi)| {
            if p.is_infinity() {
                return Aff { x: fe_zero(), y: fe_zero(), infinity: true };
            }
            let zi2 = fe_sqr(zi);
            let zi3 = fe_mul(&zi2, zi);
            Aff { x: fe_mul(&p.x, &zi2), y: fe_mul(&p.y, &zi3), infinity: false }
        })
        .collect()
}

// ─── the replay ────────────────────────────────────────────────────────────
//
// The walk records a sign tape; the replay consumes it once to build the
// coefficient. Semantically each round is
//
//     target <- (target + (-1)^sign * source) / 2   (mod p)
//
// for the divide traversal, and the doubling inverse for the multiply one. The
// circuit computes that with truncated corrections, and it is the truncations,
// not the arithmetic, that fail. Two distinct failure channels:
//
//   * The pseudo-Mersenne fold adds one of {-f, 0, +f, +2f} into the low
//     REPLAY_FOLD_WINDOW = 54 bits and DISCARDS the carry out of bit 53. That
//     is a real arithmetic error whenever the carry would have escaped, so it
//     shows up as a classical mismatch. f is about 2^32, so the escape
//     probability is about 2^-22 per active fold.
//   * The chunk boundary carries and the overflow flag are erased by a
//     comparison truncated to 22 bits, applied as a PHASE correction. When the
//     truncated comparison disagrees with the true carry the shot picks up a
//     phase error, not a wrong value, which is why these land in the
//     phase-garbage column rather than the classical one.
//
// Registers are tracked as raw 256-bit values, not residues: the truncation
// predicates depend on the actual bit pattern the register holds.

/// Raw 256-bit register value, little-endian limbs. No reduction.
type R = [u64; 4];

const R_ZERO: R = [0; 4];

fn r_from_u256(x: U256) -> R {
    let l = x.into_limbs();
    [l[0], l[1], l[2], l[3]]
}

fn r_not(a: &R) -> R {
    [!a[0], !a[1], !a[2], !a[3]]
}

fn r_bit(a: &R, i: usize) -> u64 {
    (a[i / 64] >> (i % 64)) & 1
}

/// Wrapping add, returning the carry out of bit 255.
fn r_add(a: &R, b: &R) -> (R, bool) {
    let mut out = R_ZERO;
    let mut carry = 0u128;
    for i in 0..4 {
        let cur = a[i] as u128 + b[i] as u128 + carry;
        out[i] = cur as u64;
        carry = cur >> 64;
    }
    (out, carry != 0)
}

/// Add `addend` into the low `window` bits of `acc`, returning the new value
/// and the carry out of bit `window-1`. That carry is what the circuit throws
/// away, so returning it is the whole point.
fn r_add_windowed(acc: &R, addend: &R, window: usize) -> (R, bool) {
    let mut out = *acc;
    let mut carry = 0u64;
    for i in 0..4 {
        let lo = i * 64;
        if lo >= window {
            break;
        }
        let keep = (window - lo).min(64);
        let mask = if keep == 64 { u64::MAX } else { (1u64 << keep) - 1 };
        let cur = (acc[i] & mask) as u128 + (addend[i] & mask) as u128 + carry as u128;
        out[i] = (acc[i] & !mask) | ((cur as u64) & mask);
        carry = ((cur >> keep) & 1) as u64;
    }
    (out, carry != 0)
}

/// The bits of `-f` in two's complement over `width` bits, matching
/// `twos_complement_bits`.
fn neg_f_bits(f: &R, width: usize) -> R {
    let mut out = R_ZERO;
    let mut carry = true;
    for i in 0..width {
        let inverted = r_bit(f, i) == 0;
        let b = inverted ^ carry;
        if b {
            out[i / 64] |= 1 << (i % 64);
        }
        carry &= inverted;
    }
    out
}

/// Effective exact width of a truncated constant add/subtract.
///
/// `csub_nbit_const_direct_trunc_fast` runs its borrow chain over positions
/// `0..=last` with `last = min(n-2, highest_set_bit(c) + window)`, and the apply
/// loop feeds borrow `i-1` into `acc[i]` for `i <= last+1`. So the operation is
/// exact through position `last+1` and the borrow out of there is discarded.
fn trunc_chain(hi_bit: usize, window: usize) -> usize {
    let last = (N - 2).min(hi_bit + window);
    last + 2
}

/// Conditional subtract of `c` from the low `chain` bits, high bits untouched.
/// Returns the result and whether a borrow escaped, which is the failure.
fn r_sub_windowed(acc: &R, c: &R, chain: usize) -> (R, bool) {
    // Adding the two's complement over the same width carries out exactly when
    // no borrow was needed, so an escaped borrow is a missing carry.
    let negc = neg_f_bits(c, chain);
    let (out, carry) = r_add_windowed(acc, &negc, chain);
    (out, !carry)
}

/// `mod_halve_pm`: if the value is odd, subtract f (truncated), then shift right
/// one and push the parity into the top. Used by replay rounds 0 and 1.
fn replay_halve_pm(t: &R, f: &R, window: usize) -> (R, bool) {
    let parity = r_bit(t, 0) == 1;
    let (corrected, escaped) = if parity {
        r_sub_windowed(t, f, trunc_chain(32, window))
    } else {
        (*t, false)
    };
    let mut out = R_ZERO;
    for i in 0..255 {
        if r_bit(&corrected, i + 1) == 1 {
            out[i / 64] |= 1 << (i % 64);
        }
    }
    if parity {
        out[255 / 64] |= 1 << (255 % 64);
    }
    (out, escaped)
}

/// `seed_round_one`: the target register is empty, so it takes
/// `(-1)^sign * source` directly. The negate is a complement plus a truncated
/// subtract of `f-1`, which is the second place a borrow can escape.
fn replay_seed_round_one(source: &R, sign: bool, f_minus_one: &R, window: usize) -> (R, bool) {
    if !sign {
        return (*source, false);
    }
    let complemented = r_not(source);
    r_sub_windowed(&complemented, f_minus_one, trunc_chain(32, window))
}

/// One fused halving replay round, round >= 2. Returns the new target value and
/// whether the truncated fold lost a carry.
fn replay_halve_round(t: &R, a: &R, s: bool, f: &R, negf: &R, window: usize) -> (R, bool) {
    let t1 = if s { r_not(t) } else { *t };
    let (t2, ovf) = r_add(&t1, a);
    let parity = r_bit(&t2, 0) == 1;

    let nsp = !s && parity;
    let sp = s && parity;
    let minus_f = !ovf && nsp;
    let plus_2f = ovf && sp;
    let plus_f = minus_f ^ s ^ parity;

    // The three selectors are one-hot, so the operand is a plain choice.
    let operand = if plus_f {
        *f
    } else if plus_2f {
        let (d, _) = r_add(f, f);
        d
    } else if minus_f {
        *negf
    } else {
        R_ZERO
    };

    // The fold ripples across `window` bits and drops the carry out of the top.
    let (folded, lost) = if operand == R_ZERO {
        (t2, false)
    } else {
        r_add_windowed(&t2, &operand, window)
    };

    // Undo the conditional negate, then halve: shift down one and push the
    // corrected parity bit into the top.
    let t3 = if s { r_not(&folded) } else { folded };
    let top = (r_bit(&t2, 0) == 1) ^ ovf ^ s;
    let mut out = R_ZERO;
    for i in 0..255 {
        if r_bit(&t3, i + 1) == 1 {
            out[i / 64] |= 1 << (i % 64);
        }
    }
    if top {
        out[255 / 64] |= 1 << (255 % 64);
    }
    (out, lost)
}

// ─── width envelope ────────────────────────────────────────────────────────

/// Smallest `w` for which `a` sign-extends into `w` bits, i.e. the width this
/// value actually needs at this round.
fn needed_width(a: &W) -> usize {
    let sign = if is_neg(a) { u64::MAX } else { 0 };
    for i in (0..320).rev() {
        if (a[i / 64] >> (i % 64)) & 1 != (sign & 1) {
            return i + 2; // the differing bit plus a sign bit above it
        }
    }
    1
}

/// Run the walk without the width check, recording the width each round
/// actually needs. Used to ask what schedule a given draw would permit, rather
/// than whether it survives the shipped one.
fn walk_envelope(p: &W, denominator: U256, rounds: usize, env: &mut [u16]) {
    let a = from_u256(denominator);
    let mut v = if bit(&a, 0) == 1 { a } else { sub(&a, p) };
    let mut u = *p;
    for r in 0..rounds {
        let need = needed_width(&u).max(needed_width(&v));
        if need as u16 > env[r] {
            env[r] = need as u16;
        }
        // Track the shipped geometry so the recorded envelope is the one the
        // circuit would see: the add still wraps at the scheduled width.
        let w = value_width(r);
        let even = r % 2 == 0;
        let (src, tgt) = if even { (u, v) } else { (v, u) };
        let sign = bit(&tgt, 1) ^ bit(&src, 1);
        let s = if sign == 1 { sub(&tgt, &src) } else { add(&tgt, &src) };
        let shifted = sar1(&wrap_signed(&s, w.max(need)));
        if even {
            v = shifted;
        } else {
            u = shifted;
        }
    }
}

// ─── Fiat-Shamir ───────────────────────────────────────────────────────────

/// The op stream reduced to what the Fiat-Shamir transcript absorbs: one
/// 49-byte record per op. The last 96 records are the nonce tail.
struct Transcript {
    /// SHAKE256 state with everything but the tail already absorbed.
    prefix: Shake256,
    /// The 96 tail records, verbatim; only bytes 17..25 (q_target) vary.
    tail: Vec<[u8; HASH_BYTES]>,
}

impl Transcript {
    fn load(path: &str) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| format!("open {path}: {e}"))?;
        let mut header = [0u8; MAGIC.len() + 8];
        file.read_exact(&mut header)
            .map_err(|e| format!("{path}: short header: {e}"))?;
        if &header[..MAGIC.len()] != MAGIC {
            return Err(format!("{path}: bad magic"));
        }
        let n = u64::from_le_bytes(header[MAGIC.len()..].try_into().unwrap()) as usize;
        if n < TAIL_OPS {
            return Err(format!("{path}: op stream too short for a nonce tail"));
        }

        let mut dec = zstd::stream::read::Decoder::new(BufReader::new(file))
            .map_err(|e| format!("{path}: zstd init: {e}"))?;
        dec.window_log_max(ZSTD_WINDOW_LOG_MAX)
            .map_err(|e| format!("{path}: zstd window cap: {e}"))?;

        let mut hasher = Shake256::default();
        hasher.update(b"quantum_ecc-fiat-shamir-v2");
        hasher.update(&(n as u64).to_le_bytes());

        let mut tail = Vec::with_capacity(TAIL_OPS);
        let mut rec = [0u8; OP_BYTES];
        for i in 0..n {
            dec.read_exact(&mut rec)
                .map_err(|e| format!("op {i}: short read: {e}"))?;
            // The transcript takes `kind as u8` then the six u64 fields. The
            // record stores kind as u32 with four bytes of alignment padding,
            // and every discriminant is < 18, so the low byte is the whole of it.
            let mut h = [0u8; HASH_BYTES];
            h[0] = rec[0];
            h[1..].copy_from_slice(&rec[8..OP_BYTES]);
            if i >= n - TAIL_OPS {
                if h[0] != 6 {
                    return Err(format!("tail op {i} is not an X (kind {})", h[0]));
                }
                tail.push(h);
            } else {
                hasher.update(&h);
            }
        }
        Ok(Transcript { prefix: hasher, tail })
    }

    /// Seed for one nonce. Only the tail is re-absorbed.
    fn seed(&self, nonce: u64) -> sha3::Shake256Reader {
        let mut h = self.prefix.clone();
        let mut rec;
        for b in 0..48 {
            let target: u64 = if (nonce >> b) & 1 == 1 { 1 } else { 0 };
            for half in 0..2 {
                rec = self.tail[2 * b + half];
                // q_target is the third u64 field: bytes 1+16 .. 1+24.
                rec[17..25].copy_from_slice(&target.to_le_bytes());
                h.update(&rec);
            }
        }
        h.finalize_xof()
    }
}

// ─── screening one nonce ───────────────────────────────────────────────────

struct Cfg {
    rounds: usize,
    rounds_mul: usize,
}

/// How many shots are derived per batch. The curve work for a whole block
/// shares two field inversions, so bigger is cheaper; but early abort discards
/// the unused remainder of the block, and the mean shot of first failure is
/// ~1,040, so a block much larger than this is mostly wasted work.
const BLOCK: usize = 128;

/// Screen a nonce. Returns `None` if it survives, or `Some(shot)` for the index
/// of the first shot whose walk fails.
fn screen(t: &Transcript, fb: &FixedBase, p_w: &W, nonce: u64, cfg: &Cfg) -> Option<usize> {
    let mut xof = t.seed(nonce);
    let mut kept = 0usize;

    let mut tj: Vec<Jac> = Vec::with_capacity(BLOCK);
    let mut oj: Vec<Jac> = Vec::with_capacity(BLOCK);

    while kept < NUM_TESTS {
        let want = BLOCK.min(NUM_TESTS - kept);
        tj.clear();
        oj.clear();
        // The harness draws until it has NUM_TESTS usable shots, skipping
        // degenerate ones, so the stream must be consumed in the same order.
        // Degeneracy needs a collision in a 256-bit coordinate, so in practice
        // this never fires; it is here so the model cannot silently desync.
        while tj.len() < want {
            let mut rb = [[0u8; 32]; 2];
            XofReader::read(&mut xof, &mut rb[0]);
            XofReader::read(&mut xof, &mut rb[1]);
            let k1 = fe_from_u256(U256::from_le_bytes(rb[0]));
            let k2 = fe_from_u256(U256::from_le_bytes(rb[1]));
            let a = fb.mul(&k1);
            let b = fb.mul(&k2);
            if a.is_infinity() || b.is_infinity() {
                continue;
            }
            // x_a == x_b in affine iff X_a * Z_b^2 == X_b * Z_a^2.
            if fe_mul(&a.x, &fe_sqr(&b.z)) == fe_mul(&b.x, &fe_sqr(&a.z)) {
                continue;
            }
            tj.push(a);
            oj.push(b);
        }

        // One inversion converts the whole block to affine.
        let mut both = tj.clone();
        both.extend_from_slice(&oj);
        let aff = batch_to_affine(&both);
        let (ta, oa) = aff.split_at(want);

        // A second inversion gives every chord slope, so the result x needs no
        // inversion of its own.
        let dens: Vec<Fe> = (0..want).map(|i| fe_sub(&oa[i].x, &ta[i].x)).collect();
        let inv = batch_invert(&dens);

        for i in 0..want {
            let lambda = fe_mul(&fe_sub(&oa[i].y, &ta[i].y), &inv[i]);
            let ex = fe_sub(&fe_sub(&fe_sqr(&lambda), &ta[i].x), &oa[i].x);
            let shot = kept + i;

            // Divide traversal: denominator = target.x - offset.x.
            let d1 = fe_sub(&ta[i].x, &oa[i].x);
            if !walk_ok(p_w, fe_to_u256(&d1), cfg.rounds) {
                return Some(shot);
            }
            // Multiply traversal: denominator = offset.x - result.x.
            let d2 = fe_sub(&oa[i].x, &ex);
            if !walk_ok(p_w, fe_to_u256(&d2), cfg.rounds_mul) {
                return Some(shot);
            }
        }
        kept += want;
    }
    None
}

/// The fold operand, assembled exactly as `fused_operand_controls` does: a
/// per-position XOR of the three selectors rather than a one-hot choice, so it
/// stays faithful even where the selectors are not mutually exclusive.
fn fold_operand(f: &R, negf: &R, plus_f: bool, plus_2f: bool, minus_f: bool, width: usize) -> R {
    let mut op = R_ZERO;
    for i in 0..width {
        let mut bit = false;
        if plus_f && r_bit(f, i) == 1 {
            bit = !bit;
        }
        if plus_2f && i > 0 && r_bit(f, i - 1) == 1 {
            bit = !bit;
        }
        if minus_f && r_bit(negf, i) == 1 {
            bit = !bit;
        }
        if bit {
            op[i / 64] |= 1 << (i % 64);
        }
    }
    op
}

fn r_shl1(a: &R) -> R {
    let mut out = R_ZERO;
    for i in (1..256).rev() {
        if r_bit(a, i - 1) == 1 {
            out[i / 64] |= 1 << (i % 64);
        }
    }
    out
}

/// One fused doubling replay round, round >= 2: `target <- 2*target +
/// (-1)^sign * source (mod p)`. Note the caller inverts `sign` around this cell,
/// which is why `replay_mul_escapes` passes `!s`.
fn replay_double_round(t: &R, a: &R, s: bool, f: &R, negf: &R, window: usize) -> (R, bool) {
    let doubled_out = r_bit(t, 255) == 1;
    let d = r_shl1(t);
    let t1 = if s { r_not(&d) } else { d };
    let (t2, add_out) = r_add(&t1, a);

    let sign_xor_add = s ^ add_out;
    let routed = doubled_out && sign_xor_add;
    let minus_f = routed && s;
    let plus_2f = routed ^ minus_f;
    let plus_f = doubled_out ^ add_out ^ minus_f;

    let operand = fold_operand(f, negf, plus_f, plus_2f, minus_f, window);
    let (folded, carry) = if operand == R_ZERO {
        (t2, false)
    } else {
        r_add_windowed(&t2, &operand, window)
    };
    let escaped = if minus_f {
        !carry
    } else if plus_f || plus_2f {
        carry
    } else {
        false
    };

    let out = if s { r_not(&folded) } else { folded };
    (out, escaped)
}

/// `mod_double_pm`: shift left, and if a bit fell off the top add f back
/// (truncated), since 2^256 = f mod p.
fn replay_double_pm(t: &R, f: &R, window: usize) -> (R, bool) {
    let doubled_out = r_bit(t, 255) == 1;
    let d = r_shl1(t);
    if !doubled_out {
        return (d, false);
    }
    let chain = trunc_chain(32, window);
    let (out, carry) = r_add_windowed(&d, f, chain);
    (out, carry)
}

/// `seed_round_one_inverse`: undo the seed, which should clear the register.
fn replay_seed_round_one_inverse(
    t: &R,
    source: &R,
    sign: bool,
    f_minus_one: &R,
    window: usize,
) -> (R, bool) {
    let (added, escaped) = if sign {
        let chain = trunc_chain(32, window);
        let (o, c) = r_add_windowed(t, f_minus_one, chain);
        (o, c)
    } else {
        (*t, false)
    };
    let complemented = if sign { r_not(&added) } else { added };
    let mut out = complemented;
    for i in 0..4 {
        out[i] ^= source[i];
    }
    (out, escaped)
}

/// Run the multiply traversal's replay for one shot. The rounds run in reverse,
/// and both registers start from the multiply's numerator, conditionally negated
/// by the walk's terminal signs.
fn replay_mul_escapes(
    tape: &[bool],
    coefficient0: U256,
    numerator0: U256,
    f: &R,
    negf: &R,
    f_minus_one: &R,
) -> usize {
    let mut x = r_from_u256(coefficient0);
    let mut y = r_from_u256(numerator0);
    let mut escapes = 0usize;

    for round in (0..tape.len()).rev() {
        let s = tape[round];
        let even = round % 2 == 0;
        let (src, tgt) = if even { (x, y) } else { (y, x) };

        let (next, escaped) = if round > 1 {
            // The caller wraps this cell in X(sign), so the cell sees !s.
            replay_double_round(&tgt, &src, !s, f, negf, win().1)
        } else if round == 1 {
            let (doubled, e1) = replay_double_pm(&tgt, f, win().2);
            let (seeded, e2) = replay_seed_round_one_inverse(&doubled, &src, s, f_minus_one, 32);
            (seeded, e1 || e2)
        } else {
            replay_double_pm(&tgt, f, win().2)
        };

        escapes += usize::from(escaped);
        if even { y = next } else { x = next }
    }
    // seed_round_one_inverse runs last and is the exact inverse of the seed, so
    // a correct multiply replay ends with the coefficient register cleared.
    // If it does not, the recurrence or the initial values are wrong.
    if x != R_ZERO {
        return usize::MAX;
    }
    escapes
}

/// Run the divide traversal's replay for one shot and count how many truncated
/// folds lose a carry. The coefficient register starts empty and the numerator
/// starts at `numer`, which is what `ec_add_with_division` hands the divide.
fn replay_div_escapes(tape: &[bool], numer: U256, f: &R, negf: &R, f_minus_one: &R) -> usize {
    let mut x = R_ZERO;
    let mut y = r_from_u256(numer);
    let mut escapes = 0usize;

    for (round, &s) in tape.iter().enumerate() {
        let even = round % 2 == 0;
        if round == 0 {
            let (next, escaped) = replay_halve_pm(&y, f, win().2);
            y = next;
            escapes += usize::from(escaped);
            continue;
        }
        if round == 1 {
            let (seeded, e1) = replay_seed_round_one(&y, s, f_minus_one, 32);
            let (next, e2) = replay_halve_pm(&seeded, f, win().2);
            x = next;
            escapes += usize::from(e1 || e2);
            continue;
        }
        let (src, tgt) = if even { (x, y) } else { (y, x) };
        let (next, lost) = replay_halve_round(&tgt, &src, s, f, negf, win().0);
        // Which polarity of the dropped carry is the error depends on the
        // branch: a subtract is meant to carry out, an add is not.
        let t1 = if s { r_not(&tgt) } else { tgt };
        let (t2, ovf) = r_add(&t1, &src);
        let parity = r_bit(&t2, 0) == 1;
        let minus_f = !ovf && !s && parity;
        let plus_2f = ovf && s && parity;
        let plus_f = minus_f ^ s ^ parity;
        let escaped = if minus_f { !lost } else if plus_f || plus_2f { lost } else { false };
        escapes += usize::from(escaped);
        if even { y = next } else { x = next }
    }
    escapes
}

/// Measure the per-round width envelope a single nonce's draw needs, across all
/// 9,024 shots and both traversals. Returns the raw envelope; the caller makes
/// it non-increasing, since `shrink_to` can only ever shrink.
fn envelope(t: &Transcript, fb: &FixedBase, p_w: &W, nonce: u64, cfg: &Cfg) -> Vec<u16> {
    let rounds = cfg.rounds.max(cfg.rounds_mul);
    let mut env = vec![0u16; rounds];
    let mut xof = t.seed(nonce);
    let mut kept = 0usize;
    let mut tj: Vec<Jac> = Vec::with_capacity(BLOCK);
    let mut oj: Vec<Jac> = Vec::with_capacity(BLOCK);

    while kept < NUM_TESTS {
        let want = BLOCK.min(NUM_TESTS - kept);
        tj.clear();
        oj.clear();
        while tj.len() < want {
            let mut rb = [[0u8; 32]; 2];
            XofReader::read(&mut xof, &mut rb[0]);
            XofReader::read(&mut xof, &mut rb[1]);
            let a = fb.mul(&fe_from_u256(U256::from_le_bytes(rb[0])));
            let b = fb.mul(&fe_from_u256(U256::from_le_bytes(rb[1])));
            if a.is_infinity() || b.is_infinity() {
                continue;
            }
            if fe_mul(&a.x, &fe_sqr(&b.z)) == fe_mul(&b.x, &fe_sqr(&a.z)) {
                continue;
            }
            tj.push(a);
            oj.push(b);
        }
        let mut both = tj.clone();
        both.extend_from_slice(&oj);
        let aff = batch_to_affine(&both);
        let (ta, oa) = aff.split_at(want);
        let dens: Vec<Fe> = (0..want).map(|i| fe_sub(&oa[i].x, &ta[i].x)).collect();
        let inv = batch_invert(&dens);
        for i in 0..want {
            let lambda = fe_mul(&fe_sub(&oa[i].y, &ta[i].y), &inv[i]);
            let ex = fe_sub(&fe_sub(&fe_sqr(&lambda), &ta[i].x), &oa[i].x);
            let d1 = fe_sub(&ta[i].x, &oa[i].x);
            walk_envelope(p_w, fe_to_u256(&d1), cfg.rounds, &mut env);
            let d2 = fe_sub(&oa[i].x, &ex);
            walk_envelope(p_w, fe_to_u256(&d2), cfg.rounds_mul, &mut env);
        }
        kept += want;
    }
    env
}

/// Prove the replay derivation, in two steps that fail independently.
///
/// 1. Run the replay in exact modular arithmetic, `target <- (target +/- source)/2
///    mod p`, and check it lands on the true modular inverse. That validates the
///    semantics: what the replay is supposed to compute.
/// 2. Run the register-level model, the one that mirrors the gates (conditional
///    negate, 256-bit wrapping add, pseudo-Mersenne fold, shift), and check it
///    agrees with step 1 modulo p at every round. That validates the derivation:
///    how the circuit computes it.
///
/// A mistake in the correction logic breaks step 2; a mistake in what the replay
/// means breaks step 1.
fn replay_selftest(curve: &WeierstrassEllipticCurve, p_w: &W) -> Result<(), String> {
    let modulus = curve.modulus;
    let f_u = U256::MAX.wrapping_sub(modulus).wrapping_add(U256::from(1));
    let f = r_from_u256(f_u);
    let window = win().0;
    let negf = neg_f_bits(&f, window);
    let f_minus_one = r_from_u256(U256::MAX.wrapping_sub(modulus));
    let inv2 = U256::from(2u64).inv_mod(modulus).expect("2 is invertible");

    let mut checked = 0;
    let mut truncation_errors = 0usize;
    let mut rounds_checked = 0usize;
    let mut seed = U256::from(12345u64);
    for trial in 0..8 {
        seed = seed.mul_mod(U256::from(6364136223846793005u64), modulus)
            .add_mod(U256::from(trial as u64 + 1), modulus);
        let denom = seed;
        let numer = seed.add_mod(U256::from(777u64), modulus);
        if denom.is_zero() {
            continue;
        }
        let tape = match walk_tape(p_w, denom, 698) {
            Some(t) => t,
            None => continue, // this draw does not converge; not what is under test
        };

        // Exact modular replay, and the register-level model, side by side.
        let (mut mx, mut my) = (U256::ZERO, numer);
        let (mut rx, mut ry) = (R_ZERO, r_from_u256(numer));

        for (round, &s) in tape.iter().enumerate() {
            if round == 0 {
                // Even round, so target is y. Round 0 ignores the sign entirely
                // and is a bare mod_halve_pm; the coefficient register is still
                // empty, so there is nothing to add in.
                let (rnext, escaped) = replay_halve_pm(&ry, &f, win().2);
                if escaped {
                    truncation_errors += 1;
                }
                ry = rnext;
                my = my.mul_mod(inv2, modulus);
                if U256::from_limbs(ry) % modulus != my {
                    return Err(format!("round 0 mod_halve_pm disagrees (trial {trial})"));
                }
                continue;
            }
            if round == 1 {
                // Odd round, so target is x, which is empty: seed_round_one
                // writes (-1)^sign * source into it, then it is halved.
                let (seeded, escaped_seed) = replay_seed_round_one(&ry, s, &f_minus_one, 32);
                let (rnext, escaped_halve) = replay_halve_pm(&seeded, &f, win().2);
                if escaped_seed || escaped_halve {
                    truncation_errors += 1;
                }
                rx = rnext;
                let signed = if s { modulus - my } else { my };
                mx = signed.mul_mod(inv2, modulus);
                if U256::from_limbs(rx) % modulus != mx {
                    return Err(format!("round 1 seed+halve disagrees (trial {trial})"));
                }
                continue;
            }
            let even = round % 2 == 0;
            let (msrc, mtgt) = if even { (mx, my) } else { (my, mx) };
            let signed = if s { mtgt.add_mod(modulus - msrc, modulus) } else { mtgt.add_mod(msrc, modulus) };
            let mnext = signed.mul_mod(inv2, modulus);

            let (rsrc, rtgt) = if even { (rx, ry) } else { (ry, rx) };
            let (rnext, lost) = replay_halve_round(&rtgt, &rsrc, s, &f, &negf, window);

            let raw = U256::from_limbs(rnext);
            if raw != mnext && raw % modulus == mnext {
                return Err(format!(
                    "replay round {round} congruent but not canonical: raw={raw} reduced={mnext} (trial {trial})"));
            }
            let got = raw % modulus;
            if got != mnext {
                let t1 = if s { r_not(&rtgt) } else { rtgt };
                let (t2, ovf) = r_add(&t1, &rsrc);
                let parity = r_bit(&t2, 0) == 1;
                let nsp = !s && parity;
                let sp = s && parity;
                let minus_f = !ovf && nsp;
                let plus_2f = ovf && sp;
                let plus_f = minus_f ^ s ^ parity;
                // A truncated fold that loses its carry is a real circuit error,
                // not a modeling error, so count these rather than bail: the
                // rate is what says whether the derivation is right. A correct
                // model should fire at about 2^-22 per active fold.
                let escaped = (minus_f && !lost) || ((plus_f || plus_2f) && lost);
                if escaped {
                    truncation_errors += 1;
                    // Resync onto the exact track and keep testing the rest.
                    if even { my = mnext; ry = r_from_u256(mnext) } else { mx = mnext; rx = r_from_u256(mnext) }
                    rounds_checked += 1;
                    continue;
                }
                return Err(format!(
                    "replay round {round} disagrees (trial {trial}) with no lost carry: \
                     s={s} ovf={ovf} parity={parity} plus_f={plus_f} plus_2f={plus_2f} \
                     minus_f={minus_f} nsp={nsp} sp={sp} carry_out={lost}"
                ));
            }
            rounds_checked += 1;
            if even { my = mnext; ry = rnext } else { mx = mnext; rx = rnext }
        }

        // The end state is deliberately not checked here: rounds 0 and 1 use the
        // seed and endpoint cells, and the traversal ends with two conditional
        // negates and a register XOR, none of which this test implements. What
        // is under test is the fused cell, and the round-by-round agreement
        // above is the statement that it is right. End-to-end correctness is
        // settled against the simulator by firstfail.sh, which is a stronger
        // oracle than any invariant reconstructed here.
        checked += 1;
    }
    if checked == 0 {
        return Err("no replay trial converged, nothing was validated".into());
    }
    eprintln!(
        "pp_screen: replay selftest ok ({checked} trials, {rounds_checked} fused rounds, \
         {truncation_errors} truncated folds lost a carry = 1 per {:.3e})",
        if truncation_errors == 0 { f64::INFINITY } else { rounds_checked as f64 / truncation_errors as f64 }
    );
    Ok(())
}

/// Check the fast field and group against the harness curve, which is the
/// definition of what `eval_circuit` will derive. Any disagreement here means
/// every screening result is meaningless, so this runs before any grind.
fn selftest(curve: &WeierstrassEllipticCurve, fb: &FixedBase) -> Result<(), String> {
    let modulus = curve.modulus;
    // Field: multiplication and inversion against ruint's modular arithmetic.
    let mut x = U256::from(3u64);
    for i in 0..64 {
        x = x.mul_mod(x, modulus).add_mod(U256::from(i as u64 + 7), modulus);
        let y = x.add_mod(U256::from(12345u64), modulus);
        let want = x.mul_mod(y, modulus);
        let got = fe_to_u256(&fe_mul(&fe_from_u256(x), &fe_from_u256(y)));
        if want != got {
            return Err(format!("fe_mul disagrees at i={i}: want {want}, got {got}"));
        }
        let want_inv = x.inv_mod(modulus).expect("invertible");
        let got_inv = fe_to_u256(&fe_inv(&fe_from_u256(x)));
        if want_inv != got_inv {
            return Err(format!("fe_inv disagrees at i={i}"));
        }
    }
    // Group: fixed-base scalar multiplication and the chord slope.
    let mut k = U256::from(1u64);
    for i in 0..24 {
        k = k.mul_mod(U256::from(6364136223846793005u64), curve.order)
            .add_mod(U256::from(i as u64 + 1), curve.order);
        let want = curve.mul(curve.gx, curve.gy, k);
        let got = batch_to_affine(&[fb.mul(&fe_from_u256(k))])[0];
        if got.infinity || fe_to_u256(&got.x) != want.0 || fe_to_u256(&got.y) != want.1 {
            return Err(format!("fixed-base mul disagrees at i={i}"));
        }
        let k2 = k.add_mod(U256::from(999u64), curve.order);
        let p2 = curve.mul(curve.gx, curve.gy, k2);
        let want_sum = curve.add(want.0, want.1, p2.0, p2.1);
        let a = batch_to_affine(&[fb.mul(&fe_from_u256(k))])[0];
        let b = batch_to_affine(&[fb.mul(&fe_from_u256(k2))])[0];
        let den = fe_sub(&b.x, &a.x);
        let lambda = fe_mul(&fe_sub(&b.y, &a.y), &fe_inv(&den));
        let ex = fe_sub(&fe_sub(&fe_sqr(&lambda), &a.x), &b.x);
        if fe_to_u256(&ex) != want_sum.0 {
            return Err(format!("chord x disagrees at i={i}"));
        }
    }
    Ok(())
}

// ─── driver ────────────────────────────────────────────────────────────────

fn secp256k1() -> WeierstrassEllipticCurve {
    WeierstrassEllipticCurve {
        modulus: U256::from_str_radix(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F",
            16,
        )
        .unwrap(),
        a: U256::from(0),
        b: U256::from(7),
        gx: U256::from_str_radix(
            "79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798",
            16,
        )
        .unwrap(),
        gy: U256::from_str_radix(
            "483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8",
            16,
        )
        .unwrap(),
        order: U256::from_str_radix(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141",
            16,
        )
        .unwrap(),
    }
}

fn main() {
    let mut ops_path = "ops.bin".to_string();
    let mut from: u64 = 0;
    let mut count: u64 = 1;
    let mut threads: usize = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    // Depth is taken from the builder's dump unless overridden explicitly.
    let mut cfg = Cfg { rounds: 0, rounds_mul: 0 };
    let mut geometry = "geom.tsv".to_string();
    let mut out_path: Option<String> = None;
    let mut explicit: Vec<u64> = Vec::new();
    let mut verbose = false;
    let mut envelope_mode = false;
    let mut replay_check = false;
    let mut replay_count_mode = false;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--ops" => ops_path = next(),
            "--geometry" => geometry = next(),
            "--from" => from = next().parse().expect("--from"),
            "--count" => count = next().parse().expect("--count"),
            "--threads" => threads = next().parse().expect("--threads"),
            "--rounds" => cfg.rounds = next().parse().expect("--rounds"),
            "--rounds-mul" => cfg.rounds_mul = next().parse().expect("--rounds-mul"),
            "--out" => out_path = Some(next()),
            "--nonce" => explicit.push(next().parse().expect("--nonce")),
            "--verbose" => verbose = true,
            "--envelope" => envelope_mode = true,
            "--replay-selftest" => replay_check = true,
            "--replay-count" => replay_count_mode = true,
            other => panic!("unknown argument {other}"),
        }
    }

    load_geometry(&geometry).unwrap_or_else(|e| panic!("pp_screen: {e}"));
    let (dumped_div, dumped_mul) = *DUMPED_ROUNDS.get().expect("rounds loaded");
    if cfg.rounds == 0 {
        cfg.rounds = dumped_div;
    }
    if cfg.rounds_mul == 0 {
        cfg.rounds_mul = dumped_mul;
    }
    if cfg.rounds != dumped_div || cfg.rounds_mul != dumped_mul {
        eprintln!(
            "pp_screen: WARNING depth overridden to {}/{} but the geometry dump says {}/{}; \
             the width schedule in that dump was resolved at the dumped depth",
            cfg.rounds, cfg.rounds_mul, dumped_div, dumped_mul
        );
    }
    let curve = secp256k1();
    let fb = Arc::new(FixedBase::new(fe_from_u256(curve.gx), fe_from_u256(curve.gy)));
    selftest(&curve, &fb).unwrap_or_else(|e| panic!("pp_screen selftest failed: {e}"));
    let p_w = from_u256(curve.modulus);
    if replay_check {
        replay_selftest(&curve, &p_w).unwrap_or_else(|e| panic!("pp_screen replay selftest failed: {e}"));
        return;
    }
    let t = Arc::new(Transcript::load(&ops_path).unwrap_or_else(|e| panic!("{e}")));
    eprintln!(
        "pp_screen: selftest ok, {} tail ops, rounds={} rounds_mul={}, {} threads",
        t.tail.len(),
        cfg.rounds,
        cfg.rounds_mul,
        threads
    );

    let jobs: Vec<u64> = if explicit.is_empty() {
        (from..from + count).collect()
    } else {
        explicit
    };

    if replay_count_mode {
        // Ground truth: the shipped nonce is 9,024/9,024 clean, so a correct
        // fold predicate must report zero escapes on it. Anything else means
        // the predicate is wrong, whatever the selftest says.
        let f_u = U256::MAX.wrapping_sub(curve.modulus).wrapping_add(U256::from(1));
        let f = r_from_u256(f_u);
        let negf = neg_f_bits(&f, win().0);
        let f_minus_one = r_from_u256(U256::MAX.wrapping_sub(curve.modulus));
        for nonce in &jobs {
            let mut xof = t.seed(*nonce);
            let (mut shots, mut walk_clean, mut escapes, mut shots_with_escape) = (0, 0, 0usize, 0);
            while shots < NUM_TESTS {
                let mut rb = [[0u8; 32]; 2];
                XofReader::read(&mut xof, &mut rb[0]);
                XofReader::read(&mut xof, &mut rb[1]);
                let a = fb.mul(&fe_from_u256(U256::from_le_bytes(rb[0])));
                let b = fb.mul(&fe_from_u256(U256::from_le_bytes(rb[1])));
                if a.is_infinity() || b.is_infinity() {
                    continue;
                }
                if fe_mul(&a.x, &fe_sqr(&b.z)) == fe_mul(&b.x, &fe_sqr(&a.z)) {
                    continue;
                }
                let aff = batch_to_affine(&[a, b]);
                let (ta, oa) = (aff[0], aff[1]);
                shots += 1;
                let d1 = fe_to_u256(&fe_sub(&ta.x, &oa.x));
                // The divide is handed y2 - oy as its numerator.
                let numer = fe_to_u256(&fe_sub(&ta.y, &oa.y));
                let mut e = 0usize;
                let mut clean = true;
                match walk_tape(&p_w, d1, cfg.rounds) {
                    Some(tape) => e += replay_div_escapes(&tape, numer, &f, &negf, &f_minus_one),
                    None => clean = false,
                }
                // Multiply traversal: denominator is offset.x - result.x, and
                // its numerator is the slope the divide just produced.
                let lambda = fe_mul(
                    &fe_sub(&oa.y, &ta.y),
                    &fe_inv(&fe_sub(&oa.x, &ta.x)),
                );
                let ex = fe_sub(&fe_sub(&fe_sqr(&lambda), &ta.x), &oa.x);
                let d2 = fe_to_u256(&fe_sub(&oa.x, &ex));
                let lam = fe_to_u256(&lambda);
                match walk_tape_signs(&p_w, d2, cfg.rounds_mul) {
                    Some((tape, u_sign, v_sign)) => {
                        let neg = |on: bool, v: U256| if on { curve.modulus - v } else { v };
                        e += replay_mul_escapes(
                            &tape,
                            neg(u_sign, lam),
                            neg(v_sign, lam),
                            &f,
                            &negf,
                            &f_minus_one,
                        );
                    }
                    None => clean = false,
                }
                if clean {
                    walk_clean += 1;
                }
                escapes += e;
                if e > 0 {
                    shots_with_escape += 1;
                }
            }
            println!(
                "REPLAY\t{nonce}\tshots={shots}\twalk_clean={walk_clean}\tfold_escapes={escapes}\tshots_with_escape={shots_with_escape}"
            );
        }
        return;
    }

    if envelope_mode {
        // The shipped schedule, for comparison. `bit-rounds` is the sum of the
        // per-round widths: it is what the walk and replay adds are priced in,
        // so a narrower envelope is directly fewer Toffoli.
        for nonce in &jobs {
            let raw = envelope(&t, &fb, &p_w, *nonce, &cfg);
            // shrink_to only shrinks, so the minimal admissible schedule is the
            // suffix maximum of the raw envelope.
            let mut need = raw.clone();
            for i in (0..need.len().saturating_sub(1)).rev() {
                need[i] = need[i].max(need[i + 1]);
            }
            let shipped: usize = (0..need.len()).map(value_width).sum();
            let fitted: usize = need.iter().map(|w| (*w as usize).max(8)).sum();
            let slack: i64 = (0..need.len())
                .map(|r| value_width(r) as i64 - (need[r] as i64).max(8))
                .sum();
            let violations = (0..need.len()).filter(|&r| need[r] as usize > value_width(r)).count();
            println!(
                "ENVELOPE\t{nonce}\tshipped_bitrounds={shipped}\tfitted_bitrounds={fitted}\tslack={slack}\trounds_over_shipped={violations}"
            );
            if verbose {
                let widths: Vec<String> = need.iter().map(|w| w.to_string()).collect();
                println!("ENVWIDTHS\t{nonce}\t{}", widths.join(","));
            }
        }
        return;
    }
    let cursor = Arc::new(AtomicU64::new(0));
    let screened = Arc::new(AtomicU64::new(0));
    let jobs = Arc::new(jobs);
    let cfg = Arc::new(cfg);

    let mut handles = Vec::new();
    for _ in 0..threads {
        let verbose = verbose;
        let (t, fb, jobs, cursor, screened, cfg) = (
            Arc::clone(&t),
            Arc::clone(&fb),
            Arc::clone(&jobs),
            Arc::clone(&cursor),
            Arc::clone(&screened),
            Arc::clone(&cfg),
        );
        handles.push(std::thread::spawn(move || {
            let mut hits: Vec<(u64, usize)> = Vec::new();
            loop {
                let i = cursor.fetch_add(1, Ordering::Relaxed) as usize;
                if i >= jobs.len() {
                    break;
                }
                let nonce = jobs[i];
                let first_fail = screen(&t, &fb, &p_w, nonce, &cfg);
                screened.fetch_add(1, Ordering::Relaxed);
                match first_fail {
                    None => {
                        // Flush explicitly. Rust's stdout is block-buffered when
                        // it is a pipe rather than a terminal, so on a run of
                        // hours a survivor would otherwise sit unwritten until
                        // the process exits. Nothing is lost either way, but a
                        // hunt you cannot watch is a hunt you cannot steer, and
                        // no amount of line-buffering on the CONSUMER side of the
                        // pipe fixes a producer that has not written yet.
                        println!("SURVIVOR\t{nonce}");
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                        hits.push((nonce, usize::MAX));
                    }
                    Some(shot) => {
                        if verbose {
                            println!("FIRSTFAIL\t{nonce}\t{shot}");
                        }
                        hits.push((nonce, shot));
                    }
                }
            }
            hits
        }));
    }

    let mut all: Vec<(u64, usize)> = Vec::new();
    for h in handles {
        all.extend(h.join().expect("screen thread panicked"));
    }
    all.sort_unstable();

    let survivors: Vec<u64> = all.iter().filter(|(_, s)| *s == usize::MAX).map(|(n, _)| *n).collect();
    let failed: Vec<usize> = all.iter().filter(|(_, s)| *s != usize::MAX).map(|(_, s)| *s).collect();
    let mean_shot = if failed.is_empty() {
        0.0
    } else {
        failed.iter().sum::<usize>() as f64 / failed.len() as f64
    };
    eprintln!(
        "pp_screen: screened {}, survivors {}, mean shots to first failure {:.1}",
        all.len(),
        survivors.len(),
        mean_shot
    );

    if let Some(path) = out_path {
        let mut f = File::create(&path).unwrap_or_else(|e| panic!("create {path}: {e}"));
        for n in &survivors {
            writeln!(f, "{n}").expect("write survivor");
        }
    }
}

// ─── tests ─────────────────────────────────────────────────────────────────
//
// `cargo test --bin pp_screen`, which needs the same copy into `src/bin/` that
// building the tool needs; see the README's Validation section.
//
// The width schedule and the truncation windows are read from the builder's
// dump, never recomputed here, so `walk_ok` and `replay_selftest` panic on a
// bare `cargo test`. These tests install a synthetic geometry instead of
// shipping a frozen copy of a table that has already moved three times. What
// that buys is a check on the walk and replay *logic*; agreement with the real
// schedule is what the end-to-end runs establish, and they are the right
// instrument for it.
#[cfg(test)]
mod tests {
    use super::*;

    /// Fill the geometry `OnceLock`s with a synthetic schedule.
    ///
    /// `cargo test` runs its tests as threads in one process and these are
    /// process-global, so every test has to agree on one geometry. A `Once`
    /// makes that explicit rather than leaving it to whichever test wins.
    fn install_test_geometry() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            // Uniform full width: no round can fail on width, so a walk failure
            // means non-convergence and nothing else.
            let _ = SCHEDULE.set(vec![VALUE_WIDTH as u16; 700]);
            // (fold_div, fold_mul, endpoint, chunk_cmp, flag_cmp), in the
            // shape and rough size instrument.py dumps.
            let _ = WINDOWS.set((54, 54, 30, 22, 22));
            let _ = DUMPED_ROUNDS.set((698, 698));
        });
    }

    /// A deterministic stand-in for random field elements, the same shape the
    /// existing selftests use.
    fn lcg(seed: &mut U256, modulus: U256, i: u64) -> U256 {
        *seed = seed
            .mul_mod(U256::from(6364136223846793005u64), modulus)
            .add_mod(U256::from(i + 1), modulus);
        *seed
    }

    fn w_from_i64(v: i64) -> W {
        let m = from_u256(U256::from(v.unsigned_abs()));
        if v < 0 {
            neg(&m)
        } else {
            m
        }
    }

    #[test]
    fn selftests_pass() {
        install_test_geometry();
        let curve = secp256k1();
        let fb = FixedBase::new(fe_from_u256(curve.gx), fe_from_u256(curve.gy));
        assert_eq!(selftest(&curve, &fb), Ok(()));
        let p_w = from_u256(curve.modulus);
        assert_eq!(replay_selftest(&curve, &p_w), Ok(()));
    }

    #[test]
    fn fe_mul_matches_u256_mul_mod() {
        let modulus = secp256k1().modulus;
        let mut sa = U256::from(7u64);
        let mut sb = U256::from(11u64);
        for i in 0..200u64 {
            let a = lcg(&mut sa, modulus, i);
            let b = lcg(&mut sb, modulus, i * 3 + 1);
            assert_eq!(
                fe_to_u256(&fe_mul(&fe_from_u256(a), &fe_from_u256(b))),
                a.mul_mod(b, modulus),
                "fe_mul disagrees at i={i}: a={a} b={b}"
            );
        }
    }

    #[test]
    fn fe_inv_inverts() {
        let modulus = secp256k1().modulus;
        let one = fe_from_u256(U256::from(1u64));
        let mut s = U256::from(3u64);
        for i in 0..50u64 {
            let a = lcg(&mut s, modulus, i);
            if a.is_zero() {
                continue;
            }
            let fa = fe_from_u256(a);
            assert_eq!(fe_mul(&fe_inv(&fa), &fa), one, "fe_inv wrong at i={i}: a={a}");
        }
    }

    #[test]
    fn fe_sub_undoes_fe_add() {
        let modulus = secp256k1().modulus;
        let mut sa = U256::from(13u64);
        let mut sb = U256::from(17u64);
        for i in 0..200u64 {
            let a = fe_from_u256(lcg(&mut sa, modulus, i));
            let b = fe_from_u256(lcg(&mut sb, modulus, i * 5 + 2));
            assert_eq!(fe_sub(&fe_add(&a, &b), &b), a, "add/sub not inverse at i={i}");
        }
    }

    #[test]
    fn wrap_signed_is_identity_when_it_fits() {
        // -1 sign-extends into every width, so it is the sharpest case.
        for v in [0i64, 1, -1, 2, -2, 127, -128, 65535, -65536] {
            let a = w_from_i64(v);
            for w in [8usize, 17, 32, 64, 65, 128, 191, 256, VALUE_WIDTH] {
                if fits_signed(&a, w) {
                    assert_eq!(wrap_signed(&a, w), a, "wrap_signed changed v={v} at w={w}");
                }
            }
        }
        // -1 in particular, at every width including the narrowest legal one.
        let neg_one = w_from_i64(-1);
        for w in 1..=VALUE_WIDTH {
            assert!(fits_signed(&neg_one, w), "-1 should fit at w={w}");
            assert_eq!(wrap_signed(&neg_one, w), neg_one, "wrap_signed changed -1 at w={w}");
        }
    }

    #[test]
    fn sar1_halves_and_keeps_sign() {
        assert_eq!(sar1(&w_from_i64(4)), w_from_i64(2));
        assert_eq!(sar1(&w_from_i64(-4)), w_from_i64(-2));
        assert_eq!(sar1(&w_from_i64(2)), w_from_i64(1));
        assert_eq!(sar1(&w_from_i64(-2)), w_from_i64(-1));
        // An arithmetic shift floors, so -1 stays -1 rather than reaching 0.
        assert_eq!(sar1(&w_from_i64(-1)), w_from_i64(-1));
        assert!(is_neg(&sar1(&w_from_i64(-4))));
        assert!(!is_neg(&sar1(&w_from_i64(4))));
    }

    #[test]
    fn walk_ok_on_a_convergent_input() {
        install_test_geometry();
        // p = 5, denominator = 3, worked through by hand: the pair goes
        // (5,3) -> (5,-1) -> (3,-1) -> (3,1) -> (1,1), so four rounds land on
        // (+/-1, +/-1) and three do not.
        let p = from_u256(U256::from(5u64));
        assert!(walk_ok(&p, U256::from(3u64), 4), "should converge in 4 rounds");
        assert!(!walk_ok(&p, U256::from(3u64), 3), "should not have converged in 3");
    }
}
