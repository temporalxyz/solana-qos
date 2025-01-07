use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread::Builder,
};

use agave_transaction_view::transaction_view::TransactionView;
use clap::Parser;
use mock_tx_engine::{agent::read_price, Engine, STATS};
use que::{
    headless_spmc::{consumer::Consumer, producer::Producer},
    page_size::PageSize,
    shmem::Shmem,
};
use solana_qos_common::{
    checked_drop_privileges,
    ipc_parameters::{
        IPC_FWD_TO_QOS_CAP, IPC_FWD_TO_QOS_NAME, IPC_QOS_TO_SIG_CAP,
        IPC_QOS_TO_SIG_NAME, IPC_SCH_TO_QOS_CAP, IPC_SCH_TO_QOS_NAME,
        IPC_SIG_TO_QOS_CAP, IPC_SIG_TO_QOS_NAME, IPC_STATUS_CACHE_CAP,
        IPC_STATUS_CACHE_NAME, IPC_TPU_TO_QOS_CAP, IPC_TPU_TO_QOS_NAME,
    },
    packet_bytes::PacketBytes,
    remaining_meta::QoSRemainingMeta,
    xxhash::xxHasher,
};
use solana_qos_core::{get_page_size, sig_bytes};

#[cfg(feature = "demo")]
mod tui;

#[derive(Parser)]
pub struct Args {
    /// Threads to use per TPU/FWD tx generator
    #[clap(long)]
    threads_per_generator: usize,

    #[clap(long)]
    xxhash_seed: u64,

    #[clap(long)]
    #[cfg(target_os = "linux")]
    use_huge_pages: bool,

    #[clap(long, default_value_t = false)]
    write_to_file: bool,
}

pub static START: AtomicBool = AtomicBool::new(false);

