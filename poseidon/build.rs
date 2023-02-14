use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

use ark_bls12_381::{Fr, FrParameters};
use ark_ff::{fields::FpParameters, vec};
use poseidon_paramgen::poseidon_build;

fn main() {
    let security_level = match env::var("SECURITY_LEVEL") {
        Ok(level) => match level.as_str() {
            "80" => 80,
            "128" => 128,
            "256" => 256,
            _ => panic!("Unsupported security level. Supported levels: 80, 128, 256"),
        },
        Err(_) => 128,
    };

    // t = arity + 1, so t=2 is a 1:1 hash, t=3 is a 2:1 hash etc
    // see https://spec.filecoin.io/#section-algorithms.crypto.poseidon.filecoins-poseidon-instances for similar specification used by Filecoin
    let t_values = vec![2];

    // Fr => Fp256
    let parameters =
        poseidon_build::compile::<Fr>(security_level, t_values, FrParameters::MODULUS, true);

    let output_directory = PathBuf::from(
        env::var("OUT_DIR").expect("OUT_DIR environmental variable should be always set"),
    )
    .join("parameters.rs");

    let mut file =
        BufWriter::new(File::create(output_directory).expect("can't create source file"));

    file.write_all(parameters.as_bytes())
        .expect("can write parameters to file");
}
