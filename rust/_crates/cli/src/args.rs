use std::error::Error;

use clap::Subcommand;

use super::ops::Add as AddOp;
use super::ops::Init as InitOp;
use super::ops::Pull as PullOp;
use super::ops::Push as PushOp;
use super::ops::Stat as StatOp;
use super::AppState;

pub use clap::Parser;

use std::fmt;

#[async_trait::async_trait]
pub trait Op: Send + Sync {
    type Error: Error + Send + Sync + 'static;
    type Output;

    async fn execute(&self, state: &AppState) -> Result<Self::Output, Self::Error>;
}

#[macro_export]
macro_rules! command_enum {
    ($(($variant:ident, $type:ty)),* $(,)?) => {
        #[derive(Subcommand, Debug, Clone)]
        pub enum Command {
            $($variant($type),)*
        }

        #[derive(Debug)]
        pub enum OpOutput {
            $($variant(<$type as Op>::Output),)*
        }

        #[derive(Debug, thiserror::Error)]
        pub enum OpError {
            $(
                #[error(transparent)]
                $variant(<$type as Op>::Error),
            )*
        }

        #[async_trait::async_trait]
        impl Op for Command {
            type Output = OpOutput;
            type Error = OpError;

            async fn execute(&self, state: &AppState) -> Result<Self::Output, Self::Error> {
                match self {
                    $(
                        Command::$variant(op) => {
                            op.execute(state).await
                                .map(OpOutput::$variant)
                                .map_err(OpError::$variant)
                        },
                    )*
                }
            }
        }
    };
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

use crate::command_enum;

command_enum! {
    (Init, InitOp),
    (Add, AddOp),
    (Pull, PullOp),
    (Push, PushOp),
    (Stat, StatOp),
}

impl fmt::Display for OpOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpOutput::Init((cid, path)) => write!(
                f,
                r#"
                Remote initialized with cid: {} \n
                Please register the following public key with the remote: {}
            "#,
                cid,
                path.display()
            ),
            OpOutput::Add(o) => write!(f, "{}", o),
            OpOutput::Pull(o) => write!(f, "{}", o),
            OpOutput::Push(o) => write!(f, "{}", o),
            OpOutput::Stat(o) => write!(f, "{}", o),
        }
    }
}
