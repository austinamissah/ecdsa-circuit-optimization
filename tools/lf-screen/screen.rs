//! Classical walk pre-filter for the Leapfrog Fiat-Shamir nonce search.
//!
//! A Rust port of `lf_model.py`. The 96-op X tail at the end of the op stream
//! changes no executed gate, only the Fiat-Shamir seed and therefore the 9,024
//! test inputs, so it is a free nonce. A nonce is valid only if its draw hits no
//! failure. This tool replays the two Leapfrog rail walks of every shot (the
//! divide's on `tx - ox`, the multiply's on `ox - x3`) in integer arithmetic and
//! stops at the first shot whose walk fails.
//!
//! # What is modeled, and why it is exact
//!
//! The walk is the fast rail path of `src/point_add/leapfrog.rs` under the
//! pinned recipe: two odd two's-complement rails, per-tick widths from the
//! recipe's width table, two forced steps and a choice step per tick, and the
//! barrel shift by the choice's valuation. A width failure is a resize that
//! drops a bit that is not a sign extension (the circuit frees a dirty wire);
//! the walk also fails if it ends with either rail off +-1. Both are certain
//! failures of the shot (validated: 180 of 180 predicted failures failed on
//! 14 nonces, 126,350 shots, zero false positives).
//!
//! The choice rule per tick:
//!
//! * tick 0 (`LF_T0_FREE`): no window. Equal-sign veto at valuation 2 with the
//!   sign test `m = !s1 & [bits 244..255 of |R1| > C]`, C the top 12 bits of
//!   p/5 or p/3.
//! * ticks 1..R-2 (`LF_YP8`, `LF_YP8_LATE`, `YP8_VETO=1`): window compare
//!   `Y = [|T2| > 2|B|]` on L-bit windows at the yp8 anchors; with
//!   `q = E & (Y ^ not_flipped)`, deep is vetoed at valuation 2 when q and at
//!   valuation 3 when q and the second forced step added magnitudes.
//! * tick R-1 (`LF_TAIL_FORCED=0`): no forced steps; the `LF_W1` rule,
//!   `[|T| > 4|B|]`.
//!
//! # What it does not see
//!
//! Payload-cell misses (truncated folds and compares in the arithmetic the walk
//! drives) are not modeled. They were about 5.6 of the 18.4 failing shots per
//! 9,024 at `0c3a075`, so a survivor here still needs `eval_circuit`.
//!
//! # The recipe is read, never assumed
//!
//! The window length, the late-window schedule and the width table are set in
//! `install_recipe` (last write wins). This tool parses those settings out of
//! `leapfrog.rs` at startup, and refuses to run if a setting it depends on has a
//! value it does not model.
//!
//! Build (it deliberately does not live in `src/`, so it can never be swept
//! into a submission):
//!
//!   cp tools/lf-screen/screen.rs src/bin/lf_screen.rs
//!   cargo build --release --bin lf_screen
//!   rm src/bin/lf_screen.rs
//!
//! Usage:
//!   lf_screen --ops ops.bin --from <n> --count <k> [--threads T] [--out survivors.txt] [--verbose]
//!   lf_screen --ops ops.bin --nonce <n> [--nonce <n> ...] --full     # walk failures per nonce, no early abort
//!   lf_screen --ops ops.bin --nonce <n> --dump                       # one line per shot, failure sites
//!   lf_screen --walks <k> [--seed s]                                 # random denominators, failure rate
//!
//! `ops.bin` must be the stream built from the tree whose `leapfrog.rs` is read
//! (`--src`, default `src/point_add`): the seed is a hash of the whole stream.

use alloy_primitives::U256;
use quantum_ecc::weierstrass_elliptic_curve::WeierstrassEllipticCurve;
use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

// ─── op stream framing (mirrors eval_circuit's loader) ─────────────────────

const MAGIC: &[u8; 8] = b"QECCOPSZ";
const ZSTD_WINDOW_LOG_MAX: u32 = 27;
const OP_BYTES: usize = 7 * 8;
/// Bytes absorbed per op: the kind discriminant, then six u64 fields.
const HASH_BYTES: usize = 1 + 6 * 8;
const TAIL_OPS: usize = 96;
const NUM_TESTS: usize = 9024;


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

// ─── the recipe, read from leapfrog.rs ─────────────────────────────────────

