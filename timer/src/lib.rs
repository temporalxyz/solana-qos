use core::arch::x86_64::_rdtsc;
use std::time::Instant;

#[derive(Clone, Debug)]
pub enum Timer {
    RDTSC(u64),
    Instant(Instant),
}

impl Timer {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn memoize_ticks_per_ms_and_invariant_tsc_check() {
        check_cpu_supports_invariant_tsc();
        ticks_per_ms();
    }

    pub fn new() -> Self {
        if cfg!(any(target_arch = "x86", target_arch = "x86_64"))
            && check_cpu_supports_invariant_tsc()
        {
            Timer::RDTSC(unsafe { _rdtsc() })
        } else {
            Timer::Instant(Instant::now())
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        match self {
            Timer::RDTSC(rdtsc) => {
                (unsafe { _rdtsc() - rdtsc } / ticks_per_ms())
            }
            Timer::Instant(_) => {
                unimplemented!("Instant timer disabled for now")
            }
        }
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn check_cpu_supports_invariant_tsc() -> bool {
    use std::sync::OnceLock;
    static SUPPORTS_INVARIANT_TSC: OnceLock<bool> = OnceLock::new();

    *SUPPORTS_INVARIANT_TSC.get_or_init(|| {
        let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo")
        else {
            return false;
        };

        let has_constant_tsc = cpuinfo.contains("constant_tsc");
        let has_nonstop_tsc = cpuinfo.contains("nonstop_tsc");

        has_constant_tsc && has_nonstop_tsc
    })
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn ticks_per_ms() -> u64 {
    use std::{sync::OnceLock, time::Duration};

    static TICKS_PER_MS: OnceLock<u64> = OnceLock::new();

    *TICKS_PER_MS.get_or_init(|| {
        let warm_up_duration = Duration::from_millis(1000);
        let measurement_duration = Duration::from_millis(1000);

        // Warm up
        let warm_up_start = Instant::now();
        while warm_up_start.elapsed() < warm_up_duration {
            // Spin
        }

        let start = Instant::now();
        let start_tsc = unsafe { core::arch::x86_64::_rdtsc() };

        // Measure
        while Instant::now().duration_since(start)
            < measurement_duration
        {
            // Spin
        }

        let end_tsc = unsafe { core::arch::x86_64::_rdtsc() };
        let elapsed_tsc = end_tsc - start_tsc;

        let duration_ms = measurement_duration.as_secs_f64() * 1000.0;
        let tsc_per_ms = elapsed_tsc as f64 / duration_ms;

        tsc_per_ms as u64
    })
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn ticks_per_ms() -> u64 {
    unimplemented!("rdtsc only used on x86")
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn check_cpu_supports_invariant_tsc() -> bool {
    false
}
