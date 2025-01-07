use std::{
    error::Error,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{channel, Receiver, Sender},
    },
    time::Duration,
};

use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    widgets::Paragraph,
    DefaultTerminal, Frame,
};
use solana_qos_common::packet_bytes::{PacketBytes, PACKET_SIZE};
use timer::Timer;

static WRITES: AtomicUsize = AtomicUsize::new(0);
static READS: AtomicUsize = AtomicUsize::new(0);

fn main() {
    // Create channel
    let (producer, consumer) = channel();

    // Pass the pointer to the producer and consumer.
    // The producer will initialize the SPSC so that the consumer can join.
    unsafe {
        start_producer(producer);
        start_consumer(consumer);
    }

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
             │     {wps:.1}M/s     │─────►│     {rps:.1}M/s     │
             │    {wbps:.1}Gbps    │      │    {rbps:.1}Gbps    │
             └────────────────┘      └────────────────┘
    
                             std channel
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
unsafe fn start_producer(producer: Sender<PacketBytes>) {
    std::thread::Builder::new()
        .name("producer".to_string())
        .spawn(move || {
            let packet = PacketBytes::new([42; PACKET_SIZE]);
            const BATCH_SIZE: usize = 1024;
            loop {
                for _ in 0..BATCH_SIZE {
                    producer.send(packet.clone()).unwrap();
                }
                WRITES.fetch_add(BATCH_SIZE, Ordering::Relaxed);
            }
        })
        .unwrap();
}

// Read packet in a loop
fn start_consumer(consumer: Receiver<PacketBytes>) {
    std::thread::Builder::new()
        .name("consumer".to_string())
        .spawn(move || {
            const BATCH_SIZE: usize = 1024;
            loop {
                for _ in 0..BATCH_SIZE {
                    let _ = consumer.recv().unwrap();
                }
                READS.fetch_add(BATCH_SIZE, Ordering::Relaxed);
            }
        })
        .unwrap();
}
