use xxhash_rust::{
    const_xxh3::const_custom_default_secret, xxh3::xxh3_64_with_secret,
};

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct xxHasher {
    secret: [u8; 192],
}

#[allow(non_camel_case_types)]
pub type xxHash = u64;

impl xxHasher {
    #[inline(always)]
    pub const fn initialize_with_seed(seed: u64) -> xxHasher {
        xxHasher {
            secret: const_custom_default_secret(seed),
        }
    }

    #[inline(always)]
    pub fn hash(&self, input: &[u8]) -> u64 {
        xxh3_64_with_secret(input, &self.secret)
    }
}
