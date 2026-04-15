use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(default = "default_align")]
    pub align: bool,
    pub run: HashMap<String, Process>,
}

pub fn process_config(config: Config) -> Config {
    let mut new_config = config.clone();
    for (key, process) in config.run.into_iter() {
        if process.name.is_empty() {
            let mut nproc = process.clone();
            nproc.name = key.clone();
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
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ProcessBehavior {
    Exit,
    Restart,
    #[serde(other)]
    Ignore,
}
