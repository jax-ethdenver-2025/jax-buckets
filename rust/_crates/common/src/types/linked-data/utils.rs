use super::{Block, Cid, DefaultParams, Ld, LdCodec};
use super::{DEFAULT_LD_CODEC, DEFAULT_HASH_CODE};

pub fn ld_to_cid<T: Into<Ld>>(ld: T) -> Cid {
    let block = ld_to_block(ld);
    *block.cid()
}

pub fn ld_to_block<T: Into<Ld>>(ld: T) -> Block<DefaultParams> {
    Block::<DefaultParams>::encode(DEFAULT_LD_CODEC, DEFAULT_HASH_CODE, &ld.into()).unwrap()
}

pub fn block_from_data(cid: Cid, data: Vec<u8>) -> Result<Block<DefaultParams>, anyhow::Error> {
    Block::<DefaultParams>::new(cid, data)
        .map_err(|e| anyhow::anyhow!("Error creating block: {}", e))
}

// NOTE: enforce dag cbor everything when encoding to blocks
pub fn ld_from_block(block: Block<DefaultParams>) -> Result<Ld, anyhow::Error> {
    block
        .decode::<DagCborCodec, Ld>()
        .map_err(|e| anyhow::anyhow!("Error decoding block: {}", e))
}
