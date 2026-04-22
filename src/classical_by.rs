//! Classical reference implementation of Bernstein–Yang `divstep2` for
//! modular inversion over GF(p), p = secp256k1 prime.
//!
//! Reference: D. J. Bernstein, B.-Y. Yang, "Fast constant-time gcd
//! computation and modular inversion." IACR eprint 2019/266,
//! TCHES 2019 Issue 3. Section 8 (divstep2).
//!
//! ## Purpose
//!
//! Deliverable 1 of the 2026-04-22 research task: empirically measure
//! how many divstep iterations secp256k1 inputs ACTUALLY require, so we
//! can cross-check the theoretical safegcd upper bound N = ⌈(49n+57)/17⌉.
//! For n = 256, N_bound = ⌈12601/17⌉ = 742.
//!
//! If empirical convergence is substantially below 742, the per-iter
//! cost of a quantum B-Y may still be dominated by something cheaper
//! than a fully-unrolled 742-iter loop, and the "B-Y is analytically
//! worse than Kaliski" conclusion from session 2026-04-22 may be
//! pessimistic.
//!
//! ## Algorithm
//!
//! ```text
//! divstep(δ, f, g):
//!   if δ > 0 and g is odd:   (1 − δ, g, (g − f) / 2)
//!   elif         g is odd:   (1 + δ, f, (g + f) / 2)
//!   else:                    (1 + δ, f, g / 2)
//! ```
//!
//! Invariants: f always odd; |f|, |g| ≤ max(|f₀|, |g₀|).
//!
//! For modinv of g_0 mod prime p: start (δ, f, g) = (1, p, g_0),
//! iterate until g = 0. Then f = ±1 (since gcd is 1). Track
//! (U, V, Q, R) ∈ ℤ/p satisfying
//!
//! ```text
//! 2^k · f_k = U · p + V · g_0   (mod p)
//! 2^k · g_k = Q · p + R · g_0   (mod p)
//! ```
//!
//! At termination (f = ±1, g = 0, k iters done):
//!
//! ```text
//! 2^k · f = V · g_0         (mod p)
//! g_0^{-1} = sign(f) · V · 2^{-k}   (mod p)
//! ```
//!
//! The (U, V, Q, R) recurrence (uniform 2^k scaling on BOTH f and g):
//!
//! - Case A (δ>0 ∧ g odd): (U,V,Q,R) → (2Q, 2R, Q-U, R-V), δ → 1-δ.
//! - Case B (g odd, δ≤0):  (U,V,Q,R) → (2U, 2V, Q+U, R+V), δ → 1+δ.
//! - Case C (g even):      (U,V,Q,R) → (2U, 2V,  Q,   R ), δ → 1+δ.
//!
//! Since (U, V, Q, R) can grow like 2^k, we track them modulo p.
//! (f, g) are tracked as signed 257-bit integers via (U256 magnitude, sign).

use alloy_primitives::U256;

/// secp256k1 prime: p = 2^256 − 2^32 − 977.
pub const SECP256K1_P: U256 = U256::from_limbs([
    0xFFFFFFFEFFFFFC2F,
    0xFFFFFFFFFFFFFFFF,
    0xFFFFFFFFFFFFFFFF,
    0xFFFFFFFFFFFFFFFF,
]);

/// Theoretical safegcd iteration bound for n-bit coprime inputs
/// (Bernstein–Yang 2019/266, Theorem 11.2, linearized upper bound).
///
///     N_bound(n) = ⌈(49·n + 57) / 17⌉
///
/// For n = 256, N_bound = 742.
pub fn safegcd_iters(n_bits: usize) -> usize {
    (49 * n_bits + 57 + 16) / 17
}

/// Signed 257-bit integer: an U256 magnitude plus a sign flag.
///
/// Sufficient because during divstep2, `|f|, |g| ≤ max(|f₀|, |g₀|) ≤ p < 2^256`.
/// After step A's sign flip or step B's add-of-f, the magnitude is bounded
/// by the pre-step max, so it always fits in 256 unsigned bits + sign.
#[derive(Clone, Copy, Debug)]
pub struct SInt {
    pub neg: bool,
    pub mag: U256,
}

