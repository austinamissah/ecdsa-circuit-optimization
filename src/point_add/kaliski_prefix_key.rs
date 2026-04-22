//! Exact key study for the 3-step bulk hybrid Kaliski core.
//!
//! After the 4-step window decomposition, the leading prototype target became:
//!
//!   exact 3-step bulk core + 1 ordinary residual step + tiny tail fallback.
//!
//! The next question is selector size:
//!
//! > How many low bits are actually needed, together with `cmp0, cmp1, cmp2`,
//! > to identify the exact 3-step prefix on full 4-step windows?
//!
//! The strong answer is:
//! - on actual secp256k1 trajectories, **3 low bits per side already suffice**;
//! - and on exhaustive small-integer surveys of full windows, the same
//!   `w = 3` key is already exact.
//!
//! So the exact 3-step bulk primitive appears to need only a **9-bit key**:
//!
//!   `(u mod 8, v mod 8, cmp0, cmp1, cmp2)`
//!
//! rather than the previously feared `(u mod 256, v mod 256, cmp0, cmp1, cmp2)`.

use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::U256;

use super::SECP256K1_P;
use super::kaliski_jump::{kaliski_step_uv, KCase, Sampler};
use super::test_timeout::{check_deadline, two_min_deadline};

fn encode_prefix3(cases: &[KCase]) -> Option<u8> {
    if cases.len() < 3 { return None; }
    let mut out = 0u8;
    for i in 0..3 {
        let bits = match cases[i] {
            KCase::UEven => 0u8,
            KCase::VEven => 1u8,
            KCase::UGtV  => 2u8,
            KCase::VGtU  => 3u8,
        };
        out |= bits << (2 * i);
    }
    Some(out)
}

fn prefix3_string(code: u8) -> String {
    let mut s = String::new();
    for i in 0..3 {
        if i > 0 { s.push('-'); }
        s.push_str(match (code >> (2 * i)) & 0b11 {
            0 => "UE",
            1 => "VE",
            2 => "UG",
            3 => "VG",
            _ => "??",
        });
    }
    s
}

#[derive(Debug, Clone)]
pub struct WidthStats {
    pub w: usize,
    pub key_classes: usize,
    pub mean_prefixes_per_key: f64,
    pub max_prefixes_per_key: usize,
    pub ambiguous_keys: usize,
    pub singleton_keys: usize,
}

#[derive(Debug, Clone)]
pub struct PrefixKeySummary {
    pub full_windows: usize,
    pub distinct_prefixes: usize,
    pub exact_width: Option<usize>,
    pub widths: Vec<WidthStats>,
}

#[derive(Debug, Clone)]
pub struct CompareDerivabilityStats {
    pub full_windows: usize,
    pub base_classes: usize,
    pub mean_cmp12_pairs_per_base: f64,
    pub max_cmp12_pairs_per_base: usize,
    pub ambiguous_base_classes: usize,
    pub singleton_base_classes: usize,
}

fn summarize_width_map<K: Ord>(w: usize, map: &BTreeMap<K, BTreeSet<u8>>) -> WidthStats {
    let key_classes = map.len();
    let mut total = 0usize;
    let mut maxc = 0usize;
    let mut ambiguous = 0usize;
    let mut singletons = 0usize;
    for seqs in map.values() {
        let c = seqs.len();
        total += c;
        maxc = maxc.max(c);
        if c > 1 { ambiguous += 1; }
        if c == 1 { singletons += 1; }
    }
    WidthStats {
        w,
        key_classes,
        mean_prefixes_per_key: total as f64 / key_classes as f64,
        max_prefixes_per_key: maxc,
        ambiguous_keys: ambiguous,
        singleton_keys: singletons,
    }
}

