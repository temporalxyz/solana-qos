use crate::{
    interface::QoSModel, InverseScoreEntryIp, InverseScoreEntrySigner,
    ONE,
};

use ordered_float::OrderedFloat;
use sokoban::{NodeAllocatorMap, RedBlackTree};
use solana_qos_internal_common::transaction_meta::{
    QoSTransactionMeta, F64,
};

use std::{
    borrow::Borrow, collections::BTreeMap, io::Write, net::Ipv4Addr,
};

impl<const MAX_SIGNERS: usize, const MAX_IPS: usize> QoSModel
    for IpSignerModel<MAX_SIGNERS, MAX_IPS>
{
    type AdditionalArgs = ();
    type AdditionalTransactionMeta = ();
    type AdditionalUpdateMeta = ();
    fn forward(
        &self,
        ip: u32,
        signer: &[u8; 32],
        _args: &Self::AdditionalArgs,
    ) -> F64 {
        self._forward(ip, signer)
    }

    fn update_model<'a>(
        &'a mut self,
        transactions: impl Iterator<Item = &'a QoSTransactionMeta<()>>,
        _update_meta: Self::AdditionalUpdateMeta,
    ) {
        self.update_model(transactions, 5, 5)
    }

    /// The ip that sent in a transaction with invalid signature
    type IpFeedback = u32;
    fn ip_feedback(&mut self, ip: Self::IpFeedback) {
        if let Some(score) = self.ip_score.get_mut(&ip) {
            // First update score in inverse map
            self.ip_score_inverse
                .remove(&InverseScoreEntryIp::new(*score, ip));
            self.ip_score_inverse.insert(
                InverseScoreEntryIp::new(*score * 0.01, ip),
                (),
            );

            // Then update score in map
            **score *= 0.01;
        }
    }
}

pub struct IpSignerModel<const MAX_SIGNERS: usize, const MAX_IPS: usize>
{
    signer_score: RedBlackTree<[u8; 32], F64, MAX_SIGNERS>,
    ip_score: RedBlackTree<u32, F64, MAX_IPS>,

    signer_score_inverse:
        RedBlackTree<InverseScoreEntrySigner, (), MAX_SIGNERS>,
    ip_score_inverse: RedBlackTree<InverseScoreEntryIp, (), MAX_IPS>,
}

