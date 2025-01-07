use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use agent::{
    null_transfer_transaction_with_compute_unit_price, read_price, Ip,
};
use que::{error::QueError, headless_spmc::producer::Producer};
use rand::seq::SliceRandom;
use rng::FastxxHashRng;
use solana_qos_core::get_page_size;

use solana_qos_common::{
    packet_bytes::PacketBytes, shared_stats::EngineStats,
};

use mpsc::{self, Consumer};
use timer::Timer;

pub mod agent;

pub mod rng;

pub struct Engine<const N: usize> {
    consumer: Consumer<PacketBytes, 8192>,
    spsc: Producer<PacketBytes, N>,
}

pub static STATS: EngineStats = EngineStats {
    tpu_sends: AtomicUsize::new(0),
    fwd_sends: AtomicUsize::new(0),
    sigverify_recvs: AtomicUsize::new(0),
    sigverify_sends: AtomicUsize::new(0),
    scheduler_sends: AtomicUsize::new(0),
};

impl<const N: usize> Engine<N> {
    pub fn initialize(
        shmem_id: &'static str,
        #[cfg(target_os = "linux")] use_huge_pages: bool,
        generators: usize,
        write_to_file: bool,
        start: &'static AtomicBool,
    ) -> Result<Engine<N>, EngineError> {
        let page_size = get_page_size(
            #[cfg(target_os = "linux")]
            use_huge_pages,
        );
        Ok(Engine {
            consumer: initialize_generator_threads(
                generators,
                write_to_file,
                shmem_id,
                start,
            )?,
            spsc: unsafe {
                Producer::join_or_create_shmem(shmem_id, page_size)
            }?,
        })
    }

    /// Read from the sharded mpsc and funnel into the IPC spsc.
    ///
    /// This blocks forever!
    pub fn funnel_into_ipc(mut self, fwd: bool) {
        #[cfg(not(feature = "demo"))]
        println!("waiting for active consumers to start funnel");
        while !self.spsc.consumer_heartbeat() {}
        #[cfg(not(feature = "demo"))]
        println!("active consumer detected. starting funnel");

        let timer = Timer::new();
        let mut last_check = 0;
        loop {
            if timer.elapsed_ms() / 5000 > last_check {
                last_check = timer.elapsed_ms() / 5000;
                if !self.spsc.consumer_heartbeat() {
                    break;
                }
            }
            let mut sends = 0;
            for _ in 0..16 {
                if let Some(packet_bytes) = self.consumer.pop() {
                    self.spsc.push(&packet_bytes);
                    sends += 1;
                }
            }
            self.spsc.sync();
            self.spsc.beat();

            if fwd {
                STATS
                    .fwd_sends
                    .fetch_add(sends, Ordering::Release);
            } else {
                STATS
                    .tpu_sends
                    .fetch_add(sends, Ordering::Release);
            }
        }

        #[cfg(not(feature = "demo"))]
        println!("no active consumer detected. exiting funnel");
    }
}

/// Spin up `generators` number of threads that generate signed/unsigned
/// transactions according to `sign_probability`. Serializes and sends
/// transactions as packets via a fast (unfair, non-fifo) wait-free mpsc
/// queue.
pub fn initialize_generator_threads(
    generators: usize,
    write_to_file: bool,
    file: &'static str,
    start: &'static AtomicBool,
) -> Result<Consumer<PacketBytes, 8192>, QueError> {
    let (producers, consumer) = mpsc::bounded::<PacketBytes, 8192>(
        generators,
        "tpu_engine",
        #[cfg(target_os = "linux")]
        que::page_size::PageSize::Standard,
    )?;

    producers.into_iter().zip(1..).for_each(
        move |(mut producer, id)| {
            std::thread::spawn(move || {
                while !start.load(Ordering::Relaxed) {}

                let mut ips: [Ip; 3] = [
                    Ip::new([1, 1, 1, 1], 400_000_f32.ln(), 2.0),
                    Ip::new([2, 2, 2, 2], 100_000_f32.ln(), 2.0),
                    Ip::bad([7, 7, 7, 7], 100_000_f32.ln(), 2.0),
                ];
                let mut rng = FastxxHashRng::new(0x123123 + id);

                let mut packet =
                    null_transfer_transaction_with_compute_unit_price();

                let mut file = write_to_file.then(|| {
                    BufWriter::with_capacity(
                        8 * 1024,
                        File::create(
                            Path::new("packet-data/")
                                .join(format!("{file}{id}")),
                        )
                        .expect("failed to open file"),
                    )
                });

                // Generate, serialize, and send transaction packet
                for i in 0_usize.. {
                    let ip = ips.choose_mut(&mut rng).unwrap();

                    ip.update_transfer_with_priority(&mut packet);

                    if write_to_file && i < 4_000_000 {
                        let price = unsafe { read_price(&mut packet) };
                        let addr = packet.meta().addr;
                        writeln!(
                            file.as_mut().unwrap(),
                            "{} {}",
                            price,
                            addr,
                        )
                        .unwrap();

                        if i == 4_000_000 - 1 {
                            file.as_mut().unwrap().flush().unwrap();
                        }
                    }

                    // Send packets (one dup)
                    producer.push(PacketBytes::from_packet(&packet));
                    producer.push(PacketBytes::from_packet(&packet));
                }
            });
        },
    );

    Ok(consumer)
}

#[derive(Debug)]
pub enum EngineError {
    QueError(QueError),
}

impl From<QueError> for EngineError {
    fn from(value: QueError) -> Self {
        EngineError::QueError(value)
    }
}
