use std::{
    fs::File,
    io::{BufWriter, Write},
};

use mock_tx_engine::{agent::Signer, rng::FastxxHashRng};

fn main() {
    let signer = Signer::new(100_000_000_f32.ln(), 5.0);

    println!("signer should have mean {}", signer.mean_price());

    let mut rng = FastxxHashRng::new(0xABC);

    let mut file = BufWriter::new(File::create("sample").unwrap());
    for _ in 0..100_000 {
        writeln!(&mut file, "{}", signer.sample_cu_price(&mut rng))
            .unwrap();
    }

    file.flush().unwrap();
}
