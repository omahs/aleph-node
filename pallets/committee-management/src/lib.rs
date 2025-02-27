#![cfg_attr(not(feature = "std"), no_std)]
//!
//! # Ban logic
//! In case of insufficient validator's uptime, we need to remove such validators from
//! the committee, so that the network is as healthy as possible. This is achieved by calculating
//! number of _underperformance_ sessions, which means that number of blocks produced by the
//! validator is less than some predefined threshold.
//! In other words, if a validator:
//! * performance in a session is less or equal to a configurable threshold
//! `BanConfig::minimal_expected_performance` (from 0 to 100%), and,
//! * it happened at least `BanConfig::underperformed_session_count_threshold` times,
//! then the validator is considered an underperformer and hence removed (ie _banned out_) from the
//! committee.
//!
//! ## Thresholds
//! There are two ban thresholds described above, see [`BanConfig`].
//!
//! ### Next era vs current era
//! Current and next era have distinct thresholds values, as we calculate bans during the start of the new era.
//! They follow the same logic as next era committee seats: at the time of planning the first
//! session of next the era, next values become current ones.

extern crate core;

mod impls;
mod manager;
mod migration;
mod traits;

use codec::{Decode, Encode};
use frame_support::traits::StorageVersion;
pub use manager::SessionAndEraManager;
pub use migration::PrefixMigration;
pub use pallet::*;
use primitives::{BanConfig as BanConfigStruct, BanInfo, SessionValidators};
use scale_info::TypeInfo;
use sp_std::{collections::btree_map::BTreeMap, default::Default};
pub use traits::*;

pub type TotalReward = u32;
#[derive(Decode, Encode, TypeInfo, PartialEq, Eq)]
pub struct ValidatorTotalRewards<T>(pub BTreeMap<T, TotalReward>);

#[derive(Decode, Encode, TypeInfo)]
struct CurrentAndNextSessionValidators<T> {
    pub next: SessionValidators<T>,
    pub current: SessionValidators<T>,
}

