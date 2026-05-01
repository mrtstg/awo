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
    let defined_colors = config
        .run
        .iter()
        .filter(|(_, v)| v.color.is_some())
        .map(|(_, v)| v.color.unwrap())
        .collect::<Vec<u8>>();
    let mut free_colors = Vec::from_iter(COLORS_RANGE)
        .into_iter()
        .filter(|v| !defined_colors.contains(v))
        .collect::<Vec<u8>>();
    for (key, process) in config.run.into_iter() {
        let mut nproc = process.clone();
        if process.name.is_empty() {
            nproc.name = key.clone();
        }
        match nproc.color {
            Some(_) => continue,
            None => {
                if !free_colors.is_empty() {
                    nproc.color = free_colors.pop();
                } else {
                    nproc.color = Some(rand::random_range(COLORS_RANGE));
                }
            }
        }
        new_config.run.insert(key, nproc);
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
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ProcessBehavior {
    Exit,
    Restart,
    #[serde(other)]
    Ignore,
}