/// What the walk model needs from the pinned recipe and its data files.
struct Recipe {
    /// Per-tick widths (W, n0, n1, n2, nb).
    steps: Vec<[usize; 5]>,
    /// Window anchors (`yp8_L22_138_anch.txt`), indexed by tick, last entry repeated.
    anch: Vec<usize>,
    /// `LF_YP8`: the window length L before any late entry.
    l_base: usize,
    /// `LF_YP8_LATE`: (from tick, L), in order; the last entry at or before a tick wins.
    late: Vec<(usize, usize)>,
    /// The width table's file name, for the startup line.
    table: String,
}

static RECIPE: OnceLock<Recipe> = OnceLock::new();

fn recipe() -> &'static Recipe {
    RECIPE.get().expect("recipe not loaded")
}

/// The string-literal settings of `install_recipe`, last write wins. Entries whose value is an expression
/// (`if C35_CAP { .. }`) are skipped; none of them is one this model reads.
fn recipe_settings(src: &str) -> Result<std::collections::HashMap<String, String>, String> {
    let start = src.find("pub(crate) fn install_recipe()").ok_or("leapfrog.rs: no install_recipe")?;
    let body = &src[start..];
    let end = body.find("std::env::set_var(k, v);").ok_or("leapfrog.rs: install_recipe has no set_var loop")?;
    let mut out = std::collections::HashMap::new();
    for line in body[..end].lines() {
        let l = line.trim();
        if !l.starts_with("(\"") {
            continue;
        }
        let parts: Vec<&str> = l.split('"').collect();
        // ("KEY", "VALUE"),  ->  ["(", KEY, ", ", VALUE, "),"]
        if parts.len() >= 5 && parts[2].trim() == "," {
            out.insert(parts[1].to_string(), parts[3].to_string());
        }
    }
    Ok(out)
}

fn load_recipe(src_dir: &str) -> Result<(), String> {
    let path = format!("{src_dir}/leapfrog.rs");
    let src = std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))?;
    let set = recipe_settings(&src)?;
    let get = |k: &str| set.get(k).map(String::as_str);
    // settings whose other values change the walk in ways this model does not follow
    for (k, want) in [
        ("LEAPFROG", "1"),
        ("LEAPFROG_M", "2"),
        ("LF_FAST", "1"),
        ("LF_SEED", "half"),
        ("LF_W1", "1"),
        ("LF_T0_FREE", "1"),
        ("LF_TAIL_FORCED", "0"),
        ("YP8_VETO", "1"),
    ] {
        if get(k) != Some(want) {
            return Err(format!("{path}: recipe sets {k} = {:?}, the model needs {want:?}", get(k)));
        }
    }
    if let Some(v) = get("LF_U2_ENV") {
        return Err(format!("{path}: recipe sets LF_U2_ENV = {v:?}; the model reads only the pack tables"));
    }
    for needle in ["const T0_KH: usize = 12;", "0xCCCusize", "0x666usize", "w1_chain_s(c, &wa, &wb, 1)"] {
        if !src.contains(needle) {
            return Err(format!("{path}: no `{needle}`; the tick-0 or YP8 compare changed"));
        }
    }
    let l_base: usize = get("LF_YP8").ok_or("recipe has no LF_YP8")?.parse().map_err(|e| format!("LF_YP8: {e}"))?;
    let late = match get("LF_YP8_LATE") {
        None => Vec::new(),
        Some(v) => v
            .split(',')
            .map(|e| {
                let (a, b) = e.split_once(':').ok_or(format!("LF_YP8_LATE entry {e:?}"))?;
                Ok((a.trim().parse().map_err(|x| format!("{e}: {x}"))?, b.trim().parse().map_err(|x| format!("{e}: {x}"))?))
            })
            .collect::<Result<Vec<(usize, usize)>, String>>()?,
    };
    let table = if get("LF_G10_TABLE") == Some("770") {
        "pack_t0hat_tail0_138_steps_k770_lw2.txt"
    } else {
        "pack_t0hat_tail0_138_steps_k700_lw2.txt"
    };
    if !src.contains(table) {
        return Err(format!("{path}: `steps()` no longer names {table}"));
    }
    let data = format!("{src_dir}/leapfrog_data");
    let read = |n: &str| std::fs::read_to_string(format!("{data}/{n}")).map_err(|e| format!("read {data}/{n}: {e}"));
    let steps: Vec<[usize; 5]> = read(table)?
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let v: Vec<usize> = l.split_whitespace().map(|x| x.parse().expect("width")).collect();
            [v[0], v[1], v[2], v[3], v[4]]
        })
        .collect();
    let anch: Vec<usize> = read("yp8_L22_138_anch.txt")?
        .split(|c| c == ',' || c == '\n')
        .filter_map(|v| v.trim().parse().ok())
        .collect();
    if steps.len() < 2 || anch.is_empty() {
        return Err("empty width or anchor table".into());
    }
    let _ = RECIPE.set(Recipe { steps, anch, l_base, late, table: table.to_string() });
    Ok(())
}

