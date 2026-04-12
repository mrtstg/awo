use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub run: HashMap<String, Process>,
}

#[derive(Deserialize, Debug)]
pub struct Process {
    pub command: String,
}
