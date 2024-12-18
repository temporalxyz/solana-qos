use bytemuck::{Pod, Zeroable};
use solana_sdk::packet::Packet;

/// This type is introduced as an intermediate packet representation
/// to avoid adding new trait implementations to [Packet]. The
/// intermediate representation is sent across the `que` spsc channel.
#[derive(Clone, Copy)]
#[repr(C, align(8))]
pub struct PacketBytes([u8; PACKET_SIZE]);

unsafe impl Pod for PacketBytes {}
unsafe impl Zeroable for PacketBytes {}

pub const PACKET_SIZE: usize = core::mem::size_of::<Packet>();

impl PacketBytes {
    #[inline(always)]
    pub fn from_packet<'a>(packet: &'a Packet) -> &'a PacketBytes {
        unsafe {
            core::mem::transmute::<&'a Packet, &'a PacketBytes>(&packet)
        }
    }

    /// SAFETY:
    /// Given that the bytes are guaranteed to be aligned, this does not
    /// introduce any UB. However a malformed packet could have the
    /// wrong meta.size, which can result in oob access down the
    /// line. This is, however, not the responsibility of this function,
    /// so this is safe. Packet data should be validated prior to
    /// transmutation into [PacketBytes].
    #[inline(always)]
    pub fn as_packet(self) -> Packet {
        unsafe { core::mem::transmute::<PacketBytes, Packet>(self) }
    }

    #[inline(always)]
    pub fn new(bytes: [u8; PACKET_SIZE]) -> PacketBytes {
        PacketBytes(bytes)
    }
}

/// [PacketBytes] is 8 byte aligned because it expects [Packet] to be 8
/// byte aligned. If [Packet] changes in the future in a way that
/// reduces alignment, it can still be 8 byte aligned as alignment
/// requirements are expressed in powers of two. This assertion will
/// catch the unlikely case of the alignment of [Packet] increasing.
const _ASSERT_PROPER_ALIGNMENT: () =
    assert!(core::mem::align_of::<Packet>() <= 8);

impl Default for PacketBytes {
    #[inline(always)]
    fn default() -> PacketBytes {
        PacketBytes([0; PACKET_SIZE])
    }
}