fn main() {
    // Parse cli arguments
    let args = Args::parse();

    // Initialize engines
    let tpu_engine = Engine::<IPC_TPU_TO_QOS_CAP>::initialize(
        IPC_TPU_TO_QOS_NAME,
        #[cfg(target_os = "linux")]
        args.use_huge_pages,
        args.threads_per_generator,
        args.write_to_file,
        &START,
    )
    .unwrap();
    let fwd_engine = Engine::<IPC_FWD_TO_QOS_CAP>::initialize(
        IPC_FWD_TO_QOS_NAME,
        #[cfg(target_os = "linux")]
        args.use_huge_pages,
        args.threads_per_generator,
        args.write_to_file,
        &START,
    )
    .unwrap();

    // Join recent sig cache as producer
    let page_size = get_page_size(args.use_huge_pages);
    let mut recent_signatures = unsafe {
        Producer::<[u8; 64], IPC_STATUS_CACHE_CAP>::join_or_create_shmem(IPC_STATUS_CACHE_NAME, page_size)
            .unwrap()
    };

    // Joined shared stats shmem
    let shared_stats =
        Shmem::open_or_create("qos_stats", 2048, PageSize::Standard)
            .unwrap();

    // QoS -> Sigverify Consumer
    let mut banking_consumer = unsafe {
        Consumer::<PacketBytes, IPC_QOS_TO_SIG_CAP>::join_shmem(
            IPC_QOS_TO_SIG_NAME,
            page_size,
        )
        .unwrap()
    };

    // Sigverify -> QoS Producer
    let mut sig_qos_producer = unsafe {
        Producer::<
            PacketBytes,
            IPC_SIG_TO_QOS_CAP,
        >::join_or_create_shmem(
            IPC_SIG_TO_QOS_NAME,
            page_size,
        )
        .unwrap()
    };

    // Scheduler -> QoS Producer
    let mut sch_qos_producer = unsafe {
        Producer::<
        QoSRemainingMeta<()>,
        IPC_SCH_TO_QOS_CAP,
    >::join_or_create_shmem(
        IPC_SCH_TO_QOS_NAME,
        page_size,
    )
    .unwrap()
    };

    // Remove sudo privileges
    if let Err(e) = checked_drop_privileges() {
        panic!("{e:?}");
    }

    let mock_tpu = Builder::new()
        .name("mockTPU".to_string())
        .spawn(move || {
            // Funnel into ipc
            let fwd = false;
            tpu_engine.funnel_into_ipc(fwd);
        })
        .unwrap();

    let mock_fwd = Builder::new()
        .name("mockFWD".to_string())
        .spawn(move || {
            // Funnel into ipc
            let fwd = true;
            fwd_engine.funnel_into_ipc(fwd);
        })
        .unwrap();

    let mock_banking = Builder::new()
        .name("mockBanking".to_string())
        .spawn(move || {
            while !START.load(Ordering::Relaxed) {}

            // xxHasher
            let xxhasher =
                xxHasher::initialize_with_seed(args.xxhash_seed);

            let mut received = 0_usize;
            let mut failed = 0_usize;
            let mut sched = 0_usize;

            let mut file = args.write_to_file.then(|| {
                BufWriter::with_capacity(
                    8 * 1024,
                    File::create(
                        Path::new("packet-data/").join("sigverify"),
                    )
                    .expect("failed to open file"),
                )
            });

            let mut i = 0_usize;
            loop {
                if let Some(packet_bytes) = banking_consumer.pop() {
                    let mut packet = packet_bytes.as_packet();
                    received += 1;

                    // Calculate hash for key and pseudo rng
                    let packet_hash: u64 =
                        xxhasher.packet_hash(&packet);

                    // For this mock, we read last byte to determine if it failed sigverify
                    let sig = *packet.buffer_mut().last().unwrap() == 0;
                    // sch is a random byte
                    let sch = unsafe {
                        *(&packet_hash as *const u64 as *const u8)
                    };

                    if !sig {
                        // failed sigverify
                        sig_qos_producer.push(&packet_bytes);
                        failed += 1;
                    } else {
                        let tx_view =
                            TransactionView::try_new_unsanitized(
                                packet.data(..).unwrap(),
                            )
                            .expect("qos will filter these");

                        // Passed sigverify, randomly assign
                        // scheduled status
                        let mut remaining_meta = QoSRemainingMeta {
                            packet_hash,
                            execution_nanos: 0,
                            additional_metadata: (),
                        };
                        match sch {
                            0..192 => {
                                // not scheduled, execution nanos
                                // already 0
                                sch_qos_producer.push(&remaining_meta);
                            }
                            _ => {
                                // scheduled, assign random
                                // execution time within 1us
                                // Always adding 1 to modulo => always nonzero
                                remaining_meta.execution_nanos =
                                    (packet_hash
                                        & const { 1024 * 1024 - 1 })
                                        + 1;

                                sch_qos_producer.push(&remaining_meta);
                                sched += 1;

                                // Send recent signature
                                recent_signatures.push(sig_bytes(
                                    &tx_view.signatures()[0],
                                ));
                                sch_qos_producer.sync();
                                recent_signatures.sync();
                            }
                        }
                    }

                    if args.write_to_file && i < 4_000_000 {
                        let price = unsafe { read_price(&mut packet) };
                        let addr = packet.meta().addr;
                        writeln!(
                            file.as_mut().unwrap(),
                            "{} {}",
                            price,
                            addr,
                        )
                        .ok();
                        if i == 4_000_000 - 1 {
                            file.as_mut().unwrap().flush().unwrap();
                        }
                    }

                    i += 1;
                }

                if received & const { 32 * 1024 - 1 } == 0 {
                    STATS
                        .sigverify_recvs
                        .fetch_add(received, Ordering::Release);
                    received = 0;

                    STATS
                        .sigverify_sends
                        .fetch_add(failed, Ordering::Release);
                    failed = 0;

                    STATS
                        .scheduler_sends
                        .fetch_add(sched, Ordering::Release);
                    sched = 0;
                }
            }
        })
        .unwrap();

    std::fs::create_dir_all("packet-data").ok();
    START.store(true, Ordering::Release);

    #[cfg(feature = "demo")]
    tui::FlowGraph::run(shared_stats);

    mock_tpu.join().unwrap();
    mock_fwd.join().unwrap();
    mock_banking.join().unwrap();
}
