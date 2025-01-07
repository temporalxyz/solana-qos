pub mod interface;
pub mod models;

use bytemuck::{Pod, Zeroable};
use ordered_float::OrderedFloat;
use solana_qos_internal_common::transaction_meta::F64;

pub const ONE: F64 = OrderedFloat(1.0);
pub const ZERO: F64 = OrderedFloat(0.0);

macro_rules! declare_inverse_score_entry {
    ($name:tt, $field:ident, $type:ty, $pad:literal) => {
        #[derive(
            Default,
            Debug,
            Zeroable,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Pod,
        )]
        #[repr(C)]
        pub struct $name {
            // Score must go first for correct PartialOrd
            pub score: F64,
            pub $field: $type,
            pad: [u8; $pad],
        }

        impl $name {
            const _ASSERT_NO_PAD: () = assert!(
                size_of::<$type>() + size_of::<F64>()
                    == size_of::<Self>()
            );
            const _ASSERT_SOKOBAN_SIZE_REQUIREMENT: () =
                assert!(size_of::<Self>() % 8 == 0);

            #[inline(always)]
            pub fn new(score: F64, $field: $type) -> Self {
                $name {
                    score,
                    $field,
                    pad: [0; $pad],
                }
            }
        }
    };
}

declare_inverse_score_entry!(InverseScoreEntryIp, ip, u32, 4);

declare_inverse_score_entry!(
    InverseScoreEntrySigner,
    signer,
    [u8; 32],
    0
);
