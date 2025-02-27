use primitives::{EraIndex, SessionCount};

use crate::{
    aleph_runtime::RuntimeCall::CommitteeManagement,
    api,
    pallet_committee_management::pallet::Call::{ban_from_committee, set_ban_config},
    primitives::{BanConfig, BanInfo, BanReason},
    AccountId, AsConnection, BlockHash, ConnectionApi, RootConnection, SudoCall, TxInfo, TxStatus,
};

/// Pallet CommitteeManagement read-only api.
#[async_trait::async_trait]
pub trait CommitteeManagementApi {
    /// Returns `committee-management.ban_config` storage of the committee-management pallet.
    /// * `at` - optional hash of a block to query state from
    async fn get_ban_config(&self, at: Option<BlockHash>) -> BanConfig;

    /// Returns `committee-management.session_validator_block_count` of a given validator.
    /// * `validator` - a validator stash account id
    /// * `at` - optional hash of a block to query state from
    async fn get_validator_block_count(
        &self,
        validator: AccountId,
        at: Option<BlockHash>,
    ) -> Option<u32>;

    /// Returns `committee-management.underperformed_validator_session_count` storage of a given validator.
    /// * `validator` - a validator stash account id
    /// * `at` - optional hash of a block to query state from
    async fn get_underperformed_validator_session_count(
        &self,
        validator: AccountId,
        at: Option<BlockHash>,
    ) -> Option<SessionCount>;

    /// Returns `committee-management.banned.reason` storage of a given validator.
    /// * `validator` - a validator stash account id
    /// * `at` - optional hash of a block to query state from
    async fn get_ban_reason_for_validator(
        &self,
        validator: AccountId,
        at: Option<BlockHash>,
    ) -> Option<BanReason>;

    /// Returns `committee-management.banned` storage of a given validator.
    /// * `validator` - a validator stash account id
    /// * `at` - optional hash of a block to query state from
    async fn get_ban_info_for_validator(
        &self,
        validator: AccountId,
        at: Option<BlockHash>,
    ) -> Option<BanInfo>;
    /// Returns `committee-management.session_period` const of the committee-management pallet.
    async fn get_session_period(&self) -> anyhow::Result<u32>;
}

/// any object that implements pallet committee-management api that requires sudo
#[async_trait::async_trait]
pub trait CommitteeManagementSudoApi {
    /// Issues `committee-management.set_ban_config`. It has an immediate effect.
    /// * `minimal_expected_performance` - performance ratio threshold in a session
    /// * `underperformed_session_count_threshold` - how many bad uptime sessions force validator to be removed from the committee
    /// * `clean_session_counter_delay` - underperformed session counter is cleared every subsequent `clean_session_counter_delay` sessions
    /// * `ban_period` - how many eras a validator is banned for
    /// * `status` - a [`TxStatus`] for a tx to wait for
    async fn set_ban_config(
        &self,
        minimal_expected_performance: Option<u8>,
        underperformed_session_count_threshold: Option<u32>,
        clean_session_counter_delay: Option<u32>,
        ban_period: Option<EraIndex>,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo>;

    /// Schedule a non-reserved node to be banned out from the committee at the end of the era.
    /// * `account` - account to be banned,
    /// * `ben_reason` - reaons for ban, expressed as raw bytes
    /// * `status` - a [`TxStatus`] for a tx to wait for
    async fn ban_from_committee(
        &self,
        account: AccountId,
        ban_reason: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo>;
}

#[async_trait::async_trait]
impl<C: ConnectionApi + AsConnection> CommitteeManagementApi for C {
    async fn get_ban_config(&self, at: Option<BlockHash>) -> BanConfig {
        let addrs = api::storage().committee_management().ban_config();

        self.get_storage_entry(&addrs, at).await
    }

    async fn get_validator_block_count(
        &self,
        validator: AccountId,
        at: Option<BlockHash>,
    ) -> Option<u32> {
        let addrs = api::storage()
            .committee_management()
            .session_validator_block_count(&validator);

        self.get_storage_entry_maybe(&addrs, at).await
    }

    async fn get_underperformed_validator_session_count(
        &self,
        validator: AccountId,
        at: Option<BlockHash>,
    ) -> Option<SessionCount> {
        let addrs = api::storage()
            .committee_management()
            .underperformed_validator_session_count(&validator);

        self.get_storage_entry_maybe(&addrs, at).await
    }

    async fn get_ban_reason_for_validator(
        &self,
        validator: AccountId,
        at: Option<BlockHash>,
    ) -> Option<BanReason> {
        let addrs = api::storage().committee_management().banned(validator);

        self.get_storage_entry_maybe(&addrs, at)
            .await
            .map(|x| x.reason)
    }

    async fn get_ban_info_for_validator(
        &self,
        validator: AccountId,
        at: Option<BlockHash>,
    ) -> Option<BanInfo> {
        let addrs = api::storage().committee_management().banned(validator);

        self.get_storage_entry_maybe(&addrs, at).await
    }

    async fn get_session_period(&self) -> anyhow::Result<u32> {
        let addrs = api::constants().committee_management().session_period();
        self.as_connection()
            .as_client()
            .constants()
            .at(&addrs)
            .map_err(|e| e.into())
    }
}

#[async_trait::async_trait]
impl CommitteeManagementSudoApi for RootConnection {
    async fn set_ban_config(
        &self,
        minimal_expected_performance: Option<u8>,
        underperformed_session_count_threshold: Option<u32>,
        clean_session_counter_delay: Option<u32>,
        ban_period: Option<EraIndex>,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo> {
        let call = CommitteeManagement(set_ban_config {
            minimal_expected_performance,
            underperformed_session_count_threshold,
            clean_session_counter_delay,
            ban_period,
        });

        self.sudo_unchecked(call, status).await
    }

    async fn ban_from_committee(
        &self,
        account: AccountId,
        ban_reason: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo> {
        let call = CommitteeManagement(ban_from_committee {
            banned: account,
            ban_reason,
        });
        self.sudo_unchecked(call, status).await
    }
}