/// `w1_kt`: K = L - 2 at tick t.
fn win_k(t: usize) -> usize {
    let r = recipe();
    r.late.iter().filter(|&&(from, _)| t >= from).last().map_or(r.l_base, |&(_, l)| l) - 2
}

fn anchor(t: usize) -> usize {
    let a = &recipe().anch;
    a[t.min(a.len() - 1)]
}

// ─── the walk ──────────────────────────────────────────────────────────────

/// Field width in bits.
const N: usize = 256;

/// Bits [h, h + n) of the ones'-complement magnitude (n <= 32): the window wires after conjugating with the sign.
fn mag_bits(a: &W, h: usize, n: usize) -> u64 {
    let s = if is_neg(a) { u64::MAX } else { 0 };
    let (i, o) = (h / 64, h % 64);
    let lo = a[i] ^ s;
    let hi = if i + 1 < LIMBS { a[i + 1] ^ s } else { s ^ s };
    let v = if o == 0 { lo } else { (lo >> o) | (hi << (64 - o)) };
    v & ((1u64 << n) - 1)
}

/// `[win(a) > 2^s win(b)]` on the K+2-bit windows at tick t (`w1_window` / `w1_chain_s`).
fn window_gt(a: &W, b: &W, t: usize, s: u32) -> bool {
    let k = win_k(t);
    let h = anchor(t).saturating_sub(k);
    mag_bits(a, h, k + 2) > (mag_bits(b, h, k + 2) << s)
}

/// Where a walk failed: a resize that drops a non-sign bit (tick, site), or an end off park.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fail {
    Width(usize, &'static str),
    NoPark,
    NoPark1,
}

impl Fail {
    fn site(&self) -> &'static str {
        match self {
            Fail::Width(_, s) => s,
            Fail::NoPark => "nopark",
            Fail::NoPark1 => "nopark1",
        }
    }
}

fn need(v: &W, n: usize, t: usize, site: &'static str) -> Result<(), Fail> {
    if fits_signed(v, n) { Ok(()) } else { Err(Fail::Width(t, site)) }
}

/// `t0_m_on`: m = !s1 & [V > C], V = bits 244..255 of |R1|, C = 0x333 (s0 = 0) or 0x555 (s0 = 1).
fn t0_m(r1: &W, s0: bool, s1: bool) -> bool {
    let v = mag_bits(r1, N - 12, 12);
    !s1 && v > if s0 { 0x555 } else { 0x333 }
}

/// Valuation of an even, nonzero-mod-2^64-or-not value, capped: anything >= 5 is "no barrel".
fn val(d: &W) -> u32 {
    if d[0] == 0 { 64 } else { d[0].trailing_zeros() }
}

