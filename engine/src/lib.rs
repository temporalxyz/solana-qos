use std::{
    fs::File,
    io::{BufWriter, Write},
    net::{IpAddr, Ipv4Addr},
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

use agent::{
    null_transfer_transaction_with_compute_unit_price, read_price, Ip,
};
use que::{error::QueError, headless_spmc::producer::Producer};
use rand::seq::SliceRandom;
use rng::FastxxHashRng;
use rtrb::PushError;
use solana_qos_core::get_page_size;
use solana_sdk::{
    hash::Hash,
    message::Message,
    packet::{Meta, Packet},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    system_instruction, system_transaction,
    transaction::Transaction,
};

use solana_qos_common::{
    packet_bytes::PacketBytes, shared_stats::EngineStats,
};

use mpsc::{self, Consumer};
use timer::Timer;

pub mod agent;

pub mod rng;

pub struct Engine<const N: usize> {
    consumer: Consumer<Packet>,
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
            ),
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
                if let Some(packet) = self.consumer.pop() {
                    let packet_bytes =
                        PacketBytes::from_packet(&packet);
                    self.spsc.push(packet_bytes);
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
) -> Consumer<Packet> {
    let (producers, consumer) = mpsc::bounded(generators, 8192);

    let mut i = 0;
    producers
        .into_iter()
        .for_each(move |mut producer| {
            i += 1;
            std::thread::spawn(move || {
                let mut ips: [Ip; 3] = [
                    Ip::new([1, 1, 1, 1], 400_000_f32.ln(), 2.0),
                    Ip::new([2, 2, 2, 2], 100_000_f32.ln(), 2.0),
                    Ip::bad([7, 7, 7, 7], 100_000_f32.ln(), 2.0),
                ];
                let mut rng = FastxxHashRng::new(0x123123 + i);

                let mut packet =
                    null_transfer_transaction_with_compute_unit_price();

                std::fs::create_dir_all("packet-data").ok();
                let mut file = write_to_file.then(|| {
                    BufWriter::with_capacity(
                        8 * 1024,
                        File::create(
                            Path::new("packet-data/")
                                .join(format!("{file}{i}")),
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

                    // Also send a dup
                    let mut second_packet = packet.clone();

                    // Send packet (busy retry if full)
                    let mut packet_send = packet.clone();
                    while let Err(PushError::Full(p)) =
                        producer.push(packet_send)
                    {
                        packet_send = p;
                    }

                    // Send second packet (busy retry if full)
                    while let Err(PushError::Full(p)) =
                        producer.push(second_packet)
                    {
                        second_packet = p;
                    }
                }
            });
        });

    consumer
}

/// Spin up `generators` number of threads that generate signed/unsigned
/// transactions according to `sign_probability`. Serializes and sends
/// transactions as packets via a fast (unfair, non-fifo) wait-free mpsc
/// queue.
pub fn initialize_old(
    generators: usize,
    sign_probability: Option<f64>,
) -> Consumer<Packet> {
    let (producers, consumer) = mpsc::bounded(generators, 8192);

    producers
        .into_iter()
        .for_each(move |mut producer| {
            std::thread::spawn(move || {
                // The condition of being bounded in [0, 1] is verified
                // by clap
                let sign_probability = sign_probability.unwrap_or(0.0);

                // Generate keypair set
                let keypairs = core::array::from_fn::<_, 10, _>(|_| {
                    Keypair::new()
                });

                // Generate, serialize, and send transaction packet
                for i in 0.. {
                    // Create transaction
                    let transaction = generate_maybe_signed_transaction(
                        &keypairs[i % 10],
                        sign_probability,
                    );

                    // Serialize into packet
                    let mut packet =
                        Packet::from_data(None, &transaction).unwrap();
                    // TODO: perf
                    packet.meta_mut().addr =
                        IpAddr::V4(Ipv4Addr::from_bits(
                            rand::random::<u32>() % (1 << 25),
                        ));

                    // Also send a dup or invalid
                    let mut second_packet = (rand::random::<u8>() < 16)
                        .then(|| packet.clone())
                        .unwrap_or_else(invalid_packet);

                    // Send packet (busy retry if full)
                    while let Err(PushError::Full(p)) =
                        producer.push(packet)
                    {
                        packet = p;
                    }

                    // Send second packet (dup or invalid)
                    // (busy retry if full)
                    while let Err(PushError::Full(p)) =
                        producer.push(second_packet)
                    {
                        second_packet = p;
                    }
                }
            });
        });

    consumer
}

#[inline(always)]
fn invalid_packet() -> Packet {
    Packet::new([0; 1232], Meta::default())
}

fn generate_signed_transaction(
    keypair: &Keypair,
    _p: f64,
) -> Transaction {
    system_transaction::transfer(
        keypair,
        &Pubkey::default(),
        1,
        Hash::new_unique(),
    )
}

fn generate_unsigned_transaction(
    keypair: &Keypair,
    _p: f64,
) -> Transaction {
    let mut tx = Transaction::new_unsigned(Message::new(
        &[system_instruction::transfer(
            &keypair.pubkey(),
            &Pubkey::new_unique(),
            1,
        )],
        Some(&keypair.pubkey()),
    ));
    let sig_bytes: [u8; 64] = core::array::from_fn(|_| rand::random());
    tx.signatures[0] = Signature::from(sig_bytes);
    tx
}

fn generate_maybe_signed_transaction(
    keypair: &Keypair,
    p: f64,
) -> Transaction {
    if rand::random::<f64>() < p {
        generate_signed_transaction(keypair, p)
    } else {
        generate_unsigned_transaction(keypair, p)
    }
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
