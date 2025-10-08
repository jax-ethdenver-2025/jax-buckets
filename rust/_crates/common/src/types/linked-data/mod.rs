pub use libipld::cbor::DagCborCodec;
pub use libipld::cid::multihash::Code as MhCode;
pub use libipld::cid::Error as CidError;
pub use libipld::store::DefaultParams;
pub use libipld::Block;
pub use libipld::Cid;
pub use libipld::Ipld as Ld;
pub use libipld::IpldCodec as LdCodec;

// TODO: find a better home for these

pub const DEFAULT_LD_CODEC: LdCodec = LdCodec::DagCbor;
pub const DEFAULT_LD_CODEC_STRING: &str = "dag-cbor";
pub const RAW_LD_CODEC: LdCodec = LdCodec::Raw;
pub const RAW_LD_CODEC_STRING: &str = "raw";
pub const DEFAULT_HASH_CODE: MhCode = MhCode::Blake3_256;
pub const DEFAULT_HASH_CODE_STRING: &str = "blake3";
pub const DEFAULT_CID_VERSION: u32 = 1;

mod utils;

pub use utils::{
    block_from_data,
    ipld_from_block,
    ipld_to_block,
    ipld_to_cid,
};
