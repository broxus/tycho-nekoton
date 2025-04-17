use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::sync::atomic::{AtomicI64, Ordering};

const MC_ACCEPTABLE_TIME_DIFF: u64 = 120;

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timings {
    pub last_mc_block_seqno: u32,
    pub last_mc_utime: u32,
    pub mc_time_diff: i64,
    pub smallest_known_lt: Option<u64>,
}

impl Timings {
    pub fn is_reliable(&self) -> bool {
        // just booted up
        if self == &Self::default() {
            return false;
        }

        self.mc_time_diff.unsigned_abs() < MC_ACCEPTABLE_TIME_DIFF
    }
}

impl PartialOrd for Timings {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timings {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.mc_time_diff.cmp(&other.mc_time_diff)
    }
}

pub trait Clock: Send + Sync {
    fn now_sec_u64(&self) -> u64;
    fn now_ms_f64(&self) -> f64;
    fn now_ms_u64(&self) -> u64;
}

#[derive(Copy, Clone, Debug)]
pub struct SimpleClock;

impl Clock for SimpleClock {
    #[inline]
    fn now_sec_u64(&self) -> u64 {
        now_sec_u64()
    }

    #[inline]
    fn now_ms_f64(&self) -> f64 {
        now_ms_f64()
    }

    #[inline]
    fn now_ms_u64(&self) -> u64 {
        now_ms_u64()
    }
}

#[derive(Default)]
pub struct ClockWithOffset {
    offset_as_sec: AtomicI64,
    offset_as_ms: AtomicI64,
}

impl ClockWithOffset {
    pub fn new(offset_ms: i64) -> Self {
        Self {
            offset_as_sec: AtomicI64::new(offset_ms / 1000),
            offset_as_ms: AtomicI64::new(offset_ms),
        }
    }

    pub fn update_offset(&self, offset_ms: i64) {
        self.offset_as_sec
            .store(offset_ms / 1000, Ordering::Release);
        self.offset_as_ms.store(offset_ms, Ordering::Release);
    }

    pub fn offset_ms(&self) -> i64 {
        self.offset_as_ms.load(Ordering::Acquire)
    }
}

impl Clock for ClockWithOffset {
    #[inline]
    fn now_sec_u64(&self) -> u64 {
        self.offset_as_sec
            .load(Ordering::Acquire)
            .saturating_add(now_sec_u64() as i64)
            .try_into()
            .unwrap_or_default()
    }

    #[inline]
    fn now_ms_f64(&self) -> f64 {
        self.offset_as_ms.load(Ordering::Acquire) as f64 + now_ms_f64()
    }

    #[inline]
    fn now_ms_u64(&self) -> u64 {
        self.offset_as_ms
            .load(Ordering::Acquire)
            .saturating_add(now_ms_u64() as i64)
            .try_into()
            .unwrap_or_default()
    }
}

#[derive(Copy, Clone, Default)]
pub struct ConstClock {
    time_as_sec: u64,
    time_as_ms: u64,
}

impl ConstClock {
    #[inline]
    pub const fn from_millis(millis: u64) -> Self {
        Self {
            time_as_sec: millis / 1000,
            time_as_ms: millis,
        }
    }

    #[inline]
    pub const fn from_secs(secs: u64) -> Self {
        Self {
            time_as_sec: secs,
            time_as_ms: secs * 1000,
        }
    }
}

impl Clock for ConstClock {
    #[inline]
    fn now_sec_u64(&self) -> u64 {
        self.time_as_sec
    }

    #[inline]
    fn now_ms_f64(&self) -> f64 {
        self.time_as_ms as f64
    }

    #[inline]
    fn now_ms_u64(&self) -> u64 {
        self.time_as_ms
    }
}

pub fn now_sec_u64() -> u64 {
    use crate::traits::TrustMe;
    use std::time::SystemTime;

    (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
        .trust_me()
        .as_secs()
}

pub fn now_ms_f64() -> f64 {
    use crate::traits::TrustMe;
    use std::time::SystemTime;

    (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
        .trust_me()
        .as_secs_f64()
        * 1000.0
}
pub fn now_ms_u64() -> u64 {
    use crate::traits::TrustMe;
    use std::time::SystemTime;

    let duration = (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)).trust_me();
    duration.as_secs() * 1000 + duration.subsec_millis() as u64
}
