use std::array::from_fn;

use criterion::{criterion_group, criterion_main, Criterion};
use qos_minmax::MinMaxHeap;

fn throughput(c: &mut Criterion) {
    type Element = [u8; 1232];
    // const N: usize = 25_000_usize.next_power_of_two();
    const N: usize = 15_000_usize.next_power_of_two();

    let mut heap = MinMaxHeap::<Element, N>::new();

    while heap.len() < N - 1 {
        heap.push([1; 1232]);
    }

    let mut values = vec![];
    for _ in 0..N {
        values.push(from_fn::<u8, 1232, _>(|_| rand::random::<u8>()));
    }

    let mut indices = vec![];
    for _ in 0..N {
        indices.push(rand::random::<usize>() % N);
    }

    let mut g = c.benchmark_group("Push");
    g.throughput(criterion::Throughput::Bytes(1232));

    let mut i = 0;
    g.bench_function("full-push", |b| {
        b.iter(|| {
            heap.push(values[indices[i & (N - 1)]]);
            i += 1;
        });
    });

    g.finish();
}

criterion_group!(minmax, throughput);
criterion_main!(minmax);
