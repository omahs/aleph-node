//! Module exposing some utilities regarding note generation and verification.

use ark_r1cs_std::eq::EqGadget;
use ark_relations::r1cs::SynthesisError;
use ark_std::{vec, vec::Vec};

use super::{
    tangle::{tangle, tangle_in_circuit},
    types::{
        FrontendNote, FrontendNullifier, FrontendTokenAmount, FrontendTokenId, FrontendTrapdoor,
    },
};
use crate::{environment::FpVar, shielder::convert_hash, CircuitField};

/// Verify that `note` is indeed the result of tangling `(token_id, token_amount, trapdoor,
/// nullifier)`.
///
/// For circuit use only.
pub(super) fn check_note(
    token_id: &FpVar,
    token_amount: &FpVar,
    trapdoor: &FpVar,
    nullifier: &FpVar,
    note: &FpVar,
) -> Result<(), SynthesisError> {
    let tangled = tangle_in_circuit(&vec![
        token_id.clone(),
        token_amount.clone(),
        trapdoor.clone(),
        nullifier.clone(),
    ])?;

    tangled.enforce_equal(note)
}

/// Compute note as the result of tangling `(token_id, token_amount, trapdoor, nullifier)`.
///
/// Useful for input preparation and offline note generation.
pub fn compute_note(
    token_id: FrontendTokenId,
    token_amount: FrontendTokenAmount,
    trapdoor: FrontendTrapdoor,
    nullifier: FrontendNullifier,
) -> FrontendNote {
    tangle(&[
        CircuitField::from(token_id as u64),
        CircuitField::from(token_amount),
        CircuitField::from(trapdoor),
        CircuitField::from(nullifier),
    ])
    .0
     .0
}

pub fn compute_parent_hash(left: FrontendNote, right: FrontendNote) -> FrontendNote {
    tangle(&[convert_hash(left), convert_hash(right)]).0 .0
}

/// Create a note from the first 32 bytes of `bytes`.
pub fn note_from_bytes(bytes: &[u8]) -> FrontendNote {
    [
        u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
        u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
        u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
    ]
}

fn convert(x: u64) -> [u8; 8] {
    x.to_le_bytes()
}

pub fn bytes_from_note(note: &FrontendNote) -> Vec<u8> {
    let mut res = vec![];
    for elem in note {
        let mut arr: Vec<u8> = convert(*elem).into();
        res.append(&mut arr);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_conversion() {
        let token_id: FrontendTokenId = 1;
        let token_amount: FrontendTokenAmount = 10;
        let trapdoor: FrontendTrapdoor = 17;
        let nullifier: FrontendNullifier = 19;
        let note = compute_note(token_id, token_amount, trapdoor, nullifier);

        let bytes = bytes_from_note(&note);
        let note_again = note_from_bytes(&bytes);

        assert_eq!(note, note_again);
    }
}
