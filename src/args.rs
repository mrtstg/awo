use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, name="awo")]
pub struct Args {
    /// Path to process file
    #[arg(short, long, value_name = "FILE", default_value = "./awo.toml")]
    pub config: PathBuf,
}