impl<const MAX_SIGNERS: usize, const MAX_IPS: usize>
    IpSignerModel<MAX_SIGNERS, MAX_IPS>
{
    pub fn new(
        ip_scores: impl IntoIterator<Item = (u32, f64)>,
        signer_scores: impl IntoIterator<Item = ([u8; 32], f64)>,
    ) -> IpSignerModel<MAX_SIGNERS, MAX_IPS> {
        let mut signer_score = RedBlackTree::new();
        let mut signer_score_inverse = RedBlackTree::new();
        for (signer, score) in signer_scores {
            signer_score.insert(signer, F64::from(score));
            signer_score_inverse.insert(
                InverseScoreEntrySigner::new(F64::from(score), signer),
                (),
            );
        }

        let mut ip_score = RedBlackTree::new();
        let mut ip_score_inverse = RedBlackTree::new();
        for (ip, score) in ip_scores {
            ip_score.insert(ip, F64::from(score));
            ip_score_inverse.insert(
                InverseScoreEntryIp::new(F64::from(score), ip),
                (),
            );
        }

        IpSignerModel {
            signer_score,
            ip_score,
            signer_score_inverse,
            ip_score_inverse,
        }
    }

    /// Returns combined score for this ip + signer.
    /// Panics if there are no scores!
    pub fn _forward(&self, ip: u32, signer: &[u8; 32]) -> F64 {
        // Get scores
        //
        // We use median score for null queries because that is the most
        // neutral score. Recall that pruning removes elements
        // close to the median, leaving the most discriminating
        // scores (i.e. least and most valuable sources).
        let ip_score = self
            .ip_score
            .get(&ip)
            .copied()
            .unwrap_or_else(|| self.approximate_median_ip_score());
        let signer_score = self
            .signer_score
            .get(signer)
            .copied()
            .unwrap_or_else(|| self.approximate_median_signer_score());

        ip_score * signer_score
    }

    fn approximate_median_ip_score(&self) -> F64 {
        if self.ip_score_inverse.is_empty() {
            ONE
        } else {
            self.ip_score_inverse
                .get_node(self.ip_score_inverse.root)
                .key
                .score
        }
    }

    fn approximate_median_signer_score(&self) -> F64 {
        if self.signer_score_inverse.is_empty() {
            ONE
        } else {
            self.signer_score_inverse
                .get_node(self.signer_score_inverse.root)
                .key
                .score
        }
    }

    /// Prunes from the middle of tables, keeping most valuable and
    /// least valuable signers and ips
    pub fn prune(&mut self, num_ips: usize, num_signers: usize) {
        // Prune ips
        let ips_to_delete = self
            .ip_score_inverse
            .len()
            .saturating_sub(num_ips);
        for _ in 0..ips_to_delete {
            let root_node = self
                .ip_score_inverse
                .remove_root()
                .unwrap();
            self.ip_score.remove(&root_node.key.ip);
        }

        // Prune signers
        let signers_to_delete = self
            .signer_score_inverse
            .len()
            .saturating_sub(num_signers);
        for _ in 0..signers_to_delete {
            let root_node = self
                .signer_score_inverse
                .remove_root()
                .unwrap();
            self.signer_score
                .remove(&root_node.key.signer);
        }
    }

    pub fn add_ip_score(&mut self, ip: u32, score: F64) {
        // Remove if score for ip exists already
        if let Some(score) = self.ip_score.remove(&ip) {
            self.ip_score_inverse
                .remove(&InverseScoreEntryIp::new(score, ip));
        }

        // Add score
        self.ip_score.insert(ip, score);
        self.ip_score_inverse
            .insert(InverseScoreEntryIp::new(score, ip), ());
    }

    pub fn add_signer_score(&mut self, signer: [u8; 32], score: F64) {
        // Remove if score for signer exists already
        if let Some(score) = self.signer_score.remove(&signer) {
            self.signer_score_inverse
                .remove(&InverseScoreEntrySigner::new(score, signer));
        }

        // Add score
        self.signer_score.insert(signer, score);
        self.signer_score_inverse
            .insert(InverseScoreEntrySigner::new(score, signer), ());
    }

    pub fn update_model<'a>(
        &'a mut self,
        transactions: impl IntoIterator<
            Item = impl Borrow<QoSTransactionMeta<()>>,
        >,
        prune_signers: usize,
        prune_ips: usize,
    ) {
        struct ScoreUpdateCandidate {
            score_sum: F64,
            count: u32,
        }
        impl ScoreUpdateCandidate {
            pub fn new(score: F64) -> ScoreUpdateCandidate {
                ScoreUpdateCandidate {
                    score_sum: score,
                    count: 1,
                }
            }

            pub fn update(&mut self, score: F64) {
                self.score_sum += score;
                self.count += 1;
            }

            pub fn finalize(&self) -> F64 {
                self.score_sum / F64::from(self.count as f64)
            }
        }

        let mut signer_score_candidates =
            BTreeMap::<[u8; 32], ScoreUpdateCandidate>::new();
        let mut ip_score_candidates =
            BTreeMap::<u32, ScoreUpdateCandidate>::new();
        for transaction in transactions {
            let &QoSTransactionMeta {
                ip,
                signer,
                value: score,
                additional_metadata: _,
            } = transaction.borrow();

            signer_score_candidates
                .entry(signer)
                .and_modify(|s| s.update(score))
                .or_insert_with(|| ScoreUpdateCandidate::new(score));

            ip_score_candidates
                .entry(ip)
                .and_modify(|s| s.update(score))
                .or_insert_with(|| ScoreUpdateCandidate::new(score));
        }

        let median_ip_score = self.approximate_median_ip_score();
        for (&ip, score) in self.ip_score.iter_mut() {
            let new_score = ip_score_candidates
                .remove(&ip)
                // TODO: hard coded parameter
                .filter(|sc| sc.count >= 5)
                .map(|sc| sc.finalize())
                .unwrap_or(median_ip_score);

            // 1) Calculate new score
            // 2) Replace score in inverse map
            // 3) Update score in map
            let new_score = ema(*score, new_score);
            self.ip_score_inverse
                .remove(&InverseScoreEntryIp::new(*score, ip));
            self.ip_score_inverse
                .insert(InverseScoreEntryIp::new(new_score, ip), ());
            *score = new_score;
        }

        for (ip, score_candidate) in ip_score_candidates {
            // TODO: hard coded parameter
            if score_candidate.count >= 5 {
                let score = score_candidate.finalize();
                self.ip_score.insert(ip, score);
                self.ip_score_inverse
                    .insert(InverseScoreEntryIp::new(score, ip), ());
            }
        }

        let median_signer_score =
            self.approximate_median_signer_score();
        for (&signer, score) in self.signer_score.iter_mut() {
            let new_score = signer_score_candidates
                .remove(&signer)
                // TODO: hard coded parameter
                .filter(|sc| sc.count >= 5)
                .map(|sc| sc.finalize())
                .unwrap_or(median_signer_score);

            // 1) Calculate new score
            // 2) Replace score in inverse map
            // 3) Update score in map
            let new_score = ema(*score, new_score);
            self.signer_score_inverse
                .remove(&InverseScoreEntrySigner::new(*score, signer));
            self.signer_score_inverse.insert(
                InverseScoreEntrySigner::new(new_score, signer),
                (),
            );
            *score = new_score;
        }

        for (signer, score_candidate) in signer_score_candidates {
            // TODO: hard coded parameter
            if score_candidate.count >= 5 {
                let score = score_candidate.finalize();
                self.signer_score.insert(signer, score);
                self.signer_score_inverse.insert(
                    InverseScoreEntrySigner::new(score, signer),
                    (),
                );
            }
        }

        self.prune(prune_ips, prune_signers);
    }

    pub fn save_ip_scores(&self, arg: &str) {
        let Ok(mut file) = std::fs::File::create(arg) else {
            println!("failed to create file to save ip scores");
            return;
        };

        for (ip, score) in self.ip_score.iter() {
            if let Err(e) = writeln!(
                &mut file,
                "{} {}",
                Ipv4Addr::from_bits(*ip),
                **score
            ) {
                println!("failed to write ip score: {e:?}");
                return;
            }
        }
    }
}

fn ema(old_score: F64, new_score: F64) -> F64 {
    // TODO: hard coded parameter
    const ALPHA: F64 = OrderedFloat(0.05);

    old_score * (ONE - ALPHA) + new_score * ALPHA
}
