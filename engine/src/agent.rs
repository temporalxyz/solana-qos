use std::net::Ipv4Addr;

use rand::Rng;
use rand_distr::{Distribution, LogNormal};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    packet::Packet,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer as _,
    system_instruction,
    transaction::Transaction,
};

use crate::rng::FastxxHashRng;

pub struct Ip {
    ip: [u8; 4],
    signer: Signer,
    rng: FastxxHashRng,
    bad: bool,
}

impl Ip {
    pub fn new(ip: [u8; 4], log_mean: f32, std: f32) -> Ip {
        let rng = FastxxHashRng::new(rand::random());
        Ip {
            ip,
            signer: Signer::new(log_mean, std),
            bad: false,
            rng,
        }
    }

    pub fn bad(ip: [u8; 4], log_mean: f32, std: f32) -> Ip {
        let rng = FastxxHashRng::new(rand::random());
        Ip {
            ip,
            signer: Signer::new(log_mean, std),
            bad: true,
            rng,
        }
    }

    /// For this demo, we do not need a valid blockhash.
    pub fn update_transfer_with_priority(
        &mut self,
        packet: &mut Packet,
    ) {
        // Select random signer
        let signer = &self.signer;

        // Sample price and construct CU price ix
        let lamports_per_megacu = signer.sample_cu_price(&mut self.rng);
        unsafe {
            update_compute_unit_price(
                packet.buffer_mut(),
                lamports_per_megacu,
            );

            update_blockhash(packet.buffer_mut(), &Hash::new_unique());
            update_signature_invalid(packet.buffer_mut());
        };

        if self.bad {
            let sample = self.rng.gen::<f32>();

            // Demo doesn't actually do sigverify
            //
            // 1) we produce invalid packet by invalidating msg len, producing a ParseError
            //   (TODO: change with new view in v2)
            //
            // OR
            //
            // 2) OR we make last byte 1 to signal to mock sigverify to mark this as failed
            if sample < 2.5e-1 {
                packet.meta_mut().size = 0;
                *packet.buffer_mut().last_mut().unwrap() = 0;
            } else if sample < 3e-1 {
                // last byte = 1 --> mock invalid sigverify
                *packet.buffer_mut().last_mut().unwrap() = 1;
                packet.meta_mut().size = PRICE_AND_TRANSFER_TX_TX_LEN;
            }
        } else {
            packet.meta_mut().size = PRICE_AND_TRANSFER_TX_TX_LEN;
            *packet.buffer_mut().last_mut().unwrap() = 0;
        }

        // Update ip
        packet.meta_mut().addr = self.ip().into();
    }

    #[inline(always)]
    fn ip(&self) -> Ipv4Addr {
        Ipv4Addr::new(self.ip[0], self.ip[1], self.ip[2], self.ip[3])
    }
}
pub struct Signer {
    keypair: Keypair,
    log_mean: f32,
    std: f32,
    cu_price_distribution: LogNormal<f32>,
}

impl Signer {
    pub fn new(log_mean: f32, std: f32) -> Signer {
        Signer {
            keypair: Keypair::new(),
            log_mean,
            std,
            cu_price_distribution: LogNormal::new(log_mean, std)
                .unwrap(),
        }
    }

    pub fn mean_price(&self) -> u64 {
        (self.log_mean + self.std.powi(2) / 2.0).exp() as u64
    }

    #[inline(always)]
    pub fn log_mean(&self) -> f32 {
        self.log_mean
    }

    #[inline(always)]
    pub fn std(&self) -> f32 {
        self.std
    }
    #[inline(always)]
    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }
}

impl Signer {
    pub fn sample_cu_price<R: Rng>(&self, rng: &mut R) -> u64 {
        // We cut off at 1e9 but this should be a rare edge case
        self.cu_price_distribution
            .sample(rng)
            .min(1e9) as u64
    }
}

