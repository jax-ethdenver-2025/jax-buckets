#![allow(dead_code)]
#![allow(clippy::result_large_err)]
#![allow(unused_imports)]
#![allow(unused_variables)]

mod cli;

use cli::{Args, Op, Parser};

#[tokio::main]
async fn main() {
    // Run the app and capture any errors
    let args = Args::parse();
    let op = args.command.clone();
    match op.execute().await {
        Ok(r) => {
            println!("{}", r);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("error: {:?}", e);
            std::process::exit(1);
        }
    };
}
