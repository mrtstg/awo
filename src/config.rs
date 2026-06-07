use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const COLORS_RANGE: std::ops::RangeInclusive<u8> = 2..=14;
pub const DEFAULT_ON_EXIT: ProcessBehavior = ProcessBehavior::Exit;
pub const DEFAULT_ON_ERROR: ProcessBehavior = ProcessBehavior::Exit;

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Config {
    #[serde(default = "default_align")]
    pub align: bool,
    pub run: HashMap<String, Process>,
    #[serde(default = "default_behavior")]
    pub on_exit: ProcessBehavior,
    #[serde(default = "default_error_behavior")]
    pub on_error: ProcessBehavior,
    #[serde(default = "default_restart_delay")]
    pub restart_delay: u64,
}

impl Config {
    pub fn sample() -> Config {
        Config {
            align: true,
            run: [(
                "example",
                Process {
                    command: "echo 'Hello world!'".to_string(),
                    name: "optional_custom_name".to_string(),
                    on_exit: Some(ProcessBehavior::Ignore),
                    on_error: Some(ProcessBehavior::Restart),
                    restart_delay: Some(5),
                    watch: vec!["/tmp/somefile".to_string()],
                    color: Some(7),
                    hide: false,
                    cwd: Some("/usr/bin/".to_string()),
                    env: [
                        ("ENV1".to_string(), "VALUE1".to_string()),
                        ("ENV2".to_string(), "VALUE2".to_string()),
                    ]
                    .into_iter()
                    .collect::<HashMap<String, String>>(),
                },
            )]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<HashMap<String, Process>>(),
            on_exit: ProcessBehavior::Exit,
            on_error: ProcessBehavior::Exit,
            restart_delay: 1,
        }
    }
}

pub fn process_config(config: Config, args: crate::args::Args) -> Config {
    let config_apps = config.run.iter().map(|(k, _)| k).collect::<Vec<&String>>();
    let unknown_apps = args
        .except
        .clone()
        .into_iter()
        .filter(|v| !config_apps.contains(&v));
    for app_name in unknown_apps {
        println!("Unknown app name: {}!", app_name);
    }

    let mut new_config = config.clone();
    new_config.run = new_config
        .run
        .into_iter()
        .filter(|(k, _)| !args.except.contains(k))
        .collect::<HashMap<String, Process>>();
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
    for (key, process) in config
        .run
        .into_iter()
        .filter(|(k, _)| !args.except.contains(k))
    {
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
        nproc.on_exit = Some(nproc.on_exit.unwrap_or(config.on_exit.clone()));
        nproc.on_error = Some(nproc.on_error.unwrap_or(config.on_error.clone()));
        nproc.restart_delay = Some(nproc.restart_delay.unwrap_or(config.restart_delay.clone()));
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

fn default_hide() -> bool {
    return false;
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Process {
    pub command: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_exit: Option<ProcessBehavior>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_error: Option<ProcessBehavior>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_delay: Option<u64>,
    #[serde(default = "default_watch")]
    pub watch: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
    #[serde(default = "default_hide")]
    pub hide: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessBehavior {
    Exit,
    Restart,
    #[serde(other)]
    Ignore,
}
