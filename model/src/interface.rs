use solana_qos_internal_common::transaction_meta::{
    QoSTransactionMeta, F64,
};

pub trait QoSModel {
    type AdditionalArgs;
    type AdditionalTransactionMeta;
    type AdditionalUpdateMeta;
    fn forward(
        &self,
        ip: u32,
        signer: &[u8; 32],
        args: &Self::AdditionalArgs,
    ) -> F64;
    fn update_model<'a>(
        &'a mut self,
        transactions: impl Iterator<
            Item = &'a QoSTransactionMeta<
                Self::AdditionalTransactionMeta,
            >,
        >,
        update_meta: Self::AdditionalUpdateMeta,
    );

    type IpFeedback;
    /// Could be invalid signer feedback from sigverify stage, or some
    /// other form of feedback
    fn ip_feedback(&mut self, feedback: Self::IpFeedback);
}
