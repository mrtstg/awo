mod args;
mod config;
mod manager;
use crate::manager::ProcessManager;
use clap::Parser;
use config::{process_config, Config};
use std::fs;
use std::process::exit;
use tokio;
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
                Ok(raw_cfg) => {
                    let cfg = process_config(raw_cfg);
                    let mut manager = ProcessManager::new(4096);
                    manager.ansi_print = !run_args.no_ansi;
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {
                            println!("Ctrl+C received, shutting down.");
                            manager.send(manager::ManagerEvent::StopRunning).await;
                            manager.run().await;
                        }
                        _ = async {
                            manager.init_process(&cfg).await;
                            manager.run().await;
                        } => {}
                    }
                }
            }
        }
    }
}
