use shlex::Shlex;
use std::{path::PathBuf, time::SystemTime};
use tokio::fs;

pub fn split_command(command: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    Shlex::new(command).for_each(|v| arguments.push(v));

    arguments
}

pub async fn get_modified_timestamp(path: &PathBuf) -> u64 {
    fs::metadata(path)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|d| d.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
