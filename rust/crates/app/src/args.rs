pub use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "cli")]
#[command(about = "A basic CLI example")]
pub struct Args {
    #[command(subcommand)]
    pub command: crate::Command,
}
