pub use libipld::cbor::DagCborCodec;
pub use libipld::cid::multihash::Code as MhCode;
pub use libipld::cid::Error as CidError;
pub use libipld::store::DefaultParams;
pub use libipld::Block;
pub use libipld::Cid;
pub use libipld::Ipld;
pub use libipld::IpldCodec;

// TODO: find a better home for these

pub const DEFAULT_IPLD_CODEC: IpldCodec = IpldCodec::DagCbor;
pub const DEFAULT_IPLD_CODEC_STRING: &str = "dag-cbor";
pub const RAW_IPLD_CODEC: IpldCodec = IpldCodec::Raw;
pub const RAW_IPLD_CODEC_STRING: &str = "raw";
pub const DEFAULT_HASH_CODE: MhCode = MhCode::Blake3_256;
pub const DEFAULT_HASH_CODE_STRING: &str = "blake3";
pub const DEFAULT_CID_VERSION: u32 = 1;

pub fn ipld_to_cid<T: Into<Ipld>>(ipld: T) -> Cid {
    let block = ipld_to_block(ipld);
    *block.cid()
}

pub fn ipld_to_block<T: Into<Ipld>>(ipld: T) -> Block<DefaultParams> {
    Block::<DefaultParams>::encode(DEFAULT_IPLD_CODEC, DEFAULT_HASH_CODE, &ipld.into()).unwrap()
}

pub fn block_from_data(cid: Cid, data: Vec<u8>) -> Result<Block<DefaultParams>, anyhow::Error> {
    Block::<DefaultParams>::new(cid, data)
        .map_err(|e| anyhow::anyhow!("Error creating block: {}", e))
}

// NOTE: enforce dag cbor everything when encoding to blocks
pub fn ipld_from_block(block: Block<DefaultParams>) -> Result<Ipld, anyhow::Error> {
    block
        .decode::<DagCborCodec, Ipld>()
        .map_err(|e| anyhow::anyhow!("Error decoding block: {}", e))
}
