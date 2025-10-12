use clap::{Args, Subcommand};

pub mod add;
pub mod cat;
pub mod create;
pub mod list;
pub mod ls;
pub mod share;

use crate::op::Op;
use service::http_server::api::v0::bucket::{CreateRequest, ListRequest, ShareRequest};

crate::command_enum! {
    (Create, CreateRequest),
    (List, ListRequest),
    (Add, add::Add),
    (Ls, ls::Ls),
    (Cat, cat::Cat),
    (Share, ShareRequest),
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
