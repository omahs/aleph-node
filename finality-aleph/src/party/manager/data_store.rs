use std::fmt::Debug;

use aleph_primitives::BlockNumber;
use codec::Codec;
use futures::channel::oneshot;
use log::debug;
use sc_client_api::{BlockchainEvents, HeaderBackend};
use sp_runtime::traits::{Block, Header};

use crate::{
    abft::SpawnHandleT,
    data_io::{AlephNetworkMessage, DataStore},
    network::{data::component::Receiver, RequestBlocks},
    party::{AuthoritySubtaskCommon, Task},
};

/// Runs the data store within a single session.
pub fn task<B, C, RB, R, Message>(
    subtask_common: AuthoritySubtaskCommon,
    mut data_store: DataStore<B, C, RB, Message, R>,
) -> Task
where
    B: Block,
    B::Header: Header<Number = BlockNumber>,
    C: HeaderBackend<B> + BlockchainEvents<B> + Send + Sync + 'static,
    RB: RequestBlocks<B> + 'static,
    Message: AlephNetworkMessage<B> + Debug + Send + Sync + Codec + 'static,
    R: Receiver<Message> + 'static,
{
    let AuthoritySubtaskCommon {
        spawn_handle,
        session_id,
    } = subtask_common;
    let (stop, exit) = oneshot::channel();
    let task = {
        async move {
            debug!(target: "aleph-party", "Running the data store task for {:?}", session_id);
            data_store.run(exit).await;
            debug!(target: "aleph-party", "Data store task stopped for {:?}", session_id);
        }
    };

    let handle = spawn_handle.spawn_essential("aleph/consensus_session_data_store", task);
    Task::new(handle, stop)
}