pub fn secp_full_prefix_key_survey(seed: &[u8], n_inputs: usize, max_w: usize) -> PrefixKeySummary {
    let deadline = two_min_deadline();
    let mut sampler = Sampler::new(seed, SECP256K1_P);
    let mut maps: Vec<BTreeMap<(u16, u16, u8, u8, u8), BTreeSet<u8>>> =
        (1..=max_w).map(|_| BTreeMap::new()).collect();
    let mut distinct_prefixes = BTreeSet::new();
    let mut full_windows = 0usize;

    for input_idx in 0..n_inputs {
        if (input_idx & 31) == 0 { check_deadline(deadline, "kaliski_prefix_key::secp_full_prefix_key_survey"); }
        let mut u = SECP256K1_P;
        let mut v = sampler.next();
        for _ in 0..742 {
            if v.is_zero() { break; }
            let mut cases = Vec::with_capacity(4);
            let mut uu = u;
            let mut vv = v;
            for _ in 0..4 {
                if vv.is_zero() { break; }
                let (nu, nv, kc) = kaliski_step_uv(uu, vv);
                cases.push(kc);
                uu = nu;
                vv = nv;
            }
            if let Some(prefix) = encode_prefix3(&cases) {
                if cases.len() == 4 {
                    distinct_prefixes.insert(prefix);
                    let (u1, v1, _k1) = kaliski_step_uv(u, v);
                    let (u2, v2, _k2) = kaliski_step_uv(u1, v1);
                    let cmp0 = (u > v) as u8;
                    let cmp1 = (u1 > v1) as u8;
                    let cmp2 = (u2 > v2) as u8;
                    for w in 1..=max_w {
                        let mask = (U256::from(1u64) << w).wrapping_sub(U256::from(1u64));
                        let key = (
                            (u & mask).to::<u16>(),
                            (v & mask).to::<u16>(),
                            cmp0,
                            cmp1,
                            cmp2,
                        );
                        maps[w - 1].entry(key).or_default().insert(prefix);
                    }
                    full_windows += 1;
                }
            }
            let (u1, v1, _kc) = kaliski_step_uv(u, v);
            u = u1;
            v = v1;
        }
    }

    let widths = maps.into_iter().enumerate()
        .map(|(i, map)| summarize_width_map(i + 1, &map))
        .collect::<Vec<_>>();
    let exact_width = widths.iter().find(|s| s.max_prefixes_per_key == 1).map(|s| s.w);
    PrefixKeySummary {
        full_windows,
        distinct_prefixes: distinct_prefixes.len(),
        exact_width,
        widths,
    }
}

fn summarize_cmp12_map<K: Ord>(full_windows: usize, map: &BTreeMap<K, BTreeSet<(u8, u8)>>) -> CompareDerivabilityStats {
    let base_classes = map.len();
    let mut total = 0usize;
    let mut maxc = 0usize;
    let mut ambiguous = 0usize;
    let mut singletons = 0usize;
    for vals in map.values() {
        let c = vals.len();
        total += c;
        maxc = maxc.max(c);
        if c > 1 { ambiguous += 1; }
        if c == 1 { singletons += 1; }
    }
    CompareDerivabilityStats {
        full_windows,
        base_classes,
        mean_cmp12_pairs_per_base: total as f64 / base_classes as f64,
        max_cmp12_pairs_per_base: maxc,
        ambiguous_base_classes: ambiguous,
        singleton_base_classes: singletons,
    }
}

pub fn secp_cmp12_derivability_survey(seed: &[u8], n_inputs: usize, w: usize) -> CompareDerivabilityStats {
    let deadline = two_min_deadline();
    let mut sampler = Sampler::new(seed, SECP256K1_P);
    let mask = (U256::from(1u64) << w).wrapping_sub(U256::from(1u64));
    let mut map: BTreeMap<(u16, u16, u8), BTreeSet<(u8, u8)>> = BTreeMap::new();
    let mut full_windows = 0usize;

    for input_idx in 0..n_inputs {
        if (input_idx & 31) == 0 { check_deadline(deadline, "kaliski_prefix_key::secp_cmp12_derivability_survey"); }
        let mut u = SECP256K1_P;
        let mut v = sampler.next();
        for _ in 0..742 {
            if v.is_zero() { break; }
            let (u1, v1, _k1) = kaliski_step_uv(u, v);
            if v1.is_zero() {
                u = u1;
                v = v1;
                continue;
            }
            let (u2, v2, _k2) = kaliski_step_uv(u1, v1);
            if v2.is_zero() {
                u = u1;
                v = v1;
                continue;
            }
            let (_u3, v3, _k3) = kaliski_step_uv(u2, v2);
            if v3.is_zero() {
                u = u1;
                v = v1;
                continue;
            }
            let key = ((u & mask).to::<u16>(), (v & mask).to::<u16>(), (u > v) as u8);
            let val = ((u1 > v1) as u8, (u2 > v2) as u8);
            map.entry(key).or_default().insert(val);
            full_windows += 1;
            u = u1;
            v = v1;
        }
    }

    summarize_cmp12_map(full_windows, &map)
}

