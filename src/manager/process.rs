use crate::config::Process;
use std::{
    path::PathBuf,
    process::Stdio,
    time::{Duration, SystemTime},
};
use tokio::fs;
use tokio::process::*;

pub async fn get_modified_timestamp(path: &PathBuf) -> u64 {
    match fs::metadata(path).await {
        Err(_) => return 0,
        Ok(m) => match m.modified() {
            Err(_) => return 0,
            Ok(d) => {
                return d
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs()
            }
        },
    }
}

pub fn build_process_from_config(data: Process) -> Result<Child, String> {
    let cmd = &mut Command::new("/bin/sh");
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .arg("-c")
        .arg(data.command)
        .process_group(0)
        .env("FORCE_COLOR", "1")
        .env("CLICOLOR_FORCE", "1")
        .env("COLORTERM", "truecolor");
    match cmd.spawn() {
        Ok(child) => Ok(child),
        Err(err) => return Err(err.to_string()),
    }
}