fn walk(p: &W, denominator: U256) -> Result<(), Fail> {
    let r = recipe();
    let st = &r.steps;
    let rounds = st.len();
    // seed_half_rails
    let w0 = st[0][0].max(N + 4);
    let d = from_u256(denominator);
    let r0 = if bit(&d, 0) == 1 { d } else { sub(&d, p) };
    let r1 = if r0[0] & 3 == 3 { sar1(&add(&r0, p)) } else { sar1(&sub(&r0, p)) };
    debug_assert!(fits_signed(&r0, w0) && fits_signed(&r1, w0));
    let mut rails = [r0, r1];
    for t in 0..rounds {
        let [_, n0, n1, n2, nb] = st[t];
        let wn = if t + 1 < rounds { st[t + 1][0] } else { st[t][0] };
        let (ti, si) = (t % 2, 1 - t % 2);
        let (mut tt, b) = (rails[ti], rails[si]);
        let tail = t + 1 == rounds;
        let yp8 = t > 0 && !tail;
        let mut signs = [false; 2];
        let mut t1 = tt;
        if !tail {
            for (i, (nk, sa, sb)) in [(n0, "T@n0", "B@n0"), (n1, "T@n1", "B@n1")].into_iter().enumerate() {
                need(&tt, nk, t, sa)?;
                need(&b, nk, t, sb)?;
                let s = (bit(&tt, 1) ^ bit(&b, 1)) == 1;
                t1 = tt;
                tt = sar1(&wrap_signed(&if s { sub(&tt, &b) } else { add(&tt, &b) }, nk));
                signs[i] = s;
            }
        }
        need(&tt, n2, t, "T@n2")?;
        need(&b, n2, t, "B@n2")?;
        // yp8_logic reads the source through a view at the target's width (n1 - 1, at least 6)
        let bv = if yp8 { wrap_signed(&b, (n1 - 1).max(6)) } else { b };
        let dq = (bit(&tt, 1) ^ bit(&bv, 1)) == 1;
        let dd = if dq { add(&tt, &b) } else { sub(&tt, &b) };
        debug_assert!(dd[0] & 3 == 0);
        let v = val(&dd);
        let eq = is_neg(&tt) ^ is_neg(&bv) ^ dq;
        let veto = if t == 0 {
            v == 2 && !(t0_m(&b, signs[0], signs[1]) ^ dq)
        } else if yp8 {
            let added = is_neg(&t1) == (is_neg(&b) ^ signs[1]);
            let nf = is_neg(&tt) == is_neg(&t1);
            let q = eq && (window_gt(&tt, &bv, t, 1) ^ nf);
            q && (v == 2 || (v == 3 && added))
        } else {
            v == 2 && eq && !window_gt(&tt, &b, t, 2)
        };
        let dsel = v <= 4 && !veto;
        let s3 = dsel ^ dq;
        let e = if dsel { v - 1 } else { 0 };
        tt = sar1(&wrap_signed(&if s3 { sub(&tt, &b) } else { add(&tt, &b) }, n2));
        need(&tt, nb, t, "T@nb")?;
        for _ in 0..e {
            tt = sar1(&tt);
        }
        debug_assert!(tt[0] & 1 == 1);
        need(&tt, wn, t, "T@wn")?;
        need(&b, wn, t, "B@wn")?;
        rails[ti] = tt;
    }
    if !is_pm_one(&rails[0]) {
        return Err(Fail::NoPark);
    }
    if !is_pm_one(&rails[1]) {
        return Err(Fail::NoPark1);
    }
    Ok(())
}

// ─── screening one nonce ───────────────────────────────────────────────────

/// Shots derived per batch. The curve work for a block shares two field inversions; early abort discards the rest
/// of the block, and the mean first failing shot is about 9,024 / 13, so a much larger block is mostly waste.
const BLOCK: usize = 128;

/// One derived shot: target and offset x, and the result x.
struct Shot {
    tx: Fe,
    ox: Fe,
    x3: Fe,
}

/// Derive a nonce's shots block by block, in the harness's order, handing each block to `f`; `f` returns false to
/// stop early.
fn for_shots(t: &Transcript, fb: &FixedBase, nonce: u64, mut f: impl FnMut(usize, &[Shot]) -> bool) {
    let mut xof = t.seed(nonce);
    let mut kept = 0usize;
    let mut tj: Vec<Jac> = Vec::with_capacity(BLOCK);
    let mut oj: Vec<Jac> = Vec::with_capacity(BLOCK);
    let mut shots: Vec<Shot> = Vec::with_capacity(BLOCK);
    while kept < NUM_TESTS {
        let want = BLOCK.min(NUM_TESTS - kept);
        tj.clear();
        oj.clear();
        // The harness skips degenerate draws (infinity, equal x), so the stream must be consumed the same way.
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
        shots.clear();
        for i in 0..want {
            let lambda = fe_mul(&fe_sub(&oa[i].y, &ta[i].y), &inv[i]);
            let x3 = fe_sub(&fe_sub(&fe_sqr(&lambda), &ta[i].x), &oa[i].x);
            shots.push(Shot { tx: ta[i].x, ox: oa[i].x, x3 });
        }
        if !f(kept, &shots) {
            return;
        }
        kept += want;
    }
}

/// The two walks of one shot: divide on tx - ox, multiply on ox - x3.
fn shot_walks(p: &W, s: &Shot) -> (Result<(), Fail>, Result<(), Fail>) {
    (walk(p, fe_to_u256(&fe_sub(&s.tx, &s.ox))), walk(p, fe_to_u256(&fe_sub(&s.ox, &s.x3))))
}