// Offset for the compute unit price u64 in a transaction that includes
// compute unit price and transfer
const PRICE_AND_TRANSFER_TX_PRICE_U64_OFFSET: usize = 234;
// Offset for the lamports u64 in a transaction that includes compute
// unit price and transfer
// const PRICE_AND_TRANSFER_TX_LAMPORTS_U64_OFFSET: usize = 219;
const PRICE_AND_TRANSFER_TX_BLOCKHASH_OFFSET: usize = 197;
const PRICE_AND_TRANSFER_TX_SIGNATURE_OFFSET: usize = 1;
const PRICE_AND_TRANSFER_TX_MESSAGE_OFFSET: usize = 65;
pub const PRICE_AND_TRANSFER_TX_PAYER_OFFSET: usize = 69;
pub const PRICE_AND_TRANSFER_TX_MESSAGE_LEN: usize = 162;
const PRICE_AND_TRANSFER_TX_TX_LEN: usize = 259;

#[inline(always)]
unsafe fn update_compute_unit_price(
    transaction_bytes: &mut [u8],
    new_price: u64,
) {
    core::ptr::copy_nonoverlapping(
        &new_price as *const u64 as *const u8,
        transaction_bytes
            .as_mut_ptr()
            .add(PRICE_AND_TRANSFER_TX_PRICE_U64_OFFSET),
        8,
    );
}

// #[inline(always)]
// unsafe fn update_lamports(
//     transaction_bytes: &mut [u8],
//     new_lamports: u64,
// ) {
//     core::ptr::copy_nonoverlapping(
//         &new_lamports as *const u64 as *const u8,
//         transaction_bytes
//             .as_mut_ptr()
//             .add(PRICE_AND_TRANSFER_TX_LAMPORTS_U64_OFFSET),
//         8,
//     );
// }

#[inline(always)]
unsafe fn update_blockhash(
    transaction_bytes: &mut [u8],
    new_blockhash: &Hash,
) {
    core::ptr::copy_nonoverlapping(
        new_blockhash as *const Hash as *const u8,
        transaction_bytes
            .as_mut_ptr()
            .add(PRICE_AND_TRANSFER_TX_BLOCKHASH_OFFSET),
        32,
    );
}

pub unsafe fn read_price(packet: &mut Packet) -> u64 {
    let price: *const u64 = packet
        .buffer_mut()
        .as_ptr()
        .add(PRICE_AND_TRANSFER_TX_PRICE_U64_OFFSET)
        .cast();
    core::ptr::read_unaligned(price)
}

#[inline(always)]
pub unsafe fn update_payer_and_signature(
    transaction_bytes: &mut [u8],
    keypair: &Keypair,
) {
    let new_payer = keypair.pubkey();

    core::ptr::copy_nonoverlapping(
        &new_payer as *const Pubkey as *const u8,
        transaction_bytes
            .as_mut_ptr()
            .add(PRICE_AND_TRANSFER_TX_PAYER_OFFSET),
        32,
    );

    let new_signature = keypair.sign_message(
        &transaction_bytes[PRICE_AND_TRANSFER_TX_MESSAGE_OFFSET..],
    );

    core::ptr::copy_nonoverlapping(
        &new_signature as *const Signature as *const u8,
        transaction_bytes
            .as_mut_ptr()
            .add(PRICE_AND_TRANSFER_TX_SIGNATURE_OFFSET),
        64,
    );
}

#[inline(always)]
unsafe fn update_signature_invalid(transaction_bytes: &mut [u8]) {
    let sig_bytes: [u8; 64] = core::array::from_fn(|_| rand::random());

    core::ptr::copy_nonoverlapping(
        sig_bytes.as_ptr(),
        transaction_bytes
            .as_mut_ptr()
            .add(PRICE_AND_TRANSFER_TX_SIGNATURE_OFFSET),
        64,
    );
}

