use frame_support::{
    construct_runtime,
    pallet_prelude::ConstU32,
    sp_runtime,
    sp_runtime::{
        testing::{Header, H256},
        traits::IdentityLookup,
    },
    traits::Everything,
};
use frame_system::mocking::{MockBlock, MockUncheckedExtrinsic};
use sp_io::TestExternalities;
use sp_runtime::traits::BlakeTwo256;

use crate as pallet_snarcos;

construct_runtime!(
    pub enum TestRuntime where
        Block = MockBlock<TestRuntime>,
        NodeBlock = MockBlock<TestRuntime>,
        UncheckedExtrinsic = MockUncheckedExtrinsic<TestRuntime>,
    {
        System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
        Snarcos: pallet_snarcos::{Pallet, Call, Storage, Event<T>},
    }
);

impl frame_system::Config for TestRuntime {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u128;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = ();
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

impl pallet_snarcos::Config for TestRuntime {
    type Event = Event;
    type WeightInfo = ();
    type MaximumVerificationKeyLength = ConstU32<10_000>;
}

pub(super) fn new_test_ext() -> TestExternalities {
    let t = frame_system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap();

    TestExternalities::new(t)
}