pub fn generic_cmp12_derivability_survey(limit: usize, w: usize) -> CompareDerivabilityStats {
    let deadline = two_min_deadline();
    let mask = (U256::from(1u64) << w).wrapping_sub(U256::from(1u64));
    let mut map: BTreeMap<(u16, u16, u8), BTreeSet<(u8, u8)>> = BTreeMap::new();
    let mut full_windows = 0usize;

    for u in 1..=limit {
        if (u & 31) == 0 { check_deadline(deadline, "kaliski_prefix_key::generic_cmp12_derivability_survey"); }
        for v in 1..=limit {
            let u0 = U256::from(u as u64);
            let v0 = U256::from(v as u64);
            let (u1, v1, _k1) = kaliski_step_uv(u0, v0);
            if v1.is_zero() { continue; }
            let (u2, v2, _k2) = kaliski_step_uv(u1, v1);
            if v2.is_zero() { continue; }
            let (_u3, v3, _k3) = kaliski_step_uv(u2, v2);
            if v3.is_zero() { continue; }
            let key = ((u0 & mask).to::<u16>(), (v0 & mask).to::<u16>(), (u0 > v0) as u8);
            let val = ((u1 > v1) as u8, (u2 > v2) as u8);
            map.entry(key).or_default().insert(val);
            full_windows += 1;
        }
    }

    summarize_cmp12_map(full_windows, &map)
}

