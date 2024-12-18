use mock_tx_engine::STATS;
use que::shmem::Shmem;
use solana_qos_common::shared_stats::SharedStats;
use timer::Timer;

pub struct FlowGraph;

impl FlowGraph {
    pub fn run(shared_stats_shmem: Shmem) {
        color_eyre::install().unwrap();
        let terminal = ratatui::init();
        run(terminal, shared_stats_shmem)
            .context("app loop failed")
            .unwrap();
        ratatui::restore();
    }
}

use std::time::Duration;

use color_eyre::{eyre::Context, Result};
use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    widgets::Paragraph,
    DefaultTerminal, Frame,
};

fn run(
    mut terminal: DefaultTerminal,
    shared_stats: Shmem,
) -> Result<()> {
    // Reset to synchronize counter with engine timer
    unsafe { SharedStats::reset(shared_stats.get_mut_ptr()) };

    Timer::memoize_ticks_per_ms_and_invariant_tsc_check();
    let timer = Timer::new();
    loop {
        let [tpu_sends, fwd_sends, sigverify_recvs, sigverify_sends, scheduler_sends] =
            STATS.load();

        let draw_fn = unsafe {
            draw(
                timer.clone(),
                tpu_sends,
                fwd_sends,
                sigverify_recvs,
                sigverify_sends,
                scheduler_sends,
                shared_stats.get_mut_ptr(),
            )
        };
        terminal.draw(draw_fn)?;
        if should_quit()? {
            break;
        }
    }
    Ok(())
}

unsafe fn draw(
    timer: Timer,
    tpu_sends: usize,
    fwd_sends: usize,
    sigverify_recvs: usize,
    sigverify_sends: usize,
    scheduler_sends: usize,
    shared_stats: *mut u8,
) -> impl FnOnce(&mut Frame) {
    move |frame: &mut Frame| {
        let time = timer.elapsed_ms() as f64 / 1000.0;
        let tpu = tpu_sends as f64 / time / 1e6;
        let fwd = fwd_sends as f64 / time / 1e6;
        let sig = sigverify_recvs as f64 / time / 1e6;
        let sigf = sigverify_sends as f64 / time / 1e6;
        let ss =
            (sigverify_recvs - sigverify_sends) as f64 / time / 1e6;
        let sch = scheduler_sends as f64 / time / 1e6;
        let not_sch = (sigverify_recvs
            - sigverify_sends
            - scheduler_sends) as f64
            / time
            / 1e6;

        let qos_stats = SharedStats::read(shared_stats);
        // All forms of dedup.
        // 1. we've seen twice (not yet processed, perhaps invalid or expired)
        // 2. recently processed sig
        // 3. recently processed sig post-buffer
        let dedup = (qos_stats.duplicate_packets
            + qos_stats.recently_processed
            + qos_stats.recently_processed_queued)
            as f64
            / time
            / 1e6;
        let invalid = (qos_stats.failed_sanitize
            + qos_stats.failed_view
            + qos_stats.invalid_meta_size
            + qos_stats.invalid_packet_data
            + qos_stats.non_ipv4
            + qos_stats.non_transaction_packet)
            as f64
            / time
            / 1e6;
        let leaked = qos_stats.leaked_priority as f64 / time / 1e6;
        let total = qos_stats.total_packets as f64 / time / 1e6;

        let diagram = format!(
            r"
        ┌─────┐                        ┌──────────────┐
        │ TPU ├──────{tpu:2.03}M/s─────────►│     QOS      │
        └─────┘                        │              │
        ┌─────┐                        │              │
        │ FWD ├──────{fwd:2.03}M/s─────────►│              │
        └─────┘                        │              │
                                       │              │
                                       │              │ ────> dedup {dedup:.03}M/s
                                       │              │ ──> invalid {invalid:.03}M/s
                                       │              │
        ┌─────┐◄─────{sig:.03}M/s──────────│    total     │ ───> leaked {leaked:.03}M/s
        │ SIG │                        │   {total:.3}M/s   │
        └─────┘──────{sigf:.03}M/s─────────►│              │
           │                           │              │
        {ss:.03}M/s                       │              │
           │                           │              │
           │                           │              │
           │                           │              │
           ▼                           │              │
        ┌─────┐──────{not_sch:.03}M/s─────────►│              │
        │ SCH │                        │              │
        └─────┘──────{sch:.03}M/s─────────►│              │
                                       └──────────────┘

                               Solana QoS
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
    }
}

fn should_quit() -> Result<bool> {
    if event::poll(Duration::from_millis(100))
        .context("event poll failed")?
    {
        if let Event::Key(key) =
            event::read().context("event read failed")?
        {
            return Ok(KeyCode::Char('q') == key.code);
        }
    }
    Ok(false)
}
