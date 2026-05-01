use serde::Deserialize;
use std::collections::HashMap;

const COLORS_RANGE: std::ops::RangeInclusive<u8> = 2..=14;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(default = "default_align")]
    pub align: bool,
    pub run: HashMap<String, Process>,
}

pub fn process_config(config: Config) -> Config {
    let mut new_config = config.clone();
    let mut free_colors = Vec::from_iter(COLORS_RANGE);
    for (key, process) in config.run.into_iter() {
        if process.name.is_empty() {
            let mut nproc = process.clone();
            nproc.name = key.clone();
            if !free_colors.is_empty() {
                if !free_colors.contains(&nproc.color) {
                    let ncolor = free_colors.pop();
                    nproc.color = ncolor.unwrap_or(nproc.color);
                } else {
                    free_colors.retain(|v| *v != nproc.color);
                }
            }
            new_config.run.insert(key, nproc);
        }
    }
    return new_config;
}

fn default_align() -> bool {
    return true;
}

fn default_behavior() -> ProcessBehavior {
    return ProcessBehavior::Ignore;
}

fn default_error_behavior() -> ProcessBehavior {
    return ProcessBehavior::Exit;
}

fn default_name() -> String {
    return "".to_string();
}

fn default_restart_delay() -> u64 {
    return 1;
}

fn default_watch() -> Vec<String> {
    return Vec::new();
}

fn random_color() -> u8 {
    rand::random_range(COLORS_RANGE)
}

#[derive(Deserialize, Debug, Clone)]
pub struct Process {
    pub command: String,
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_behavior")]
    pub on_exit: ProcessBehavior,
    #[serde(default = "default_error_behavior")]
    pub on_error: ProcessBehavior,
    #[serde(default = "default_restart_delay")]
    pub restart_delay: u64,
    #[serde(default = "default_watch")]
    pub watch: Vec<String>,
    #[serde(default = "random_color")]
    pub color: u8,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ProcessBehavior {
    Exit,
    Restart,
    #[serde(other)]
    Ignore,
}
