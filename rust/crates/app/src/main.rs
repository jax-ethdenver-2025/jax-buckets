mod args;
mod op;
mod ops;

use args::Args;
use clap::{Parser, Subcommand};
use op::Op;
use ops::{Service, Version};

command_enum! {
    (Service, Service),
    (Version, Version),
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.command.execute().await {
        Ok(output) => {
            println!("{}", output);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
