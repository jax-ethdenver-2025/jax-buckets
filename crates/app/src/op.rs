use std::error::Error;
use std::path::PathBuf;

use url::Url;

use service::http_server::api::client::{ApiClient, ApiError};

#[derive(Clone)]
pub struct OpContext {
    /// API client (always initialized with default or custom URL)
    pub client: ApiClient,
    /// Optional custom config path (defaults to ~/.jax)
    pub config_path: Option<PathBuf>,
}

impl OpContext {
    /// Create context with custom remote URL and optional config path
    pub fn new(remote: Url, config_path: Option<PathBuf>) -> Result<Self, ApiError> {
        Ok(Self {
            client: ApiClient::new(&remote)?,
            config_path,
        })
    }
}

#[async_trait::async_trait]
pub trait Op: Send + Sync {
    type Error: Error + Send + Sync + 'static;
    type Output;

    async fn execute(&self, ctx: &OpContext) -> Result<Self::Output, Self::Error>;
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
            $($variant(<$type as $crate::Op>::Output),)*
        }

        #[derive(Debug, thiserror::Error)]
        pub enum OpError {
            $(
                #[error(transparent)]
                $variant(<$type as $crate::Op>::Error),
            )*
        }

        #[async_trait::async_trait]
        impl $crate::Op for Command {
            type Output = OpOutput;
            type Error = OpError;

            async fn execute(&self, ctx: &$crate::op::OpContext) -> Result<Self::Output, Self::Error> {
                match self {
                    $(
                        Command::$variant(op) => {
                            op.execute(ctx).await
                                .map(OpOutput::$variant)
                                .map_err(OpError::$variant)
                        },
                    )*
                }
            }
        }

        impl std::fmt::Display for OpOutput {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        OpOutput::$variant(output) => write!(f, "{}", output),
                    )*
                }
            }
        }
    };
}
