#!/usr/bin/env python3
"""Add TLM_SCHED_J2_DELTA: widen the gcd register-width schedule by a constant.

`SCHED_J2[i]` is how many bits of `u` survive divstep i -- gcd.rs:1230 pops and
frees everything above it -- so the schedule is a deliberate truncation, and
memory/02-lambda.md prices "SCHED_J2 drops a nonzero bit, walk still terminates"
at 2.80 mismatches per 9,024. memory/05-qubit-reduction.md measured the lever in
the other direction: narrowing 160 tail entries bought -0.49% score for +3.6
lambda. Widening should run that trade backwards.

Both vectors move in lockstep. 05-qubit-reduction.md step 5: the divstep error
depends only on `s = SCHED_J2[i] - cmp_window(i)`, and moving one without the
other takes that channel from 8.36 to 4,646 mismatches. Adding the same delta to
both preserves `s` exactly: `cmp_window` is `min(gap_j2(i), current_n)` with
`current_n = sched_j2(i)` and `GAP_J2[i] = SCHED_J2[i] + 1` over the tail, so
`min(sched+1+d, sched+d) = sched+d = current_n` for every d.

A constant delta also keeps both schedules non-increasing in i, which the pop
loop requires, and the clamp is the allocated width of `u`. Delta 0 is the
identity.

Usage: python3 patch_schedj2_delta.py <path-to-repo-root>
"""
import sys, pathlib

root = pathlib.Path(sys.argv[1])
p = root / 'src/point_add/trailmix_ludicrous/schedule.rs'
b = p.read_bytes()
nl = b'\r\n' if b.count(b'\r\n') else b'\n'


def sub(old, new):
    global b
    o = nl.join(l.encode() for l in old.split('\n'))
    n = nl.join(l.encode() for l in new.split('\n'))
    assert b.count(o) == 1, f"anchor not unique: {old[:60]!r}"
    b = b.replace(o, n)


sub('''pub fn sched_j2(i: usize) -> u16 {
    SCHED_J2_BASE[i.min(SCHED_J2_BASE.len() - 1)]
}''',
    '''pub fn sched_j2(i: usize) -> u16 {
    SCHED_J2_BASE[i.min(SCHED_J2_BASE.len() - 1)]
        .saturating_add(sched_j2_delta())
        .min(256)
}

/// `TLM_SCHED_J2_DELTA` widens BOTH divstep-width schedules by a constant
/// number of bits.
///
/// `SCHED_J2[i]` is how many bits of `u` survive divstep i (gcd.rs:1230 pops and
/// frees everything above it), so the schedule is a truncation:
/// memory/02-lambda.md prices "SCHED_J2 drops a nonzero bit, walk still
/// terminates" at 2.80 mismatches per 9,024, and memory/05-qubit-reduction.md
/// measured the lever in the other direction -- narrowing 160 tail entries
/// bought -0.49% score for +3.6 lambda.
///
/// `GAP_J2` moves with it, and must: per 05-qubit-reduction.md step 5 the
/// divstep error depends only on `s = SCHED_J2[i] - cmp_window(i)`, and moving
/// one without the other takes that channel from 8.36 to 4,646 mismatches.
/// Adding the same delta to both preserves `s`, since `cmp_window` is
/// `min(gap_j2(i), current_n)` with `current_n = sched_j2(i)`.
///
/// The clamp is the allocated width of `u`; a constant delta keeps both
/// schedules non-increasing in i, which the pop loop requires. Delta 0 is the
/// identity.
fn sched_j2_delta() -> u16 {
    std::env::var("TLM_SCHED_J2_DELTA")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(0)
}''')

sub('''pub fn gap_j2(i: usize) -> u16 {
    GAP_J2_BASE[i.min(GAP_J2_BASE.len() - 1)]
}''',
    '''pub fn gap_j2(i: usize) -> u16 {
    GAP_J2_BASE[i.min(GAP_J2_BASE.len() - 1)]
        .saturating_add(sched_j2_delta())
        .min(256)
}''')

p.write_bytes(b)
print(f"patched {p}")
