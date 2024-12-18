use crate::{packet_bytes::PacketBytes, transaction_meta::F64};
use derivative::Derivative;
use solana_sdk::packet::Packet;

#[derive(Debug, Derivative, PartialEq, Eq, Clone)]
#[derivative(PartialOrd, Ord)]

pub struct ScoredTransaction {
    pub score: F64,

    #[derivative(PartialOrd = "ignore", Ord = "ignore")]
    pub sig_key: u64,

    #[derivative(PartialOrd = "ignore", Ord = "ignore")]
    pub packet: Packet,

    #[derivative(PartialOrd = "ignore", Ord = "ignore")]
    pub ipv4: u32,
}

impl ScoredTransaction {
    #[inline(always)]
    pub fn packet_bytes(&self) -> &PacketBytes {
        PacketBytes::from_packet(&self.packet)
    }
}
