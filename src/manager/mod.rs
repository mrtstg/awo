mod process;
mod time;
use crate::config::{Config, Process, ProcessBehavior};
use crate::manager::process::{get_modified_timestamp, split_command};
use ansi_term::Color;
use glob::glob;
use nix::unistd::Pid;
use std::collections::HashMap;
use std::process::Stdio;
use std::result::Result;
use std::time::SystemTime;
use std::{
    hash::{DefaultHasher, Hasher},
    time::Duration,
};
use time::wait_min_delay;
use tokio::process::Child;
use tokio::process::*;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, BufReader},
    sync::mpsc::{channel, Receiver, Sender},
};
use tokio_util::sync::CancellationToken;

pub enum ManagerEvent {
    CreateProcess(Process),
    RestartProcess(Process, Child),
    ProcessExited(i32, Process),
    ProcessStdout(String, Process),
    ProcessStderr(String, Process),
    ProcessKilled(Process),
    ProcessCreated(Process, u32),
    StopRunning,
}

pub struct ProcessManager {
    sender: Sender<ManagerEvent>,
    receiver: Receiver<ManagerEvent>,
    padding_width: usize,
    padding_enabled: bool,
    cancel_token: CancellationToken,
    pid_map: HashMap<String, u32>,
    awaiting_exit: bool,
    pub ansi_print: bool,
}

