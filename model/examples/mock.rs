use qos_model::models::ip_signer::IpSignerModel;
use solana_qos_common::transaction_meta::QoSTransactionMeta;

fn main() {
    // Load some pretrained model
    let mut model = mock_pretrained_model();

    // Transaction metadata captured in banking (or replay) stage
    #[rustfmt::skip]
    let transactions: Vec<QoSTransactionMeta<()>> = vec![
        // ip, signer, total fee, execution time nanos
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [0; 32], 1000, 1000, ()),
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [0; 32], 1000, 1000, ()),
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [0; 32], 1000, 1000, ()),
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [0; 32], 1000, 1000, ()),
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [0; 32], 1000, 1000, ()),
        //
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [1; 32], 1000, 1000, ()),
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [1; 32], 1000, 1000, ()),
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [1; 32], 1000, 1000, ()),
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [1; 32], 1000, 1000, ()),
        QoSTransactionMeta::new_for_tests(0xdeadbeef, [1; 32], 1000, 1000, ()),
    ];

    // Update model
    let prune_signers = 5;
    let prune_ips = 5;
    model.update_model(transactions.iter(), prune_signers, prune_ips);
}

fn mock_pretrained_model() -> IpSignerModel<5, 5> {
    let ip_scores: Vec<(u32, f64)> = vec![
        (0xdeadbeef, 0.1),
        (0xbeefdead, 0.2),
        (0x0dadbad0, 0.1),
        (0xfabafaba, 0.3),
    ];

    let signer_scores: Vec<([u8; 32], f64)> = vec![
        ([0; 32], 0.1),
        ([1; 32], 0.2),
        ([2; 32], 0.1),
        ([3; 32], 0.3),
    ];

    let model = IpSignerModel::new(
        ip_scores.iter().copied(),
        signer_scores.iter().copied(),
    );
    model
}
