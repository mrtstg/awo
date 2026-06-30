mod args;
mod config;
mod manager;
use crate::manager::ProcessManager;
use clap::Parser;
use config::{process_config, Config};
use std::fs;
use std::process::exit;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let run_args = args::Args::parse();
    if run_args.sample {
        let cfg = toml::to_string_pretty(&Config::sample());
        if let Ok(s) = cfg {
            println!("{}", s);
        }
        exit(0);
    }

    let cfg_read = fs::read_to_string(&run_args.config);
    match cfg_read {
        Err(e) => {
            eprintln!("Failed reading config: {:?}", e);
            exit(1)
        }
        Ok(cfg_string) => {
            let parse_res: Result<Config, _> = toml::from_str(&cfg_string);
            match parse_res {
                Err(e) => {
                    eprintln!("Failed to parse config: {:?}", e);
                    exit(1)
                }
                Ok(raw_cfg) => {
                    let mut manager = ProcessManager::new(4096);
                    manager.ansi_print = !run_args.no_ansi;

                    let cfg = process_config(raw_cfg, &run_args);
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {
                            println!("Ctrl+C received, shutting down.");
                            manager.send(manager::ManagerEvent::StopRunning).await?;

                            if let Err(e) = manager.run().await  {
                                eprintln!("Error occurred while running manager: {:?}", e);
                            }
                        }

                        _ = async {
                            manager.init_process(&cfg).await?;

                            if let Err(e) = manager.run().await  {
                                eprintln!("Error occurred while running manager: {:?}", e);
                            }

                            Ok::<(), anyhow::Error>(())
                        } => {}
                    }
                }
            }
        }
    }

    Ok(())
}