#[inline(always)]
pub fn null_transfer_transaction_with_compute_unit_price() -> Packet {
    let compute_unit_price_ix =
        ComputeBudgetInstruction::set_compute_unit_price(115);
    let transfer = system_instruction::transfer(
        &Pubkey::new_unique(),
        &Pubkey::new_unique(),
        0,
    );

    let mut tx = Transaction::new_with_payer(
        &[compute_unit_price_ix, transfer],
        None,
    );
    tx.message.recent_blockhash = Hash::new_from_array([42; 32]);

    Packet::from_data(None, &tx).unwrap()
}

#[test]
fn update_transaction() {
    use agave_transaction_view::transaction_view::TransactionView;
    use solana_sdk::compute_budget;
    use solana_sdk::compute_budget::ComputeBudgetInstruction;

    let mut packet =
        null_transfer_transaction_with_compute_unit_price();
    assert_eq!(packet.meta().size, PRICE_AND_TRANSFER_TX_TX_LEN);
    assert_eq!(
        packet
            .buffer_mut()
            .iter()
            .position(|b| *b == 115),
        Some(PRICE_AND_TRANSFER_TX_PRICE_U64_OFFSET)
    );
    assert_eq!(
        packet
            .buffer_mut()
            .windows(32)
            .position(|w| *w == [42; 32]),
        Some(PRICE_AND_TRANSFER_TX_BLOCKHASH_OFFSET)
    );

    let len = packet.meta().size;
    println!("len = {len}");
    assert!(bincode::deserialize::<Transaction>(
        &packet.buffer_mut()[..len]
    )
    .is_ok());
    TransactionView::try_new_sanitized(&packet.buffer_mut()[..len])
        .unwrap();

    // Update price
    let mut old = packet.clone();
    unsafe {
        update_compute_unit_price(&mut packet.buffer_mut()[..len], 1);
    }
    let tx = bincode::deserialize::<Transaction>(
        &packet.buffer_mut()[..len],
    )
    .unwrap();
    assert_eq!(
        *tx.message.instructions[0]
            .program_id(&tx.message.account_keys),
        compute_budget::ID,
    );
    assert_eq!(
        tx.message.instructions[0].data,
        ComputeBudgetInstruction::set_compute_unit_price(1).data,
        "\n{:?} vs \n{:?}",
        &old.buffer_mut()[..len],
        &packet.buffer_mut()[..len]
    );

    assert!(TransactionView::try_new_sanitized(
        &packet.buffer_mut()[..len]
    )
    .is_ok());

    let hash = Hash::new_unique();
    unsafe {
        update_blockhash(&mut packet.buffer_mut()[..len], &hash);
    }
    let tx = bincode::deserialize::<Transaction>(
        &packet.buffer_mut()[..len],
    )
    .unwrap();
    assert_eq!(tx.message.recent_blockhash, hash);
    TransactionView::try_new_sanitized(&packet.buffer_mut()[..len])
        .unwrap();

    let keypair = Keypair::new();
    unsafe {
        update_payer_and_signature(
            &mut packet.buffer_mut()[..len],
            &keypair,
        );
    }
    let x = bincode::deserialize::<Transaction>(
        &packet.buffer_mut()[..len],
    )
    .unwrap();
    assert_eq!(x.message.instructions[1].accounts, vec![0, 1]);
    assert_eq!(x.message.account_keys[0], keypair.pubkey());
    assert!(TransactionView::try_new_sanitized(
        &packet.buffer_mut()[..len]
    )
    .is_ok());
}

#[test]
fn test_read_price() {
    let mut packet =
        null_transfer_transaction_with_compute_unit_price();

    // Update price
    unsafe {
        // Initialized with price = 115
        assert_eq!(read_price(&mut packet), 115);

        // Update to 139
        update_compute_unit_price(&mut packet.buffer_mut(), 139);
        assert_eq!(read_price(&mut packet), 139);
    };
}
