mod process;
mod watcher;

use crate::config::{Config, Process, ProcessBehavior};
use crate::manager::process::split_command;
use crate::manager::watcher::{FileWatcher, WatcherEvent};

use ansi_term::Color;
use anyhow::Context;
use nix::unistd::Pid;
use std::collections::HashMap;
use std::process::Stdio;
use std::result::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio::process::Child;
use tokio::process::*;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc::{channel, Receiver, Sender},
};
use tokio_util::sync::CancellationToken;

pub enum ManagerEvent {
    CreateProcess(Arc<Process>),
    ProcessExited(i32, Arc<Process>),
    ProcessStdout(String, Arc<Process>),
    ProcessStderr(String, Arc<Process>),
    ProcessKilled(Arc<Process>),
    ProcessCreated(Arc<Process>, u32),
    LogProcess(Arc<Process>, String),
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

    pub fn build_process_from_config(&self, data: Arc<Process>) -> Result<Child, String> {
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

        if let Some(cwd) = &data.cwd {
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

        for (key, value) in &data.env {
            cmd.env(key, value);
        }

        match cmd.spawn() {
            Ok(child) => Ok(child),
            Err(err) => return Err(err.to_string()),
        }
    }

    pub async fn init_process(&mut self, config: &Config) -> anyhow::Result<()> {
        for process in config.run.values() {
            self.send(ManagerEvent::CreateProcess(Arc::new(process.clone())))
                .await?;
        }

        self.padding_width = config.run.values().map(|v| v.name.len()).max().unwrap_or(0);
        self.padding_enabled = config.align;

        Ok(())
    }

    pub async fn send(&mut self, payload: ManagerEvent) -> anyhow::Result<()> {
        self.sender.send(payload).await?;

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        while let Some(event) = self.receiver.recv().await {
            match event {
                ManagerEvent::LogProcess(process_cfg, msg) => {
                    self.print_log(&process_cfg, msg.as_str());
                }
                ManagerEvent::CreateProcess(p) => {
                    if !self.awaiting_exit {
                        self.handle_create(p).await?
                    }
                }
                ManagerEvent::ProcessExited(code, p) => {
                    self.pid_map.remove(&p.name);
                    self.handle_exit(code, p).await?;
                }
                ManagerEvent::ProcessStdout(msg, p) => {
                    if !p.hide {
                        self.print_log(&p, &msg)
                    }
                }
                ManagerEvent::ProcessStderr(msg, p) => {
                    if !p.hide {
                        self.print_log(&p, &msg)
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
                    self.print_log(&p, &format!("Sent SIGTERM to process {}", p.name));
                    self.pid_map.remove(&p.name);

                    if self.pid_map.is_empty() && self.awaiting_exit {
                        println!("All process terminated. Exiting...");
                        break;
                    }
                }
                ManagerEvent::ProcessCreated(p, pid) => {
                    self.print_log(&p, &format!("Created process with pid {}", pid));
                    self.pid_map.insert(p.name.clone(), pid);
                }
            }
        }

        Ok(())
    }

    async fn handle_create(&self, process_cfg: Arc<Process>) -> anyhow::Result<()> {
        let sender = self.sender.clone();
        match self.build_process_from_config(process_cfg.clone()) {
            Err(e) => {
                eprintln!("Failed to create process {}: {:?}", process_cfg.name, e);

                sender
                    .send(ManagerEvent::ProcessExited(-1, process_cfg))
                    .await?;
            }
            Ok(mut child) => {
                if let Some(pid) = child.id() {
                    sender
                        .send(ManagerEvent::ProcessCreated(process_cfg.clone(), pid))
                        .await?;
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

                Self::spawn_process_monitor(
                    child,
                    sender.clone(),
                    process_cfg,
                    self.cancel_token.clone(),
                )
                .await;
            }
        }

        Ok(())
    }

    async fn spawn_process_monitor(
        mut child: Child,
        sender: Sender<ManagerEvent>,
        process_cfg: Arc<Process>,
        cancel_token: CancellationToken,
    ) {
        let sender = sender.clone();
        let process_cfg = process_cfg.clone();

        // Set up file watcher if watch patterns are defined
        let (watcher_rx, watcher_cancel) = if process_cfg.watch.is_empty() {
            (None, None)
        } else {
            let (tx, rx) = channel::<WatcherEvent>(32);
            let cancel = CancellationToken::new();
            let watcher = FileWatcher::new(process_cfg.watch.clone(), tx, cancel.clone());
            tokio::spawn(async move {
                let _ = watcher.start_watching().await;
            });
            (Some(rx), Some(cancel))
        };

        let cancel_for_monitor = cancel_token.clone();
        let watcher_cancel_for_handler = watcher_cancel.clone();
        let mut watcher_rx_opt = watcher_rx;

        tokio::spawn(async move {
            loop {
                if cancel_for_monitor.is_cancelled() {
                    if let Some(pid_u32) = child.id() {
                        let pid = pid_u32.try_into().context("overflow").ok();
                        if let Some(pid) = pid {
                            let _ = nix::sys::signal::killpg(
                                Pid::from_raw(pid),
                                nix::sys::signal::Signal::SIGTERM,
                            );
                        }
                    }
                    let _ = child.wait().await;
                    let _ = sender
                        .send(ManagerEvent::ProcessKilled(process_cfg.clone()))
                        .await;
                    return;
                }

                if let Some(ref mut rx) = watcher_rx_opt {
                    match rx.try_recv() {
                        Ok(WatcherEvent::InitialScanComplete(path_amount)) => {
                            let _ = sender
                                .send(ManagerEvent::LogProcess(
                                    process_cfg.clone(),
                                    format!("File watcher is watching {} paths", path_amount),
                                ))
                                .await;
                        }
                        Ok(WatcherEvent::FileChanged(paths)) => {
                            let _ = sender
                                .send(ManagerEvent::LogProcess(
                                    process_cfg.clone(),
                                    format!(
                                        "Following paths has changed: {}",
                                        paths
                                            .into_iter()
                                            .map(|x| x.to_string_lossy().into_owned())
                                            .collect::<Vec<String>>()
                                            .join(", ")
                                    ),
                                ))
                                .await;
                            if let Some(pid_u32) = child.id() {
                                let pid = pid_u32.try_into().context("overflow").ok();
                                if let Some(pid) = pid {
                                    let _ = nix::sys::signal::killpg(
                                        Pid::from_raw(pid),
                                        nix::sys::signal::Signal::SIGTERM,
                                    );
                                }
                            }
                            let _ = child.wait().await;
                            if let Some(cancel) = watcher_cancel_for_handler.clone() {
                                cancel.cancel();
                            }
                            let delay = {
                                let d = process_cfg.restart_delay.unwrap_or(1);
                                if d > 0 {
                                    d
                                } else {
                                    1
                                }
                            };

                            tokio::time::sleep(Duration::from_secs(delay)).await;
                            let _ = sender
                                .send(ManagerEvent::CreateProcess(process_cfg.clone()))
                                .await;
                            return;
                        }
                        Err(_) => {}
                    }
                }

                match child.try_wait() {
                    Ok(Some(status)) => {
                        let code = status.code().unwrap_or(0);
                        let _ = sender
                            .send(ManagerEvent::ProcessExited(code, process_cfg.clone()))
                            .await;
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("Error waiting for process: {}", e);
                        return;
                    }
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
    }

    fn spawn_log_reader<R: AsyncRead + Unpin + Send + 'static>(
        &self,
        sender: Sender<ManagerEvent>,
        process: Arc<Process>,
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

                        if let Err(e) = sender.send(event).await {
                            eprintln!("Failed to send log event: {}", e);
                        }
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        });
    }

    async fn handle_exit(&self, exit_code: i32, process_cfg: Arc<Process>) -> anyhow::Result<()> {
        self.print_log(&process_cfg, &format!("Exited with code {}", exit_code));

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
                self.print_log(&process_cfg, "Stopping process manager...");

                self.sender.send(ManagerEvent::StopRunning).await?;
            }
            ProcessBehavior::Restart => {
                self.print_log(&process_cfg, "Restarting...");
                let sender = self.sender.clone();
                let delay = process_cfg.restart_delay.unwrap_or(1);

                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    sender
                        .send(ManagerEvent::CreateProcess(process_cfg))
                        .await?;

                    Ok::<(), anyhow::Error>(())
                });
            }
            ProcessBehavior::Ignore => {}
        }

        Ok(())
    }

    fn print_log(&self, process_data: &Process, msg: &str) {
        let process_name = if self.padding_enabled {
            &format!("{:<width$}", process_data.name, width = self.padding_width)
        } else {
            &process_data.name
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
