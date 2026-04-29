mod process;
mod time;
use crate::config::{Config, Process, ProcessBehavior};
use crate::manager::process::get_modified_timestamp;
use glob::glob;
use libc;
use process::build_process_from_config;
use std::collections::HashMap;
use std::result::Result;
use std::time::SystemTime;
use std::{
    hash::{DefaultHasher, Hasher},
    time::Duration,
};
use time::wait_min_delay;
use tokio::process::Child;
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
    ProcessKilled(String),
    ProcessCreated(String, u32),
    StopRunning,
}

pub struct ProcessManager {
    sender: Sender<ManagerEvent>,
    receiver: Receiver<ManagerEvent>,
    padding_width: usize,
    padding_enabled: bool,
    cancel_token: CancellationToken,
    pid_map: HashMap<String, u32>,
    exit_on_empty: bool,
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
            exit_on_empty: false,
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
                ManagerEvent::CreateProcess(p) => self.handle_create(p).await,
                ManagerEvent::ProcessExited(code, p) => {
                    self.pid_map.remove(&p.name);
                    self.handle_exit(code, p).await;
                }
                ManagerEvent::ProcessStdout(msg, p) => self.print_log(&p.name, msg),
                ManagerEvent::ProcessStderr(msg, p) => self.print_log(&p.name, msg),
                ManagerEvent::RestartProcess(p, child) => self.handle_restart(p, child).await,
                ManagerEvent::StopRunning => {
                    println!("Awaiting process stop");
                    self.cancel_token.cancel();
                    self.exit_on_empty = true;
                    if self.pid_map.is_empty() {
                        break;
                    }
                }
                ManagerEvent::ProcessKilled(p) => {
                    self.print_log(&p, format!("Sent SIGTERM to process {}", p));
                    self.pid_map.remove(&p);
                    if self.pid_map.is_empty() && self.exit_on_empty {
                        println!("All process terminated. Exiting...");
                        break;
                    }
                }
                ManagerEvent::ProcessCreated(p, pid) => {
                    self.print_log(&p, format!("Created process with pid {}", pid));
                    self.pid_map.insert(p, pid);
                }
            }
        }
    }

    async fn handle_restart(&mut self, process_cfg: Process, mut child: Child) {
        self.print_log(&process_cfg.name, "Killing process...".to_string());
        let start = SystemTime::now();
        unsafe {
            if let Some(pid_u32) = child.id() {
                let pid = pid_u32.try_into().unwrap();
                libc::killpg(pid, libc::SIGTERM);
            }
        }
        let _ = child.wait().await;
        let _ = wait_min_delay(start, Duration::from_secs(process_cfg.restart_delay)).await;
        self.send(ManagerEvent::CreateProcess(process_cfg)).await;
    }

    async fn handle_create(&self, process_cfg: Process) {
        let sender = self.sender.clone();
        match build_process_from_config(process_cfg.clone()) {
            Err(e) => {
                eprintln!("Failed to create process {}: {:?}", process_cfg.name, e);
                let _ = sender
                    .send(ManagerEvent::ProcessExited(0, process_cfg))
                    .await;
            }
            Ok(mut child) => {
                if let Some(pid) = child.id() {
                    let _ = sender
                        .send(ManagerEvent::ProcessCreated(process_cfg.name.clone(), pid))
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
                let process_name = process_cfg.name.clone();
                tokio::spawn(async move {
                    let mut prev_hash: u64 = 0;
                    tokio::select! {
                        _ = token.cancelled() => {
                            unsafe {
                                if let Some(pid_u32) = pid {
                                    let pid = pid_u32.try_into().unwrap();
                                    libc::killpg(pid, libc::SIGTERM);
                                    let _ = sender
                                        .send(ManagerEvent::ProcessKilled(process_name))
                                        .await;
                                }
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
        self.print_log(&process_cfg.name, format!("Exited with code {}", exit_code));

        let behavior = if exit_code == 0 {
            process_cfg.on_exit.clone()
        } else {
            process_cfg.on_error.clone()
        };

        match behavior {
            ProcessBehavior::Exit => {
                self.print_log(&process_cfg.name, "Stopping process manager...".to_string());
                let _ = self.sender.send(ManagerEvent::StopRunning).await;
            }
            ProcessBehavior::Restart => {
                self.print_log(&process_cfg.name, "Restarting...".to_string());
                tokio::time::sleep(Duration::from_secs(process_cfg.restart_delay)).await;
                let _ = self
                    .sender
                    .send(ManagerEvent::CreateProcess(process_cfg))
                    .await;
            }
            ProcessBehavior::Ignore => {}
        }
    }

    fn print_log(&self, name: &str, msg: String) {
        if self.padding_enabled {
            println!("{:<width$} | {}", name, msg, width = self.padding_width);
        } else {
            println!("{} | {}", name, msg);
        }
    }
}
