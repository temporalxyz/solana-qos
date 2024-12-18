use fd_bs58::encode_32;
use qos_model::{
    interface::QoSModel, models::ip_signer::IpSignerModel,
};

fn main() {
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

    let model = IpSignerModel::<8, 8>::new(
        ip_scores.iter().copied(),
        signer_scores.iter().copied(),
    );

    // ip, signer
    let queries = [
        (0xdeadbeef, [0; 32]),
        (0xbeefdead, [1; 32]),
        (0xdeadbeef, [5; 32]),
    ];

    for (ip, signer) in queries {
        let score = model.forward(ip, &signer, &());
        println!(
            "score {score} for ip {ip} signer {}",
            encode_32(signer)
        )
    }
}