impl<T> Default for CurrentAndNextSessionValidators<T> {
    fn default() -> Self {
        Self {
            next: Default::default(),
            current: Default::default(),
        }
    }
}
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);
pub(crate) const LOG_TARGET: &str = "pallet-committee-management";

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        dispatch::DispatchResult, ensure, pallet_prelude::*, BoundedVec, Twox64Concat,
    };
    use frame_system::{ensure_root, pallet_prelude::OriginFor};
    use primitives::{
        BanHandler, BanReason, BlockCount, SessionCount, SessionValidators, ValidatorProvider,
    };
    use sp_runtime::Perbill;
    use sp_staking::EraIndex;
    use sp_std::vec::Vec;

    use crate::{
        traits::{EraInfoProvider, ValidatorRewardsHandler},
        BanConfigStruct, BanInfo, CurrentAndNextSessionValidators, ValidatorExtractor,
        ValidatorTotalRewards, STORAGE_VERSION,
    };

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Something that handles bans
        type BanHandler: BanHandler<AccountId = Self::AccountId>;
        /// Something that provides information about era.
        type EraInfoProvider: EraInfoProvider<AccountId = Self::AccountId>;
        /// Something that provides information about validator.
        type ValidatorProvider: ValidatorProvider<AccountId = Self::AccountId>;
        /// Something that handles addition of rewards for validators.
        type ValidatorRewardsHandler: ValidatorRewardsHandler<AccountId = Self::AccountId>;
        /// Something that handles removal of the validators
        type ValidatorExtractor: ValidatorExtractor<AccountId = Self::AccountId>;
        /// Nr of blocks in the session.
        #[pallet::constant]
        type SessionPeriod: Get<u32>;
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// A lookup how many blocks a validator produced.
    #[pallet::storage]
    pub type SessionValidatorBlockCount<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, BlockCount, ValueQuery>;

    /// Total possible reward per validator for the current era.
    #[pallet::storage]
    pub type ValidatorEraTotalReward<T: Config> =
        StorageValue<_, ValidatorTotalRewards<T::AccountId>, OptionQuery>;

    /// Current era config for ban functionality, see [`BanConfig`]
    #[pallet::storage]
    pub type BanConfig<T> = StorageValue<_, BanConfigStruct, ValueQuery>;

    /// A lookup for a number of underperformance sessions for a given validator
    #[pallet::storage]
    pub type UnderperformedValidatorSessionCount<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, SessionCount, ValueQuery>;

    /// Validators to be removed from non reserved list in the next era
    #[pallet::storage]
    pub type Banned<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, BanInfo>;

    /// SessionValidators in the current session.
    #[pallet::storage]
    pub(crate) type CurrentAndNextSessionValidatorsStorage<T: Config> =
        StorageValue<_, CurrentAndNextSessionValidators<T::AccountId>, ValueQuery>;

    #[pallet::error]
    pub enum Error<T> {
        /// Raised in any scenario [`BanConfig`] is invalid
        /// * `performance_ratio_threshold` must be a number in range [0; 100]
        /// * `underperformed_session_count_threshold` must be a positive number,
        /// * `clean_session_counter_delay` must be a positive number.
        InvalidBanConfig,

        /// Ban reason is too big, ie given vector of bytes is greater than
        /// [`Config::MaximumBanReasonLength`]
        BanReasonTooBig,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Ban thresholds for the next era has changed
        SetBanConfig(BanConfigStruct),

        /// Validators have been banned from the committee
        BanValidators(Vec<(T::AccountId, BanInfo)>),
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Sets ban config, it has an immediate effect
        #[pallet::call_index(1)]
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn set_ban_config(
            origin: OriginFor<T>,
            minimal_expected_performance: Option<u8>,
            underperformed_session_count_threshold: Option<u32>,
            clean_session_counter_delay: Option<u32>,
            ban_period: Option<EraIndex>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let mut current_committee_ban_config = BanConfig::<T>::get();

            if let Some(minimal_expected_performance) = minimal_expected_performance {
                ensure!(
                    minimal_expected_performance <= 100,
                    Error::<T>::InvalidBanConfig
                );
                current_committee_ban_config.minimal_expected_performance =
                    Perbill::from_percent(minimal_expected_performance as u32);
            }
            if let Some(underperformed_session_count_threshold) =
                underperformed_session_count_threshold
            {
                ensure!(
                    underperformed_session_count_threshold > 0,
                    Error::<T>::InvalidBanConfig
                );
                current_committee_ban_config.underperformed_session_count_threshold =
                    underperformed_session_count_threshold;
            }
            if let Some(clean_session_counter_delay) = clean_session_counter_delay {
                ensure!(
                    clean_session_counter_delay > 0,
                    Error::<T>::InvalidBanConfig
                );
                current_committee_ban_config.clean_session_counter_delay =
                    clean_session_counter_delay;
            }
            if let Some(ban_period) = ban_period {
                ensure!(ban_period > 0, Error::<T>::InvalidBanConfig);
                current_committee_ban_config.ban_period = ban_period;
            }

            BanConfig::<T>::put(current_committee_ban_config.clone());
            Self::deposit_event(Event::SetBanConfig(current_committee_ban_config));

            Ok(())
        }

        /// Schedule a non-reserved node to be banned out from the committee at the end of the era
        #[pallet::call_index(2)]
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn ban_from_committee(
            origin: OriginFor<T>,
            banned: T::AccountId,
            ban_reason: Vec<u8>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let bounded_description: BoundedVec<_, _> = ban_reason
                .try_into()
                .map_err(|_| Error::<T>::BanReasonTooBig)?;

            let reason = BanReason::OtherReason(bounded_description);
            Self::ban_validator(&banned, reason);

            Ok(())
        }

        /// Cancel the ban of the node
        #[pallet::call_index(3)]
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn cancel_ban(origin: OriginFor<T>, banned: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            Banned::<T>::remove(banned);

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub committee_ban_config: BanConfigStruct,
        pub session_validators: SessionValidators<T::AccountId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                committee_ban_config: Default::default(),
                session_validators: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <BanConfig<T>>::put(self.committee_ban_config.clone());
            <CurrentAndNextSessionValidatorsStorage<T>>::put(CurrentAndNextSessionValidators {
                current: self.session_validators.clone(),
                next: self.session_validators.clone(),
            })
        }
    }
}
