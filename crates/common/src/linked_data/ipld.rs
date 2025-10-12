pub use ipld_core::cid::multihash::Multihash;
pub use ipld_core::cid::{multibase, Cid, Error as CidError};
pub use ipld_core::codec::Codec;
pub use ipld_core::ipld::Ipld as LinkedData;

// Raw ipld codec
pub const LD_RAW_CODEC: u64 = 0x55;
pub const LD_CBOR_CODEC: u64 = 0x71;

pub const BLAKE3_HASH_CODE: u64 = 0x1e;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("encoding error")]
    EncodeError,
    #[error("decoding error")]
    DecodeError,
}

// TODO (amiller68): this seems silly, but saves
//  some boilerplate
pub trait BlockEncoded<C>: serde::Serialize + serde::de::DeserializeOwned
where
    C: Codec<Self>,
{
    fn encode(&self) -> Result<Vec<u8>, CodecError> {
        C::encode_to_vec(self).map_err(|_| CodecError::EncodeError)
    }
    fn decode(data: &[u8]) -> Result<Self, CodecError> {
        C::decode_from_slice(data).map_err(|_e| CodecError::DecodeError)
    }
    fn codec(&self) -> u64 {
        C::CODE
    }
}
