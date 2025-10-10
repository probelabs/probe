use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};

static EID001_ABS_PATH: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
static EID002_UID_PATH_MISMATCH: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
static EID003_MALFORMED_UID: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
static EID004_ZERO_LINE: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
static EID009_NON_RELATIVE_FILE: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
static EID010_SELF_LOOP: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
static EID011_ORPHAN_SOURCE: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
static EID012_ORPHAN_TARGET: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
static EID013_LINE_MISMATCH: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));

pub fn inc(code: &str) {
    match code {
        "EID001" => {
            EID001_ABS_PATH.fetch_add(1, Ordering::Relaxed);
        }
        "EID002" => {
            EID002_UID_PATH_MISMATCH.fetch_add(1, Ordering::Relaxed);
        }
        "EID003" => {
            EID003_MALFORMED_UID.fetch_add(1, Ordering::Relaxed);
        }
        "EID004" => {
            EID004_ZERO_LINE.fetch_add(1, Ordering::Relaxed);
        }
        "EID009" => {
            EID009_NON_RELATIVE_FILE.fetch_add(1, Ordering::Relaxed);
        }
        "EID010" => {
            EID010_SELF_LOOP.fetch_add(1, Ordering::Relaxed);
        }
        "EID011" => {
            EID011_ORPHAN_SOURCE.fetch_add(1, Ordering::Relaxed);
        }
        "EID012" => {
            EID012_ORPHAN_TARGET.fetch_add(1, Ordering::Relaxed);
        }
        "EID013" => {
            EID013_LINE_MISMATCH.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }
}

pub fn snapshot() -> crate::protocol::EdgeAuditInfo {
    crate::protocol::EdgeAuditInfo {
        eid001_abs_path: EID001_ABS_PATH.load(Ordering::Relaxed),
        eid002_uid_path_mismatch: EID002_UID_PATH_MISMATCH.load(Ordering::Relaxed),
        eid003_malformed_uid: EID003_MALFORMED_UID.load(Ordering::Relaxed),
        eid004_zero_line: EID004_ZERO_LINE.load(Ordering::Relaxed),
        eid009_non_relative_file_path: EID009_NON_RELATIVE_FILE.load(Ordering::Relaxed),
        eid010_self_loop: EID010_SELF_LOOP.load(Ordering::Relaxed),
        eid011_orphan_source: EID011_ORPHAN_SOURCE.load(Ordering::Relaxed),
        eid012_orphan_target: EID012_ORPHAN_TARGET.load(Ordering::Relaxed),
        eid013_line_mismatch: EID013_LINE_MISMATCH.load(Ordering::Relaxed),
    }
}

#[allow(dead_code)]
pub fn clear() {
    EID001_ABS_PATH.store(0, Ordering::Relaxed);
    EID002_UID_PATH_MISMATCH.store(0, Ordering::Relaxed);
    EID003_MALFORMED_UID.store(0, Ordering::Relaxed);
    EID004_ZERO_LINE.store(0, Ordering::Relaxed);
    EID009_NON_RELATIVE_FILE.store(0, Ordering::Relaxed);
    EID010_SELF_LOOP.store(0, Ordering::Relaxed);
    EID011_ORPHAN_SOURCE.store(0, Ordering::Relaxed);
    EID012_ORPHAN_TARGET.store(0, Ordering::Relaxed);
    EID013_LINE_MISMATCH.store(0, Ordering::Relaxed);
}
