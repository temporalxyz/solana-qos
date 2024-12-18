use std::{
    fs::File,
    io::{BufWriter, Write},
};

use mock_tx_engine::rng::FastxxHashRng;
use rand::Rng;

fn main() {
    let mut rng = FastxxHashRng::new(0x123);

    let mut file = BufWriter::new(File::create("rng").unwrap());
    for _ in 0..100_000 {
        writeln!(&mut file, "{}", rng.gen::<f32>()).unwrap();
    }

    file.flush().unwrap();
}
