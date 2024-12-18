use std::{
    error::Error,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use que::{
    headless_spmc::{consumer::Consumer, producer::Producer},
    page_size::PageSize,
    shmem::Shmem,
    Channel,
};
use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    widgets::Paragraph,
    DefaultTerminal, Frame,
};
use solana_qos_common::{
    checked_drop_privileges,
    packet_bytes::{PacketBytes, PACKET_SIZE},
};
use timer::Timer;

const N: usize = 16384;

static WRITES: AtomicUsize = AtomicUsize::new(0);
static READS: AtomicUsize = AtomicUsize::new(0);

fn main() {
    // Open or create "que_demo" shared memory, backed by huge pages
    let page_size = PageSize::Huge;
    let size = size_of::<Channel<PacketBytes, N>>() as i64;
    let huge_page_shmem =
        Shmem::open_or_create("que_demo", size, page_size).unwrap();

    // Pass the pointer to the producer and consumer.
    // The producer will initialize the SPSC so that the consumer can join.
    let buffer = huge_page_shmem.get_mut_ptr();

    // Initialize producer and consumer
    let producer = unsafe {
        Producer::<PacketBytes, N>::initialize_in(buffer).unwrap()
    };
    // Join with consumer
    let consumer =
        unsafe { Consumer::<PacketBytes, N>::join(buffer).unwrap() };

    // Remove sudo privileges
    if let Err(e) = checked_drop_privileges() {
        panic!("{e:?}");
    }

    start_producer(producer);
    start_consumer(consumer);

    // Display results
    display();
}

fn display() {
    color_eyre::install().unwrap();
    let terminal = ratatui::init();
    run(terminal).unwrap();
    ratatui::restore();
}

fn run(mut terminal: DefaultTerminal) -> Result<(), Box<dyn Error>> {
    let timer = Timer::new();
    loop {
        // Load counters
        let reads = READS.load(Ordering::Relaxed);
        let writes = WRITES.load(Ordering::Relaxed);

        // Calculate stats
        let elapsed_s = (timer.elapsed_ms() as f64) / 1e3;
        let rps = (reads as f64) / elapsed_s / 1e6;
        let wps = (writes as f64) / elapsed_s / 1e6;
        let rbps = ((reads * PACKET_SIZE * 8) as f64) / elapsed_s / 1e9;
        let wbps = ((reads * PACKET_SIZE * 8) as f64) / elapsed_s / 1e9;

        // Display
        terminal.draw(|frame: &mut Frame| {
            let diagram = format!(
                r"
             ┌────────────────┐      ┌────────────────┐
             │    Producer    │      │    Consumer    │
             │     {wps:.1}M/s    │─────►│     {rps:.1}M/s    │
             │    {wbps:.1}Gbps   │      │    {rbps:.1}Gbps   │
             └────────────────┘      └────────────────┘
    
                                Que
            _______                                   _ 
           |__   __|                                 | |
              | | ___ _ __ ___  _ __   ___  _ __ __ _| |
              | |/ _ \ '_ ` _ \| '_ \ / _ \| '__/ _` | |
              | |  __/ | | | | | |_) | (_) | | | (_| | |
              |_|\___|_| |_| |_| .__/ \___/|_|  \__,_|_|
                               | |                      
                               |_|                                  
                        
                                                    
        "
            );

            let paragraph = Paragraph::new(diagram);

            frame.render_widget(paragraph, frame.area());
        })?;

        if should_quit()? {
            return Ok(());
        }
    }
}

fn should_quit() -> Result<bool, Box<dyn Error>> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            return Ok(KeyCode::Char('q') == key.code);
        }
    }
    Ok(false)
}

/// Push a null packet in a loop
fn start_producer(mut producer: Producer<PacketBytes, N>) {
    std::thread::Builder::new()
        .name("producer".to_string())
        .spawn(move || {
            let packet = PacketBytes::new([42; PACKET_SIZE]);
            const BATCH_SIZE: usize = 1024;
            loop {
                for _ in 0..BATCH_SIZE {
                    producer.push(&packet);
                }
                producer.sync();
                WRITES.fetch_add(BATCH_SIZE, Ordering::Relaxed);
            }
        })
        .unwrap();
}

// Read packet in a loop
fn start_consumer(mut consumer: Consumer<PacketBytes, N>) {
    std::thread::Builder::new()
        .name("consumer".to_string())
        .spawn(move || {
            const BATCH_SIZE: usize = 1024;
            loop {
                for _ in 0..BATCH_SIZE {
                    while consumer.pop().is_none() {
                        // busy loop until we pop an element
                    }
                }
                READS.fetch_add(BATCH_SIZE, Ordering::Relaxed);
            }
        })
        .unwrap();
}
