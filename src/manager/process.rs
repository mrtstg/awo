use crate::config::Process;
use shlex::Shlex;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::*,
};

fn split_command(command: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    Shlex::new(command).for_each(|v| arguments.push(v));
    arguments
}

pub async fn get_child_stdout(child: &mut Child) -> Vec<String> {
    match child.stdout {
        None => return Vec::new(),
        Some(ref mut stdout) => {
            let reader = BufReader::new(stdout);
            let mut buffer = Vec::new();
            let mut lines = reader.lines();
            while let Ok(line) = lines.next_line().await {
                match line {
                    None => break,
                    Some(l) => buffer.push(l),
                }
            }
            return buffer;
        }
    }
}

pub fn build_process_from_config(data: Process) -> Result<Child, String> {
    let command = split_command(&data.command);
    if command.len() == 0 {
        return Err("Failed to parse command".to_string());
    }

    let mut cmd = &mut Command::new(command[0].clone());
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if command.len() > 1 {
        cmd = cmd.args(&command[1..])
    }
    match cmd.spawn() {
        Ok(child) => Ok(child),
        Err(err) => return Err(err.to_string()),
    }
}
