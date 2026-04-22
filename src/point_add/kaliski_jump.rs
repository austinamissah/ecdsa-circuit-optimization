//! Classical analysis for a possible **hybrid Kaliski-jump** moonshot.
//!
//! Idea: keep the existing Kaliski cleanup machinery (`r,s,m_hist`) but try to
//! batch *local* `(u, v)` updates over a small fixed number of steps `t`, keyed
//! by the low `w` bits of `(u, v)`. If the resulting t-step transition matrices
//! come from a very small family per low-bit class, then a compressed QROM could
//! replace several expensive per-step parity/compare/cswap/sub/halve operations.
//!
//! This file is **classical-only** research infrastructure. It does not affect
//! the quantum circuit.
//!
//! Standard almost-inverse / binary-GCD step on nonnegative integers:
//!
//! ```text
//! if u even:                   (u, v) ← (u/2, v)
//! elif v even:                (u, v) ← (u, v/2)
//! elif u > v:                 (u, v) ← ((u-v)/2, v)
//! else:                       (u, v) ← (u, (v-u)/2)
//! ```
//!
//! Each branch can be represented as a linear map with a shared `1/2` factor:
//!
//! ```text
//! U-even:  (u', v') = (1/2) * [[1,  0], [0, 2]] * (u, v)
//! V-even:  (u', v') = (1/2) * [[2,  0], [0, 1]] * (u, v)
//! U>V:     (u', v') = (1/2) * [[1, -1], [0, 2]] * (u, v)
//! V>U:     (u', v') = (1/2) * [[2,  0], [-1,1]] * (u, v)
//! ```
//!
//! Over `t` steps, we can accumulate an integer matrix `P_t` such that:
//!
//! ```text
//! (u_t, v_t)^T = (1 / 2^t) * P_t * (u_0, v_0)^T
//! ```
//!
//! The research questions here are:
//! 1. How many distinct `P_t` appear along actual secp256k1 Kaliski trajectories?
//! 2. For a fixed low-bit class `(u mod 2^w, v mod 2^w)`, how many distinct
//!    `P_t` values occur? If this is very small, a compressed lookup might work.
//! 3. How big do the entries of `P_t` get in practice (vs. the trivial 2^t bound)?

use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::U256;
use sha3::digest::{ExtendableOutput, Update, XofReader};

use super::SECP256K1_P;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mat2 {
    pub a00: i128,
    pub a01: i128,
    pub a10: i128,
    pub a11: i128,
}

impl Mat2 {
    pub const ID: Mat2 = Mat2 { a00: 1, a01: 0, a10: 0, a11: 1 };

    pub fn mul(self, rhs: Mat2) -> Mat2 {
        Mat2 {
            a00: self.a00 * rhs.a00 + self.a01 * rhs.a10,
            a01: self.a00 * rhs.a01 + self.a01 * rhs.a11,
            a10: self.a10 * rhs.a00 + self.a11 * rhs.a10,
            a11: self.a10 * rhs.a01 + self.a11 * rhs.a11,
        }
    }

