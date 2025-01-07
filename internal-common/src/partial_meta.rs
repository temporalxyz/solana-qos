use std::net::Ipv4Addr;

use super::transaction_meta::{QoSTransactionMeta, F64};
use bytemuck::Pod;
use solana_qos_common::remaining_meta::QoSRemainingMeta;
use solana_sdk::pubkey::Pubkey;

/// The subset of metadata available prior to sigverify and execution
pub struct QoSPartialMeta {
    pub ip: u32,
    pub signer: [u8; 32],
    pub total_fee: u64,
    pub cus: u32,
}

impl QoSPartialMeta {
    #[inline(always)]
    pub fn new(
        ip: &Ipv4Addr,
        signer: &Pubkey,
        total_fee: u64,
        cus: u32,
    ) -> QoSPartialMeta {
        QoSPartialMeta {
            ip: u32::from_le_bytes(ip.octets()),
            signer: signer.to_bytes(),
            total_fee,
            cus,
        }
    }

    #[inline(always)]
    pub fn merge<A: Pod>(
        self,
        remaining_meta: QoSRemainingMeta<A>,
    ) -> QoSTransactionMeta<A> {
        let execution_nanos = if remaining_meta.execution_nanos == 0 {
            // TODO: Fixed constant
            // Value non-included transactions at 100 micros
            100_000
        } else {
            remaining_meta.execution_nanos
        };

        QoSTransactionMeta {
            ip: self.ip,
            signer: self.signer,
            value: F64::from(
                self.total_fee as f64 / execution_nanos as f64,
            ),
            additional_metadata: remaining_meta.additional_metadata,
        }
    }
}
