mod args;
mod op;
mod ops;

use args::Args;
use clap::{Parser, Subcommand};
use op::Op;
use ops::{Bucket, Service, Version};

command_enum! {
    (Bucket, Bucket),
    (Service, Service),
    (Version, Version),
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Build context - always has API client initialized
    let ctx = match op::OpContext::new(args.remote) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Error: Failed to create API client: {}", e);
            std::process::exit(1);
        }
    };

    match args.command.execute(&ctx).await {
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
