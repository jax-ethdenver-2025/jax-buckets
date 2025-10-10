#[allow(clippy::module_inception)]
mod mount;
mod pins;

pub use mount::{Bucket, BucketError};
// Temporary aliases for backward compatibility
pub use Bucket as Mount;
pub use BucketError as MountError;
