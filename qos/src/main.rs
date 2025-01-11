use std::{
    net::IpAddr,
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use clap::Parser;
use log::{info, warn};
use qos_model::{
    interface::QoSModel, models::ip_signer::IpSignerModel,
};
use que::{
    headless_spmc::{consumer::Consumer, producer::Producer},
    page_size::PageSize,
};
use solana_qos_common::{
    checked_drop_privileges,
    ipc_parameters::*,
    packet_bytes::PacketBytes,
    remaining_meta::QoSRemainingMeta,
    shared_stats::Stats,
    xxhash::{xxHash, xxHasher},
};
use solana_qos_core::{
    banking::TransactionContainer, get_page_size, try_process_packet,
    u64_key,
};
use solana_qos_internal_common::{
    packet_bytes, partial_meta::QoSPartialMeta,
    transaction_meta::QoSTransactionMeta,
};
use timer::Timer;

#[cfg(feature = "demo")]
use {que::shmem::Shmem, solana_qos_common::shared_stats::SharedStats};

use qos_lru::LRUCache;

static EXIT: AtomicBool = AtomicBool::new(false);

type SignatureBytes = [u8; 64];

#[derive(Parser)]
pub struct Args {
    #[cfg(target_os = "linux")]
    #[clap(long)]
    use_huge_pages: bool,

    #[clap(long)]
    xxhash_seed: u64,

    #[clap(long, default_value_t = 1_000_000)]
    target_pps: usize,

    #[clap(long, default_value_t = 10_000)]
    max_signers: usize,

    #[clap(long, default_value_t = 10_000)]
    max_ips: usize,
}

#[allow(unused_must_use)]
fn main() {
    // Parse command line arguments
    let args = Args::parse();

    // Rename main thread
    unsafe {
        libc::pthread_setname_np(
            libc::pthread_self(),
            "solana-qos".as_ptr().cast(),
        );
    }

    // Add ctrlc handler
    ctrlc::set_handler(|| {
        warn!("received exit signal");
        EXIT.store(true, Ordering::Relaxed);
    });

    // Initialize logging
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Initialize all IPC channels
    let page_size = get_page_size(
        #[cfg(target_os = "linux")]
        args.use_huge_pages,
    );
    let (
        mut tpu_consumer,
        mut fwd_consumer,
        mut re1_consumer,
        mut re2_consumer,
        mut sig_consumer,
        mut sch_consumer,
        sig_producer,
        mut recent_sig_consumer,
    ) = join_ipc(page_size);

    // Initialize stats
    let mut stats = Stats::new();
    #[cfg(feature = "demo")]
    let stats_shmem =
        Shmem::open_or_create("qos_stats", 2048, PageSize::Standard)
            .unwrap();

    // Remove sudo privileges
    if let Err(e) = checked_drop_privileges() {
        panic!("{e:?}");
    }

    // Write seed to first 8 bytes of scheduler channel metadata
    unsafe {
        let metadata_ptr: NonNull<AtomicU64> =
            sch_consumer.get_padding_ptr().cast();
        metadata_ptr
            .as_ref()
            .store(args.xxhash_seed, Ordering::Release);
    }

    // Initialize QoS Model
    let mut qos_model = IpSignerModel::new([], []);
    let mut qos_tx_partial_metas =
        LRUCache::<_, _, { 1024 * 1024 }>::new_boxed();
    let mut qos_tx_complete_metas = Vec::with_capacity(1024 * 1024);

    // Initialize container with banking stage transmitter
    let mut container =
        TransactionContainer::new(Some(sig_producer), args.target_pps);

    // Initialize LRU cache for filtering recently confirmed signatures
    let mut recent_signatures =
        LRUCache::<u64, (), { 1024 * 1024 }>::new_boxed();

    // Initialize hasher
    let xxhasher = xxHasher::initialize_with_seed(args.xxhash_seed);

    // If on x86, tune rdtsc-based timer
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Timer::memoize_ticks_per_ms_and_invariant_tsc_check();

    // Start timer
    let timer = Timer::new();

    info!("starting qos");
    while !EXIT.load(Ordering::Relaxed) {
        // Consume packets
        //
        // NOTE: four consumers are used because when using a modified co-hosted relayer with qos, there is still some residual traffic to the host's original (and now unadvertised) TPU.
        for consumer in [
            &mut tpu_consumer,
            &mut fwd_consumer,
            &mut re1_consumer,
            &mut re2_consumer,
        ] {
            consume_transaction_packets(
                consumer,
                &mut qos_model,
                &mut qos_tx_partial_metas,
                &mut stats,
                &mut container,
                &xxhasher,
                &recent_signatures,
            );
        }

        // Consume recent signatures.
        consume_recent_signatures(
            &mut recent_sig_consumer,
            &mut recent_signatures,
            &mut stats,
        );

        // Log periodically
        static mut LAST_LOG: u64 = 0;
        let elapsed_ms = timer.elapsed_ms();
        let elapsed_5s = elapsed_ms / 5000;
        if unsafe { elapsed_5s > LAST_LOG } {
            unsafe { LAST_LOG = elapsed_5s };
            log_stats(&timer, &mut stats);
        }
        #[cfg(feature = "demo")]
        unsafe {
            static mut LAST_SAVE: u64 = 0;
            let elapsed_100ms = elapsed_ms / 100;
            if elapsed_100ms > LAST_SAVE {
                LAST_SAVE = elapsed_100ms;
                if SharedStats::update(
                    stats_shmem.get_mut_ptr(),
                    &stats,
                ) {
                    stats = Stats::default();
                }
            }
        }

        // Try to complete partial metas, send complete metas to db,
        // update model
        consume_remaining_metas(
            &mut sch_consumer,
            &mut qos_tx_partial_metas,
            &mut qos_tx_complete_metas,
            &mut qos_model,
            &mut stats,
            args.max_signers,
            args.max_ips,
        );

        // Handle any failed sigverify signals
        consume_sigverify_signals(&mut sig_consumer, &mut qos_model);
    }

    info!("received exit signal");
    qos_model.save_ip_scores("ip_scores");
    info!("graceful exit complete");
}

fn consume_recent_signatures(
    recent_sig_consumer: &mut Consumer<[u8; 64], { 1024 * 1024 }>,
    recent_signatures: &mut LRUCache<u64, (), { 1024 * 1024 }>,
    stats: &mut Stats,
) {
    while let Some(signature) = recent_sig_consumer.pop() {
        let key = u64_key(&signature);
        recent_signatures.put(key, ());

        stats.recent_signatures_received += 1;
    }
}

fn consume_remaining_metas(
    sch_consumer: &mut Consumer<
        QoSRemainingMeta<()>,
        IPC_SCH_TO_QOS_CAP,
    >,
    qos_tx_partial_metas: &mut LRUCache<
        xxHash,
        QoSPartialMeta,
        { 1024 * 1024 },
    >,
    qos_tx_complete_metas: &mut Vec<QoSTransactionMeta<()>>,
    qos_model: &mut IpSignerModel<16384, 16384>,
    stats: &mut Stats,
    max_signers: usize,
    max_ips: usize,
) {
    while let Some(remaining_meta) = sch_consumer.pop() {
        // Merge meta if still in LRU.
        if let Some((_packet_hash, partial_meta)) =
            qos_tx_partial_metas.pop(&remaining_meta.packet_hash)
        {
            // Complete metadata entry
            let was_scheduled = remaining_meta.execution_nanos > 0;
            let mut complete_entry = partial_meta.merge(remaining_meta);
            if was_scheduled {
                *complete_entry.value *= 10.0;
            }
            qos_tx_complete_metas.push(complete_entry);

            stats.completed += 1;
            if stats.completed % 1_000_000 == 0 {
                log::info!(
                    "fully processed {} transactions",
                    stats.completed
                );
            }

            // TODO: there may be a better way to do this and
            // I don't like this hardcoded threshold.
            // At current traffic (2.0.21) this is roughly every block.
            if qos_tx_complete_metas.len() > 400 {
                qos_model.update_model(
                    qos_tx_complete_metas.drain(..),
                    max_signers,
                    max_ips,
                );
                qos_model.save_ip_scores("scores");
                break;
            }
        } else {
            log::debug!(
                "partial meta for packet hash {} dropped before being merged",
                remaining_meta.packet_hash
            );
        }
    }
}

fn consume_sigverify_signals(
    sig_consumer: &mut Consumer<PacketBytes, IPC_SIG_TO_QOS_CAP>,
    qos_model: &mut IpSignerModel<16384, 16384>,
) {
    while let Some(sigverify_failed) = sig_consumer.pop() {
        process_failed_sigverify(sigverify_failed, qos_model);
    }
}

fn process_failed_sigverify(
    sigverify_failed: PacketBytes,
    qos_model: &mut IpSignerModel<16384, 16384>,
) {
    // Parse ip from packet
    let packet = packet_bytes::as_packet(sigverify_failed);
    let IpAddr::V4(ip) = packet.meta().addr else {
        unreachable!(
            "ipv4 has been enforced by this stage: {:?}",
            &packet.meta().addr,
        );
    };
    let ip: u32 = u32::from_le_bytes(ip.octets());

    qos_model.ip_feedback(ip);
}

fn consume_transaction_packets(
    consumer: &mut Consumer<PacketBytes, IPC_TPU_TO_QOS_CAP>,
    qos_model: &mut IpSignerModel<16384, 16384>,
    qos_tx_partial_metas: &mut LRUCache<
        xxHash,
        QoSPartialMeta,
        { 1024 * 1024 },
    >,
    stats: &mut Stats,
    banking: &mut TransactionContainer,
    xxhasher: &xxHasher,
    recent_signatures: &LRUCache<u64, (), { 1024 * 1024 }>,
) {
    for _ in 0..1_000 {
        if let Some(packet_bytes) = consumer.pop() {
            // Process packet and score transaction
            let Ok(scored_transaction) = try_process_packet(
                packet_bytes::as_packet(packet_bytes),
                Some(recent_signatures),
                qos_model,
                qos_tx_partial_metas,
                stats,
                xxhasher,
            ) else {
                continue;
            };

            // Record zero score transactions
            if *scored_transaction.score == 0.0 {
                stats.zero_score += 1;
            }

            // Send to bank/sigverify
            banking.queue(scored_transaction, stats);
            banking.maybe_transmit(stats, recent_signatures);
        } else {
            break;
        }
    }
    consumer.beat();
    banking.beat();
}

#[cold]
fn log_stats(timer: &Timer, stats: &mut Stats) {
    info!(
        "stats: {stats:?}; average = {:.3}/s",
        stats.total_packets as f64 * 1e3
            / (timer.elapsed_ms().max(1) as f64),
    );
}

fn join_ipc(
    page_size: PageSize,
) -> (
    Consumer<PacketBytes, IPC_TPU_TO_QOS_CAP>,
    Consumer<PacketBytes, IPC_FWD_TO_QOS_CAP>,
    Consumer<PacketBytes, IPC_RE1_TO_QOS_CAP>,
    Consumer<PacketBytes, IPC_RE2_TO_QOS_CAP>,
    Consumer<PacketBytes, IPC_SIG_TO_QOS_CAP>,
    Consumer<QoSRemainingMeta<()>, IPC_SCH_TO_QOS_CAP>,
    Producer<PacketBytes, IPC_QOS_TO_SIG_CAP>,
    Consumer<SignatureBytes, { 1024 * 1024 }>,
) {
    let tpu_consumer = unsafe {
        Consumer::<PacketBytes, IPC_TPU_TO_QOS_CAP>::join_shmem(
            IPC_TPU_TO_QOS_NAME,
            page_size,
        )
        .unwrap()
    };
    let fwd_consumer = unsafe {
        Consumer::<PacketBytes, IPC_FWD_TO_QOS_CAP>::join_shmem(
            IPC_FWD_TO_QOS_NAME,
            page_size,
        )
        .unwrap()
    };

    let re1_consumer = unsafe {
        Consumer::<PacketBytes, IPC_RE1_TO_QOS_CAP>::join_shmem(
            IPC_RE1_TO_QOS_NAME,
            page_size,
        )
        .unwrap()
    };
    let re2_consumer = unsafe {
        Consumer::<PacketBytes, IPC_RE2_TO_QOS_CAP>::join_shmem(
            IPC_RE2_TO_QOS_NAME,
            page_size,
        )
        .unwrap()
    };

    let sig_consumer = unsafe {
        Consumer::<PacketBytes, IPC_SIG_TO_QOS_CAP>::join_shmem(
            IPC_SIG_TO_QOS_NAME,
            page_size,
        )
        .unwrap()
    };

    let sch_consumer = unsafe {
        Consumer::<
        QoSRemainingMeta<()>,
        IPC_SCH_TO_QOS_CAP,
    >::join_shmem(
        IPC_SCH_TO_QOS_NAME,
        page_size,
    )
    .unwrap()
    };

    let sig_producer = unsafe {
        Producer::<
            PacketBytes,
            IPC_QOS_TO_SIG_CAP,
        >::join_or_create_shmem(IPC_QOS_TO_SIG_NAME, page_size)
        .unwrap()
    };

    let recent_sig_consumer = unsafe {
        Consumer::<SignatureBytes, IPC_STATUS_CACHE_CAP>::join_shmem(
            IPC_STATUS_CACHE_NAME,
            page_size,
        )
        .unwrap()
    };

    (
        tpu_consumer,
        fwd_consumer,
        re1_consumer,
        re2_consumer,
        sig_consumer,
        sch_consumer,
        sig_producer,
        recent_sig_consumer,
    )
}
