use clap::{Args, Subcommand};

pub mod create;
pub mod list;

use crate::op::Op;
use service::http_server::api::v0::bucket::{CreateRequest, ListRequest};

crate::command_enum! {
    (Create, CreateRequest),
    (List, ListRequest),
}

// Rename the generated Command to BucketCommand for clarity
pub type BucketCommand = Command;

#[derive(Args, Debug, Clone)]
pub struct Bucket {
    #[command(subcommand)]
    pub command: BucketCommand,
}

#[async_trait::async_trait]
impl Op for Bucket {
    type Error = OpError;
    type Output = OpOutput;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        self.command.execute(ctx).await
    }
}
