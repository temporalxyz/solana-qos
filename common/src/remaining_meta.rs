use bytemuck::{AnyBitPattern, Pod, Zeroable};

use crate::xxhash::xxHash;

#[derive(Debug, Clone, Copy, Zeroable)]
#[repr(C, align(8))]
#[cfg_attr(test, derive(PartialEq))]
pub struct QoSRemainingMeta<A: Sized + AnyBitPattern> {
    /// The xx3 hash of the packet bytes associated with this
    /// transaction. Used as the LRU cache key.
    pub packet_hash: xxHash,

    /// Execution time (zero if not scheduled)
    pub execution_nanos: u64,

    /// Additional metadata (model-specific)
    pub additional_metadata: A,
}

unsafe impl<A: Pod + AnyBitPattern> Pod for QoSRemainingMeta<A> {}

impl<A: Pod + AnyBitPattern> QoSRemainingMeta<A> {
    pub const SIZE: usize = core::mem::size_of::<Self>();
    pub const _ASSERT_ALIGN: () =
        assert!(core::mem::align_of::<A>() <= 8);

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY:
        //
        // A is Pod with align <= 8
        unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                Self::SIZE,
            )
        }
    }

    /// This assumes size and alignment are correct!
    #[inline(always)]
    pub unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // SAFETY:
        //
        // A is Pod with align <= 8
        unsafe { &*bytes.as_ptr().cast() }
    }
}

#[test]
fn round_trip_several_variants() {
    // Unit
    let remaining_meta = QoSRemainingMeta {
        packet_hash: 3_u64,
        execution_nanos: 123,
        additional_metadata: (),
    };
    assert_eq!(&remaining_meta, unsafe {
        QoSRemainingMeta::from_bytes_unchecked(
            remaining_meta.as_bytes(),
        )
    });

    // Primitive
    let remaining_meta = QoSRemainingMeta {
        packet_hash: 3_u64,

        execution_nanos: 123,
        additional_metadata: 0x69_u64,
    };
    assert_eq!(&remaining_meta, unsafe {
        QoSRemainingMeta::from_bytes_unchecked(
            remaining_meta.as_bytes(),
        )
    });

    // User defined
    #[derive(Pod, Clone, Copy, Zeroable, PartialEq, Debug)]
    #[repr(C, packed)]
    pub struct MyType {
        foo: u64,
        bar: u16,
    }
    let remaining_meta = QoSRemainingMeta {
        packet_hash: 3_u64,
        execution_nanos: 123,
        additional_metadata: MyType {
            foo: 0x69_u64,
            bar: 0x420_u16,
        },
    };
    assert_eq!(&remaining_meta, unsafe {
        QoSRemainingMeta::from_bytes_unchecked(
            remaining_meta.as_bytes(),
        )
    });
}
