use crate::{QoSTransactionMeta, F32};

// TODO. For now, just ip4 and signer
pub type ParsedPacket = (u32, [u8; 32]);

pub trait QoSModel {
    type AdditionalTransactionMeta;
    type AdditionalUpdateMeta;
    fn forward(&self, parsed_packet: ParsedPacket) -> F32;
    fn update_model<'a>(
        &'a mut self,
        transactions: impl Iterator<Item = &'a QoSTransactionMeta<Self::AdditionalTransactionMeta>>,
        update_meta: Self::AdditionalUpdateMeta,
    );
}