pub fn generic_full_prefix_key_survey(limit: usize, max_w: usize) -> PrefixKeySummary {
    let deadline = two_min_deadline();
    let mut maps: Vec<BTreeMap<(u16, u16, u8, u8, u8), BTreeSet<u8>>> =
        (1..=max_w).map(|_| BTreeMap::new()).collect();
    let mut distinct_prefixes = BTreeSet::new();
    let mut full_windows = 0usize;

    for u in 1..=limit {
        if (u & 31) == 0 { check_deadline(deadline, "kaliski_prefix_key::generic_full_prefix_key_survey"); }
        for v in 1..=limit {
            let mut uu = U256::from(u as u64);
            let mut vv = U256::from(v as u64);
            let mut cases = Vec::with_capacity(4);
            let start_u = uu;
            let start_v = vv;
            let mut states = Vec::with_capacity(3);
            for _ in 0..4 {
                if vv.is_zero() { break; }
                states.push((uu, vv));
                let (nu, nv, kc) = kaliski_step_uv(uu, vv);
                cases.push(kc);
                uu = nu;
                vv = nv;
            }
            if cases.len() == 4 {
                let prefix = encode_prefix3(&cases).unwrap();
                distinct_prefixes.insert(prefix);
                let (u0, v0) = states[0];
                let (u1, v1) = states[1];
                let (u2, v2) = states[2];
                let cmp0 = (u0 > v0) as u8;
                let cmp1 = (u1 > v1) as u8;
                let cmp2 = (u2 > v2) as u8;
                for w in 1..=max_w {
                    let mask = (U256::from(1u64) << w).wrapping_sub(U256::from(1u64));
                    let key = (
                        (start_u & mask).to::<u16>(),
                        (start_v & mask).to::<u16>(),
                        cmp0,
                        cmp1,
                        cmp2,
                    );
                    maps[w - 1].entry(key).or_default().insert(prefix);
                }
                full_windows += 1;
            }
        }
    }

    let widths = maps.into_iter().enumerate()
        .map(|(i, map)| summarize_width_map(i + 1, &map))
        .collect::<Vec<_>>();
    let exact_width = widths.iter().find(|s| s.max_prefixes_per_key == 1).map(|s| s.w);
    PrefixKeySummary {
        full_windows,
        distinct_prefixes: distinct_prefixes.len(),
        exact_width,
        widths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_full_prefix_key_exactness_test() {
        let s = generic_full_prefix_key_survey(512, 6);
        eprintln!("=== Generic full-window 3-step prefix key survey (limit=512) ===");
        eprintln!("full windows       : {}", s.full_windows);
        eprintln!("distinct prefixes  : {}", s.distinct_prefixes);
        eprintln!("exact width        : {:?}", s.exact_width);
        for row in &s.widths {
            eprintln!(
                "w={} keys={} mean={:.3} max={} ambiguous={} singleton={}",
                row.w,
                row.key_classes,
                row.mean_prefixes_per_key,
                row.max_prefixes_per_key,
                row.ambiguous_keys,
                row.singleton_keys,
            );
        }
        eprintln!("===============================================================");
        assert_eq!(s.distinct_prefixes, 40);
        assert_eq!(s.exact_width, Some(3));
        assert!(s.widths[1].max_prefixes_per_key > 1); // w=2 not enough
        assert_eq!(s.widths[2].key_classes, 312);      // w=3 exact generic key space
    }

    #[test]
    fn secp_full_prefix_key_exactness_test() {
        let s = secp_full_prefix_key_survey(b"kaliski-prefix-key-seed-v1", 10_000, 6);
        eprintln!("=== secp256k1 full-window 3-step prefix key survey ===");
        eprintln!("full windows       : {}", s.full_windows);
        eprintln!("distinct prefixes  : {}", s.distinct_prefixes);
        eprintln!("exact width        : {:?}", s.exact_width);
        for row in &s.widths {
            eprintln!(
                "w={} keys={} mean={:.3} max={} ambiguous={} singleton={}",
                row.w,
                row.key_classes,
                row.mean_prefixes_per_key,
                row.max_prefixes_per_key,
                row.ambiguous_keys,
                row.singleton_keys,
            );
        }
        eprintln!("example prefixes   : {} / {} / {}",
            prefix3_string(0b00_00_00),
            prefix3_string(0b10_00_10),
            prefix3_string(0b11_11_01),
        );
        eprintln!("======================================================");
        assert!(s.full_windows > 3_500_000);
        assert_eq!(s.distinct_prefixes, 36);
        assert_eq!(s.exact_width, Some(3));
        assert!(s.widths[1].max_prefixes_per_key > 1); // w=2 not enough on secp either
        assert_eq!(s.widths[2].max_prefixes_per_key, 1);
    }

    #[test]
    fn generic_cmp12_derivability_test() {
        let s = generic_cmp12_derivability_survey(512, 3);
        eprintln!("=== Generic cmp1/cmp2 derivability from (u_low3,v_low3,cmp0) ===");
        eprintln!("full windows        : {}", s.full_windows);
        eprintln!("base classes        : {}", s.base_classes);
        eprintln!("mean cmp12/base     : {:.3}", s.mean_cmp12_pairs_per_base);
        eprintln!("max cmp12/base      : {}", s.max_cmp12_pairs_per_base);
        eprintln!("ambiguous bases     : {}", s.ambiguous_base_classes);
        eprintln!("singleton bases     : {}", s.singleton_base_classes);
        eprintln!("===============================================================");
        assert!(s.max_cmp12_pairs_per_base > 1);
        assert!(s.ambiguous_base_classes > 0);
    }

    #[test]
    fn secp_cmp12_derivability_test() {
        let s = secp_cmp12_derivability_survey(b"kaliski-prefix-key-seed-v1", 10_000, 3);
        eprintln!("=== secp cmp1/cmp2 derivability from (u_low3,v_low3,cmp0) ===");
        eprintln!("full windows        : {}", s.full_windows);
        eprintln!("base classes        : {}", s.base_classes);
        eprintln!("mean cmp12/base     : {:.3}", s.mean_cmp12_pairs_per_base);
        eprintln!("max cmp12/base      : {}", s.max_cmp12_pairs_per_base);
        eprintln!("ambiguous bases     : {}", s.ambiguous_base_classes);
        eprintln!("singleton bases     : {}", s.singleton_base_classes);
        eprintln!("============================================================");
        assert!(s.max_cmp12_pairs_per_base > 1);
        assert!(s.ambiguous_base_classes > 0);
    }
}
