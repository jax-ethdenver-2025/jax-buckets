pub use clap::Parser;

use url::Url;

#[derive(Parser, Debug)]
#[command(name = "cli")]
#[command(about = "A basic CLI example")]
pub struct Args {
    #[arg(long, global = true, default_value = "http://localhost:3000")]
    pub remote: Url,

    #[command(subcommand)]
    pub command: crate::Command,
}
