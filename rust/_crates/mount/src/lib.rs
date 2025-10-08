#[allow(unused_imports)]
#[allow(dead_code)]
mod api;
mod ipfs_rpc;
mod mount;
mod types;

pub mod prelude {
    pub use crate::api::{
        api_requests::{PullRoot, PushRoot},
        ApiClient,
    };
    pub use crate::ipfs_rpc::IpfsRpc;
    pub use crate::mount::{BlockCache, Mount, MountError};
    pub use crate::types::Schema;
    pub use crate::types::{Cid, Ipld, Manifest, MhCode, NodeLink, Object, Version};
}

pub mod error {
    pub use crate::api::ApiError;
    pub use crate::ipfs_rpc::IpfsRpcError;
    pub use crate::mount::MountError;
    pub use crate::types::CidError;
}
