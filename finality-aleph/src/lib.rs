extern crate core;

use std::{fmt::Debug, hash::Hash, path::PathBuf, sync::Arc};

use aleph_primitives::BlockNumber;
use codec::{Decode, Encode, Output};
use derive_more::Display;
use futures::{
    channel::{mpsc, oneshot},
    Future,
};
use sc_client_api::{Backend, BlockchainEvents, Finalizer, LockImportRun, TransactionFor};
use sc_consensus::BlockImport;
use sc_network::NetworkService;
use sc_network_common::ExHashT;
use sc_service::SpawnTaskHandle;
use sp_api::{NumberFor, ProvideRuntimeApi};
use sp_blockchain::{HeaderBackend, HeaderMetadata};
use sp_keystore::CryptoStore;
use sp_runtime::traits::{BlakeTwo256, Block, Header};
use tokio::time::Duration;

use crate::{
    abft::{CurrentNetworkData, LegacyNetworkData, CURRENT_VERSION, LEGACY_VERSION},
    aggregation::{CurrentRmcNetworkData, LegacyRmcNetworkData},
    network::data::split::Split,
    session::{SessionBoundaries, SessionBoundaryInfo, SessionId},
    VersionedTryFromError::{ExpectedNewGotOld, ExpectedOldGotNew},
};

mod abft;
mod aggregation;
mod compatibility;
mod crypto;
mod data_io;
mod finalization;
mod import;
mod justification;
pub mod metrics;
mod network;
mod nodes;
mod party;
mod session;
mod session_map;
// TODO: remove when module is used
#[allow(dead_code)]
mod sync;
#[cfg(test)]
pub mod testing;

pub use abft::{Keychain, NodeCount, NodeIndex, Recipient, SignatureSet, SpawnHandle};
pub use aleph_primitives::{AuthorityId, AuthorityPair, AuthoritySignature};
pub use import::AlephBlockImport;
pub use justification::{AlephJustification, JustificationNotification};
pub use network::{Protocol, ProtocolNaming};
pub use nodes::{run_nonvalidator_node, run_validator_node};
pub use session::SessionPeriod;

use crate::compatibility::{Version, Versioned};
pub use crate::metrics::Metrics;

/// Constant defining how often components of finality-aleph should report their state
const STATUS_REPORT_INTERVAL: Duration = Duration::from_secs(20);

#[derive(Clone, Debug, Encode, Decode)]
enum Error {
    SendData,
}

/// Returns a NonDefaultSetConfig for the specified protocol.
pub fn peers_set_config(
    naming: ProtocolNaming,
    protocol: Protocol,
) -> sc_network_common::config::NonDefaultSetConfig {
    let mut config = sc_network_common::config::NonDefaultSetConfig::new(
        naming.protocol_name(&protocol),
        // max_notification_size should be larger than the maximum possible honest message size (in bytes).
        // Max size of alert is UNIT_SIZE * MAX_UNITS_IN_ALERT ~ 100 * 5000 = 50000 bytes
        // Max size of parents response UNIT_SIZE * N_MEMBERS ~ 100 * N_MEMBERS
        // When adding other (large) message types we need to make sure this limit is fine.
        1024 * 1024,
    );

    config.set_config = sc_network_common::config::SetConfig::default();
    config.add_fallback_names(naming.fallback_protocol_names(&protocol));
    config
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd, Encode, Decode)]
pub struct MillisecsPerBlock(pub u64);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd, Encode, Decode)]
pub struct UnitCreationDelay(pub u64);

pub type LegacySplitData<B> = Split<LegacyNetworkData<B>, LegacyRmcNetworkData<B>>;
pub type CurrentSplitData<B> = Split<CurrentNetworkData<B>, CurrentRmcNetworkData<B>>;

impl<B: Block> Versioned for LegacyNetworkData<B> {
    const VERSION: Version = Version(LEGACY_VERSION);
}

impl<B: Block> Versioned for CurrentNetworkData<B> {
    const VERSION: Version = Version(CURRENT_VERSION);
}

/// The main purpose of this data type is to enable a seamless transition between protocol versions at the Network level. It
/// provides a generic implementation of the Decode and Encode traits (LE byte representation) by prepending byte
/// representations for provided type parameters with their version (they need to implement the `Versioned` trait). If one
/// provides data types that declares equal versions, the first data type parameter will have priority while decoding. Keep in
/// mind that in such case, `decode` might fail even if the second data type would be able decode provided byte representation.
#[derive(Clone)]
pub enum VersionedEitherMessage<L, R> {
    Left(L),
    Right(R),
}

impl<L: Versioned + Decode, R: Versioned + Decode> Decode for VersionedEitherMessage<L, R> {
    fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
        let version = Version::decode(input)?;
        if version == L::VERSION {
            return Ok(VersionedEitherMessage::Left(L::decode(input)?));
        }
        if version == R::VERSION {
            return Ok(VersionedEitherMessage::Right(R::decode(input)?));
        }
        Err("Invalid version while decoding VersionedEitherMessage".into())
    }
}