impl SInt {
    pub fn zero() -> Self { Self { neg: false, mag: U256::ZERO } }
    pub fn from_u(x: U256) -> Self { Self { neg: false, mag: x } }
    pub fn neg_of(&self) -> Self {
        if self.mag.is_zero() { *self } else { Self { neg: !self.neg, mag: self.mag } }
    }
    pub fn bit0(&self) -> bool {
        // Low bit is the same regardless of sign in two's-complement view,
        // but for sign-magnitude we just use the magnitude's low bit —
        // divstep only cares about parity, and |mag| and +/-mag have
        // identical parity.
        self.mag.bit(0)
    }
    pub fn is_zero(&self) -> bool { self.mag.is_zero() }
    pub fn is_one_positive(&self) -> bool { !self.neg && self.mag == U256::from(1) }
    pub fn is_one_negative(&self) -> bool { self.neg && self.mag == U256::from(1) }

    /// (a + b) as signed, assuming both fit in U256 magnitude.
    pub fn add(a: Self, b: Self) -> Self {
        match (a.neg, b.neg) {
            (false, false) => Self { neg: false, mag: a.mag.wrapping_add(b.mag) },
            (true, true)   => Self { neg: true,  mag: a.mag.wrapping_add(b.mag) },
            (false, true)  => sub_mag(a.mag, b.mag),
            (true, false)  => sub_mag(b.mag, a.mag),
        }
    }

    /// (a − b).
    pub fn sub(a: Self, b: Self) -> Self { Self::add(a, b.neg_of()) }

    /// Shift right by 1, respecting sign.
    /// Preconditions: value is even (bit0 = 0).
    pub fn shr1_even(&self) -> Self {
        debug_assert!(!self.bit0(), "shr1_even called on odd value");
        Self { neg: self.neg, mag: self.mag >> 1 }
    }

    pub fn abs_u256(&self) -> U256 { self.mag }
}

fn sub_mag(a: U256, b: U256) -> SInt {
    // returns a − b (signed).
    if a >= b {
        SInt { neg: false, mag: a.wrapping_sub(b) }
    } else {
        SInt { neg: true, mag: b.wrapping_sub(a) }
    }
}

/// (U, V, Q, R) tracked mod p.
#[derive(Clone, Copy, Debug)]
pub struct Coeffs {
    pub uu: U256,
    pub vv: U256,
    pub qq: U256,
    pub rr: U256,
}

impl Coeffs {
    pub fn initial() -> Self {
        Self { uu: U256::from(1), vv: U256::ZERO, qq: U256::ZERO, rr: U256::from(1) }
    }
}

fn addm(a: U256, b: U256, p: U256) -> U256 { a.add_mod(b, p) }
fn subm(a: U256, b: U256, p: U256) -> U256 {
    // (a - b) mod p
    let (r, borrow) = a.overflowing_sub(b);
    if borrow { r.wrapping_add(p) } else { r }
}
fn negm(a: U256, p: U256) -> U256 { if a.is_zero() { a } else { p.wrapping_sub(a) } }
fn mulm(a: U256, b: U256, p: U256) -> U256 { a.mul_mod(b, p) }

/// Outcome of running divstep2 until convergence (g = 0) or hitting max_iters.
#[derive(Debug)]
pub struct DivstepsRun {
    pub converged: bool,
    pub iters_done: usize,
    pub max_abs_delta: i64,
    pub final_f: SInt,
    pub final_g: SInt,
    pub final_coeffs: Coeffs,
}

