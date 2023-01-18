#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract(env = snarcos_extension::DefaultEnvironment)]
mod preimage {

    use ink::{
        codegen::EmitEvent,
        prelude::{format, string::String, vec::Vec},
        reflect::ContractEventBase,
        storage::Mapping,
        ToAccountId,
    };
    use snarcos_extension::VerificationKeyIdentifier;

    const PREIMAGE_VK_IDENTIFIER: VerificationKeyIdentifier = [b'p', b'i', b'm', b'g'];

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum PreimageContractError {
        AlreadyCommited,
        NotCommited, // InkEnvError(String),
    }

    #[ink(storage)]
    pub struct Preimage {
        commitments: Mapping<AccountId, Vec<u8>>,
    }

    impl Preimage {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                commitments: Mapping::default(),
            }
        }

        #[ink(message)]
        pub fn commit(&mut self, hash: Vec<u8>) -> Result<(), PreimageContractError> {
            let caller = Self::env().caller();

            if self.commitments.contains(caller) {
                return Err(PreimageContractError::AlreadyCommited);
            }

            self.commitments.insert(caller, &hash);
            Ok(())
        }

        #[ink(message)]
        pub fn reveal(
            &mut self,
            hash: Vec<u8>,
            proof: Vec<u8>,
        ) -> Result<(), PreimageContractError> {
            let caller = Self::env().caller();

            if !self.commitments.contains(caller) {
                return Err(PreimageContractError::NotCommited);
            }

            // TODO : verify

            self.commitments.remove(caller);
            Ok(())
        }
    }
}
