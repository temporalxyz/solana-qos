use bytemuck::{Pod, Zeroable};

/// This type is introduced as an intermediate packet representation
/// to avoid adding new trait implementations to [Packet]. The
/// intermediate representation is sent across the `que` spsc channel.
#[derive(Clone, Copy)]
#[repr(C, align(8))]
pub struct PacketBytes([u8; PACKET_SIZE]);

unsafe impl Pod for PacketBytes {}
unsafe impl Zeroable for PacketBytes {}

pub const PACKET_SIZE: usize = 1264;

impl PacketBytes {
    #[inline(always)]
    pub fn new(bytes: [u8; PACKET_SIZE]) -> PacketBytes {
        PacketBytes(bytes)
    }
}
impl Default for PacketBytes {
    #[inline(always)]
    fn default() -> PacketBytes {
        PacketBytes([0; PACKET_SIZE])
    }
}