/// Run divstep2 starting from (δ, f, g) = (1, p, g_0). Iterates until g = 0
/// (convergence) or until `max_iters`. Tracks coefficients mod p.
///
/// Returns the state. If `converged == true`, `final_f ∈ {±1}`.
pub fn run_divsteps(g_0: U256, p: U256, max_iters: usize) -> DivstepsRun {
    assert!(p.bit(0), "p must be odd");
    assert!(g_0 < p, "g_0 must be in [0, p)");
    let mut delta: i64 = 1;
    let mut f = SInt::from_u(p);
    let mut g = SInt::from_u(g_0);
    let mut c = Coeffs::initial();
    let mut max_abs_delta: i64 = 1;
    let mut converged_iter: Option<usize> = None;

    for k in 0..max_iters {
        if g.is_zero() {
            converged_iter = Some(k);
            break;
        }
        // Branch:
        let branch_a = delta > 0 && g.bit0();
        let g_odd = g.bit0();

        if branch_a {
            // (δ, f, g) → (1 − δ, g, (g − f)/2)
            let new_delta = 1 - delta;
            let new_f = g;
            let new_g = SInt::sub(g, f).shr1_even();
            // Coeffs: (U,V,Q,R) → (2Q, 2R, Q-U, R-V)
            let new_uu = addm(c.qq, c.qq, p);
            let new_vv = addm(c.rr, c.rr, p);
            let new_qq = subm(c.qq, c.uu, p);
            let new_rr = subm(c.rr, c.vv, p);
            delta = new_delta;
            f = new_f;
            g = new_g;
            c = Coeffs { uu: new_uu, vv: new_vv, qq: new_qq, rr: new_rr };
        } else if g_odd {
            // (δ, f, g) → (1 + δ, f, (g + f)/2)
            let new_delta = 1 + delta;
            let new_g = SInt::add(g, f).shr1_even();
            let new_uu = addm(c.uu, c.uu, p);
            let new_vv = addm(c.vv, c.vv, p);
            let new_qq = addm(c.qq, c.uu, p);
            let new_rr = addm(c.rr, c.vv, p);
            delta = new_delta;
            g = new_g;
            c = Coeffs { uu: new_uu, vv: new_vv, qq: new_qq, rr: new_rr };
        } else {
            // (δ, f, g) → (1 + δ, f, g / 2)
            let new_delta = 1 + delta;
            let new_g = g.shr1_even();
            let new_uu = addm(c.uu, c.uu, p);
            let new_vv = addm(c.vv, c.vv, p);
            delta = new_delta;
            g = new_g;
            c = Coeffs { uu: new_uu, vv: new_vv, qq: c.qq, rr: c.rr };
        }
        let abs_d = delta.unsigned_abs() as i64;
        if abs_d > max_abs_delta { max_abs_delta = abs_d; }
    }

    let iters_done = converged_iter.unwrap_or(max_iters);
    DivstepsRun {
        converged: converged_iter.is_some(),
        iters_done,
        max_abs_delta,
        final_f: f,
        final_g: g,
        final_coeffs: c,
    }
}

/// Given a successful divsteps run, recover g_0^{-1} mod p.
///
/// From the invariant 2^k · f = V · g_0 (mod p) at termination (f = ±1):
///   g_0^{-1} ≡ sign(f) · V · 2^{-k}  (mod p)
pub fn recover_modinv(run: &DivstepsRun, p: U256) -> Option<U256> {
    if !run.converged { return None; }
    if !(run.final_f.is_one_positive() || run.final_f.is_one_negative()) {
        return None;
    }
    // Compute 2^{-iters_done} mod p via Fermat exponent (p-2) reversed, but
    // more cheaply: 2^{-1} mod p = (p + 1) / 2; iterate.
    let two_inv = (p.wrapping_add(U256::from(1))) >> 1;
    let mut two_inv_k = U256::from(1);
    let mut base = two_inv;
    let mut e = run.iters_done as u64;
    while e > 0 {
        if e & 1 == 1 { two_inv_k = mulm(two_inv_k, base, p); }
        e >>= 1;
        if e > 0 { base = mulm(base, base, p); }
    }
    let v_scaled = mulm(run.final_coeffs.vv, two_inv_k, p);
    if run.final_f.is_one_positive() {
        Some(v_scaled)
    } else {
        Some(negm(v_scaled, p))
    }
}

