use core::hint::black_box;
use std::collections::HashMap;

use criterion::{criterion_group, criterion_main, Criterion};
use qos_model::{
    interface::QoSModel,
    models::{
        ip_signer::IpSignerModel, ip_signer_stake::IpSignerStakeModel,
    },
};
use rand::{seq::SliceRandom, thread_rng};
use solana_qos_common::transaction_meta::QoSTransactionMeta;

fn ip_signer(c: &mut Criterion) {
    // Fetch mock ip signer model
    let (model, ips, signers) =
        mock_ip_signer_model(100_000_000, 3_000, 10_000);
    let some_ip = choose(&ips);
    let some_signer = choose(&signers);

    let mut ip_signer = c.benchmark_group("IpSigner");
    ip_signer.throughput(criterion::Throughput::Elements(1));

    ip_signer.bench_function("IpSigner", |b| {
        b.iter(|| black_box(model.forward(some_ip, &some_signer, &())));
    });
}

fn ip_signer_stake(c: &mut Criterion) {
    // Fetch mock ip signer model
    let (model, ips, signers) =
        mock_ip_signer_stake_model(100_000_000, 3_000, 10_000);
    let some_ip = choose(&ips);
    let some_signer = choose(&signers);

    let mut ip_signer = c.benchmark_group("IpSignerStake");
    ip_signer.throughput(criterion::Throughput::Elements(1));

    ip_signer.bench_function("IpSignerStake", |b| {
        b.iter(|| black_box(model.forward(some_ip, &some_signer, &())));
    });
}

criterion_group!(evaluation, ip_signer, ip_signer_stake);

criterion_main!(evaluation);

fn mock_ip_signer_model(
    transactions: usize,
    num_ips: usize,
    num_signers: usize,
) -> (IpSignerModel<2048, 2048>, Vec<u32>, Vec<[u8; 32]>) {
    // We first generate some random transaction metas.
    // These don't need to be reflective of mainnet to benchmark
    // evaluation.
    let ips: Vec<u32> = (0..num_ips)
        .map(|_| rand::random::<u32>())
        .collect();
    let signers: Vec<[u8; 32]> = (0..num_signers)
        .map(|_| rand::random::<[u8; 32]>())
        .collect();
    let execution_time_gen = || rand::random::<u64>() % 1_000_000;
    let fee_gen = || 5000 + rand::random::<u64>() % 100_000_000;
    let transaction_metas = (0..transactions).map(|_| {
        QoSTransactionMeta::new_for_tests(
            choose(&ips),
            choose(&signers),
            fee_gen(),
            execution_time_gen(),
            (),
        )
    });

    // Initialize model
    let mut model = IpSignerModel::new([], []);
    model.update_model(transaction_metas, num_ips, num_signers);

    (model, ips, signers)
}

fn mock_ip_signer_stake_model(
    transactions: usize,
    num_ips: usize,
    num_signers: usize,
) -> (IpSignerStakeModel<2048, 2048>, Vec<u32>, Vec<[u8; 32]>) {
    // We first generate some random transaction metas.
    // These don't need to be reflective of mainnet to benchmark
    // evaluation.
    let ips: Vec<u32> = (0..num_ips)
        .map(|_| rand::random::<u32>())
        .collect();
    let signers: Vec<[u8; 32]> = (0..num_signers)
        .map(|_| rand::random::<[u8; 32]>())
        .collect();
    let execution_time_gen = || rand::random::<u64>() % 1_000_000;
    let fee_gen = || 5000 + rand::random::<u64>() % 100_000_000;
    let transaction_metas = (0..transactions).map(|_| {
        QoSTransactionMeta::new_for_tests(
            choose(&ips),
            choose(&signers),
            fee_gen(),
            execution_time_gen(),
            (),
        )
    });

    // Generate some random stake parameters
    let mut stake_lookup = HashMap::new();
    for &ip in &ips {
        stake_lookup
            .insert(ip, 1_000_000 * (rand::random::<u64>() % 100));
    }
    let total_stake = 500_000 * 1_000_000_000;

    // Initialize model
    let mut model = IpSignerStakeModel::new(
        [],
        [],
        stake_lookup.clone(),
        total_stake,
    );
    model.update_model(
        transaction_metas,
        num_ips,
        num_signers,
        (total_stake, stake_lookup),
    );

    (model, ips, signers)
}

fn choose<T: Copy>(items: &[T]) -> T {
    items
        .choose(&mut thread_rng())
        .copied()
        .unwrap()
}
