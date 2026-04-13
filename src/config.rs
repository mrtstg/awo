use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
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

fn default_name() -> String {
    return "".to_string();
}

#[derive(Deserialize, Debug, Clone)]
pub struct Process {
    pub command: String,
    #[serde(default = "default_name")]
    pub name: String,
}
