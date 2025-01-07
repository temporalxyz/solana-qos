use solana_qos_common::packet_bytes::PacketBytes;
use solana_sdk::packet::Packet;

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
pub fn as_packet(packet_bytes: PacketBytes) -> Packet {
    unsafe { core::mem::transmute::<PacketBytes, Packet>(packet_bytes) }
}

/// [PacketBytes] is 8 byte aligned because it expects [Packet] to be 8
/// byte aligned. If [Packet] changes in the future in a way that
/// reduces alignment, it can still be 8 byte aligned as alignment
/// requirements are expressed in powers of two. This assertion will
/// catch the unlikely case of the alignment of [Packet] increasing.
const _ASSERT_PROPER_ALIGNMENT: () =
    assert!(core::mem::align_of::<Packet>() <= 8);
