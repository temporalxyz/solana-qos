//! The locks used in here are not robust and are only used for demonstrative purposes.
//! Do not reuse!

use std::sync::atomic::{AtomicUsize, Ordering};

#[repr(C, align(128))]
pub struct SharedStats {
    pub lock: Lock,
    pub stats: Stats,
    pub should_reset: u64,
}

impl SharedStats {
    pub unsafe fn reset(shared_stats: *mut u8) {
        let shared_stats: *mut SharedStats = shared_stats.cast();
        (*shared_stats).lock.take();
        (*shared_stats).should_reset = 1;
        (*shared_stats).stats = Stats::default();
        (*shared_stats).lock.release();
    }

    pub unsafe fn read(shared_stats: *mut u8) -> Stats {
        let shared_stats: *mut SharedStats = shared_stats.cast();
        (*shared_stats).lock.take();
        let stats = (*shared_stats).stats.clone();
        (*shared_stats).lock.release();
        stats
    }

    pub unsafe fn update(shared_stats: *mut u8, stats: &Stats) -> bool {
        let shared_stats: *mut SharedStats = shared_stats.cast();
        (*shared_stats).lock.take();
        let should_reset = (*shared_stats).should_reset;
        if should_reset == 0 {
            (*shared_stats).stats = stats.clone();
        }
        (*shared_stats).should_reset = 0;
        (*shared_stats).lock.release();
        should_reset != 0
    }
}

#[derive(Default, Debug, Clone)]
#[repr(C, align(128))]
pub struct Stats {
    pub total_packets: usize,
    pub non_ipv4: usize,
    pub non_transaction_packet: usize,
    pub recently_processed: usize,
    pub recently_processed_queued: usize,
    pub recent_signatures_received: usize,
    pub invalid_meta_size: usize,
    pub failed_sanitize: usize,
    pub failed_view: usize,
    pub invalid_packet_data: usize,
    pub leaked_priority: usize,
    pub duplicate_packets: usize,
    pub banking_transmissions: usize,
    pub zero_score: usize,
    pub completed: usize,
}

impl Stats {
    pub fn new() -> Stats {
        Default::default()
    }
}

#[repr(C, align(128))]
pub struct Lock {
    lock: AtomicUsize,
}

impl Lock {
    fn take(&self) {
        while self
            .lock
            .compare_exchange(
                0,
                1,
                Ordering::Release,
                Ordering::Acquire,
            )
            .is_err()
        {}
    }

    fn release(&self) {
        while self
            .lock
            .compare_exchange(
                1,
                0,
                Ordering::Release,
                Ordering::Acquire,
            )
            .is_err()
        {}
    }
}

pub struct EngineStats {
    pub tpu_sends: AtomicUsize,
    pub fwd_sends: AtomicUsize,
    pub sigverify_recvs: AtomicUsize,
    pub sigverify_sends: AtomicUsize,
    pub scheduler_sends: AtomicUsize,
}

impl EngineStats {
    pub fn load(&self) -> [usize; 5] {
        let EngineStats {
            tpu_sends,
            fwd_sends,
            sigverify_recvs,
            sigverify_sends,
            scheduler_sends,
        } = self;

        [
            tpu_sends.load(Ordering::Relaxed),
            fwd_sends.load(Ordering::Relaxed),
            sigverify_recvs.load(Ordering::Relaxed),
            sigverify_sends.load(Ordering::Relaxed),
            scheduler_sends.load(Ordering::Relaxed),
        ]
    }
}