    pub fn max_abs(&self) -> i128 {
        [self.a00.abs(), self.a01.abs(), self.a10.abs(), self.a11.abs()]
            .into_iter()
            .max()
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KCase {
    UEven,
    VEven,
    UGtV,
    VGtU,
}

impl KCase {
    pub fn matrix(self) -> Mat2 {
        match self {
            KCase::UEven => Mat2 { a00: 1, a01: 0, a10: 0, a11: 2 },
            KCase::VEven => Mat2 { a00: 2, a01: 0, a10: 0, a11: 1 },
            KCase::UGtV  => Mat2 { a00: 1, a01: -1, a10: 0, a11: 2 },
            KCase::VGtU  => Mat2 { a00: 2, a01: 0, a10: -1, a11: 1 },
        }
    }
}

#[inline(always)]
fn kaliski_case(u: U256, v: U256) -> KCase {
    if !u.bit(0) {
        KCase::UEven
    } else if !v.bit(0) {
        KCase::VEven
    } else if u > v {
        KCase::UGtV
    } else {
        KCase::VGtU
    }
}

#[inline(always)]
fn kaliski_step_uv(u: U256, v: U256) -> (U256, U256, KCase) {
    match kaliski_case(u, v) {
        KCase::UEven => (u >> 1, v, KCase::UEven),
        KCase::VEven => (u, v >> 1, KCase::VEven),
        KCase::UGtV  => ((u.wrapping_sub(v)) >> 1, v, KCase::UGtV),
        KCase::VGtU  => (u, (v.wrapping_sub(u)) >> 1, KCase::VGtU),
    }
}

#[derive(Clone, Debug)]
pub struct WindowObs {
    pub low_u: u16,
    pub low_v: u16,
    pub mat: Mat2,
    pub cases: Vec<KCase>,
}

/// Observe a t-step Kaliski window starting from full-width `(u, v)`.
/// Returns `(u_t, v_t, obs)`.
pub fn observe_window(mut u: U256, mut v: U256, w: usize, t: usize) -> (U256, U256, WindowObs) {
    assert!(w <= 16, "low-bit class currently stored as u16");
    let low_mask = if w == 16 {
        U256::from(0xFFFFu64)
    } else {
        (U256::from(1u64) << w).wrapping_sub(U256::from(1u64))
    };
    let low_u = (u & low_mask).to::<u16>();
    let low_v = (v & low_mask).to::<u16>();
    let mut mat = Mat2::ID;
    let mut cases = Vec::with_capacity(t);
    for _ in 0..t {
        if v.is_zero() { break; }
        let (nu, nv, kc) = kaliski_step_uv(u, v);
        mat = kc.matrix().mul(mat);
        cases.push(kc);
        u = nu;
        v = nv;
    }
    (u, v, WindowObs { low_u, low_v, mat, cases })
}

pub struct Sampler {
    reader: Box<dyn XofReader>,
    p: U256,
}

impl Sampler {
    pub fn new(seed: &[u8], p: U256) -> Self {
        let mut hasher = sha3::Shake128::default();
        hasher.update(seed);
        Self { reader: Box::new(hasher.finalize_xof()), p }
    }

    pub fn next(&mut self) -> U256 {
        loop {
            let mut buf = [0u8; 32];
            self.reader.read(&mut buf);
            let x = U256::from_le_slice(&buf);
            if x < self.p && !x.is_zero() {
                return x;
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct HybridStats {
    pub inputs: usize,
    pub windows: usize,
    pub distinct_global_mats: usize,
    pub max_entry_abs: i128,
    pub mean_log2_entry_abs: f64,
    pub low_classes_seen: usize,
    pub mean_mats_per_class: f64,
    pub max_mats_per_class: usize,
    pub singleton_classes: usize,
    pub most_common_class_count: usize,
    pub most_common_class: Option<(u16, u16)>,
}

/// Sample actual secp256k1 Kaliski trajectories and measure the compressibility
/// of t-step local transition matrices keyed by low-w bits.
pub fn hybrid_kaliski_window_survey(
    seed: &[u8],
    n_inputs: usize,
    w: usize,
    t: usize,
) -> HybridStats {
    let mut sampler = Sampler::new(seed, SECP256K1_P);
    let mut global_mats: BTreeSet<Mat2> = BTreeSet::new();
    let mut by_class: BTreeMap<(u16, u16), BTreeSet<Mat2>> = BTreeMap::new();
    let mut windows = 0usize;
    let mut max_entry_abs = 0i128;
    let mut sum_log2_entry_abs = 0.0f64;
    let mut counted_mats = 0usize;

    for _ in 0..n_inputs {
        let mut u = SECP256K1_P;
        let mut v = sampler.next();
        // Use the same deterministic iteration budget as the BY survey.
        for _ in 0..742 {
            if v.is_zero() { break; }
            let (nu, nv, obs) = observe_window(u, v, w, t);
            global_mats.insert(obs.mat);
            by_class.entry((obs.low_u, obs.low_v)).or_default().insert(obs.mat);
            let abs = obs.mat.max_abs();
            if abs > max_entry_abs { max_entry_abs = abs; }
            if abs > 0 {
                sum_log2_entry_abs += (abs as f64).log2();
                counted_mats += 1;
            }
            windows += 1;
            // Advance ONE step only; windows overlap. This matches the eventual
            // use-case: at runtime we would choose whether to batch starting at
            // every cycle boundary.
            let (u1, v1, _kc) = kaliski_step_uv(u, v);
            u = u1;
            v = v1;
        }
    }

    let low_classes_seen = by_class.len();
    let mut total_mats_per_class = 0usize;
    let mut max_mats_per_class = 0usize;
    let mut singleton_classes = 0usize;
    let mut most_common_class_count = 0usize;
    let mut most_common_class = None;
    for (cls, mats) in &by_class {
        let c = mats.len();
        total_mats_per_class += c;
        if c > max_mats_per_class { max_mats_per_class = c; }
        if c == 1 { singleton_classes += 1; }
        if c > most_common_class_count {
            most_common_class_count = c;
            most_common_class = Some(*cls);
        }
    }

    HybridStats {
        inputs: n_inputs,
        windows,
        distinct_global_mats: global_mats.len(),
        max_entry_abs,
        mean_log2_entry_abs: if counted_mats == 0 { 0.0 } else { sum_log2_entry_abs / counted_mats as f64 },
        low_classes_seen,
        mean_mats_per_class: if low_classes_seen == 0 { 0.0 } else { total_mats_per_class as f64 / low_classes_seen as f64 },
        max_mats_per_class,
        singleton_classes,
        most_common_class_count,
        most_common_class,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_window_smoke() {
        let u = SECP256K1_P;
        let v = U256::from(123456789u64);
        let (_u2, _v2, obs) = observe_window(u, v, 8, 4);
        assert!(obs.cases.len() >= 1);
        assert!(obs.mat.max_abs() >= 1);
    }

    #[test]
    fn hybrid_kaliski_window_survey_test() {
        for &(w, t) in &[(6usize, 4usize), (8usize, 4usize), (8usize, 6usize)] {
            let s = hybrid_kaliski_window_survey(b"hybrid-kaliski-window-seed-v1", 10_000, w, t);
            eprintln!("=== hybrid Kaliski window survey (w={}, t={}) ===", w, t);
            eprintln!("inputs               : {}", s.inputs);
            eprintln!("windows              : {}", s.windows);
            eprintln!("distinct global mats : {}", s.distinct_global_mats);
            eprintln!("max |entry|          : {}", s.max_entry_abs);
            eprintln!("mean log2 |entry|    : {:.3}", s.mean_log2_entry_abs);
            eprintln!("classes seen         : {}", s.low_classes_seen);
            eprintln!("mean mats/class      : {:.3}", s.mean_mats_per_class);
            eprintln!("max mats/class       : {}", s.max_mats_per_class);
            eprintln!("singleton classes    : {}", s.singleton_classes);
            eprintln!("most common class ct : {}", s.most_common_class_count);
            if let Some((ucls, vcls)) = s.most_common_class {
                eprintln!("most common class    : (u_low={}, v_low={})", ucls, vcls);
            }
            eprintln!("===============================================");
            assert!(s.windows > 0);
            assert!(s.distinct_global_mats >= 1);
        }
    }
}
