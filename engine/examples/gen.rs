use mock_tx_engine::agent::{
    null_transfer_transaction_with_compute_unit_price, Ip,
};
use timer::Timer;

fn main() {
    Timer::memoize_ticks_per_ms_and_invariant_tsc_check();

    // This has some signers associated with it
    let ipv4 = [0, 0, 0, 0];
    let log_mean = 1_000_000.0_f32.ln();
    let std = 3.0;
    let mut ip = Ip::new(ipv4, log_mean, std);
    let mut packet =
        null_transfer_transaction_with_compute_unit_price();

    let timer = Timer::new();
    for _ in 0..1_000_000 {
        // Replace packet data
        ip.update_transfer_with_priority(&mut packet);
    }
    println!("done in {} millis", timer.elapsed_ms());
}
