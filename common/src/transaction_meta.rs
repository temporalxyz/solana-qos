use ordered_float::OrderedFloat;

pub type F64 = OrderedFloat<f64>;

#[derive(Debug)]
pub struct QoSTransactionMeta<A> {
    // Ipv4 source address
    pub ip: u32,

    /// Primary signer, i.e. fee payer
    pub signer: [u8; 32],

    /// Fee in lamports / execution time in nanos
    pub value: F64,

    pub additional_metadata: A,
}

impl<A> QoSTransactionMeta<A> {
    // test only. will panic if nanos == 0
    pub fn new_for_tests(
        ip: u32,
        signer: [u8; 32],
        fee: u64,
        execution_nanos: u64,
        additional_metadata: A,
    ) -> QoSTransactionMeta<A> {
        QoSTransactionMeta {
            ip,
            signer,
            value: F64::from(fee as f64 / execution_nanos as f64),
            additional_metadata,
        }
    }
}
