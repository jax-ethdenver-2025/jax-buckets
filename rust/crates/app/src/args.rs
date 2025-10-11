pub use clap::Parser;

use std::path::PathBuf;
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "cli")]
#[command(about = "A basic CLI example")]
pub struct Args {
    #[arg(long, global = true, default_value = "http://localhost:3000")]
    pub remote: Url,

    /// Path to the jax config directory (defaults to ~/.jax)
    #[arg(long, global = true)]
    pub config_path: Option<PathBuf>,

    #[command(subcommand)]
    pub command: crate::Command,
}
