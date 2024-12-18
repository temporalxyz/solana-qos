use timer::Timer;

fn main() {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Timer::memoize_ticks_per_ms_and_invariant_tsc_check();

    let timer = Timer::new();

    #[allow(deprecated)]
    std::thread::sleep_ms(1000);

    println!("timer measured {} ms", timer.elapsed_ms());
}
