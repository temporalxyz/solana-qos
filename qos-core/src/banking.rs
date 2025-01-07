use qos_lru::LRUCache;
use qos_minmax::MinMaxHeap;
use que::headless_spmc::producer::Producer as QueProducer;
use solana_qos_common::{
    ipc_parameters::IPC_QOS_TO_SIG_CAP, packet_bytes::PacketBytes,
};
use timer::Timer;

use crate::{ScoredTransaction, Stats};

/// Stores and prioritizes scored transactions, and periodically
/// transmits them to the sigverify stage.
pub struct TransactionContainer {
    /// The value 16384 was determined based on a benchmark that pushed
    /// values when full. Benchmark reached ≈16 Gbps on a Intel(R)
    /// Xeon(R) Gold 5218N CPU using random 1232 byte entries.
    pub priority_queue_heap: MinMaxHeap<ScoredTransaction, 16384>,

    /// This sends over scored and prioritized transactions over to be
    /// sigverified and scheduled.
    pub transmitter:
        Option<QueProducer<PacketBytes, IPC_QOS_TO_SIG_CAP>>,

    /// Records the time the transmitter last sent a batch of high
    /// priority transactions to the sigverify stage.
    last_send: Timer,

    /// Max number of packets per loop to transmit to banking stage
    max_send: usize,
}

impl TransactionContainer {
    const SEND_INTERVAL_MS: usize = 100;
    pub fn new(
        transmitter: Option<
            QueProducer<PacketBytes, IPC_QOS_TO_SIG_CAP>,
        >,
        target_pps: usize,
    ) -> TransactionContainer {
        TransactionContainer {
            transmitter,
            priority_queue_heap: MinMaxHeap::new(),
            last_send: Timer::new(),
            max_send: target_pps * Self::SEND_INTERVAL_MS / 1000,
        }
    }

    pub fn beat(&self) {
        if let Some(ref transmitter) = self.transmitter {
            transmitter.beat();
        }
    }

    pub fn queue(
        &mut self,
        scored_transaction: ScoredTransaction,
        stats: &mut Stats,
    ) {
        // Add transaction to queue.
        // If full, this internally evicts lowest priority transaction
        let is_full = self
            .priority_queue_heap
            .push(scored_transaction)
            .is_some();

        if is_full {
            stats.leaked_priority += 1;
        }
    }

    pub fn maybe_retrieve<'a>(
        &'a mut self,
        stats: &'a mut Stats,
        recent_signatures: Option<
            &'a LRUCache<u64, (), { 1024 * 1024 }>,
        >,
    ) -> Option<impl Iterator<Item = ScoredTransaction> + 'a> {
        // Check to see if it's been a while since we've sent to
        // sigverify
        let send_tick = self.last_send.elapsed_ms()
            >= Self::SEND_INTERVAL_MS as u64;

        if send_tick {
            // Construct priority queue iterator for high
            // priority transactions
            let high_prio_iterator = self
                .priority_queue_heap
                .get_max_values()
                .filter(move |tx| {
                    if recent_signatures
                        .is_some_and(|rs| rs.contains(tx.sig_key))
                    {
                        stats.recently_processed_queued += 1;
                        false
                    } else {
                        true
                    }
                })
                .take(self.max_send);

            self.last_send = Timer::new();

            Some(high_prio_iterator)
        } else {
            None
        }
    }

    /// Panics if there is no transmitter!
    ///
    /// If using maybe transmit, you are probably running alongside validator so recent_signatures is not an Option
    pub fn maybe_transmit<'a>(
        &'a mut self,
        stats: &'a mut Stats,
        recent_signatures: &'a LRUCache<u64, (), { 1024 * 1024 }>,
    ) {
        if let Some(ref mut tx_mut) = self.transmitter {
            // Check to see if it's been a while since we've sent to
            // sigverify
            let send_tick = self.last_send.elapsed_ms()
                >= Self::SEND_INTERVAL_MS as u64;

            if send_tick {
                // Construct priority queue iterator for high
                // priority transactions
                let high_prio_iterator = self
                    .priority_queue_heap
                    .get_max_values()
                    .filter(|tx| {
                        if recent_signatures.contains(tx.sig_key) {
                            stats.recently_processed_queued += 1;
                            false
                        } else {
                            true
                        }
                    })
                    .take(self.max_send);

                // Send batch
                let mut sent = 0_usize;
                for transaction in high_prio_iterator {
                    tx_mut.push(transaction.packet_bytes());
                    sent += 1;
                }

                // Sync the tail to publish the batch
                tx_mut.sync();

                // Update last send attempt time
                if sent > 0 {
                    stats.banking_transmissions += sent;
                    self.last_send = Timer::new();
                }
            }
        } else {
            panic!("NO TRANSMITTER")
        }
    }
}
