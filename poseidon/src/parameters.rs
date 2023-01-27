use ark_bls12_381::Fr;
use ark_sponge::poseidon::PoseidonParameters as ArkSpongePoseidonParameters;
// use once_cell::sync::Lazy;
use poseidon_parameters::{Alpha, PoseidonParameters};

// Poseidon parameters generated for the Fr (Fp256) finite field
pub mod fr_parameters {
    use ark_std::vec;
    include!(concat!(env!("OUT_DIR"), "/parameters.rs"));
}

// Parameters for the 1:1 hash instance of Poseidon
// pub static RATE_1_PARAMETERS: Lazy<PoseidonParameters<Fr>> = Lazy::new(fr_parameters::rate_1);
// pub static RATE_1_PARAMETERS: PoseidonParameters<Fr> = fr_parameters::rate_1();

// taken from Penumbra (https://github.com/penumbra-zone/poseidon377/blob/a2d8c7a3288e2e877ac88a4d8fd3cc4ff2b52c04/poseidon377/src/r1cs.rs#L12)
pub(crate) fn to_ark_sponge_poseidon_parameters(
    params: PoseidonParameters<Fr>,
) -> ArkSpongePoseidonParameters<Fr> {
    let alpha = match params.alpha {
        Alpha::Exponent(exp) => exp as u64,
        Alpha::Inverse => panic!("ark-sponge does not allow inverse alpha"),
    };
    let capacity = 1;
    let rate = params.t - capacity;
    let full_rounds = params.rounds.r_F;
    let partial_rounds = params.rounds.r_P;

    // TODO : 'compile' could return ark and mds as collections of field element
    ArkSpongePoseidonParameters {
        full_rounds,
        partial_rounds,
        alpha,
        ark: params.arc.into(),
        mds: params.mds.into(),
        rate,
        capacity,
    }
}
