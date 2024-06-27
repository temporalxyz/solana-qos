use ordered_float::NotNan;

pub mod interface;
pub mod models;

pub type F32 = NotNan<f32>;
// const ZERO: F32 = unsafe { F32::new_unchecked(0.0) };
const ONE: F32 = unsafe { F32::new_unchecked(1.0) };

#[derive(Debug)]
pub struct QoSTransactionMeta<A> {
    // Ipv4 source address
    pub ip: u32,

    /// Primary signer, i.e. fee payer
    pub signer: [u8; 32],

    /// Fee in lamports / execution time in nanos
    pub value: F32,

    pub additional_metadata: A,
}

impl<A> QoSTransactionMeta<A> {
    pub fn new(
        ip: u32,
        signer: [u8; 32],
        fee: u64,
        execution_nanos: u64,
        additional_metadata: A,
    ) -> QoSTransactionMeta<A> {
        QoSTransactionMeta {
            ip,
            signer,
            value: F32::new(fee as f32 / execution_nanos as f32).unwrap(),
            additional_metadata,
        }
    }
}
