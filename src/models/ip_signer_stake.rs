use crate::{
    interface::{ParsedPacket, QoSModel},
    QoSTransactionMeta, F32, ONE,
};

use std::{
    borrow::Borrow,
    collections::{btree_map::Entry, BTreeMap, HashMap},
};

type Stake = u64;
type TotalStake = Stake;
type Ip4 = u32;

impl QoSModel for IpSignerStakeModel {
    type AdditionalTransactionMeta = ();
    type AdditionalUpdateMeta = (TotalStake, HashMap<Ip4, Stake>);
    fn forward(&self, parsed_packet: ParsedPacket) -> F32 {
        self.forward(parsed_packet.0, &parsed_packet.1)
    }

    fn update_model<'a>(
        &'a mut self,
        transactions: impl Iterator<Item = &'a QoSTransactionMeta<()>>,
        update_meta: Self::AdditionalUpdateMeta,
    ) {
        self.update_model(transactions, 5, 5, update_meta)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct IpSignerStakeModel {
    signer_score: BTreeMap<[u8; 32], F32>,
    ip_score: BTreeMap<u32, F32>,

    signer_score_inverse: Vec<(F32, [u8; 32])>,
    ip_score_inverse: Vec<(F32, u32)>,

    stake_lookup: HashMap<Ip4, Stake>,
    total_stake: u64,
}

impl IpSignerStakeModel {
    pub fn new(
        ip_scores: impl IntoIterator<Item = (u32, f32)>,
        signer_scores: impl IntoIterator<Item = ([u8; 32], f32)>,
        stake_lookup: HashMap<Ip4, Stake>,
        total_stake: u64,
    ) -> IpSignerStakeModel {
        let mut signer_score = BTreeMap::new();
        let mut signer_score_inverse = Vec::new();
        for (signer, score) in signer_scores {
            signer_score.insert(signer, F32::new(score).unwrap());
            insert_into_vec(
                &mut signer_score_inverse,
                (F32::new(score).unwrap(), signer),
            );
        }

        let mut ip_score = BTreeMap::new();
        let mut ip_score_inverse = Vec::new();
        for (ip, score) in ip_scores {
            ip_score.insert(ip, F32::new(score).unwrap());
            insert_into_vec(&mut ip_score_inverse, (F32::new(score).unwrap(), ip));
        }

        IpSignerStakeModel {
            signer_score,
            ip_score,
            signer_score_inverse,
            ip_score_inverse,
            stake_lookup,
            total_stake,
        }
    }

    /// Returns combined score for this ip + signer.
    /// Panics if there are no scores!
    pub fn forward(&self, ip: u32, signer: &[u8; 32]) -> F32 {
        // Get scores
        //
        // We use median score for null queries because that is the most neutral score.
        // Recall that pruning removes elements close to the median, leaving the most
        // discriminating scores (i.e. least and most valuable sources).
        let ip_score = *self
            .ip_score
            .get(&ip)
            .unwrap_or_else(|| self.median_ip_score());
        let signer_score = *self
            .signer_score
            .get(signer)
            .unwrap_or_else(|| self.median_signer_score());
        let stake_score = stake_score(
            self.stake_lookup
                .get(&ip)
                .copied()
                .unwrap_or(0),
            self.total_stake as u64,
        );

        (ip_score + signer_score) * stake_score
    }

    fn median_ip_score(&self) -> &F32 {
        &self.ip_score_inverse[self.ip_score_inverse.len() / 2].0
    }

    fn median_signer_score(&self) -> &F32 {
        &self.signer_score_inverse[self.signer_score_inverse.len() / 2].0
    }

    /// Prunes from the middle of tables, keeping most valuable and least valuable signers and ips
    pub fn prune(&mut self, num_ips: usize, num_signers: usize) {
        // Prune ips
        let ips_to_delete = self
            .ip_score_inverse
            .len()
            .saturating_sub(num_ips);
        let start = self.ip_score_inverse.len() / 2 - ips_to_delete / 2;
        let end = start + ips_to_delete;
        for (_score, ip) in self.ip_score_inverse.drain(start..end) {
            self.ip_score.remove(&ip);
        }

        // Prune signers
        let signers_to_delete = self
            .signer_score_inverse
            .len()
            .saturating_sub(num_signers);
        let start = self.signer_score_inverse.len() / 2 - signers_to_delete / 2;
        let end = start + signers_to_delete;
        for (_score, signer) in self
            .signer_score_inverse
            .drain(start..end)
        {
            self.signer_score.remove(&signer);
        }
    }

    pub fn add_ip_score(&mut self, ip: u32, score: F32) {
        // Remove if score for ip exists already
        if let Some((ip, score)) = self.ip_score.remove_entry(&ip) {
            remove_from_vec(&mut self.ip_score_inverse, &(score, ip));
        }

        // Add score
        self.ip_score.insert(ip, score);
        insert_into_vec(&mut self.ip_score_inverse, (score, ip));
    }

    pub fn add_signer_score(&mut self, signer: [u8; 32], score: F32) {
        // Remove if score for signer exists already
        if let Some((signer, score)) = self.signer_score.remove_entry(&signer) {
            remove_from_vec(&mut self.signer_score_inverse, &(score, signer));
        }

        // Add score
        self.signer_score.insert(signer, score);
        insert_into_vec(&mut self.signer_score_inverse, (score, signer));
    }

    pub fn update_model<'a>(
        &'a mut self,
        transactions: impl IntoIterator<Item = impl Borrow<QoSTransactionMeta<()>>>,
        prune_signers: usize,
        prune_ips: usize,
        (total_stake, stake_lookup): <Self as QoSModel>::AdditionalUpdateMeta,
    ) {
        struct ScoreUpdateCandidate {
            score_sum: F32,
            count: u32,
        }
        impl ScoreUpdateCandidate {
            pub fn new(score: F32) -> ScoreUpdateCandidate {
                ScoreUpdateCandidate {
                    score_sum: score,
                    count: 1,
                }
            }

            pub fn update(&mut self, score: F32) {
                self.score_sum += score;
                self.count += 1;
            }

            pub fn finalize(&self) -> F32 {
                self.score_sum / F32::new(self.count as f32).unwrap()
            }
        }

        let mut signer_score_candidates = BTreeMap::<[u8; 32], ScoreUpdateCandidate>::new();
        let mut ip_score_candidates = BTreeMap::<u32, ScoreUpdateCandidate>::new();
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

        for (ip, score_candidate) in ip_score_candidates {
            // TODO: hard coded parameter
            if score_candidate.count >= 5 {
                match self.ip_score.entry(ip) {
                    // If vacant, just add
                    Entry::Vacant(ve) => {
                        ve.insert(score_candidate.finalize());
                        insert_into_vec(
                            &mut self.ip_score_inverse,
                            (score_candidate.finalize(), ip),
                        );
                    }

                    // If occupied:
                    // 1) Calculate new score
                    // 2) Replace score in inverse map
                    // 3) Update score in map
                    Entry::Occupied(ref mut score) => {
                        let new_score = ema(*score.get(), score_candidate.finalize());
                        remove_from_vec(&mut self.ip_score_inverse, &(*score.get(), ip));
                        insert_into_vec(&mut self.ip_score_inverse, (new_score, ip));
                        *score.get_mut() = new_score;
                    }
                }
            }
        }

        for (signer, score_candidate) in signer_score_candidates {
            // TODO: hard coded parameter
            if score_candidate.count >= 5 {
                match self.signer_score.entry(signer) {
                    // If vacant, just add
                    Entry::Vacant(ve) => {
                        ve.insert(score_candidate.finalize());
                        insert_into_vec(
                            &mut self.signer_score_inverse,
                            (score_candidate.finalize(), signer),
                        );
                    }

                    // If occupied:
                    // 1) Calculate new score
                    // 2) Replace score in inverse map
                    // 3) Update score in map
                    Entry::Occupied(ref mut score) => {
                        let new_score = ema(*score.get(), score_candidate.finalize());
                        remove_from_vec(&mut self.signer_score_inverse, &(*score.get(), signer));
                        insert_into_vec(&mut self.signer_score_inverse, (new_score, signer));
                        *score.get_mut() = new_score;
                    }
                }
            }
        }

        self.prune(prune_ips, prune_signers);

        // Update stake
        self.total_stake = total_stake;
        self.stake_lookup = stake_lookup;
    }
}

fn remove_from_vec<T: Ord>(v: &mut Vec<T>, value: &T) -> T {
    match v.binary_search(value) {
        Ok(i) => v.remove(i),
        Err(_) => unreachable!(),
    }
}

#[inline]
fn insert_into_vec<T: Ord>(v: &mut Vec<T>, value: T) {
    match v.binary_search(&value) {
        Err(i) => v.insert(i, value),
        Ok(_) => unreachable!(),
    }
}

fn ema(old_score: F32, new_score: F32) -> F32 {
    // TODO: hard coded parameter
    const ALPHA: F32 = unsafe { F32::new_unchecked(0.5) };

    old_score * (ONE - ALPHA) + new_score * ALPHA
}

// Multiplier bounded between 1 and 2
fn stake_score(stake: u64, total_stake: u64) -> f32 {
    (stake + total_stake) as f32 / total_stake as f32
}
