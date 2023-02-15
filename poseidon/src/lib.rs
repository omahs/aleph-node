#![cfg_attr(not(feature = "std"), no_std)]

use ark_bls12_381::Fr;

#[cfg(feature = "circuit")]
pub mod circuit;
pub mod hash;
mod parameters;

/// Poseidon paper suggests using domain separation for concretely encoding the use case in the
/// capacity element (which is fine as it is 256 bits large and has a lot of bits to fill).
const DOMAIN_SEPARATOR: u64 = 2137;

pub fn domain_separator() -> Fr {
    Fr::from(DOMAIN_SEPARATOR)
}
