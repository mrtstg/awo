mod args;
use clap::Parser;
use std::fs;

#[tokio::main]
async fn main() {
    let run_args = args::Args::parse();
    let cfg_read = fs::read_to_string(run_args.config);
    match cfg_read {
        Err(e) => println!("Failed reading config: {:?}", e),
        Ok(cfg_string) => {
            // todo: parse
        }
    }
}
