//! Non-cryptographically secure fast rng based on xxhash
use rand::RngCore;
use xxhash_rust::{
    const_xxh3::{self, const_custom_default_secret},
    xxh3::xxh3_64_with_secret,
};

const UNSAFE_SECRET_SEED: u64 = u64::from_le_bytes(*b"xUNSAFEx");
const UNSAFE_SECRET: [u8; 192] =
    const_custom_default_secret(UNSAFE_SECRET_SEED);

pub struct FastxxHashRng {
    state: u64,
}
impl FastxxHashRng {
    pub const fn new(seed: u64) -> FastxxHashRng {
        let seed_as_bytes =
            unsafe { core::mem::transmute::<&u64, &[u8; 8]>(&seed) };

        FastxxHashRng {
            state: const_xxh3::xxh3_64_with_secret(
                seed_as_bytes,
                &UNSAFE_SECRET,
            ),
        }
    }
}

impl RngCore for FastxxHashRng {
    fn next_u32(&mut self) -> u32 {
        // We increment in the rare case there is a loop
        let state_as_bytes = unsafe {
            core::mem::transmute::<&u64, &[u8; 8]>(&self.state)
        };
        let old_state = self.state;
        self.state =
            xxh3_64_with_secret(state_as_bytes, &UNSAFE_SECRET);

        // Very rare edge case here
        if old_state == self.state {
            println!("extremely rare edge cyclic case hit");
            self.state = self.state.rotate_right(32);
        }

        // Take lower bits
        self.state as u32
    }

    fn next_u64(&mut self) -> u64 {
        let state_as_bytes = unsafe {
            core::mem::transmute::<&u64, &[u8; 8]>(&self.state)
        };
        self.state =
            xxh3_64_with_secret(state_as_bytes, &UNSAFE_SECRET);
        self.state
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut bytes_written = 0;

        let mut bytes_remaining = dest.len() - bytes_written;
        while bytes_remaining != 0 {
            // Since we are sourcing from rand u64s, we need to write at most 8 bytes at a time
            let bytes_to_write = bytes_remaining.min(8);

            unsafe {
                let new_bytes: &[u8; 8] =
                    core::mem::transmute::<&u64, &[u8; 8]>(
                        &self.next_u64(),
                    );

                core::ptr::copy_nonoverlapping(
                    new_bytes.as_ptr(),
                    dest.as_mut_ptr().add(bytes_written),
                    bytes_to_write,
                );
            }

            bytes_written += bytes_to_write;
            bytes_remaining -= bytes_to_write;
        }
    }

    fn try_fill_bytes(
        &mut self,
        dest: &mut [u8],
    ) -> Result<(), rand::Error> {
        self.fill_bytes(dest);

        Ok(())
    }
}