/// First shot whose divide or multiply walk fails, or None if every walk of the nonce is clean.
fn screen(t: &Transcript, fb: &FixedBase, p: &W, nonce: u64) -> Option<usize> {
    let mut first = None;
    for_shots(t, fb, nonce, |base, shots| {
        for (i, s) in shots.iter().enumerate() {
            // divide first: if it fails the multiply need not be walked
            if walk(p, fe_to_u256(&fe_sub(&s.tx, &s.ox))).is_err() || walk(p, fe_to_u256(&fe_sub(&s.ox, &s.x3))).is_err() {
                first = Some(base + i);
                return false;
            }
        }
        true
    });
    first
}

/// splitmix64, for the random-denominator mode.
fn splitmix(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

// ─── self-test and curve ───────────────────────────────────────────────────

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
    let mut src_dir = "src/point_add".to_string();
    let mut from: u64 = 0;
    let mut count: u64 = 1;
    let mut threads: usize = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let mut out_path: Option<String> = None;
    let mut explicit: Vec<u64> = Vec::new();
    let mut verbose = false;
    let mut full = false;
    let mut dump = false;
    let mut walks: Option<u64> = None;
    let mut seed: u64 = 1;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--ops" => ops_path = next(),
            "--src" => src_dir = next(),
            "--from" => from = next().parse().expect("--from"),
            "--count" => count = next().parse().expect("--count"),
            "--threads" => threads = next().parse().expect("--threads"),
            "--out" => out_path = Some(next()),
            "--nonce" => explicit.push(next().parse().expect("--nonce")),
            "--verbose" => verbose = true,
            "--full" => full = true,
            "--dump" => dump = true,
            "--walks" => walks = Some(next().parse().expect("--walks")),
            "--seed" => seed = next().parse().expect("--seed"),
            other => panic!("unknown argument {other}"),
        }
    }

    load_recipe(&src_dir).unwrap_or_else(|e| panic!("lf_screen: {e}"));
    let r = recipe();
    let curve = secp256k1();
    let p_w = from_u256(curve.modulus);

    if let Some(k) = walks {
        // Random denominators in [1, p): the per-walk failure rate, for comparison with lf_model.py.
        let mut s = seed;
        let mut sites: std::collections::BTreeMap<&'static str, u64> = Default::default();
        for _ in 0..k {
            let d = loop {
                let v = U256::from_limbs([splitmix(&mut s), splitmix(&mut s), splitmix(&mut s), splitmix(&mut s)]);
                if !v.is_zero() && v < curve.modulus {
                    break v;
                }
            };
            if let Err(f) = walk(&p_w, d) {
                *sites.entry(f.site()).or_default() += 1;
            }
        }
        let n: u64 = sites.values().sum();
        println!("WALKS\t{k}\tfail={n}\trate={:.3e}\t{sites:?}", n as f64 / k as f64);
        return;
    }

    let fb = Arc::new(FixedBase::new(fe_from_u256(curve.gx), fe_from_u256(curve.gy)));
    selftest(&curve, &fb).unwrap_or_else(|e| panic!("lf_screen selftest failed: {e}"));
    let t = Arc::new(Transcript::load(&ops_path).unwrap_or_else(|e| panic!("{e}")));
    eprintln!(
        "lf_screen: selftest ok, {} tail ops, {} ticks ({}), L {} late {:?}, {} threads",
        t.tail.len(),
        r.steps.len(),
        r.table,
        r.l_base,
        r.late,
        threads
    );
    let jobs: Vec<u64> = if explicit.is_empty() { (from..from + count).collect() } else { explicit };

    if dump {
        println!("nonce\tshot\ttx\tox\tx3\tdiv\tmul");
        for &nonce in &jobs {
            for_shots(&t, &fb, nonce, |base, shots| {
                for (i, s) in shots.iter().enumerate() {
                    let (d, m) = shot_walks(&p_w, s);
                    let site = |x: Result<(), Fail>| x.err().map_or("-", |f| f.site());
                    println!(
                        "{nonce}\t{}\t{:x}\t{:x}\t{:x}\t{}\t{}",
                        base + i,
                        fe_to_u256(&s.tx),
                        fe_to_u256(&s.ox),
                        fe_to_u256(&s.x3),
                        site(d),
                        site(m)
                    );
                }
                true
            });
        }
        return;
    }

    let cursor = Arc::new(AtomicU64::new(0));
    let jobs = Arc::new(jobs);
    let mut handles = Vec::new();
    for _ in 0..threads {
        let (t, fb, jobs, cursor) = (Arc::clone(&t), Arc::clone(&fb), Arc::clone(&jobs), Arc::clone(&cursor));
        handles.push(std::thread::spawn(move || {
            let mut hits: Vec<(u64, usize)> = Vec::new();
            loop {
                let i = cursor.fetch_add(1, Ordering::Relaxed) as usize;
                if i >= jobs.len() {
                    break;
                }
                let nonce = jobs[i];
                if full {
                    // every shot walked: walk-failing shots per nonce, the model's share of lambda
                    let (mut shots, mut fails, mut div, mut mul) = (0usize, 0usize, 0usize, 0usize);
                    for_shots(&t, &fb, nonce, |_, ss| {
                        for s in ss {
                            let (d, m) = shot_walks(&p_w, s);
                            shots += 1;
                            fails += (d.is_err() || m.is_err()) as usize;
                            div += d.is_err() as usize;
                            mul += m.is_err() as usize;
                        }
                        true
                    });
                    println!("FULL\t{nonce}\tshots={shots}\twalk_fail={fails}\tdiv={div}\tmul={mul}");
                    let _ = std::io::stdout().flush();
                    hits.push((nonce, fails));
                    continue;
                }
                match screen(&t, &fb, &p_w, nonce) {
                    None => {
                        // flushed: stdout to a pipe is block-buffered, and a survivor should be visible at once
                        println!("SURVIVOR\t{nonce}");
                        let _ = std::io::stdout().flush();
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
    if full {
        let n = all.len() as f64;
        let mean = all.iter().map(|&(_, f)| f as f64).sum::<f64>() / n;
        let var = all.iter().map(|&(_, f)| (f as f64 - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        eprintln!("lf_screen: {} nonces, walk-failing shots per nonce {:.3} +- {:.3} (se)", all.len(), mean, (var / n).sqrt());
        return;
    }
    let survivors: Vec<u64> = all.iter().filter(|(_, s)| *s == usize::MAX).map(|(n, _)| *n).collect();
    let failed: Vec<usize> = all.iter().filter(|(_, s)| *s != usize::MAX).map(|(_, s)| *s).collect();
    let mean_shot = if failed.is_empty() { 0.0 } else { failed.iter().sum::<usize>() as f64 / failed.len() as f64 };
    eprintln!("lf_screen: screened {}, survivors {}, mean shots to first failure {:.1}", all.len(), survivors.len(), mean_shot);
    if let Some(path) = out_path {
        let mut f = File::create(&path).unwrap_or_else(|e| panic!("create {path}: {e}"));
        for n in &survivors {
            writeln!(f, "{n}").expect("write survivor");
        }
    }
}

// ─── tests ─────────────────────────────────────────────────────────────────
//
// `cargo test --release --bin lf_screen`, with the same copy into `src/bin/` that building needs. They read the
// recipe from `src/point_add`, so they check the model against the tree it is built in.
#[cfg(test)]
mod tests {
    use super::*;

    fn w_from_i64(v: i64) -> W {
        let m = from_u256(U256::from(v.unsigned_abs()));
        if v < 0 { neg(&m) } else { m }
    }

    #[test]
    fn mag_bits_is_ones_complement() {
        assert_eq!(mag_bits(&w_from_i64(0b1011_0000), 4, 4), 0b1011);
        // -1 -> magnitude 0, -6 = ...11010 -> !x = 5
        assert_eq!(mag_bits(&w_from_i64(-1), 0, 20), 0);
        assert_eq!(mag_bits(&w_from_i64(-6), 0, 8), 5);
        // a window straddling a limb boundary
        let x = add(&from_u256(U256::from(1u64) << 70), &from_u256(U256::from(1u64) << 60));
        assert_eq!(mag_bits(&x, 58, 14), (1 << 12) | (1 << 2));
    }

    #[test]
    fn recipe_parses_and_walks_park() {
        load_recipe("src/point_add").expect("recipe");
        assert_eq!(recipe().steps.len(), 138);
        let p = from_u256(secp256k1().modulus);
        // a typical walk parks; the per-walk failure rate is ~6e-4, so 40 random walks failing more than twice
        // would mean the model is broken rather than unlucky
        let mut s = 7u64;
        let mut bad = 0;
        for _ in 0..40 {
            let d = U256::from_limbs([splitmix(&mut s), splitmix(&mut s), splitmix(&mut s), splitmix(&mut s) >> 1]);
            bad += walk(&p, d).is_err() as usize;
        }
        assert!(bad <= 2, "{bad} of 40 walks failed");
    }
}
