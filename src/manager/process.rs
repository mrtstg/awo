use crate::config::Process;
use std::process::Stdio;
use tokio::process::*;

pub fn build_process_from_config(data: Process) -> Result<Child, String> {
    let cmd = &mut Command::new("/bin/sh");
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .arg("-c")
        .arg(data.command);
    match cmd.spawn() {
        Ok(child) => Ok(child),
        Err(err) => return Err(err.to_string()),
    }
}
