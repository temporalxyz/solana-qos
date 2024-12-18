use std::time::{Duration, Instant};

use bytemuck::Pod;
use criterion::{
    criterion_group, criterion_main, Criterion, Throughput,
};
use sokoban::{NodeAllocatorMap, RedBlackTree};

const CAPACITY: usize = 1024 * 1024;

fn boxed_rbt<
    K: Ord + Default + Pod + std::fmt::Debug,
    V: Default + Pod,
    const N: usize,
>() -> Box<RedBlackTree<K, V, N>> {
    let layout = std::alloc::Layout::new::<RedBlackTree<K, V, N>>();
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
    if ptr.is_null() {
        panic!("alloc failed");
    }

    let mut tree: Box<RedBlackTree<K, V, N>> =
        unsafe { Box::from_raw(ptr.cast()) };
    tree.initialize();

    tree
}

fn rbt(c: &mut Criterion) {
    let mut ip_rbt = boxed_rbt::<u32, f64, CAPACITY>();

    // Fill tree
    for ip in 0..CAPACITY as u32 {
        ip_rbt.insert(ip, 0.0);
    }

    let mut g = c.benchmark_group("lookup");
    g.throughput(Throughput::Elements(1));

    let mut i = 0;
    g.bench_function("ip-rbt", |b| {
        b.iter(|| {
            let v = ip_rbt.get(&i);
            i = (i + 1) % CAPACITY as u32;
            v
        })
    });
    g.finish();
    let mut g = c.benchmark_group("insert-random");
    g.throughput(Throughput::Elements(1));

    g.bench_function("ip-rbt", |b| {
        b.iter_custom(|iters| {
            // Initialize accumulator
            let mut time = Duration::default();

            for _ in 0..iters {
                // remove one element
                let random_addr =
                    1 + (rand::random::<u32>() % CAPACITY as u32);
                let random_element_key =
                    ip_rbt.get_node(random_addr).key;
                let random_element = ip_rbt
                    .remove(&random_element_key)
                    .unwrap();

                // time insert one element
                let timer = Instant::now();
                ip_rbt
                    .insert(random_element_key, random_element)
                    .unwrap();
                time += timer.elapsed();
            }

            time
        })
    });
    g.finish();

    let mut g = c.benchmark_group("remove-root");
    g.throughput(Throughput::Elements(1));

    g.bench_function("ip-rbt", |b| {
        b.iter_custom(|iters| {
            // Initialize accumulator
            let mut time = Duration::default();

            for _ in 0..iters {
                // time remove root
                let timer = Instant::now();
                let root_key = ip_rbt.get_node(ip_rbt.root).key;
                let root_value = ip_rbt.remove(&root_key).unwrap();
                time += timer.elapsed();

                // add it back in
                ip_rbt.insert(root_key, root_value);
            }

            time
        })
    });
    g.finish();

    let mut signer_rbt = boxed_rbt::<[u8; 32], f64, CAPACITY>();

    // Fill tree
    for signer in 0..CAPACITY as u32 {
        let mut signer_array = [0; 32];
        signer_array[..4].copy_from_slice(&signer.to_le_bytes());
        signer_rbt.insert(signer_array, 0.0);
    }

    let mut g = c.benchmark_group("lookup");
    g.throughput(Throughput::Elements(1));

    let mut i = [0_u32; 8];
    g.bench_function("signer-rbt", |b| {
        b.iter(|| {
            let v = signer_rbt.get(bytemuck::cast_ref(&i));
            i[0] = (i[0] + 1) % CAPACITY as u32;
            v
        })
    });

    g.finish();

    let mut g = c.benchmark_group("insert-random");
    g.throughput(Throughput::Elements(1));

    g.bench_function("signer-rbt", |b| {
        b.iter_custom(|iters| {
            // Initialize accumulator
            let mut time = Duration::default();

            for _ in 0..iters {
                // remove one element
                let random_addr =
                    1 + (rand::random::<u32>() % CAPACITY as u32);
                let random_element_key =
                    signer_rbt.get_node(random_addr).key;
                let random_element = signer_rbt
                    .remove(&random_element_key)
                    .unwrap();

                // time insert one element
                let timer = Instant::now();
                signer_rbt
                    .insert(random_element_key, random_element)
                    .unwrap();
                time += timer.elapsed();
            }

            time
        })
    });
    g.finish();

    let mut g = c.benchmark_group("remove-root");
    g.throughput(Throughput::Elements(1));

    g.bench_function("signer-rbt", |b| {
        b.iter_custom(|iters| {
            // Initialize accumulator
            let mut time = Duration::default();

            for _ in 0..iters {
                // time remove root
                let timer = Instant::now();
                let root_key = signer_rbt.get_node(signer_rbt.root).key;
                let root_value = signer_rbt.remove(&root_key).unwrap();
                time += timer.elapsed();

                // add it back in
                signer_rbt.insert(root_key, root_value);
            }

            time
        })
    });
    g.finish();
}

criterion_group!(lookup, rbt);
criterion_main!(lookup);
