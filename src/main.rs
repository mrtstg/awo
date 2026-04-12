mod args;
mod config;
use clap::Parser;
use config::Config;
use std::fs;
use std::process::exit;
use toml;

#[tokio::main]
async fn main() {
    let run_args = args::Args::parse();
    let cfg_read = fs::read_to_string(run_args.config);
    match cfg_read {
        Err(e) => {
            println!("Failed reading config: {:?}", e);
            exit(1)
        }
        Ok(cfg_string) => {
            let parse_res: Result<Config, _> = toml::from_str(&cfg_string);
            match parse_res {
                Err(e) => {
                    println!("Failed to parse config: {:?}", e);
                    exit(1)
                }
                Ok(cfg) => {
                    println!("{:?}", cfg);
                }
            }
        }
    }
}