/// Fermat-little-theorem modular inverse: a^{p-2} mod p.
/// Reference to cross-check against divstep2.
pub fn fermat_modinv(a: U256, p: U256) -> U256 {
    assert!(!a.is_zero());
    let exp = p.wrapping_sub(U256::from(2));
    let mut result = U256::from(1);
    let mut base = a % p;
    for i in 0..256 {
        if exp.bit(i) { result = mulm(result, base, p); }
        base = mulm(base, base, p);
    }
    result
}

/// Deterministic 256-bit sample stream seeded from SHAKE128.
///
/// Produces a sequence of U256 values < p for use as test inputs.
pub struct Sampler {
    reader: Box<dyn sha3::digest::XofReader>,
    p: U256,
}

impl Sampler {
    pub fn new(seed: &[u8], p: U256) -> Self {
        use sha3::digest::{ExtendableOutput, Update};
        let mut hasher = sha3::Shake128::default();
        hasher.update(seed);
        Self { reader: Box::new(hasher.finalize_xof()), p }
    }

    pub fn next(&mut self) -> U256 {
        use sha3::digest::XofReader;
        loop {
            let mut buf = [0u8; 32];
            self.reader.read(&mut buf);
            // Interpret as little-endian U256.
            let x = U256::from_le_slice(&buf);
            if x < self.p && !x.is_zero() {
                return x;
            }
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
//  Jumpdivstep: classical Lehmer-style batching of divstep2
// ═════════════════════════════════════════════════════════════════════════
//
// Per BY 2019/266 §9, `jumpdivsteps_N` compresses N divsteps into a 2×2
// integer transition matrix M by running divstep2 on the TOP bits of (f, g)
// only. Output is M, f_N, g_N such that
//
//     (f_N, g_N)^T = (1/2^N) · M · (f, g)^T
//
// with |M[i][j]| ≤ 2^N.  For reversible quantum implementation, the
// REVERSIBLE per-jump cost is dominated by applying M (up to N-bit classical
// entries) to full-precision (f, g, U, V, Q, R). If we empirically observe
// that matrix entries are typically MUCH smaller than 2^N (say, most are
// ≤ 2^{N/2}), then the reversible cost estimate (using 2^N as entry bound)
// is pessimistic.

/// Transition matrix returned by `jumpdivstep`. Entries are signed;
/// magnitudes ≤ 2^w after w divsteps.
#[derive(Clone, Copy, Debug)]
pub struct TransitionMatrix {
    pub m00: i128, pub m01: i128,
    pub m10: i128, pub m11: i128,
    pub delta_final: i64,
}

/// Run `w` divsteps on the low-`w`-bit representation of (f, g), producing
/// a transition matrix. Also updates δ.
///
/// This is the CLASSICAL matrix computation, done at circuit build time
/// for a reversible implementation.
///
/// Precondition: f_low is odd (low bit 1); g_low may be anything; only the
/// low `w` bits of each matter.
pub fn jumpdivstep(mut delta: i64, mut f_low: u128, mut g_low: u128, w: usize) -> TransitionMatrix {
    assert!(w <= 64, "w must fit in i128 signed math");
    assert!(f_low & 1 == 1, "f_low must be odd");
    // Identity matrix in i128.
    let (mut a00, mut a01) = (1i128, 0i128);
    let (mut a10, mut a11) = (0i128, 1i128);
    for _ in 0..w {
        let g_odd = (g_low & 1) != 0;
        if delta > 0 && g_odd {
            // (f, g) → (g, (g - f) / 2). Matrix row swap then g row -= f row, halve.
            // In matrix form: new_a_row = old_b_row;
            //                 new_b_row = (old_b_row - old_a_row);
            // (Halving happens at the end of each step implicitly on the g row.)
            let tmp_f = f_low; f_low = g_low; g_low = tmp_f.wrapping_neg();
            let (ta, tb) = (a00, a01); a00 = a10; a01 = a11;
            a10 = a10.wrapping_sub(ta); a11 = a11.wrapping_sub(tb);
            delta = -delta;
        }
        let g_odd = (g_low & 1) != 0;
        delta += 1;
        if g_odd {
            g_low = g_low.wrapping_add(f_low);
            a10 = a10.wrapping_add(a00);
            a11 = a11.wrapping_add(a01);
        }
        // Halve g (arithmetic right-shift for signed interpretation).
        g_low >>= 1; // lose low bit (always 0 now).
        // Every step doubles (a00, a01) because f is unchanged and g is halved:
        // we're tracking 2^k · f_k and 2^k · g_k. Actually for jumpdivstep the
        // matrix M satisfies 2^N · (f_N, g_N) = M · (f, g), which means M
        // accumulates a factor of 2^N over N steps. We do NOT scale a00, a01
        // — they're already the correct row of the scaled matrix.
    }
    TransitionMatrix { m00: a00, m01: a01, m10: a10, m11: a11, delta_final: delta }
}

/// Empirical measurement of matrix-entry magnitude distribution.
#[derive(Clone, Debug, Default)]
pub struct JumpStats {
    pub samples: usize,
    pub w: usize,
    pub max_entry_abs: i128,
    pub sum_log2_entry_abs: f64, // mean log2 of entry magnitude
    pub nonzero_entries: usize,
}

/// Sample M transitions with w divsteps each, using random (f_low, g_low, δ).
pub fn jump_matrix_entry_survey(
    seed: &[u8],
    n_samples: usize,
    w: usize,
) -> JumpStats {
    use sha3::digest::{ExtendableOutput, Update, XofReader};
    let mut hasher = sha3::Shake128::default();
    hasher.update(seed);
    let mut reader = hasher.finalize_xof();

    let mut stats = JumpStats { samples: 0, w, max_entry_abs: 0, sum_log2_entry_abs: 0.0, nonzero_entries: 0 };
    let mut buf = [0u8; 24]; // 8 bytes f_low + 8 bytes g_low + 8 bytes delta
    for _ in 0..n_samples {
        reader.read(&mut buf);
        let mut f_low = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        f_low |= 1; // ensure odd
        let g_low = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        let delta = (u64::from_le_bytes(buf[16..24].try_into().unwrap()) % 41) as i64 - 20;
        let m = jumpdivstep(delta, f_low as u128, g_low as u128, w);
        for &e in &[m.m00, m.m01, m.m10, m.m11] {
            let abs = e.wrapping_abs();
            if abs > stats.max_entry_abs { stats.max_entry_abs = abs; }
            if abs > 0 {
                stats.sum_log2_entry_abs += (abs as f64).log2();
                stats.nonzero_entries += 1;
            }
        }
        stats.samples += 1;
    }
    stats
}

/// Aggregate statistics from running divsteps on N samples.
#[derive(Debug, Default, Clone)]
pub struct SurveyStats {
    pub samples: usize,
    pub all_converged: bool,
    pub min_iters: usize,
    pub max_iters: usize,
    pub sum_iters: u128,
    pub max_abs_delta: i64,
    pub modinv_matches: usize,
    pub modinv_mismatches: usize,
}

impl SurveyStats {
    pub fn mean_iters(&self) -> f64 {
        if self.samples == 0 { 0.0 } else { (self.sum_iters as f64) / (self.samples as f64) }
    }
}

/// Run divsteps on `n_samples` random inputs from the given sampler, using
/// `max_iters` as the hard ceiling. Returns aggregate stats.
pub fn survey(
    sampler: &mut Sampler,
    n_samples: usize,
    p: U256,
    max_iters: usize,
) -> SurveyStats {
    let mut stats = SurveyStats {
        samples: 0,
        all_converged: true,
        min_iters: usize::MAX,
        max_iters: 0,
        sum_iters: 0,
        max_abs_delta: 0,
        modinv_matches: 0,
        modinv_mismatches: 0,
    };
    for _ in 0..n_samples {
        let x = sampler.next();
        let run = run_divsteps(x, p, max_iters);
        if !run.converged { stats.all_converged = false; }
        let k = run.iters_done;
        stats.samples += 1;
        if k < stats.min_iters { stats.min_iters = k; }
        if k > stats.max_iters { stats.max_iters = k; }
        stats.sum_iters += k as u128;
        if run.max_abs_delta > stats.max_abs_delta { stats.max_abs_delta = run.max_abs_delta; }

        let expected = fermat_modinv(x, p);
        match recover_modinv(&run, p) {
            Some(v) if v == expected => { stats.modinv_matches += 1; }
            _ => { stats.modinv_mismatches += 1; }
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quick smoke test: a few hand-picked inputs on secp256k1.
    #[test]
    fn divstep_smoke() {
        let p = SECP256K1_P;
        let inputs: &[U256] = &[
            U256::from(1),
            U256::from(2),
            U256::from(3),
            U256::from(0xDEADBEEFu64),
            U256::from_limbs([0x0123456789ABCDEF, 0xFEDCBA9876543210,
                              0x0F0F0F0F0F0F0F0F, 0x1234567890ABCDEF]),
            p.wrapping_sub(U256::from(1)),
        ];
        let max_iters = safegcd_iters(256);
        for x in inputs {
            let run = run_divsteps(*x, p, max_iters);
            assert!(run.converged, "did not converge for x={}", x);
            let got = recover_modinv(&run, p).expect("recovery");
            let expected = fermat_modinv(*x, p);
            assert_eq!(got, expected, "modinv mismatch x={}", x);
        }
    }

    /// Deliverable 1: 10,000-sample empirical survey.
    ///
    /// Run with `cargo test --release classical_by::tests::survey_10k -- --nocapture`.
    #[test]
    fn survey_10k() {
        let p = SECP256K1_P;
        let n = 256;
        let theoretical_bound = safegcd_iters(n);
        // Generous ceiling so the sampler can never report false-no-convergence.
        let max_iters = theoretical_bound + 100;
        let mut sampler = Sampler::new(b"divstep2-survey-seed-v1", p);
        let stats = survey(&mut sampler, 10_000, p, max_iters);

        eprintln!("=== B-Y divstep2 empirical survey on secp256k1 ===");
        eprintln!("samples            : {}", stats.samples);
        eprintln!("all_converged      : {}", stats.all_converged);
        eprintln!("theoretical bound  : {}", theoretical_bound);
        eprintln!("min iters observed : {}", stats.min_iters);
        eprintln!("max iters observed : {}", stats.max_iters);
        eprintln!("mean iters         : {:.2}", stats.mean_iters());
        eprintln!("max |δ| observed   : {}", stats.max_abs_delta);
        eprintln!("modinv matches     : {}", stats.modinv_matches);
        eprintln!("modinv mismatches  : {}", stats.modinv_mismatches);
        eprintln!("=================================================");

        assert!(stats.all_converged, "some sample did not converge");
        assert_eq!(stats.modinv_mismatches, 0, "modular-inverse mismatches");
        assert!(stats.max_iters <= theoretical_bound,
                "observed max iters {} exceeds theoretical bound {}",
                stats.max_iters, theoretical_bound);
    }

    #[test]
    fn jumpdivstep_matrix_entry_survey() {
        // Stress-test the hidden constant in jumpdivstep: if matrix entries are
        // typically much smaller than 2^w, earlier pessimistic cost estimates
        // for reversible matrix-apply were too high.
        let samples = 100_000;
        for &w in &[4usize, 8, 12, 16] {
            let stats = jump_matrix_entry_survey(b"jumpdivstep-matrix-seed-v1", samples, w);
            let mean_log2 = if stats.nonzero_entries == 0 { 0.0 } else { stats.sum_log2_entry_abs / (stats.nonzero_entries as f64) };
            eprintln!("=== jumpdivstep matrix-entry survey (w={}) ===", w);
            eprintln!("samples                 : {}", stats.samples);
            eprintln!("max |entry| observed    : {}", stats.max_entry_abs);
            eprintln!("max log2 |entry|        : {:.3}", (stats.max_entry_abs as f64).log2());
            eprintln!("mean log2 |entry|       : {:.3}", mean_log2);
            eprintln!("theoretical max log2    : {}", w);
            eprintln!("===========================================");
            // Sanity: entries should never exceed 2^w in magnitude.
            assert!(stats.max_entry_abs <= (1i128 << w), "w={} entry {} exceeded 2^w", w, stats.max_entry_abs);
        }
    }
}
