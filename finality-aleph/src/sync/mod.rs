use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

use codec::Codec;

mod data;
mod forest;
mod handler;
#[cfg(test)]
mod mock;
mod substrate;
mod task_queue;
mod ticker;

pub use substrate::SessionVerifier;

const LOG_TARGET: &str = "aleph-block-sync";

/// The identifier of a connected peer.
pub trait PeerId: Clone + Hash + Eq {}

impl<T: Clone + Hash + Eq> PeerId for T {}

/// The identifier of a block, the least amount of knowledge we can have about a block.
pub trait BlockIdentifier: Clone + Hash + Debug + Eq + Codec + Send + Sync + 'static {
    /// The block number, useful when reasoning about hopeless forks.
    fn number(&self) -> u32;
}

/// Informs the sync that it should attempt to acquire the specified data.
pub trait Requester<BI: BlockIdentifier> {
    /// The sync should attempt to acquire justifications for this block.
    fn request_justification(&self, id: BI);
}

/// The header of a block, containing information about the parent relation.
pub trait Header: Clone + Codec + Send + Sync + 'static {
    type Identifier: BlockIdentifier;

    /// The identifier of this block.
    fn id(&self) -> Self::Identifier;

    /// The identifier of this block's parent.
    fn parent_id(&self) -> Option<Self::Identifier>;
}

/// The verified justification of a block, including a header.
pub trait Justification: Clone + Send + Sync + 'static {
    type Header: Header;
    type Unverified: Clone + Codec + Debug + Send + Sync + 'static;

    /// The header of the block.
    fn header(&self) -> &Self::Header;

    /// Return an unverified version of this, for sending over the network.
    fn into_unverified(self) -> Self::Unverified;
}

type BlockIdFor<J> = <<J as Justification>::Header as Header>::Identifier;

/// A verifier of justifications.
pub trait Verifier<J: Justification> {
    type Error: Display;

    /// Verifies the raw justification and returns a full justification if successful, otherwise an
    /// error.
    fn verify(&mut self, justification: J::Unverified) -> Result<J, Self::Error>;
}

/// A facility for finalizing blocks using justifications.
pub trait Finalizer<J: Justification> {
    type Error: Display;

    /// Finalize a block using this justification. Since the justification contains the header, we
    /// don't need to additionally specify the block.
    fn finalize(&self, justification: J) -> Result<(), Self::Error>;
}

/// A notification about the chain status changing.
#[derive(Clone, Debug)]
pub enum ChainStatusNotification<BI: BlockIdentifier> {
    /// A block has been imported.
    BlockImported(BI),
    /// A block has been finalized.
    BlockFinalized(BI),
}

/// A stream of notifications about the chain status in the database changing.
#[async_trait::async_trait]
pub trait ChainStatusNotifier<BI: BlockIdentifier> {
    type Error: Display;

    /// Returns a chain status notification when it is available.
    async fn next(&mut self) -> Result<ChainStatusNotification<BI>, Self::Error>;
}

/// The status of a block in the database.
pub enum BlockStatus<J: Justification> {
    /// The block is justified and thus finalized.
    Justified(J),
    /// The block is present, might be finalized if a descendant is justified.
    Present(J::Header),
    /// The block is not known.
    Unknown,
}

/// The knowledge about the chain status.
pub trait ChainStatus<J: Justification> {
    type Error: Display;

    /// The status of the block.
    fn status_of(
        &self,
        id: <J::Header as Header>::Identifier,
    ) -> Result<BlockStatus<J>, Self::Error>;

    /// The justification at this block number, if we have it. Should return None if the
    /// request is above the top finalized.
    fn finalized_at(&self, number: u32) -> Result<Option<J>, Self::Error>;

    /// The header of the best block.
    fn best_block(&self) -> Result<J::Header, Self::Error>;

    /// The justification of the top finalized block.
    fn top_finalized(&self) -> Result<J, Self::Error>;
}
