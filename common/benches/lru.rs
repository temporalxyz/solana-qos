use std::num::NonZeroUsize;

use criterion::{
    criterion_group, criterion_main, Criterion, Throughput,
};
use lru::LruCache;
use nohash_hasher::BuildNoHashHasher;
use solana_qos_common::partial_meta::QoSPartialMeta;
use xxhash_rust::xxh3::xxh3_64;

const CAPACITY: usize = 1024 * 1024;

const VALUE_SIZE: usize = core::mem::size_of::<QoSPartialMeta>();

fn lru_crate(c: &mut Criterion) {
    let mut lru_cache = LruCache::<
        u64,
        [u8; VALUE_SIZE],
        BuildNoHashHasher<u64>,
    >::with_hasher(
        NonZeroUsize::new(CAPACITY).unwrap(),
        BuildNoHashHasher::<u64>::default(),
    );

    // Fill cache
    for i in 0..CAPACITY {
        lru_cache.push(xxh3_64(&i.to_le_bytes()), [0; VALUE_SIZE]);
    }

    let mut g = c.benchmark_group("cache");
    // Measure throughput in max packet data (1264 bytes) per second
    g.throughput(Throughput::Bytes(1264));

    let mut i = CAPACITY;
    g.bench_function(
        "lru-crate",
        #[inline(always)]
        |b| {
            b.iter(
                #[inline(always)]
                || {
                    lru_cache.put(
                        xxh3_64(&i.to_le_bytes()),
                        [0; VALUE_SIZE],
                    );
                    i += 1;
                },
            )
        },
    );
}

#[inline(never)]
fn custom(c: &mut Criterion) {
    let mut lru_cache = qos_lru::LRUCache::<
        u64,
        [u8; VALUE_SIZE],
        CAPACITY,
    >::new_boxed();

    // Fill the cache
    let mut i = 0_u64;
    while lru_cache
        .put(xxh3_64(&i.to_le_bytes()), [0; VALUE_SIZE])
        .0
        .is_none()
    {
        i += 1;
    }

    let mut g = c.benchmark_group("cache");
    // Measure throughput in max packet data (1264 bytes) per second
    g.throughput(Throughput::Bytes(1264));

    g.bench_function(
        "lru",
        // #[inline(always)]
        |b| {
            b.iter(
                // #[inline(always)]
                || {
                    lru_cache.put(
                        xxh3_64(&i.to_le_bytes()),
                        [0; VALUE_SIZE],
                    );
                    i += 1;
                },
            )
        },
    );

    g.finish();
}

criterion_group!(lru, lru_crate, custom);
criterion_main!(lru);
