use log::info;
use primitives::{BlockNumber, EraIndex, SessionIndex};

use crate::{
    pallets::{elections::ElectionsApi, staking::StakingApi},
    BlockHash, Connection,
};

/// Block info API.
#[async_trait::async_trait]
pub trait BlocksApi {
    /// Returns the first block of a session.
    /// * `session` - number of the session to query the first block from
    async fn first_block_of_session(
        &self,
        session: SessionIndex,
    ) -> anyhow::Result<Option<BlockHash>>;

    /// Returns hash of a given block if the given block exists, otherwise `None`
    /// * `block` - number of the block
    async fn get_block_hash(&self, block: BlockNumber) -> anyhow::Result<Option<BlockHash>>;

    /// Returns the most recent block from the current best chain.
    async fn get_best_block(&self) -> anyhow::Result<Option<BlockNumber>>;

    /// Returns the most recent block from the finalized chain.
    async fn get_finalized_block_hash(&self) -> anyhow::Result<BlockHash>;

    /// Returns number of a given block hash, if the given block exists, otherwise `None`
    /// * `block` - hash of the block to query its number
    async fn get_block_number(&self, block: BlockHash) -> anyhow::Result<Option<BlockNumber>>;
}

/// Interaction logic between pallet session and pallet staking.
#[async_trait::async_trait]
pub trait SessionEraApi {
    /// Returns which era given session is.
    /// * `session` - session index
    async fn get_active_era_for_session(&self, session: SessionIndex) -> anyhow::Result<EraIndex>;
}

impl Connection {
    async fn get_block_number_inner(
        &self,
        block: Option<BlockHash>,
    ) -> anyhow::Result<Option<BlockNumber>> {
        self.client
            .rpc()
            .header(block)
            .await
            .map(|maybe_header| maybe_header.map(|header| header.number))
            .map_err(|e| e.into())
    }
}

#[async_trait::async_trait]
impl BlocksApi for Connection {
    async fn first_block_of_session(
        &self,
        session: SessionIndex,
    ) -> anyhow::Result<Option<BlockHash>> {
        let period = self.get_session_period().await?;
        let block_num = period * session;

        self.get_block_hash(block_num).await
    }

    async fn get_block_hash(&self, block: BlockNumber) -> anyhow::Result<Option<BlockHash>> {
        info!(target: "aleph-client", "querying block hash for number #{}", block);
        self.client
            .rpc()
            .block_hash(Some(block.into()))
            .await
            .map_err(|e| e.into())
    }

    async fn get_best_block(&self) -> anyhow::Result<Option<BlockNumber>> {
        self.get_block_number_inner(None).await
    }

    async fn get_finalized_block_hash(&self) -> anyhow::Result<BlockHash> {
        self.client
            .rpc()
            .finalized_head()
            .await
            .map_err(|e| e.into())
    }

    async fn get_block_number(&self, block: BlockHash) -> anyhow::Result<Option<BlockNumber>> {
        self.get_block_number_inner(Some(block)).await
    }
}

#[async_trait::async_trait]
impl SessionEraApi for Connection {
    async fn get_active_era_for_session(&self, session: SessionIndex) -> anyhow::Result<EraIndex> {
        let block = self.first_block_of_session(session).await?;
        Ok(self.get_active_era(block).await)
    }
}
