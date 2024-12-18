use std::net::IpAddr;

use agave_transaction_view::transaction_view::TransactionView;
use error::{PacketProcessorError, PacketProcessorResult};
use que::page_size::PageSize;
use solana_qos_common::{
    scored_transaction::ScoredTransaction, xxhash::xxHash,
};
use solana_sdk::{
    packet::{Packet, PACKET_DATA_SIZE},
    pubkey::Pubkey,
};

pub mod banking;
pub mod error;

pub use {
    qos_lru::LRUCache,
    qos_model::{
        interface::QoSModel, models::ip_signer::IpSignerModel,
    },
    solana_qos_common::{
        partial_meta::QoSPartialMeta, remaining_meta::QoSRemainingMeta,
        shared_stats::Stats, sig_bytes, u64_key, xxhash::xxHasher,
    },
    timer::Timer,
};

pub fn try_process_packet<
    const IPS: usize,
    const SIGNERS: usize,
    const CACHE_SIZE: usize,
    const SIG_CACHE_SIZE: usize,
>(
    packet: Packet,
    recent_signatures: Option<&LRUCache<u64, (), SIG_CACHE_SIZE>>,
    qos_model: &mut IpSignerModel<SIGNERS, IPS>,
    qos_tx_partial_metas: &mut LRUCache<
        xxHash,
        QoSPartialMeta,
        CACHE_SIZE,
    >,
    stats: &mut Stats,
    xxhasher: &xxHasher,
) -> PacketProcessorResult<ScoredTransaction> {
    // Increment total packets
    stats.total_packets += 1;

    // Try to fetch ip address
    let meta = packet.meta();
    let IpAddr::V4(ipv4) = meta.addr else {
        // Ignore non-ipv4 packets
        stats.non_ipv4 += 1;
        return Err(PacketProcessorError::AddrNotIpv4);
    };
    // Validate size
    if likely_stable::unlikely(meta.size > PACKET_DATA_SIZE) {
        stats.invalid_meta_size += 1;
        return Err(PacketProcessorError::InvalidMetadata);
    }

    // Try to parse packet
    let transaction = match packet
        .data(..)
        .map(TransactionView::try_new_unsanitized)
    {
        Some(Ok(unsanitized)) => {
            // Santize
            let Ok(sanitized) = unsanitized.sanitize() else {
                stats.failed_sanitize += 1;
                return Err(PacketProcessorError::FailedSanitize);
            };

            sanitized
        }
        Some(Err(_)) => {
            // Source is sending bad data. Reduce score
            qos_model.ip_feedback(ipv4.to_bits());
            stats.failed_view += 1;
            return Err(PacketProcessorError::FailedTransactionView);
        }
        None => {
            stats.invalid_packet_data += 1;
            return Err(PacketProcessorError::NonTransactionPacket);
        }
    };

    // Check to see if this tx has been recently processed
    let signature = &transaction.signatures()[0];
    let sig_key = u64_key(sig_bytes(signature));
    if recent_signatures.is_some_and(|rs| rs.contains(sig_key)) {
        stats.recently_processed += 1;
        return Err(PacketProcessorError::RecentlyProcessed);
    }

    // Get partial meta and calculate score
    let Some(fee_payer) = fee_payer(&transaction) else {
        stats.invalid_packet_data += 1;
        return Err(PacketProcessorError::InvalidMetadata);
    };
    let tx_fee = total_fee(&transaction);
    let partial_meta = QoSPartialMeta::new(
        &ipv4,
        fee_payer,
        tx_fee.total_fee,
        tx_fee.requested_cus,
    );
    let score =
        qos_model.forward(partial_meta.ip, &partial_meta.signer, &())
            * (partial_meta.total_fee as f64
                / partial_meta.cus.max(1) as f64);

    // Store partial meta
    let packet_key = xxhasher.packet_hash(&packet);
    match qos_tx_partial_metas.put(packet_key, partial_meta) {
        (Some((_packet_hash, partial_meta)), _) => {
            log::debug!(
                "partial meta LRU is full and packet from {:?} was dropped",
                partial_meta.ip.to_le_bytes()
            )
        }

        // Duplicate packet found in LRU.
        (None, true) => {
            stats.duplicate_packets += 1;
            return Err(PacketProcessorError::DuplicatePacket);
        }

        (None, false) => {
            // Nondup, no leak.
            // Proceed with transmitting to sigverify
        }
    }

    Ok(ScoredTransaction {
        score,
        sig_key,
        packet,
        ipv4: u32::from_le_bytes(ipv4.octets()),
    })
}

#[inline(always)]
fn fee_payer<'a>(
    view: &'a TransactionView<true, &'a [u8]>,
) -> Option<&'a Pubkey> {
    view.static_account_keys().get(0)
}

pub fn total_fee(
    view: &TransactionView<true, &[u8]>,
) -> CaveyTransactionFee {
    // Set cu limit & price ix discriminator
    const SET_CU_LIMIT: u8 = 0x2;
    const SET_CU_PRICE: u8 = 0x3;

    let static_account_keys = view.static_account_keys();

    let mut requested_cus = None;
    let mut cu_price = None;
    let mut duplicate = false;

    // We only check these 8 for strictly bound compute
    let first_ixs = view.instructions_iter().take(8);

    first_ixs
        .filter(|ix| {
            static_account_keys.get(ix.program_id_index as usize)
                == Some(&solana_sdk::compute_budget::ID)
        })
        .for_each(|cbi /* compute budget instruction */| {
            if cbi.data.len() == 5 && cbi.data[0] == SET_CU_LIMIT {
                if requested_cus.is_some() {
                    duplicate = true;
                } else {
                    requested_cus = Some(unsafe {
                        core::ptr::read_unaligned::<u32>(
                            cbi.data.as_ptr().add(1) as *const u32,
                        )
                    });
                }
            } else if cbi.data.len() == 9 && cbi.data[0] == SET_CU_PRICE
            {
                if cu_price.is_some() {
                    duplicate = true;
                } else {
                    cu_price = Some(unsafe {
                        core::ptr::read_unaligned::<u64>(
                            cbi.data.as_ptr().add(1) as *const u64,
                        )
                    });
                }
            }
        });

    // Calculate signature cost
    let signature_cost = 5000 * view.signatures().len() as u64;

    // Use read values with default fallback
    let requested_cus = requested_cus.unwrap_or(200_000);
    let cu_price = cu_price.unwrap_or(0);

    // Calculate total fee
    let total_fee = signature_cost
        + u128::min(
            requested_cus as u128 * cu_price as u128 / 1_000_000,
            u64::MAX as u128,
        ) as u64;

    CaveyTransactionFee {
        total_fee,
        cu_price,
        requested_cus,
    }
}

#[repr(C)]
pub struct CaveyTransactionFee {
    pub cu_price: u64,
    pub total_fee: u64,
    pub requested_cus: u32,
}

#[cfg(target_os = "linux")]
pub fn get_page_size(use_huge_pages: bool) -> PageSize {
    use_huge_pages
        .then_some(PageSize::Huge)
        .unwrap_or(PageSize::Standard)
}

#[cfg(not(target_os = "linux"))]
pub fn get_page_size() -> PageSize {
    PageSize::Standard
}
