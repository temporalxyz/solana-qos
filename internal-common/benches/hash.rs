use std::hash::{Hash, Hasher};

use criterion::{
    criterion_group, criterion_main, Criterion, Throughput,
};

fn ahash(c: &mut Criterion) {
    let msg = [0; 1264];

    let mut g = c.benchmark_group("hash");
    g.throughput(Throughput::Bytes(1264));

    g.bench_function("ahash", |b| {
        b.iter(|| {
            let mut hasher = ahash::AHasher::default();
            msg.hash(&mut hasher);
            hasher.finish()
        })
    });
}

fn xxhash(c: &mut Criterion) {
    let msg = [0; 1264];

    let mut g = c.benchmark_group("hash");
    g.throughput(Throughput::Bytes(1264));

    let xxh_secret: [u8; 192] =
        xxhash_rust::const_xxh3::const_custom_default_secret(0x042069);
    g.bench_function("xxhash", |b| {
        b.iter(|| {
            xxhash_rust::xxh3::xxh3_128_with_secret(&msg, &xxh_secret)
        })
    });
}

fn blake(c: &mut Criterion) {
    let msg = [0; 1264];

    let mut g = c.benchmark_group("hash");
    g.throughput(Throughput::Bytes(1264));
    g.bench_function("blake", |b| b.iter(|| blake3::hash(&msg)));
}

criterion_group!(hash, ahash, blake, xxhash);
criterion_main!(hash);
