use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    version,
    about,
    long_about = "Process manager for developers written in Rust.",
    name = "awo"
)]
pub struct Args {
    /// Path to process file
    #[arg(short, long, value_name = "FILE", default_value = "./awo.toml")]
    pub config: PathBuf,
    /// Disable ANSI coloring
    #[arg(short, long, default_value_t = false)]
    pub no_ansi: bool,
    /// Print sample config
    #[arg(short, long, default_value_t = false)]
    pub sample: bool,
    /// Exclude app from lauching
    #[arg(short, long)]
    pub except: Vec<String>,
    /// Force hiding output of app
    #[arg(short, long)]
    pub hide: Vec<String>,
}