impl ProcessManager {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = channel(capacity);
        ProcessManager {
            sender,
            receiver,
            padding_width: 0,
            padding_enabled: true,
            cancel_token: CancellationToken::new(),
            pid_map: HashMap::new(),
            awaiting_exit: false,
            ansi_print: true,
        }
    }

    pub fn build_process_from_config(&self, data: Process) -> Result<Child, String> {
        let args = split_command(&data.command);
        if args.is_empty() {
            return Err("Command is empty".to_string());
        }
        let cmd = &mut Command::new(args[0].clone());
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .process_group(0);
        if let Some(cwd) = data.cwd {
            cmd.current_dir(cwd);
        }
        if args.len() > 1 {
            cmd.args(&args[1..]);
        }
        if self.ansi_print {
            cmd.env("FORCE_COLOR", "1")
                .env("CLICOLOR_FORCE", "1")
                .env("COLORTERM", "truecolor");
        }
        for (key, value) in data.env {
            cmd.env(key, value);
        }
        match cmd.spawn() {
            Ok(child) => Ok(child),
            Err(err) => return Err(err.to_string()),
        }
    }

    pub async fn init_process(&mut self, config: &Config) {
        for process in config.run.values() {
            self.send(ManagerEvent::CreateProcess(process.clone()))
                .await
        }
        self.padding_width = config.run.values().map(|v| v.name.len()).max().unwrap_or(0);
        self.padding_enabled = config.align;
    }

    pub async fn send(&mut self, payload: ManagerEvent) {
        let _ = self.sender.send(payload).await;
    }

    pub async fn run(&mut self) {
        while let Some(event) = self.receiver.recv().await {
            match event {
                ManagerEvent::CreateProcess(p) => {
                    if !self.awaiting_exit {
                        self.handle_create(p).await
                    }
                }
                ManagerEvent::ProcessExited(code, p) => {
                    self.pid_map.remove(&p.name);
                    self.handle_exit(code, p).await;
                }
                ManagerEvent::ProcessStdout(msg, p) => {
                    if !p.hide {
                        self.print_log(&p, msg)
                    }
                }
                ManagerEvent::ProcessStderr(msg, p) => {
                    if !p.hide {
                        self.print_log(&p, msg)
                    }
                }
                ManagerEvent::RestartProcess(p, child) => {
                    if !self.awaiting_exit {
                        self.handle_restart(p, child).await
                    }
                }
                ManagerEvent::StopRunning => {
                    println!("Awaiting process stop");
                    self.cancel_token.cancel();
                    self.awaiting_exit = true;
                    if self.pid_map.is_empty() {
                        break;
                    }
                }
                ManagerEvent::ProcessKilled(p) => {
                    self.print_log(&p, format!("Sent SIGTERM to process {}", p.name));
                    self.pid_map.remove(&p.name);
                    if self.pid_map.is_empty() && self.awaiting_exit {
                        println!("All process terminated. Exiting...");
                        break;
                    }
                }
                ManagerEvent::ProcessCreated(p, pid) => {
                    self.print_log(&p, format!("Created process with pid {}", pid));
                    self.pid_map.insert(p.name, pid);
                }
            }
        }
    }

    async fn handle_restart(&mut self, process_cfg: Process, mut child: Child) {
        self.print_log(&process_cfg, "Killing process...".to_string());
        let start = SystemTime::now();
        if let Some(pid_u32) = child.id() {
            let pid = pid_u32.try_into().unwrap();
            let _ = nix::sys::signal::killpg(Pid::from_raw(pid), nix::sys::signal::Signal::SIGTERM);
        }
        let _ = child.wait().await;
        let _ = wait_min_delay(
            start,
            Duration::from_secs(process_cfg.restart_delay.unwrap_or(1)),
        )
        .await;
        self.send(ManagerEvent::CreateProcess(process_cfg)).await;
    }

    async fn handle_create(&self, process_cfg: Process) {
        let sender = self.sender.clone();
        match self.build_process_from_config(process_cfg.clone()) {
            Err(e) => {
                eprintln!("Failed to create process {}: {:?}", process_cfg.name, e);
                let _ = sender
                    .send(ManagerEvent::ProcessExited(0, process_cfg))
                    .await;
            }
            Ok(mut child) => {
                if let Some(pid) = child.id() {
                    let _ = sender
                        .send(ManagerEvent::ProcessCreated(process_cfg.clone(), pid))
                        .await;
                }

                if let Some(out) = child.stdout.take() {
                    self.spawn_log_reader(
                        sender.clone(),
                        process_cfg.clone(),
                        BufReader::new(out),
                        true,
                    );
                }

                if let Some(err) = child.stderr.take() {
                    self.spawn_log_reader(
                        sender.clone(),
                        process_cfg.clone(),
                        BufReader::new(err),
                        false,
                    );
                }

                let token = self.cancel_token.clone();
                let pid = child.id();
                let process_cfg_clone = process_cfg.clone();
                tokio::spawn(async move {
                    let mut prev_hash: u64 = 0;
                    tokio::select! {
                        _ = token.cancelled() => {
                            if let Some(pid_u32) = pid {
                                let pid = pid_u32.try_into().unwrap();
                                let _ = nix::sys::signal::killpg(Pid::from_raw(pid), nix::sys::signal::Signal::SIGTERM);
                                let _ = sender
                                    .send(ManagerEvent::ProcessKilled(process_cfg_clone))
                                    .await;
                            }
                        }
                        _ = async {
                            loop {
                                if process_cfg.watch.len() > 0 {
                                    let start = SystemTime::now();
                                    let mut hasher = DefaultHasher::new();
                                    for pattern in &process_cfg.watch {
                                        for path in glob(pattern).unwrap().filter_map(Result::ok) {
                                            hasher.write(&get_modified_timestamp(&path).await.to_ne_bytes());
                                        }
                                    }
                                    let new_hash = hasher.finish();
                                    if prev_hash != 0 {
                                        if new_hash != prev_hash {
                                            let _ = sender
                                                .send(ManagerEvent::RestartProcess(process_cfg, child))
                                                .await;
                                            break;
                                        }
                                    }
                                    prev_hash = new_hash;
                                    let _ = wait_min_delay(start, Duration::from_millis(500)).await;
                                }

                                let start = SystemTime::now();
                                if let Ok(wait) = child.try_wait() {
                                    if let Some(status) = wait {
                                        let code = status.code().unwrap_or(0);
                                        let _ = sender
                                            .send(ManagerEvent::ProcessExited(code, process_cfg.clone()))
                                            .await;
                                        break;
                                    }
                                }
                                let _ = wait_min_delay(start, Duration::from_millis(100)).await;
                            }
                        } => {}
                    }
                });
            }
        }
    }

    fn spawn_log_reader<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
        &self,
        sender: Sender<ManagerEvent>,
        process: Process,
        reader: R,
        is_stdout: bool,
    ) {
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(resp) = lines.next_line().await {
                match resp {
                    None => break,
                    Some(line) => {
                        let event = if is_stdout {
                            ManagerEvent::ProcessStdout(line, process.clone())
                        } else {
                            ManagerEvent::ProcessStderr(line, process.clone())
                        };
                        let _ = sender.send(event).await;
                    }
                }
            }
        });
    }

    async fn handle_exit(&self, exit_code: i32, process_cfg: Process) {
        self.print_log(&process_cfg, format!("Exited with code {}", exit_code));

        let behavior = if exit_code == 0 {
            process_cfg
                .on_exit
                .clone()
                .unwrap_or(crate::config::DEFAULT_ON_EXIT)
        } else {
            process_cfg
                .on_error
                .clone()
                .unwrap_or(crate::config::DEFAULT_ON_ERROR)
        };

        match behavior {
            ProcessBehavior::Exit => {
                self.print_log(&process_cfg, "Stopping process manager...".to_string());
                let _ = self.sender.send(ManagerEvent::StopRunning).await;
            }
            ProcessBehavior::Restart => {
                self.print_log(&process_cfg, "Restarting...".to_string());
                tokio::time::sleep(Duration::from_secs(process_cfg.restart_delay.unwrap_or(1)))
                    .await;
                let _ = self
                    .sender
                    .send(ManagerEvent::CreateProcess(process_cfg))
                    .await;
            }
            ProcessBehavior::Ignore => {}
        }
    }

    fn print_log(&self, process_data: &Process, msg: String) {
        let process_name = if self.padding_enabled {
            format!("{:<width$}", process_data.name, width = self.padding_width)
        } else {
            process_data.name.clone()
        };
        if self.ansi_print {
            println!(
                "{} | {}",
                // setting white as default color if config does not provides any
                Color::Fixed(process_data.color.unwrap_or(7)).paint(process_name),
                msg,
            );
        } else {
            println!("{} | {}", process_name, msg);
        }
    }
}