impl<L: Versioned + Encode, R: Versioned + Encode> Encode for VersionedEitherMessage<L, R> {
    fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
        match self {
            VersionedEitherMessage::Left(left) => {
                L::VERSION.encode_to(dest);
                left.encode_to(dest);
            }
            VersionedEitherMessage::Right(right) => {
                R::VERSION.encode_to(dest);
                right.encode_to(dest);
            }
        }
    }

    fn size_hint(&self) -> usize {
        match self {
            VersionedEitherMessage::Left(left) => L::VERSION.size_hint() + left.size_hint(),
            VersionedEitherMessage::Right(right) => R::VERSION.size_hint() + right.size_hint(),
        }
    }
}

pub type VersionedNetworkData<B> = VersionedEitherMessage<LegacySplitData<B>, CurrentSplitData<B>>;

#[derive(Debug, Display, Clone)]
pub enum VersionedTryFromError {
    ExpectedNewGotOld,
    ExpectedOldGotNew,
}

impl<B: Block> TryFrom<VersionedNetworkData<B>> for LegacySplitData<B> {
    type Error = VersionedTryFromError;

    fn try_from(value: VersionedNetworkData<B>) -> Result<Self, Self::Error> {
        Ok(match value {
            VersionedEitherMessage::Left(data) => data,
            VersionedEitherMessage::Right(_) => return Err(ExpectedOldGotNew),
        })
    }
}
impl<B: Block> TryFrom<VersionedNetworkData<B>> for CurrentSplitData<B> {
    type Error = VersionedTryFromError;

    fn try_from(value: VersionedNetworkData<B>) -> Result<Self, Self::Error> {
        Ok(match value {
            VersionedEitherMessage::Left(_) => return Err(ExpectedNewGotOld),
            VersionedEitherMessage::Right(data) => data,
        })
    }
}

impl<B: Block> From<LegacySplitData<B>> for VersionedNetworkData<B> {
    fn from(data: LegacySplitData<B>) -> Self {
        VersionedEitherMessage::Left(data)
    }
}

impl<B: Block> From<CurrentSplitData<B>> for VersionedNetworkData<B> {
    fn from(data: CurrentSplitData<B>) -> Self {
        VersionedEitherMessage::Right(data)
    }
}

pub trait ClientForAleph<B, BE>:
    LockImportRun<B, BE>
    + Finalizer<B, BE>
    + ProvideRuntimeApi<B>
    + BlockImport<B, Transaction = TransactionFor<BE, B>, Error = sp_consensus::Error>
    + HeaderBackend<B>
    + HeaderMetadata<B, Error = sp_blockchain::Error>
    + BlockchainEvents<B>
where
    BE: Backend<B>,
    B: Block,
{
}

impl<B, BE, T> ClientForAleph<B, BE> for T
where
    BE: Backend<B>,
    B: Block,
    T: LockImportRun<B, BE>
        + Finalizer<B, BE>
        + ProvideRuntimeApi<B>
        + HeaderBackend<B>
        + HeaderMetadata<B, Error = sp_blockchain::Error>
        + BlockchainEvents<B>
        + BlockImport<B, Transaction = TransactionFor<BE, B>, Error = sp_consensus::Error>,
{
}

type Hasher = abft::HashWrapper<BlakeTwo256>;

#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub struct HashNum<H, N> {
    hash: H,
    num: N,
}

impl<H> HashNum<H, BlockNumber> {
    fn new(hash: H, num: BlockNumber) -> Self {
        HashNum { hash, num }
    }
}

impl<H> From<(H, BlockNumber)> for HashNum<H, BlockNumber> {
    fn from(pair: (H, BlockNumber)) -> Self {
        HashNum::new(pair.0, pair.1)
    }
}

pub type BlockHashNum<B> = HashNum<<B as Block>::Hash, NumberFor<B>>;

pub struct AlephConfig<B: Block, H: ExHashT, C, SC, BB> {
    pub network: Arc<NetworkService<B, H>>,
    pub client: Arc<C>,
    pub blockchain_backend: BB,
    pub select_chain: SC,
    pub spawn_handle: SpawnTaskHandle,
    pub keystore: Arc<dyn CryptoStore>,
    pub justification_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
    pub metrics: Option<Metrics<<B::Header as Header>::Hash>>,
    pub session_period: SessionPeriod,
    pub millisecs_per_block: MillisecsPerBlock,
    pub unit_creation_delay: UnitCreationDelay,
    pub backup_saving_path: Option<PathBuf>,
    pub external_addresses: Vec<String>,
    pub validator_port: u16,
    pub protocol_naming: ProtocolNaming,
}

pub trait BlockchainBackend<B: Block> {
    fn children(&self, parent_hash: <B as Block>::Hash) -> Vec<<B as Block>::Hash>;
    fn info(&self) -> sp_blockchain::Info<B>;
    fn header(
        &self,
        block_id: sp_api::BlockId<B>,
    ) -> sp_blockchain::Result<Option<<B as Block>::Header>>;
}
